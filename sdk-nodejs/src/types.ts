/**
 * TypeScript type definitions for the SDK
 */

import { QueryResult as BindingQueryResult } from 'graphlite';

/**
 * Re-export QueryResult from bindings
 */
export type QueryResult = BindingQueryResult;

/**
 * Constructor type for classes that can be instantiated
 */
export type Constructor<T = any> = new (...args: any[]) => T;

/**
 * Row data from query results
 */
export type Row = Record<string, any>;

/**
 * Transaction state
 */
export enum TransactionState {
  Active = 'ACTIVE',
  Committed = 'COMMITTED',
  RolledBack = 'ROLLED_BACK',
}

/**
 * Options for opening a database
 */
export interface DatabaseOptions {
  /** Whether to create the database if it doesn't exist */
  create?: boolean;
}

/**
 * Options for creating a session
 */
export interface SessionOptions {
  /** Session timeout in milliseconds */
  timeout?: number;
}
