//! GraphLite FFI - C-compatible Foreign Function Interface
//!
//! This crate provides a C-compatible API for GraphLite, enabling bindings
//! for Python, Java, JavaScript, and other languages.
//!
//! # Safety
//!
//! All functions in this module are `unsafe` from Rust's perspective because they:
//! - Accept raw pointers from foreign code
//! - Return raw pointers that must be freed by the caller
//! - Cross the FFI boundary where Rust's safety guarantees don't apply
//!
//! Callers must ensure:
//! - Pointers are valid and non-null (unless documented otherwise)
//! - Returned strings are freed with `graphlite_free_string`
//! - Database handles are closed with `graphlite_close`
//! - No concurrent access to the same handle without synchronization

use graphlite::QueryCoordinator;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::sync::Arc;

/// Opaque handle to a GraphLite database instance
///
/// This handle wraps a QueryCoordinator and must be freed with `graphlite_close`
#[repr(C)]
pub struct GraphLiteDB {
    coordinator: Arc<QueryCoordinator>,
}

/// Error codes returned by FFI functions
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GraphLiteErrorCode {
    /// Operation succeeded
    Success = 0,
    /// Null pointer was passed
    NullPointer = 1,
    /// Invalid UTF-8 string
    InvalidUtf8 = 2,
    /// Failed to open database
    DatabaseOpenError = 3,
    /// Failed to create session
    SessionError = 4,
    /// Query execution failed
    QueryError = 5,
    /// Internal panic occurred
    PanicError = 6,
    /// JSON serialization failed
    JsonError = 7,
}

/// Initialize GraphLite database from path
///
/// # Arguments
/// * `path` - C string with database path (must not be null)
/// * `error_out` - Output parameter for error code (can be null if caller doesn't need it)
///
/// # Returns
/// * Opaque handle to database on success
/// * null pointer on error (check `error_out` for details)
///
/// # Safety
/// * `path` must be a valid null-terminated C string
/// * Returned handle must be freed with `graphlite_close`
///
/// # Example
/// ```c
/// GraphLiteErrorCode error;
/// GraphLiteDB* db = graphlite_open("/path/to/db", &error);
/// if (db == NULL) {
///     printf("Error: %d\n", error);
///     return -1;
/// }
/// // ... use database ...
/// graphlite_close(db);
/// ```
#[no_mangle]
pub unsafe extern "C" fn graphlite_open(
    path: *const c_char,
    error_out: *mut GraphLiteErrorCode,
) -> *mut GraphLiteDB {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        // Check for null pointer
        if path.is_null() {
            set_error(error_out, GraphLiteErrorCode::NullPointer);
            return ptr::null_mut();
        }

        // Convert C string to Rust string
        let c_str = unsafe { CStr::from_ptr(path) };
        let path_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::InvalidUtf8);
                return ptr::null_mut();
            }
        };

        // Create QueryCoordinator
        match QueryCoordinator::from_path(path_str) {
            Ok(coordinator) => {
                set_error(error_out, GraphLiteErrorCode::Success);
                Box::into_raw(Box::new(GraphLiteDB { coordinator }))
            }
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::DatabaseOpenError);
                ptr::null_mut()
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_error(error_out, GraphLiteErrorCode::PanicError);
            ptr::null_mut()
        }
    }
}

/// Create a simple session for the given username
///
/// # Arguments
/// * `db` - Database handle (must not be null)
/// * `username` - C string with username (must not be null)
/// * `error_out` - Output parameter for error code (can be null)
///
/// # Returns
/// * C string with session ID on success (must be freed with `graphlite_free_string`)
/// * null pointer on error
///
/// # Safety
/// * `db` must be a valid handle from `graphlite_open`
/// * `username` must be a valid null-terminated C string
/// * Returned string must be freed with `graphlite_free_string`
#[no_mangle]
pub unsafe extern "C" fn graphlite_create_session(
    db: *mut GraphLiteDB,
    username: *const c_char,
    error_out: *mut GraphLiteErrorCode,
) -> *mut c_char {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        // Check for null pointers
        if db.is_null() {
            set_error(error_out, GraphLiteErrorCode::NullPointer);
            return ptr::null_mut();
        }
        if username.is_null() {
            set_error(error_out, GraphLiteErrorCode::NullPointer);
            return ptr::null_mut();
        }

        let db_ref = unsafe { &*db };

        // Convert C string to Rust string
        let c_str = unsafe { CStr::from_ptr(username) };
        let username_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::InvalidUtf8);
                return ptr::null_mut();
            }
        };

        // Create session
        match db_ref.coordinator.create_simple_session(username_str) {
            Ok(session_id) => match CString::new(session_id) {
                Ok(c_string) => {
                    set_error(error_out, GraphLiteErrorCode::Success);
                    c_string.into_raw()
                }
                Err(_) => {
                    set_error(error_out, GraphLiteErrorCode::InvalidUtf8);
                    ptr::null_mut()
                }
            },
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::SessionError);
                ptr::null_mut()
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_error(error_out, GraphLiteErrorCode::PanicError);
            ptr::null_mut()
        }
    }
}

/// Execute a GQL query and return results as JSON
///
/// # Arguments
/// * `db` - Database handle (must not be null)
/// * `session_id` - C string with session ID (must not be null)
/// * `query` - C string with GQL query (must not be null)
/// * `error_out` - Output parameter for error code (can be null)
///
/// # Returns
/// * JSON string with query results on success (must be freed with `graphlite_free_string`)
/// * null pointer on error
///
/// # Safety
/// * `db` must be a valid handle from `graphlite_open`
/// * `session_id` must be from `graphlite_create_session`
/// * `query` must be a valid null-terminated C string
/// * Returned JSON string must be freed with `graphlite_free_string`
///
/// # JSON Format
/// ```json
/// {
///   "variables": ["col1", "col2"],
///   "rows": [
///     {"col1": "value1", "col2": 123},
///     {"col1": "value2", "col2": 456}
///   ],
///   "row_count": 2
/// }
/// ```
#[no_mangle]
pub unsafe extern "C" fn graphlite_query(
    db: *mut GraphLiteDB,
    session_id: *const c_char,
    query: *const c_char,
    error_out: *mut GraphLiteErrorCode,
) -> *mut c_char {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        // Check for null pointers
        if db.is_null() || session_id.is_null() || query.is_null() {
            set_error(error_out, GraphLiteErrorCode::NullPointer);
            return ptr::null_mut();
        }

        let db_ref = unsafe { &*db };

        // Convert C strings to Rust strings
        let session_c_str = unsafe { CStr::from_ptr(session_id) };
        let session_str = match session_c_str.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::InvalidUtf8);
                return ptr::null_mut();
            }
        };

        let query_c_str = unsafe { CStr::from_ptr(query) };
        let query_str = match query_c_str.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::InvalidUtf8);
                return ptr::null_mut();
            }
        };

        // Execute query
        match db_ref.coordinator.process_query(query_str, session_str) {
            Ok(result) => {
                // Serialize to JSON
                match serde_json::to_string(&result) {
                    Ok(json) => match CString::new(json) {
                        Ok(c_string) => {
                            set_error(error_out, GraphLiteErrorCode::Success);
                            c_string.into_raw()
                        }
                        Err(_) => {
                            set_error(error_out, GraphLiteErrorCode::JsonError);
                            ptr::null_mut()
                        }
                    },
                    Err(_) => {
                        set_error(error_out, GraphLiteErrorCode::JsonError);
                        ptr::null_mut()
                    }
                }
            }
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::QueryError);
                ptr::null_mut()
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_error(error_out, GraphLiteErrorCode::PanicError);
            ptr::null_mut()
        }
    }
}

/// Close a session
///
/// # Arguments
/// * `db` - Database handle (must not be null)
/// * `session_id` - C string with session ID (must not be null)
/// * `error_out` - Output parameter for error code (can be null)
///
/// # Returns
/// * Error code (Success = 0, error otherwise)
///
/// # Safety
/// * `db` must be a valid handle from `graphlite_open`
/// * `session_id` must be from `graphlite_create_session`
#[no_mangle]
pub unsafe extern "C" fn graphlite_close_session(
    db: *mut GraphLiteDB,
    session_id: *const c_char,
    error_out: *mut GraphLiteErrorCode,
) -> GraphLiteErrorCode {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        if db.is_null() || session_id.is_null() {
            set_error(error_out, GraphLiteErrorCode::NullPointer);
            return GraphLiteErrorCode::NullPointer;
        }

        let db_ref = unsafe { &*db };

        let session_c_str = unsafe { CStr::from_ptr(session_id) };
        let session_str = match session_c_str.to_str() {
            Ok(s) => s,
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::InvalidUtf8);
                return GraphLiteErrorCode::InvalidUtf8;
            }
        };

        match db_ref.coordinator.close_session(session_str) {
            Ok(_) => {
                set_error(error_out, GraphLiteErrorCode::Success);
                GraphLiteErrorCode::Success
            }
            Err(_) => {
                set_error(error_out, GraphLiteErrorCode::SessionError);
                GraphLiteErrorCode::SessionError
            }
        }
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            set_error(error_out, GraphLiteErrorCode::PanicError);
            GraphLiteErrorCode::PanicError
        }
    }
}

/// Free a string returned by GraphLite FFI functions
///
/// # Arguments
/// * `s` - C string to free (can be null, in which case this is a no-op)
///
/// # Safety
/// * `s` must be a string returned by a GraphLite FFI function
/// * Must not be called more than once on the same string
/// * Must not be called on strings not allocated by GraphLite
#[no_mangle]
pub unsafe extern "C" fn graphlite_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

/// Close database connection and free resources
///
/// # Arguments
/// * `db` - Database handle to close (can be null, in which case this is a no-op)
///
/// # Safety
/// * `db` must be a handle from `graphlite_open`
/// * Must not be called more than once on the same handle
/// * Must not be used after calling this function
#[no_mangle]
pub unsafe extern "C" fn graphlite_close(db: *mut GraphLiteDB) {
    if !db.is_null() {
        unsafe {
            drop(Box::from_raw(db));
        }
    }
}

/// Get the version string of GraphLite
///
/// # Returns
/// * Static C string with version (e.g., "0.1.0")
/// * Must NOT be freed (it's a static string)
#[no_mangle]
pub extern "C" fn graphlite_version() -> *const c_char {
    // Using a static string so it doesn't need to be freed
    const VERSION: &[u8] = b"0.1.0\0";
    VERSION.as_ptr() as *const c_char
}

// Helper function to set error code if output pointer is not null
fn set_error(error_out: *mut GraphLiteErrorCode, code: GraphLiteErrorCode) {
    if !error_out.is_null() {
        unsafe {
            *error_out = code;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_version() {
        let version_ptr = graphlite_version();
        assert!(!version_ptr.is_null());
        let version = unsafe { CStr::from_ptr(version_ptr) };
        assert_eq!(version.to_str().unwrap(), "0.1.0");
    }

    #[test]
    fn test_null_pointer_handling() {
        let mut error = GraphLiteErrorCode::Success;

        // Test null path
        let db = unsafe { graphlite_open(ptr::null(), &mut error) };
        assert!(db.is_null());
        assert_eq!(error, GraphLiteErrorCode::NullPointer);
    }

    #[test]
    fn test_open_close() {
        let mut error = GraphLiteErrorCode::Success;
        let path = CString::new("/tmp/test_ffi_db").unwrap();

        let db = unsafe { graphlite_open(path.as_ptr(), &mut error) };
        assert!(!db.is_null());
        assert_eq!(error, GraphLiteErrorCode::Success);

        unsafe { graphlite_close(db) };
    }
}
