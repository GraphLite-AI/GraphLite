#ifndef GRAPHLITE_H
#define GRAPHLITE_H

#pragma once

/**
 * Error codes returned by FFI functions
 */
typedef enum GraphLiteErrorCode {
  /**
   * Operation succeeded
   */
  Success = 0,
  /**
   * Null pointer was passed
   */
  NullPointer = 1,
  /**
   * Invalid UTF-8 string
   */
  InvalidUtf8 = 2,
  /**
   * Failed to open database
   */
  DatabaseOpenError = 3,
  /**
   * Failed to create session
   */
  SessionError = 4,
  /**
   * Query execution failed
   */
  QueryError = 5,
  /**
   * Internal panic occurred
   */
  PanicError = 6,
  /**
   * JSON serialization failed
   */
  JsonError = 7,
} GraphLiteErrorCode;

typedef struct Arc_QueryCoordinator Arc_QueryCoordinator;

/**
 * Opaque handle to a GraphLite database instance
 *
 * This handle wraps a QueryCoordinator and must be freed with `graphlite_close`
 */
typedef struct GraphLiteDB {
  struct Arc_QueryCoordinator coordinator;
} GraphLiteDB;

/**
 * Initialize GraphLite database from path
 *
 * # Arguments
 * * `path` - C string with database path (must not be null)
 * * `error_out` - Output parameter for error code (can be null if caller doesn't need it)
 *
 * # Returns
 * * Opaque handle to database on success
 * * null pointer on error (check `error_out` for details)
 *
 * # Safety
 * * `path` must be a valid null-terminated C string
 * * Returned handle must be freed with `graphlite_close`
 *
 * # Example
 * ```c
 * GraphLiteErrorCode error;
 * GraphLiteDB* db = graphlite_open("/path/to/db", &error);
 * if (db == NULL) {
 *     printf("Error: %d\n", error);
 *     return -1;
 * }
 * // ... use database ...
 * graphlite_close(db);
 * ```
 */
struct GraphLiteDB *graphlite_open(const char *path, enum GraphLiteErrorCode *error_out);

/**
 * Create a simple session for the given username
 *
 * # Arguments
 * * `db` - Database handle (must not be null)
 * * `username` - C string with username (must not be null)
 * * `error_out` - Output parameter for error code (can be null)
 *
 * # Returns
 * * C string with session ID on success (must be freed with `graphlite_free_string`)
 * * null pointer on error
 *
 * # Safety
 * * `db` must be a valid handle from `graphlite_open`
 * * `username` must be a valid null-terminated C string
 * * Returned string must be freed with `graphlite_free_string`
 */
char *graphlite_create_session(struct GraphLiteDB *db,
                               const char *username,
                               enum GraphLiteErrorCode *error_out);

/**
 * Execute a GQL query and return results as JSON
 *
 * # Arguments
 * * `db` - Database handle (must not be null)
 * * `session_id` - C string with session ID (must not be null)
 * * `query` - C string with GQL query (must not be null)
 * * `error_out` - Output parameter for error code (can be null)
 *
 * # Returns
 * * JSON string with query results on success (must be freed with `graphlite_free_string`)
 * * null pointer on error
 *
 * # Safety
 * * `db` must be a valid handle from `graphlite_open`
 * * `session_id` must be from `graphlite_create_session`
 * * `query` must be a valid null-terminated C string
 * * Returned JSON string must be freed with `graphlite_free_string`
 *
 * # JSON Format
 * ```json
 * {
 *   "variables": ["col1", "col2"],
 *   "rows": [
 *     {"col1": "value1", "col2": 123},
 *     {"col1": "value2", "col2": 456}
 *   ],
 *   "row_count": 2
 * }
 * ```
 */
char *graphlite_query(struct GraphLiteDB *db,
                      const char *session_id,
                      const char *query,
                      enum GraphLiteErrorCode *error_out);

/**
 * Close a session
 *
 * # Arguments
 * * `db` - Database handle (must not be null)
 * * `session_id` - C string with session ID (must not be null)
 * * `error_out` - Output parameter for error code (can be null)
 *
 * # Returns
 * * Error code (Success = 0, error otherwise)
 *
 * # Safety
 * * `db` must be a valid handle from `graphlite_open`
 * * `session_id` must be from `graphlite_create_session`
 */
enum GraphLiteErrorCode graphlite_close_session(struct GraphLiteDB *db,
                                                const char *session_id,
                                                enum GraphLiteErrorCode *error_out);

/**
 * Free a string returned by GraphLite FFI functions
 *
 * # Arguments
 * * `s` - C string to free (can be null, in which case this is a no-op)
 *
 * # Safety
 * * `s` must be a string returned by a GraphLite FFI function
 * * Must not be called more than once on the same string
 * * Must not be called on strings not allocated by GraphLite
 */
void graphlite_free_string(char *s);

/**
 * Close database connection and free resources
 *
 * # Arguments
 * * `db` - Database handle to close (can be null, in which case this is a no-op)
 *
 * # Safety
 * * `db` must be a handle from `graphlite_open`
 * * Must not be called more than once on the same handle
 * * Must not be used after calling this function
 */
void graphlite_close(struct GraphLiteDB *db);

/**
 * Get the version string of GraphLite
 *
 * # Returns
 * * Static C string with version (e.g., "0.1.0")
 * * Must NOT be freed (it's a static string)
 */
const char *graphlite_version(void);

#endif  /* GRAPHLITE_H */
