/**
 * Database connection management
 * 
 * This module provides the main entry point for working with GraphLite databases.
 * It follows SQLite patterns adapted for graph databases.
 */

import { GraphLite as BindingGraphLite } from 'graphlite';
import { ConnectionError, SessionError } from './errors';
import { Session } from './session';

/**
 * Main entry point for GraphLite database operations
 * 
 * Represents a connection to a GraphLite database. Despite being an embedded
 * database, we use "connection" terminology following the SQLite convention.
 * 
 * @example
 * ```typescript
 * import { GraphLite } from '@graphlite/sdk';
 * 
 * // Open a database
 * const db = GraphLite.open('./mydb');
 * 
 * // Create a session for a user
 * const session = db.session('admin');
 * 
 * // Execute queries
 * const result = await session.query('MATCH (n:Person) RETURN n');
 * 
 * // Cleanup
 * db.close();
 * ```
 */
export class GraphLite {
  private _db: BindingGraphLite | null;
  private _closed: boolean;

  /**
   * Internal constructor - use GraphLite.open() instead
   */
  private constructor(db: BindingGraphLite) {
    this._db = db;
    this._closed = false;
  }

  /**
   * Open a GraphLite database at the given path
   * 
   * Creates or opens a database at the specified path and initializes
   * all necessary components. This is the main entry point for working
   * with GraphLite databases.
   * 
   * @param path - Path to the database directory
   * @returns GraphLite instance
   * @throws {ConnectionError} If database cannot be opened
   * 
   * @example
   * ```typescript
   * const db = GraphLite.open('./mydb');
   * ```
   */
  public static open(path: string): GraphLite {
    try {
      const db = new BindingGraphLite(path);
      return new GraphLite(db);
    } catch (error) {
      throw new ConnectionError(
        `Failed to open database at ${path}`,
        error as Error
      );
    }
  }

  /**
   * Create a new session for the given user
   * 
   * Sessions provide user context for permissions and security. Each session
   * maintains its own transaction state and is isolated from other sessions.
   * 
   * Unlike SQLite which doesn't have sessions, GraphLite uses sessions for:
   * - User authentication and permissions
   * - Transaction isolation
   * - Audit logging
   * 
   * @param username - Username for the session
   * @returns Session instance
   * @throws {SessionError} If session creation fails
   * 
   * @example
   * ```typescript
   * const session = db.session('admin');
   * const result = await session.query('MATCH (n) RETURN n');
   * ```
   */
  public session(username: string): Session {
    this._ensureOpen();

    try {
      const sessionId = this._db!.createSession(username);
      return new Session(sessionId, this._db!, username);
    } catch (error) {
      throw new SessionError(
        `Failed to create session for user '${username}'`,
        error as Error
      );
    }
  }

  /**
   * Close the database and cleanup resources
   * 
   * After calling close(), the database cannot be used anymore.
   * All active sessions will be closed.
   * 
   * @example
   * ```typescript
   * const db = GraphLite.open('./mydb');
   * try {
   *   // ... use database
   * } finally {
   *   db.close();
   * }
   * ```
   */
  public close(): void {
    if (this._closed || !this._db) {
      return;
    }

    this._db.close();
    this._db = null;
    this._closed = true;
  }

  /**
   * Check if database is open
   * 
   * @returns true if database is open, false otherwise
   */
  public isOpen(): boolean {
    return !this._closed && this._db !== null;
  }

  /**
   * Get GraphLite version
   * 
   * @returns Version string
   * 
   * @example
   * ```typescript
   * console.log(`GraphLite version: ${GraphLite.version()}`);
   * ```
   */
  public static version(): string {
    return BindingGraphLite.version();
  }

  /**
   * Ensure database is open, throw error if closed
   * @private
   */
  private _ensureOpen(): void {
    if (this._closed) {
      throw new ConnectionError('Database is closed');
    }
    if (!this._db) {
      throw new ConnectionError('Database handle is null');
    }
  }
}
