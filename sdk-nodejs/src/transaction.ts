/**
 * Transaction support with ACID guarantees
 * 
 * This module provides transaction support following the rusqlite pattern:
 * - Transactions automatically roll back when disposed (unless committed)
 * - Explicit commit() required to persist changes
 * - Supports TypeScript 5.2+ using/await using for automatic cleanup
 */

import { Session } from './session';
import { TransactionError } from './errors';
import { QueryResult, TransactionState } from './types';

/**
 * Represents an active database transaction
 * 
 * Transactions provide ACID guarantees for multi-statement operations.
 * Following the rusqlite pattern:
 * - Transactions automatically **roll back** when disposed
 * - Must explicitly call `commit()` to persist changes
 * - This prevents accidentally forgetting to commit or rollback
 * 
 * @example
 * ```typescript
 * // TypeScript 5.2+ with 'using' (recommended)
 * await using tx = await session.transaction();
 * await tx.execute("INSERT (:Person {name: 'Alice'})");
 * await tx.execute("INSERT (:Person {name: 'Bob'})");
 * await tx.commit(); // Changes are persisted
 * // Automatic rollback on scope exit if not committed
 * 
 * // Legacy pattern (works on all TypeScript versions)
 * const tx = await session.transaction();
 * try {
 *   await tx.execute("INSERT (:Person {name: 'Alice'})");
 *   await tx.execute("INSERT (:Person {name: 'Bob'})");
 *   await tx.commit();
 * } catch (error) {
 *   await tx.rollback();
 *   throw error;
 * }
 * ```
 */
export class Transaction {
  private readonly _session: Session;
  private _state: TransactionState;

  /**
   * Internal constructor - use Session.transaction() instead
   */
  private constructor(session: Session) {
    this._session = session;
    this._state = TransactionState.Active;
  }

  /**
   * Begin a new transaction
   * 
   * This is called internally by Session.transaction().
   * The transaction will automatically roll back when disposed unless committed.
   * 
   * @internal
   */
  public static async begin(session: Session): Promise<Transaction> {
    const tx = new Transaction(session);
    
    try {
      // Execute BEGIN TRANSACTION
      await session.execute('BEGIN TRANSACTION');
    } catch (error) {
      throw new TransactionError(
        'Failed to begin transaction',
        error as Error
      );
    }

    return tx;
  }

  /**
   * Execute a GQL statement within this transaction
   * 
   * @param statement - GQL statement to execute
   * @throws {TransactionError} If transaction is already finished or execution fails
   * 
   * @example
   * ```typescript
   * await using tx = await session.transaction();
   * await tx.execute("INSERT (:Person {name: 'Alice'})");
   * await tx.execute("INSERT (:Person {name: 'Bob'})");
   * await tx.commit();
   * ```
   */
  public async execute(statement: string): Promise<void> {
    this._ensureActive();

    try {
      await this._session.execute(statement);
    } catch (error) {
      throw new TransactionError(
        `Execute failed: ${statement.substring(0, 100)}`,
        error as Error
      );
    }
  }

  /**
   * Execute a query within this transaction and return results
   * 
   * @param query - GQL query to execute
   * @returns Query results
   * @throws {TransactionError} If transaction is already finished or query fails
   * 
   * @example
   * ```typescript
   * await using tx = await session.transaction();
   * await tx.execute("INSERT (:Person {name: 'Alice', age: 30})");
   * const result = await tx.query("MATCH (p:Person) RETURN p");
   * await tx.commit();
   * ```
   */
  public async query(query: string): Promise<QueryResult> {
    this._ensureActive();

    try {
      return await this._session.query(query);
    } catch (error) {
      throw new TransactionError(
        `Query failed: ${query.substring(0, 100)}`,
        error as Error
      );
    }
  }

  /**
   * Commit the transaction
   * 
   * Persists all changes made within this transaction. After calling commit(),
   * the transaction cannot be used further.
   * 
   * @throws {TransactionError} If transaction is already finished or commit fails
   * 
   * @example
   * ```typescript
   * await using tx = await session.transaction();
   * await tx.execute("INSERT (:Person {name: 'Alice'})");
   * await tx.commit(); // Changes are now persistent
   * ```
   */
  public async commit(): Promise<void> {
    this._ensureActive();

    try {
      await this._session.execute('COMMIT');
      this._state = TransactionState.Committed;
    } catch (error) {
      throw new TransactionError('Failed to commit transaction', error as Error);
    }
  }

  /**
   * Rollback the transaction
   * 
   * Discards all changes made within this transaction. This is called
   * automatically when the transaction is disposed, so explicit rollback
   * is rarely needed.
   * 
   * @throws {TransactionError} If rollback fails
   * 
   * @example
   * ```typescript
   * const tx = await session.transaction();
   * try {
   *   await tx.execute("INSERT (:Person {name: 'Alice'})");
   *   throw new Error('Something went wrong');
   * } catch (error) {
   *   await tx.rollback(); // Explicit rollback
   *   throw error;
   * }
   * ```
   */
  public async rollback(): Promise<void> {
    // Already committed or rolled back, nothing to do
    if (this._state !== TransactionState.Active) {
      return;
    }

    try {
      await this._session.execute('ROLLBACK');
      this._state = TransactionState.RolledBack;
    } catch (error) {
      throw new TransactionError('Failed to rollback transaction', error as Error);
    }
  }

  /**
   * Get current transaction state
   * 
   * @returns Current state (ACTIVE, COMMITTED, or ROLLED_BACK)
   */
  public state(): TransactionState {
    return this._state;
  }

  /**
   * Check if transaction is active
   * 
   * @returns true if transaction is active, false otherwise
   */
  public isActive(): boolean {
    return this._state === TransactionState.Active;
  }

  /**
   * Symbol.asyncDispose implementation for TypeScript 5.2+ 'using' keyword
   * 
   * This enables automatic rollback when transaction goes out of scope.
   * 
   * @example
   * ```typescript
   * await using tx = await session.transaction();
   * await tx.execute("INSERT ...");
   * // Automatic rollback here if not committed
   * ```
   */
  public async [Symbol.asyncDispose](): Promise<void> {
    // Auto-rollback if transaction is still active
    if (this._state === TransactionState.Active) {
      try {
        await this.rollback();
      } catch (error) {
        // Ignore rollback errors during disposal
        // (transaction may already be invalid)
      }
    }
  }

  /**
   * Ensure transaction is active, throw error if not
   * @private
   */
  private _ensureActive(): void {
    if (this._state === TransactionState.Committed) {
      throw new TransactionError('Transaction already committed');
    }
    if (this._state === TransactionState.RolledBack) {
      throw new TransactionError('Transaction already rolled back');
    }
  }
}
