/**
 * GraphLite - Node.js bindings for GraphLite embedded graph database
 * 
 * High-level JavaScript API wrapping the C FFI layer.
 */

const { loadLibrary } = require('./ffi-loader');
const { GraphLiteError, ErrorCode } = require('./errors');
const QueryResult = require('./query-result');

// Load the FFI library once
let _lib;
try {
  _lib = loadLibrary();
} catch (error) {
  // Library loading will fail if FFI lib is not built
  // We'll throw a more helpful error when GraphLite is instantiated
  _lib = null;
}

/**
 * GraphLite database connection
 * 
 * @example
 * const db = new GraphLite('./mydb');
 * const session = db.createSession('admin');
 * const result = await db.query(session, 'MATCH (n) RETURN n');
 * console.log(result.rows);
 * db.close();
 * 
 * @example
 * // Using async/await
 * async function queryDatabase() {
 *   const db = new GraphLite('./mydb');
 *   try {
 *     const session = db.createSession('user');
 *     const result = await db.query(session, 'MATCH (p:Person) RETURN p');
 *     return result.rows;
 *   } finally {
 *     db.close();
 *   }
 * }
 */
class GraphLite {
  /**
   * Create and open a GraphLite database
   * @param {string} path - Path to database directory
   * @throws {GraphLiteError} If database cannot be opened
   */
  constructor(path) {
    if (!_lib) {
      throw new GraphLiteError(
        ErrorCode.DATABASE_OPEN_ERROR,
        'GraphLite FFI library not loaded. Please build it first: cargo build --release -p graphlite-ffi'
      );
    }

    if (!path || typeof path !== 'string') {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Database path must be a non-empty string'
      );
    }

    this._db = null;
    this._sessions = new Set();
    this._closed = false;

    // Open database
    const errorCode = [0]; // Array to hold error code
    this._db = _lib.graphlite_open(path, errorCode);

    if (!this._db) {
      throw GraphLiteError.fromErrorCode(
        errorCode[0],
        `Failed to open database at ${path}`
      );
    }
  }

  /**
   * Create a new session for the given user
   * @param {string} username - Username for the session
   * @returns {string} Session ID
   * @throws {GraphLiteError} If session creation fails
   */
  createSession(username) {
    this._ensureOpen();

    if (!username || typeof username !== 'string') {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Username must be a non-empty string'
      );
    }

    const errorCode = [0];
    const sessionIdPtr = _lib.graphlite_create_session(
      this._db,
      username,
      errorCode
    );

    if (!sessionIdPtr) {
      throw GraphLiteError.fromErrorCode(
        errorCode[0],
        `Failed to create session for user '${username}'`
      );
    }

    // String is automatically managed by koffi
    const sessionId = sessionIdPtr;
    
    this._sessions.add(sessionId);
    return sessionId;
  }

  /**
   * Execute a GQL query and return results
   * @param {string} sessionId - Session ID from createSession()
   * @param {string} query - GQL query string
   * @returns {Promise<QueryResult>} Query results
   * @throws {GraphLiteError} If query execution fails
   */
  async query(sessionId, query) {
    return this._querySync(sessionId, query);
  }

  /**
   * Execute a GQL query synchronously (internal)
   * @param {string} sessionId - Session ID
   * @param {string} query - GQL query
   * @returns {QueryResult} Query results
   * @throws {GraphLiteError} If query fails
   * @private
   */
  _querySync(sessionId, query) {
    this._ensureOpen();

    if (!sessionId || typeof sessionId !== 'string') {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Session ID must be a non-empty string'
      );
    }

    if (!query || typeof query !== 'string') {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Query must be a non-empty string'
      );
    }

    const errorCode = [0];
    const resultPtr = _lib.graphlite_query(
      this._db,
      sessionId,
      query,
      errorCode
    );

    // Check error code first
    if (errorCode[0] !== ErrorCode.SUCCESS) {
      const queryPreview = query.length > 100 ? query.substring(0, 100) + '...' : query;
      throw GraphLiteError.fromErrorCode(
        errorCode[0],
        `Query failed: ${queryPreview}`
      );
    }

    // If no error but no result, return empty result (for DDL/DML statements)
    if (!resultPtr) {
      return new QueryResult({ variables: [], rows: [] });
    }

    try {
      // Parse JSON result (koffi automatically handles string conversion)
      const resultJson = resultPtr;
      const resultData = JSON.parse(resultJson);
      return new QueryResult(resultData);
    } catch (error) {
      if (error instanceof GraphLiteError) {
        throw error;
      }
      throw new GraphLiteError(
        ErrorCode.JSON_ERROR,
        `Failed to parse query result: ${error.message}`
      );
    }
  }

  /**
   * Execute a statement without returning results
   * Useful for DDL/DML statements that don't return data
   * @param {string} sessionId - Session ID from createSession()
   * @param {string} statement - GQL statement to execute
   * @returns {Promise<void>}
   * @throws {GraphLiteError} If execution fails
   */
  async execute(sessionId, statement) {
    await this.query(sessionId, statement);
  }

  /**
   * Close a session
   * @param {string} sessionId - Session ID to close
   * @returns {Promise<void>}
   * @throws {GraphLiteError} If session close fails
   */
  async closeSession(sessionId) {
    if (this._closed || !this._db) {
      return;
    }

    if (!sessionId || typeof sessionId !== 'string') {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Session ID must be a non-empty string'
      );
    }

    const errorCode = [0];
    const result = _lib.graphlite_close_session(
      this._db,
      sessionId,
      errorCode
    );

    if (result !== 0) {
      throw GraphLiteError.fromErrorCode(
        errorCode[0],
        `Failed to close session ${sessionId}`
      );
    }

    this._sessions.delete(sessionId);
  }

  /**
   * Close the database and all open sessions
   */
  close() {
    if (this._closed || !this._db) {
      return;
    }

    // Close all open sessions (ignore errors during cleanup)
    for (const sessionId of Array.from(this._sessions)) {
      try {
        const errorCode = [0];
        _lib.graphlite_close_session(this._db, sessionId, errorCode);
      } catch (error) {
        // Ignore errors during cleanup
      }
    }
    this._sessions.clear();

    // Close database
    _lib.graphlite_close(this._db);
    this._db = null;
    this._closed = true;
  }

  /**
   * Get GraphLite version
   * @returns {string} Version string
   */
  static version() {
    if (!_lib) {
      return 'unknown (FFI library not loaded)';
    }

    const versionPtr = _lib.graphlite_version();
    if (!versionPtr) {
      return 'unknown';
    }

    // Version string is static, managed by koffi
    return versionPtr;
  }

  /**
   * Check if database is open
   * @returns {boolean}
   */
  isOpen() {
    return !this._closed && this._db !== null;
  }

  /**
   * Get number of open sessions
   * @returns {number}
   */
  getSessionCount() {
    return this._sessions.size;
  }

  /**
   * Ensure database is open, throw error if closed
   * @private
   */
  _ensureOpen() {
    if (this._closed) {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Database is closed'
      );
    }
    if (!this._db) {
      throw new GraphLiteError(
        ErrorCode.NULL_POINTER,
        'Database handle is null'
      );
    }
  }
}

module.exports = GraphLite;
