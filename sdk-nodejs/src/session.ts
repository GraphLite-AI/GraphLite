/**
 * Session management for GraphLite database
 * 
 * Sessions provide user context and are required for executing queries.
 * Unlike SQLite, GraphLite uses sessions for user authentication, permissions,
 * and transaction isolation.
 */

import { GraphLite as BindingGraphLite, QueryResult } from 'graphlite';
import { QueryError } from './errors';
import { Transaction } from './transaction';
import { QueryBuilder } from './query-builder';

/**
 * Represents an active database session
 * 
 * Sessions provide user context for executing queries and managing transactions.
 * 
 * @example
 * ```typescript
 * const db = GraphLite.open('./mydb');
 * const session = db.session('admin');
 * 
 * const result = await session.query('MATCH (n:Person) RETURN n');
 * console.log(result.rows);
 * ```
 */
export class Session {
  private readonly _sessionId: string;
  private readonly _db: BindingGraphLite;
  private readonly _username: string;

  /**
   * Internal constructor - use GraphLite.session() instead
   */
  constructor(sessionId: string, db: BindingGraphLite, username: string) {
    this._sessionId = sessionId;
    this._db = db;
    this._username = username;
  }

  /**
   * Get the session ID
   */
  public id(): string {
    return this._sessionId;
  }

  /**
   * Get the username associated with this session
   */
  public username(): string {
    return this._username;
  }

  /**
   * Execute a GQL query and return results
   * 
   * @param query - GQL query string
   * @returns Query results
   * @throws {QueryError} If query execution fails
   * 
   * @example
   * ```typescript
   * const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
   * for (const row of result.rows) {
   *   console.log(`${row['p.name']}: ${row['p.age']}`);
   * }
   * ```
   */
  public async query(query: string): Promise<QueryResult> {
    try {
      return await this._db.query(this._sessionId, query);
    } catch (error) {
      throw new QueryError(
        `Query failed: ${query.substring(0, 100)}${query.length > 100 ? '...' : ''}`,
        error as Error
      );
    }
  }

  /**
   * Execute a statement without returning results
   * 
   * Useful for DDL statements (CREATE SCHEMA, CREATE GRAPH) and DML statements
   * where you don't need the results.
   * 
   * @param statement - GQL statement to execute
   * @throws {QueryError} If execution fails
   * 
   * @example
   * ```typescript
   * await session.execute('CREATE SCHEMA IF NOT EXISTS example');
   * await session.execute('SESSION SET SCHEMA example');
   * await session.execute('INSERT (:Person {name: "Alice", age: 30})');
   * ```
   */
  public async execute(statement: string): Promise<void> {
    try {
      await this._db.execute(this._sessionId, statement);
    } catch (error) {
      throw new QueryError(
        `Execute failed: ${statement.substring(0, 100)}${statement.length > 100 ? '...' : ''}`,
        error as Error
      );
    }
  }

  /**
   * Begin a new transaction
   * 
   * Returns a Transaction object that provides ACID guarantees.
   * The transaction will automatically roll back when disposed unless committed.
   * 
   * @returns Transaction instance
   * @throws {TransactionError} If transaction cannot be started
   * 
   * @example
   * ```typescript
   * // TypeScript 5.2+ with 'using'
   * await using tx = await session.transaction();
   * await tx.execute("INSERT (:Person {name: 'Alice'})");
   * await tx.commit();
   * 
   * // Legacy pattern
   * const tx = await session.transaction();
   * try {
   *   await tx.execute("INSERT (:Person {name: 'Alice'})");
   *   await tx.commit();
   * } finally {
   *   await tx.rollback(); // Auto-rollback if not committed
   * }
   * ```
   */
  public async transaction(): Promise<Transaction> {
    return Transaction.begin(this);
  }

  /**
   * Create a new query builder for fluent query construction
   * 
   * @returns QueryBuilder instance
   * 
   * @example
   * ```typescript
   * const result = await session.queryBuilder()
   *   .match('(p:Person)')
   *   .where('p.age > 25')
   *   .return('p.name, p.age')
   *   .orderBy('p.age DESC')
   *   .limit(10)
   *   .execute();
   * ```
   */
  public queryBuilder(): QueryBuilder {
    return new QueryBuilder(this);
  }

  /**
   * Get access to the underlying binding database instance
   * 
   * @internal
   * This is for internal use by Transaction and QueryBuilder
   */
  public _getDb(): BindingGraphLite {
    return this._db;
  }
}
