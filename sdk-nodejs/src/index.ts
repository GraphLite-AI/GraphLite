/**
 * GraphLite SDK - High-level Node.js API for GraphLite
 * 
 * This package provides a high-level, developer-friendly SDK on top of GraphLite's
 * FFI bindings. It offers ergonomic patterns, type safety, session management,
 * query builders, and transaction support.
 * 
 * @example
 * ```typescript
 * import { GraphLite } from '@graphlite/sdk';
 * 
 * // Open database
 * const db = GraphLite.open('./mydb');
 * 
 * // Create session
 * const session = db.session('admin');
 * 
 * // Execute query
 * const result = await session.query('MATCH (p:Person) RETURN p.name');
 * 
 * // Use transactions
 * await using tx = await session.transaction();
 * await tx.execute("INSERT (:Person {name: 'Alice'})");
 * await tx.commit();
 * ```
 * 
 * @module @graphlite/sdk
 */

// Core exports
export { GraphLite } from './connection';
export { Session } from './session';
export { Transaction } from './transaction';
export { QueryBuilder } from './query-builder';
export { TypedResult } from './typed-result';

// Error exports
export {
  GraphLiteSDKError,
  ConnectionError,
  SessionError,
  QueryError,
  TransactionError,
  DeserializationError,
} from './errors';

// Type exports
export {
  QueryResult,
  Constructor,
  Row,
  TransactionState,
  DatabaseOptions,
  SessionOptions,
} from './types';

// Re-export useful types from bindings
export { ErrorCode, GraphLiteError } from 'graphlite';
