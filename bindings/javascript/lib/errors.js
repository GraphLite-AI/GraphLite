/**
 * Error codes and error handling for GraphLite
 */

/**
 * GraphLite error codes matching the C FFI layer
 */
class ErrorCode {
  static SUCCESS = 0;
  static NULL_POINTER = 1;
  static INVALID_UTF8 = 2;
  static DATABASE_OPEN_ERROR = 3;
  static SESSION_ERROR = 4;
  static QUERY_ERROR = 5;
  static PANIC_ERROR = 6;
  static JSON_ERROR = 7;

  /**
   * Get human-readable name for error code
   * @param {number} code - Error code
   * @returns {string} Error name
   */
  static getName(code) {
    const names = {
      0: 'SUCCESS',
      1: 'NULL_POINTER',
      2: 'INVALID_UTF8',
      3: 'DATABASE_OPEN_ERROR',
      4: 'SESSION_ERROR',
      5: 'QUERY_ERROR',
      6: 'PANIC_ERROR',
      7: 'JSON_ERROR',
    };
    return names[code] || 'UNKNOWN_ERROR';
  }

  /**
   * Get descriptive message for error code
   * @param {number} code - Error code
   * @returns {string} Error description
   */
  static getDescription(code) {
    const descriptions = {
      0: 'Operation succeeded',
      1: 'Null pointer was passed',
      2: 'Invalid UTF-8 string',
      3: 'Failed to open database',
      4: 'Failed to create or manage session',
      5: 'Query execution failed',
      6: 'Internal panic occurred',
      7: 'JSON serialization/deserialization failed',
    };
    return descriptions[code] || 'Unknown error occurred';
  }
}

/**
 * GraphLite error class
 */
class GraphLiteError extends Error {
  /**
   * Create a GraphLite error
   * @param {number} code - Error code from ErrorCode
   * @param {string} message - Error message
   */
  constructor(code, message) {
    const errorName = ErrorCode.getName(code);
    const fullMessage = `GraphLite error (${errorName}): ${message}`;
    super(fullMessage);
    
    this.name = 'GraphLiteError';
    this.code = code;
    this.errorName = errorName;
    
    // Maintains proper stack trace for where error was thrown (V8 only)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, GraphLiteError);
    }
  }

  /**
   * Create error from FFI error code
   * @param {number} code - FFI error code
   * @param {string} context - Context message
   * @returns {GraphLiteError}
   */
  static fromErrorCode(code, context = '') {
    const description = ErrorCode.getDescription(code);
    const message = context ? `${context}: ${description}` : description;
    return new GraphLiteError(code, message);
  }

  /**
   * Check if this is a specific error code
   * @param {number} code - Error code to check
   * @returns {boolean}
   */
  isCode(code) {
    return this.code === code;
  }

  /**
   * Convert to JSON representation
   * @returns {Object}
   */
  toJSON() {
    return {
      name: this.name,
      code: this.code,
      errorName: this.errorName,
      message: this.message,
      stack: this.stack,
    };
  }
}

module.exports = {
  ErrorCode,
  GraphLiteError,
};
