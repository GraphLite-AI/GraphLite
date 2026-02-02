/**
 * SDK-specific error classes
 * 
 * These wrap errors from the low-level bindings with additional context
 * and follow Node.js error conventions.
 */

/**
 * Base error class for all GraphLite SDK errors
 */
export class GraphLiteSDKError extends Error {
  public readonly cause?: Error;

  constructor(message: string, cause?: Error) {
    super(message);
    this.name = 'GraphLiteSDKError';
    this.cause = cause;

    // Maintains proper stack trace for where our error was thrown (V8 only)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }
}

/**
 * Error thrown when database connection fails
 */
export class ConnectionError extends GraphLiteSDKError {
  constructor(message: string, cause?: Error) {
    super(message, cause);
    this.name = 'ConnectionError';
  }
}

/**
 * Error thrown when session operations fail
 */
export class SessionError extends GraphLiteSDKError {
  constructor(message: string, cause?: Error) {
    super(message, cause);
    this.name = 'SessionError';
  }
}

/**
 * Error thrown when query execution fails
 */
export class QueryError extends GraphLiteSDKError {
  constructor(message: string, cause?: Error) {
    super(message, cause);
    this.name = 'QueryError';
  }
}

/**
 * Error thrown when transaction operations fail
 */
export class TransactionError extends GraphLiteSDKError {
  constructor(message: string, cause?: Error) {
    super(message, cause);
    this.name = 'TransactionError';
  }
}

/**
 * Error thrown when result deserialization fails
 */
export class DeserializationError extends GraphLiteSDKError {
  constructor(message: string, cause?: Error) {
    super(message, cause);
    this.name = 'DeserializationError';
  }
}
