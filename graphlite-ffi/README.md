# GraphLite FFI - C-Compatible Foreign Function Interface

This crate provides a C-compatible API for GraphLite, enabling language bindings for Python, Swift, Java, JavaScript, Go, WASM, and other languages.

## Overview

The FFI layer exposes GraphLite's core functionality through a stable C ABI:

- **Database Operations**: Open/close database connections
- **Session Management**: Create and manage user sessions
- **Query Execution**: Execute GQL queries and retrieve results as JSON
- **Error Handling**: Comprehensive error codes for all operations
- **Memory Safety**: Explicit resource management with clear ownership

## Building

### As a Dynamic Library (.so, .dylib, .dll)

```bash
cargo build --release
```

The library will be at:
- Linux: `target/release/libgraphliteffi.so`
- macOS: `target/release/libgraphliteffi.dylib`

### As a Static Library (.a, .lib)

```bash
cargo build --release --lib
```

### Generate C Header

The C header file `graphlite.h` is automatically generated during build using `cbindgen`.

## C API Example

```c
#include "graphlite.h"
#include <stdio.h>
#include <stdlib.h>

int main() {
    GraphLiteErrorCode error;

    // Open database
    GraphLiteDB* db = graphlite_open("/tmp/mydb", &error);
    if (db == NULL) {
        printf("Failed to open database: %d\n", error);
        return 1;
    }

    // Create session
    char* session_id = graphlite_create_session(db, "user1", &error);
    if (session_id == NULL) {
        printf("Failed to create session: %d\n", error);
        graphlite_close(db);
        return 1;
    }

    // Execute query
    const char* query = "MATCH (n) RETURN n LIMIT 10";
    char* result_json = graphlite_query(db, session_id, query, &error);
    if (result_json == NULL) {
        printf("Query failed: %d\n", error);
    } else {
        printf("Result: %s\n", result_json);
        graphlite_free_string(result_json);
    }

    // Cleanup
    graphlite_close_session(db, session_id, &error);
    graphlite_free_string(session_id);
    graphlite_close(db);

    return 0;
}
```

## API Reference

### Core Functions

#### `graphlite_open`
```c
GraphLiteDB* graphlite_open(const char* path, GraphLiteErrorCode* error_out);
```
Opens a GraphLite database at the specified path.

**Parameters:**
- `path`: Path to database directory
- `error_out`: Optional pointer to receive error code

**Returns:** Database handle or NULL on error

---

#### `graphlite_create_session`
```c
char* graphlite_create_session(GraphLiteDB* db, const char* username, GraphLiteErrorCode* error_out);
```
Creates a new session for the specified user.

**Parameters:**
- `db`: Database handle
- `username`: Username for the session
- `error_out`: Optional pointer to receive error code

**Returns:** Session ID string (must be freed) or NULL on error

---

#### `graphlite_query`
```c
char* graphlite_query(GraphLiteDB* db, const char* session_id, const char* query, GraphLiteErrorCode* error_out);
```
Executes a GQL query and returns results as JSON.

**Parameters:**
- `db`: Database handle
- `session_id`: Session ID from `graphlite_create_session`
- `query`: GQL query string
- `error_out`: Optional pointer to receive error code

**Returns:** JSON string (must be freed) or NULL on error

**JSON Result Format:**
```json
{
  "variables": ["col1", "col2"],
  "rows": [
    {"col1": "value1", "col2": 123}
  ],
  "row_count": 1
}
```

---

#### `graphlite_close_session`
```c
GraphLiteErrorCode graphlite_close_session(GraphLiteDB* db, const char* session_id, GraphLiteErrorCode* error_out);
```
Closes a session and frees associated resources.

**Parameters:**
- `db`: Database handle
- `session_id`: Session ID to close
- `error_out`: Optional pointer to receive error code

**Returns:** Error code

---

#### `graphlite_free_string`
```c
void graphlite_free_string(char* s);
```
Frees a string returned by GraphLite functions.

**Parameters:**
- `s`: String to free (can be NULL)

---

#### `graphlite_close`
```c
void graphlite_close(GraphLiteDB* db);
```
Closes the database and frees all resources.

**Parameters:**
- `db`: Database handle to close (can be NULL)

---

#### `graphlite_version`
```c
const char* graphlite_version(void);
```
Returns the GraphLite version string.

**Returns:** Static version string (do NOT free)

### Error Codes

```c
typedef enum {
    Success = 0,          // Operation succeeded
    NullPointer = 1,      // Null pointer passed
    InvalidUtf8 = 2,      // Invalid UTF-8 string
    DatabaseOpenError = 3, // Failed to open database
    SessionError = 4,     // Session operation failed
    QueryError = 5,       // Query execution failed
    PanicError = 6,       // Internal panic occurred
    JsonError = 7,        // JSON serialization failed
} GraphLiteErrorCode;
```

## Memory Management

**Important:** The FFI layer requires explicit memory management:

1. **Database handles** - Must be closed with `graphlite_close()`
2. **Session IDs** - Must be freed with `graphlite_free_string()`
3. **Query results** - Must be freed with `graphlite_free_string()`
4. **Error handling** - Check return values and error codes

**Memory Leak Example (DON'T DO THIS):**
```c
// DON'T: Memory leak - session_id not freed
char* session_id = graphlite_create_session(db, "user", NULL);
graphlite_close(db); // session_id leaked!
```

**Correct Usage:**
```c
// DO: Proper cleanup
char* session_id = graphlite_create_session(db, "user", &error);
if (session_id != NULL) {
    // ... use session ...
    graphlite_free_string(session_id); // Free before close
}
graphlite_close(db);
```

## Thread Safety

- **Database handle (`GraphLiteDB`)**: Thread-safe for concurrent queries
- **Session IDs**: Can be used from any thread
- **Strings**: Must not be accessed after being freed

## Building Language Bindings

This FFI layer is designed as the foundation for language-specific bindings:

- **Python**: Use `ctypes` or PyO3
- **Swift**: Use C interop or UniFFI
- **Java**: Use JNA (Java Native Access)
- **JavaScript**: Use Node.js N-API (neon)
- **Go**: Use `cgo`
- **WASM**: Use `wasm-bindgen`

See the `bindings/` directory for language-specific implementations.

## Testing

Run the Rust test suite:

```bash
cargo test
```

For C integration tests, see `tests/c_integration/`.

## License

Apache-2.0
