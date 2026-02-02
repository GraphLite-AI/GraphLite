/**
 * Typed result deserialization
 * 
 * This module provides utilities for deserializing query results into
 * TypeScript classes with type safety.
 */

import { QueryResult, Row, Constructor } from './types';
import { DeserializationError } from './errors';

/**
 * Wrapper for query results with typed deserialization
 * 
 * TypedResult provides methods to deserialize query results into TypeScript
 * classes, making it easier to work with strongly-typed data.
 * 
 * @example
 * ```typescript
 * class Person {
 *   constructor(public name: string, public age: number) {}
 * }
 * 
 * const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
 * const typed = new TypedResult(result);
 * const people = typed.deserializeRows(Person);
 * 
 * for (const person of people) {
 *   console.log(`${person.name}: ${person.age}`);
 * }
 * ```
 */
export class TypedResult<T = any> {
  private readonly _result: QueryResult;

  /**
   * Create a TypedResult from a QueryResult
   * 
   * @param result - Query result from bindings
   */
  constructor(result: QueryResult) {
    this._result = result;
  }

  /**
   * Get all rows as plain objects
   * 
   * @returns Array of row objects
   */
  public rows(): Row[] {
    return this._result.rows;
  }

  /**
   * Get the number of rows
   * 
   * @returns Number of rows in the result
   */
  public rowCount(): number {
    return this._result.rowCount;
  }

  /**
   * Get column names from the result
   * 
   * @returns Array of column names
   */
  public columnNames(): string[] {
    return this._result.variables || [];
  }

  /**
   * Check if result is empty
   * 
   * @returns true if no rows, false otherwise
   */
  public isEmpty(): boolean {
    return this._result.rowCount === 0;
  }

  /**
   * Get a single row by index
   * 
   * @param index - Row index (0-based)
   * @returns Row object or undefined if index out of bounds
   */
  public getRow(index: number): Row | undefined {
    return this._result.rows[index];
  }

  /**
   * Deserialize all rows into instances of the given class
   * 
   * The class constructor should accept properties in the same order as
   * the query result columns, or the class should have properties that
   * match the column names.
   * 
   * @param type - Class constructor to instantiate
   * @returns Array of instances
   * @throws {DeserializationError} If deserialization fails
   * 
   * @example
   * ```typescript
   * class Person {
   *   constructor(public name: string, public age: number) {}
   * }
   * 
   * const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
   * const typed = new TypedResult(result);
   * const people = typed.deserializeRows(Person);
   * ```
   */
  public deserializeRows<U>(type: Constructor<U>): U[] {
    try {
      return this._result.rows.map(row => this.deserializeRow(row, type));
    } catch (error) {
      throw new DeserializationError(
        'Failed to deserialize rows',
        error as Error
      );
    }
  }

  /**
   * Deserialize a single row into an instance of the given class
   * 
   * @param row - Row object to deserialize
   * @param type - Class constructor to instantiate
   * @returns Instance of the class
   * @throws {DeserializationError} If deserialization fails
   * 
   * @example
   * ```typescript
   * class Person {
   *   constructor(public name: string, public age: number) {}
   * }
   * 
   * const row = { name: 'Alice', age: 30 };
   * const person = typed.deserializeRow(row, Person);
   * ```
   */
  public deserializeRow<U>(row: Row, type: Constructor<U>): U {
    try {
      // Try constructor with row values in order
      const values = Object.values(row);
      
      try {
        // Attempt to call constructor with values
        return new type(...values);
      } catch (constructorError) {
        // If constructor fails, try property assignment
        const instance = Object.create(type.prototype);
        Object.assign(instance, row);
        return instance;
      }
    } catch (error) {
      throw new DeserializationError(
        `Failed to deserialize row into ${type.name}`,
        error as Error
      );
    }
  }

  /**
   * Get the first row deserialized as the given type
   * 
   * @param type - Class constructor to instantiate
   * @returns Instance of the class or null if no rows
   * @throws {DeserializationError} If deserialization fails
   * 
   * @example
   * ```typescript
   * class Person {
   *   constructor(public name: string, public age: number) {}
   * }
   * 
   * const result = await session.query('MATCH (p:Person {name: "Alice"}) RETURN p.name, p.age');
   * const typed = new TypedResult(result);
   * const person = typed.first(Person);
   * 
   * if (person) {
   *   console.log(`Found: ${person.name}, age ${person.age}`);
   * }
   * ```
   */
  public first<U>(type: Constructor<U>): U | null {
    if (this.isEmpty()) {
      return null;
    }

    try {
      return this.deserializeRow(this._result.rows[0], type);
    } catch (error) {
      throw new DeserializationError(
        'Failed to deserialize first row',
        error as Error
      );
    }
  }

  /**
   * Get a scalar value (first column of first row)
   * 
   * Useful for queries that return a single value like COUNT, SUM, etc.
   * 
   * @returns The scalar value or null if no rows
   * 
   * @example
   * ```typescript
   * const result = await session.query('MATCH (p:Person) RETURN count(p)');
   * const typed = new TypedResult(result);
   * const count = typed.scalar();
   * 
   * console.log(`Total persons: ${count}`);
   * ```
   */
  public scalar(): any {
    if (this.isEmpty()) {
      return null;
    }

    const firstRow = this._result.rows[0];
    const firstValue = Object.values(firstRow)[0];
    return firstValue;
  }

  /**
   * Map rows using a transformer function
   * 
   * @param transformer - Function to transform each row
   * @returns Array of transformed values
   * 
   * @example
   * ```typescript
   * const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
   * const typed = new TypedResult(result);
   * const names = typed.map(row => row['p.name']);
   * ```
   */
  public map<U>(transformer: (row: Row) => U): U[] {
    return this._result.rows.map(transformer);
  }

  /**
   * Filter rows using a predicate function
   * 
   * @param predicate - Function that returns true for rows to keep
   * @returns Array of filtered rows
   * 
   * @example
   * ```typescript
   * const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
   * const typed = new TypedResult(result);
   * const adults = typed.filter(row => row['p.age'] >= 18);
   * ```
   */
  public filter(predicate: (row: Row) => boolean): Row[] {
    return this._result.rows.filter(predicate);
  }

  /**
   * Get a specific column from all rows
   * 
   * @param columnName - Name of the column
   * @returns Array of column values
   * 
   * @example
   * ```typescript
   * const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
   * const typed = new TypedResult(result);
   * const names = typed.column('p.name');
   * ```
   */
  public column(columnName: string): any[] {
    return this._result.rows.map(row => row[columnName]);
  }

  /**
   * Get the underlying QueryResult
   * 
   * @returns The raw QueryResult from bindings
   */
  public raw(): QueryResult {
    return this._result;
  }
}
