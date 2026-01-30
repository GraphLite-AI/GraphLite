/**
 * TypeScript definitions for GraphLite Node.js bindings
 */

declare module 'graphlite' {
  /**
   * Error codes returned by GraphLite operations
   */
  export enum ErrorCode {
    SUCCESS = 0,
    NULL_POINTER = 1,
    INVALID_UTF8 = 2,
    DATABASE_OPEN_ERROR = 3,
    SESSION_ERROR = 4,
    QUERY_ERROR = 5,
    PANIC_ERROR = 6,
    JSON_ERROR = 7,
  }

  /**
   * GraphLite error class
   */
  export class GraphLiteError extends Error {
    code: number;
    errorName: string;

    constructor(code: number, message: string);

    static fromErrorCode(code: number, context?: string): GraphLiteError;
    isCode(code: number): boolean;
    toJSON(): {
      name: string;
      code: number;
      errorName: string;
      message: string;
      stack?: string;
    };
  }

  /**
   * Query result with convenient access methods
   */
  export class QueryResult {
    variables: string[];
    rows: Array<Record<string, any>>;
    rowCount: number;

    constructor(data: any);

    /**
     * Get the first row or null if no rows
     */
    first(): Record<string, any> | null;

    /**
     * Get all values from a specific column
     */
    column(columnName: string): any[];

    /**
     * Convert to array of rows
     */
    toArray(): Array<Record<string, any>>;

    /**
     * Get raw JSON representation
     */
    toJSON(): any;

    /**
     * Get a subset of columns from all rows
     */
    select(...columnNames: string[]): Array<Record<string, any>>;

    /**
     * Filter rows based on a predicate function
     */
    filter(predicate: (row: Record<string, any>) => boolean): Array<Record<string, any>>;

    /**
     * Map rows using a transformer function
     */
    map<T>(transformer: (row: Record<string, any>) => T): T[];

    /**
     * Check if result has any rows
     */
    isEmpty(): boolean;

    /**
     * Get string representation
     */
    toString(): string;

    /**
     * Iterator support
     */
    [Symbol.iterator](): Iterator<Record<string, any>>;
  }

  /**
   * GraphLite database connection
   */
  export class GraphLite {
    /**
     * Create and open a GraphLite database
     * @param path - Path to database directory
     * @throws {GraphLiteError} If database cannot be opened
     */
    constructor(path: string);

    /**
     * Create a new session for the given user
     * @param username - Username for the session
     * @returns Session ID
     * @throws {GraphLiteError} If session creation fails
     */
    createSession(username: string): string;

    /**
     * Execute a GQL query and return results
     * @param sessionId - Session ID from createSession()
     * @param query - GQL query string
     * @returns Query results
     * @throws {GraphLiteError} If query execution fails
     */
    query(sessionId: string, query: string): Promise<QueryResult>;

    /**
     * Execute a statement without returning results
     * @param sessionId - Session ID from createSession()
     * @param statement - GQL statement to execute
     * @throws {GraphLiteError} If execution fails
     */
    execute(sessionId: string, statement: string): Promise<void>;

    /**
     * Close a session
     * @param sessionId - Session ID to close
     * @throws {GraphLiteError} If session close fails
     */
    closeSession(sessionId: string): Promise<void>;

    /**
     * Close the database and all open sessions
     */
    close(): void;

    /**
     * Get GraphLite version
     */
    static version(): string;

    /**
     * Check if database is open
     */
    isOpen(): boolean;

    /**
     * Get number of open sessions
     */
    getSessionCount(): number;
  }

  export default GraphLite;
}
