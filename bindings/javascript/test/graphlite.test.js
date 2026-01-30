/**
 * Unit tests for GraphLite JavaScript bindings
 */

const { GraphLite, GraphLiteError, ErrorCode, QueryResult } = require('..');
const fs = require('fs');
const path = require('path');

// Test database path
const TEST_DB_PATH = path.join(__dirname, 'test_db_unit');

// Clean up test database
function cleanupTestDb() {
  if (fs.existsSync(TEST_DB_PATH)) {
    fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
  }
}

describe('GraphLite', () => {
  beforeAll(() => {
    cleanupTestDb();
  });

  afterAll(() => {
    cleanupTestDb();
  });

  describe('Constructor', () => {
    test('should open database with valid path', () => {
      const db = new GraphLite(TEST_DB_PATH);
      expect(db).toBeInstanceOf(GraphLite);
      expect(db.isOpen()).toBe(true);
      db.close();
    });

    test('should throw error with invalid path type', () => {
      expect(() => new GraphLite(null)).toThrow(GraphLiteError);
      expect(() => new GraphLite(undefined)).toThrow(GraphLiteError);
      expect(() => new GraphLite(123)).toThrow(GraphLiteError);
    });
  });

  describe('Session Management', () => {
    let db;

    beforeEach(() => {
      db = new GraphLite(TEST_DB_PATH);
    });

    afterEach(() => {
      db.close();
    });

    test('should create session', () => {
      const session = db.createSession('testuser');
      expect(typeof session).toBe('string');
      expect(session.length).toBeGreaterThan(0);
    });

    test('should throw error with invalid username', () => {
      expect(() => db.createSession(null)).toThrow(GraphLiteError);
      expect(() => db.createSession('')).toThrow(GraphLiteError);
    });

    test('should track open sessions', () => {
      const session1 = db.createSession('user1');
      const session2 = db.createSession('user2');
      expect(db.getSessionCount()).toBe(2);
    });

    test('should close session', async () => {
      const session = db.createSession('user');
      expect(db.getSessionCount()).toBe(1);
      await db.closeSession(session);
      expect(db.getSessionCount()).toBe(0);
    });
  });

  describe('Query Execution', () => {
    let db;
    let session;

    beforeEach(async () => {
      db = new GraphLite(TEST_DB_PATH);
      session = db.createSession('admin');
      
      // Setup schema and graph
      await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS test');
      await db.execute(session, 'SESSION SET SCHEMA test');
      await db.execute(session, 'CREATE GRAPH IF NOT EXISTS g');
      await db.execute(session, 'SESSION SET GRAPH g');
    });

    afterEach(() => {
      db.close();
    });

    test('should execute query and return QueryResult', async () => {
      const result = await db.query(session, 'MATCH (n) RETURN n LIMIT 0');
      expect(result).toBeInstanceOf(QueryResult);
      expect(result.rowCount).toBe(0);
    });

    test('should insert and query data', async () => {
      await db.execute(session, "INSERT (:Person {name: 'Alice', age: 30})");
      
      const result = await db.query(session, 'MATCH (p:Person) RETURN p.name, p.age');
      expect(result.rowCount).toBe(1);
      
      const row = result.first();
      expect(row['p.name']).toBe('Alice');
      expect(row['p.age']).toBe(30);
    });

    test('should handle multiple rows', async () => {
      await db.execute(session, `
        INSERT (:Person {name: 'Alice', age: 30}),
               (:Person {name: 'Bob', age: 25})
      `);
      
      const result = await db.query(session, 'MATCH (p:Person) RETURN p.name ORDER BY p.name');
      expect(result.rowCount).toBe(2);
      expect(result.rows[0]['p.name']).toBe('Alice');
      expect(result.rows[1]['p.name']).toBe('Bob');
    });
  });

  describe('Database State', () => {
    test('should track open/closed state', () => {
      const db = new GraphLite(TEST_DB_PATH);
      expect(db.isOpen()).toBe(true);
      db.close();
      expect(db.isOpen()).toBe(false);
    });

    test('should throw error when using closed database', () => {
      const db = new GraphLite(TEST_DB_PATH);
      db.close();
      expect(() => db.createSession('user')).toThrow(GraphLiteError);
    });
  });

  describe('Static Methods', () => {
    test('should return version string', () => {
      const version = GraphLite.version();
      expect(typeof version).toBe('string');
      expect(version.length).toBeGreaterThan(0);
    });
  });
});

describe('QueryResult', () => {
  test('should create from valid data', () => {
    const data = {
      variables: ['name', 'age'],
      rows: [
        { values: { name: { String: 'Alice' }, age: { Number: 30 } } },
        { values: { name: { String: 'Bob' }, age: { Number: 25 } } },
      ],
    };

    const result = new QueryResult(data);
    expect(result.variables).toEqual(['name', 'age']);
    expect(result.rowCount).toBe(2);
    expect(result.rows[0].name).toBe('Alice');
    expect(result.rows[0].age).toBe(30);
  });

  test('should handle empty result', () => {
    const data = { variables: [], rows: [] };
    const result = new QueryResult(data);
    expect(result.isEmpty()).toBe(true);
    expect(result.first()).toBe(null);
  });

  test('should extract values correctly', () => {
    const data = {
      variables: ['str', 'num', 'bool', 'null'],
      rows: [
        {
          values: {
            str: { String: 'test' },
            num: { Number: 42 },
            bool: { Boolean: true },
            null: { Null: null },
          },
        },
      ],
    };

    const result = new QueryResult(data);
    const row = result.first();
    expect(row.str).toBe('test');
    expect(row.num).toBe(42);
    expect(row.bool).toBe(true);
    expect(row.null).toBe(null);
  });

  test('should provide column method', () => {
    const data = {
      variables: ['name'],
      rows: [
        { values: { name: { String: 'Alice' } } },
        { values: { name: { String: 'Bob' } } },
      ],
    };

    const result = new QueryResult(data);
    const names = result.column('name');
    expect(names).toEqual(['Alice', 'Bob']);
  });

  test('should be iterable', () => {
    const data = {
      variables: ['x'],
      rows: [
        { values: { x: { Number: 1 } } },
        { values: { x: { Number: 2 } } },
      ],
    };

    const result = new QueryResult(data);
    const values = [];
    for (const row of result) {
      values.push(row.x);
    }
    expect(values).toEqual([1, 2]);
  });

  test('should filter rows', () => {
    const data = {
      variables: ['age'],
      rows: [
        { values: { age: { Number: 25 } } },
        { values: { age: { Number: 30 } } },
        { values: { age: { Number: 35 } } },
      ],
    };

    const result = new QueryResult(data);
    const filtered = result.filter(row => row.age >= 30);
    expect(filtered.length).toBe(2);
  });

  test('should map rows', () => {
    const data = {
      variables: ['age'],
      rows: [
        { values: { age: { Number: 25 } } },
        { values: { age: { Number: 30 } } },
      ],
    };

    const result = new QueryResult(data);
    const doubled = result.map(row => row.age * 2);
    expect(doubled).toEqual([50, 60]);
  });

  test('should select columns', () => {
    const data = {
      variables: ['name', 'age', 'city'],
      rows: [
        {
          values: {
            name: { String: 'Alice' },
            age: { Number: 30 },
            city: { String: 'NYC' },
          },
        },
      ],
    };

    const result = new QueryResult(data);
    const selected = result.select('name', 'city');
    expect(selected[0]).toEqual({ name: 'Alice', city: 'NYC' });
    expect(selected[0].age).toBeUndefined();
  });
});

describe('ErrorCode', () => {
  test('should have correct error codes', () => {
    expect(ErrorCode.SUCCESS).toBe(0);
    expect(ErrorCode.NULL_POINTER).toBe(1);
    expect(ErrorCode.QUERY_ERROR).toBe(5);
  });

  test('should get error name', () => {
    expect(ErrorCode.getName(0)).toBe('SUCCESS');
    expect(ErrorCode.getName(5)).toBe('QUERY_ERROR');
  });

  test('should get error description', () => {
    const desc = ErrorCode.getDescription(5);
    expect(typeof desc).toBe('string');
    expect(desc.length).toBeGreaterThan(0);
  });
});

describe('GraphLiteError', () => {
  test('should create error with code and message', () => {
    const error = new GraphLiteError(ErrorCode.QUERY_ERROR, 'Test error');
    expect(error).toBeInstanceOf(Error);
    expect(error).toBeInstanceOf(GraphLiteError);
    expect(error.code).toBe(ErrorCode.QUERY_ERROR);
    expect(error.errorName).toBe('QUERY_ERROR');
    expect(error.message).toContain('Test error');
  });

  test('should create from error code', () => {
    const error = GraphLiteError.fromErrorCode(ErrorCode.QUERY_ERROR, 'Context');
    expect(error.code).toBe(ErrorCode.QUERY_ERROR);
    expect(error.message).toContain('Context');
  });

  test('should check error code', () => {
    const error = new GraphLiteError(ErrorCode.QUERY_ERROR, 'Test');
    expect(error.isCode(ErrorCode.QUERY_ERROR)).toBe(true);
    expect(error.isCode(ErrorCode.SESSION_ERROR)).toBe(false);
  });

  test('should convert to JSON', () => {
    const error = new GraphLiteError(ErrorCode.QUERY_ERROR, 'Test');
    const json = error.toJSON();
    expect(json.name).toBe('GraphLiteError');
    expect(json.code).toBe(ErrorCode.QUERY_ERROR);
    expect(json.errorName).toBe('QUERY_ERROR');
  });
});
