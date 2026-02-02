/**
 * Query builder for fluent GQL query construction
 * 
 * This module provides a builder API for constructing GQL queries in a
 * type-safe and ergonomic way.
 */

import { Session } from './session';
import { QueryResult } from './types';

/**
 * Fluent API for building GQL queries
 * 
 * QueryBuilder provides a convenient way to construct complex GQL queries
 * without manually concatenating strings.
 * 
 * @example
 * ```typescript
 * const result = await session.queryBuilder()
 *   .match('(p:Person)')
 *   .where('p.age > 25')
 *   .where('p.city = "NYC"')
 *   .return('p.name, p.age, p.city')
 *   .orderBy('p.age DESC')
 *   .limit(10)
 *   .execute();
 * ```
 */
export class QueryBuilder {
  private readonly _session: Session;
  private _matchPatterns: string[] = [];
  private _whereClauses: string[] = [];
  private _withClauses: string[] = [];
  private _returnClause: string | null = null;
  private _orderBy: string | null = null;
  private _skip: number | null = null;
  private _limit: number | null = null;

  /**
   * Internal constructor - use Session.queryBuilder() instead
   */
  constructor(session: Session) {
    this._session = session;
  }

  /**
   * Add a MATCH pattern
   * 
   * Can be called multiple times to add multiple MATCH patterns.
   * 
   * @param pattern - Graph pattern to match (without the MATCH keyword)
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .match('(p)-[:KNOWS]->(f:Person)')
   *   .return('p.name, f.name');
   * ```
   */
  public match(pattern: string): this {
    this._matchPatterns.push(pattern);
    return this;
  }

  /**
   * Add a WHERE clause condition
   * 
   * Can be called multiple times - conditions are AND'ed together.
   * 
   * @param condition - Condition to add (without the WHERE keyword)
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .where('p.age > 25')
   *   .where('p.name STARTS WITH "A"')
   *   .return('p');
   * ```
   */
  public where(condition: string): this {
    this._whereClauses.push(condition);
    return this;
  }

  /**
   * Add a WITH clause
   * 
   * WITH clauses are used for query chaining and intermediate results.
   * 
   * @param clause - WITH clause content (without the WITH keyword)
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .with('p, p.age as age')
   *   .where('age > 25')
   *   .return('p.name');
   * ```
   */
  public with(clause: string): this {
    this._withClauses.push(clause);
    return this;
  }

  /**
   * Set the RETURN clause
   * 
   * Specifies what to return from the query. Required for MATCH queries.
   * 
   * @param clause - Return clause content (without the RETURN keyword)
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .return('p.name, p.age, p.city');
   * ```
   */
  public return(clause: string): this {
    this._returnClause = clause;
    return this;
  }

  /**
   * Set the ORDER BY clause
   * 
   * @param clause - ORDER BY clause content (without the ORDER BY keywords)
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .return('p.name, p.age')
   *   .orderBy('p.age DESC, p.name ASC');
   * ```
   */
  public orderBy(clause: string): this {
    this._orderBy = clause;
    return this;
  }

  /**
   * Set the SKIP value
   * 
   * Skips the first N results.
   * 
   * @param n - Number of results to skip
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .return('p.name')
   *   .orderBy('p.name')
   *   .skip(10)
   *   .limit(10);
   * ```
   */
  public skip(n: number): this {
    this._skip = n;
    return this;
  }

  /**
   * Set the LIMIT value
   * 
   * Limits the number of results returned.
   * 
   * @param n - Maximum number of results to return
   * @returns this for chaining
   * 
   * @example
   * ```typescript
   * builder
   *   .match('(p:Person)')
   *   .return('p.name')
   *   .limit(100);
   * ```
   */
  public limit(n: number): this {
    this._limit = n;
    return this;
  }

  /**
   * Build the query string without executing
   * 
   * Returns the constructed GQL query as a string.
   * 
   * @returns Constructed GQL query string
   * 
   * @example
   * ```typescript
   * const query = builder
   *   .match('(p:Person)')
   *   .where('p.age > 25')
   *   .return('p.name')
   *   .build();
   * 
   * console.log(query);
   * // Output: "MATCH (p:Person) WHERE p.age > 25 RETURN p.name"
   * ```
   */
  public build(): string {
    const parts: string[] = [];

    // MATCH clauses
    for (const pattern of this._matchPatterns) {
      parts.push(`MATCH ${pattern}`);
    }

    // WHERE clauses
    if (this._whereClauses.length > 0) {
      parts.push(`WHERE ${this._whereClauses.join(' AND ')}`);
    }

    // WITH clauses
    for (const clause of this._withClauses) {
      parts.push(`WITH ${clause}`);
    }

    // RETURN clause
    if (this._returnClause) {
      parts.push(`RETURN ${this._returnClause}`);
    }

    // ORDER BY clause
    if (this._orderBy) {
      parts.push(`ORDER BY ${this._orderBy}`);
    }

    // SKIP clause
    if (this._skip !== null) {
      parts.push(`SKIP ${this._skip}`);
    }

    // LIMIT clause
    if (this._limit !== null) {
      parts.push(`LIMIT ${this._limit}`);
    }

    return parts.join(' ');
  }

  /**
   * Execute the query and return results
   * 
   * @returns Query results
   * @throws {QueryError} If query execution fails
   * 
   * @example
   * ```typescript
   * const result = await session.queryBuilder()
   *   .match('(p:Person)')
   *   .where('p.age > 25')
   *   .return('p.name, p.age')
   *   .execute();
   * 
   * for (const row of result.rows) {
   *   console.log(`${row['p.name']}: ${row['p.age']}`);
   * }
   * ```
   */
  public async execute(): Promise<QueryResult> {
    const query = this.build();
    return await this._session.query(query);
  }
}
