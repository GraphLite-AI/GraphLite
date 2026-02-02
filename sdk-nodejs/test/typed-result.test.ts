/**
 * Tests for TypedResult
 */

import { TypedResult, DeserializationError } from '../src';

// Mock QueryResult class for testing
class MockQueryResult {
  public variables: string[];
  public rows: any[];
  public rowCount: number;

  constructor(variables: string[], rows: any[]) {
    this.variables = variables;
    this.rows = rows;
    this.rowCount = rows.length;
  }

  first() {
    return this.rows.length > 0 ? this.rows[0] : null;
  }

  column(columnName: string) {
    return this.rows.map(row => row[columnName]);
  }

  toArray() {
    return this.rows;
  }

  toJSON() {
    return { variables: this.variables, rows: this.rows, rowCount: this.rowCount };
  }

  select(...columnNames: string[]) {
    return this.rows.map(row => {
      const selected: any = {};
      for (const col of columnNames) {
        if (col in row) {
          selected[col] = row[col];
        }
      }
      return selected;
    });
  }

  filter(predicate: (row: any) => boolean) {
    return this.rows.filter(predicate);
  }

  map(transformer: (row: any) => any) {
    return this.rows.map(transformer);
  }

  isEmpty() {
    return this.rowCount === 0;
  }

  toString() {
    return `QueryResult(rows=${this.rowCount}, variables=[${this.variables.join(', ')}])`;
  }

  [Symbol.iterator]() {
    return this.rows[Symbol.iterator]();
  }
}

// Mock QueryResult for testing
const mockQueryResult = new MockQueryResult(
  ['name', 'age', 'city'],
  [
    { name: 'Alice', age: 30, city: 'NYC' },
    { name: 'Bob', age: 25, city: 'SF' },
    { name: 'Charlie', age: 35, city: 'NYC' },
  ]
);

const emptyQueryResult = new MockQueryResult([], []);

// Test class for deserialization
class Person {
  constructor(
    public name: string,
    public age: number,
    public city: string
  ) {}
}

describe('TypedResult', () => {
  describe('constructor', () => {
    it('should create TypedResult from QueryResult', () => {
      const typed = new TypedResult(mockQueryResult);
      expect(typed).toBeInstanceOf(TypedResult);
    });
  });

  describe('rows()', () => {
    it('should return all rows', () => {
      const typed = new TypedResult(mockQueryResult);
      const rows = typed.rows();
      
      expect(rows).toHaveLength(3);
      expect(rows[0].name).toBe('Alice');
    });
  });

  describe('rowCount()', () => {
    it('should return row count', () => {
      const typed = new TypedResult(mockQueryResult);
      expect(typed.rowCount()).toBe(3);
    });
  });

  describe('columnNames()', () => {
    it('should return column names', () => {
      const typed = new TypedResult(mockQueryResult);
      const columns = typed.columnNames();
      
      expect(columns).toEqual(['name', 'age', 'city']);
    });
  });

  describe('isEmpty()', () => {
    it('should return false for non-empty result', () => {
      const typed = new TypedResult(mockQueryResult);
      expect(typed.isEmpty()).toBe(false);
    });

    it('should return true for empty result', () => {
      const typed = new TypedResult(emptyQueryResult);
      expect(typed.isEmpty()).toBe(true);
    });
  });

  describe('getRow()', () => {
    it('should return row at index', () => {
      const typed = new TypedResult(mockQueryResult);
      const row = typed.getRow(1);
      
      expect(row?.name).toBe('Bob');
      expect(row?.age).toBe(25);
    });

    it('should return undefined for invalid index', () => {
      const typed = new TypedResult(mockQueryResult);
      expect(typed.getRow(10)).toBeUndefined();
    });
  });

  describe('deserializeRows()', () => {
    it('should deserialize all rows', () => {
      const typed = new TypedResult(mockQueryResult);
      const people = typed.deserializeRows(Person);
      
      expect(people).toHaveLength(3);
      expect(people[0]).toBeInstanceOf(Person);
      expect(people[0].name).toBe('Alice');
      expect(people[0].age).toBe(30);
      expect(people[0].city).toBe('NYC');
    });

    it('should return empty array for empty result', () => {
      const typed = new TypedResult(emptyQueryResult);
      const people = typed.deserializeRows(Person);
      
      expect(people).toHaveLength(0);
    });
  });

  describe('deserializeRow()', () => {
    it('should deserialize single row', () => {
      const typed = new TypedResult(mockQueryResult);
      const person = typed.deserializeRow(mockQueryResult.rows[0], Person);
      
      expect(person).toBeInstanceOf(Person);
      expect(person.name).toBe('Alice');
      expect(person.age).toBe(30);
      expect(person.city).toBe('NYC');
    });
  });

  describe('first()', () => {
    it('should return first row deserialized', () => {
      const typed = new TypedResult(mockQueryResult);
      const person = typed.first(Person);
      
      expect(person).toBeInstanceOf(Person);
      expect(person?.name).toBe('Alice');
    });

    it('should return null for empty result', () => {
      const typed = new TypedResult(emptyQueryResult);
      const person = typed.first(Person);
      
      expect(person).toBeNull();
    });
  });

  describe('scalar()', () => {
    it('should return first value of first row', () => {
      const typed = new TypedResult(mockQueryResult);
      const value = typed.scalar();
      
      expect(value).toBe('Alice');
    });

    it('should return null for empty result', () => {
      const typed = new TypedResult(emptyQueryResult);
      expect(typed.scalar()).toBeNull();
    });

    it('should work with count queries', () => {
      const countResult = new MockQueryResult(['count'], [{ count: 42 }]);
      
      const typed = new TypedResult(countResult as any);
      expect(typed.scalar()).toBe(42);
    });
  });

  describe('map()', () => {
    it('should map rows using transformer', () => {
      const typed = new TypedResult(mockQueryResult);
      const names = typed.map(row => row.name);
      
      expect(names).toEqual(['Alice', 'Bob', 'Charlie']);
    });
  });

  describe('filter()', () => {
    it('should filter rows using predicate', () => {
      const typed = new TypedResult(mockQueryResult);
      const filtered = typed.filter(row => row.age >= 30);
      
      expect(filtered).toHaveLength(2);
      expect(filtered[0].name).toBe('Alice');
      expect(filtered[1].name).toBe('Charlie');
    });
  });

  describe('column()', () => {
    it('should extract single column', () => {
      const typed = new TypedResult(mockQueryResult);
      const ages = typed.column('age');
      
      expect(ages).toEqual([30, 25, 35]);
    });
  });

  describe('raw()', () => {
    it('should return underlying QueryResult', () => {
      const typed = new TypedResult(mockQueryResult);
      const raw = typed.raw();
      
      expect(raw).toBe(mockQueryResult);
    });
  });
});
