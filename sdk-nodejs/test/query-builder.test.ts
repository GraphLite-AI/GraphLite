/**
 * Tests for QueryBuilder
 */

import { GraphLite, Session } from '../src';
import * as fs from 'fs';
import * as path from 'path';

const TEST_DB_PATH = path.join(__dirname, 'test_db_qb');

describe('QueryBuilder', () => {
  let db: GraphLite;
  let session: Session;

  beforeEach(async () => {
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
    db = GraphLite.open(TEST_DB_PATH);
    session = db.session('admin');
    
    await session.execute('CREATE SCHEMA IF NOT EXISTS test');
    await session.execute('SESSION SET SCHEMA test');
    await session.execute('CREATE GRAPH IF NOT EXISTS g');
    await session.execute('SESSION SET GRAPH g');
  });

  afterEach(() => {
    db.close();
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
  });

  describe('build()', () => {
    it('should build simple MATCH query', () => {
      const query = session.queryBuilder()
        .match('(p:Person)')
        .return('p.name')
        .build();
      
      expect(query).toBe('MATCH (p:Person) RETURN p.name');
    });

    it('should build query with WHERE clause', () => {
      const query = session.queryBuilder()
        .match('(p:Person)')
        .where('p.age > 25')
        .return('p.name')
        .build();
      
      expect(query).toBe('MATCH (p:Person) WHERE p.age > 25 RETURN p.name');
    });

    it('should build query with multiple WHERE clauses', () => {
      const query = session.queryBuilder()
        .match('(p:Person)')
        .where('p.age > 25')
        .where('p.city = "NYC"')
        .return('p.name')
        .build();
      
      expect(query).toBe('MATCH (p:Person) WHERE p.age > 25 AND p.city = "NYC" RETURN p.name');
    });

    it('should build query with ORDER BY', () => {
      const query = session.queryBuilder()
        .match('(p:Person)')
        .return('p.name')
        .orderBy('p.name DESC')
        .build();
      
      expect(query).toBe('MATCH (p:Person) RETURN p.name ORDER BY p.name DESC');
    });

    it('should build query with SKIP and LIMIT', () => {
      const query = session.queryBuilder()
        .match('(p:Person)')
        .return('p.name')
        .skip(10)
        .limit(5)
        .build();
      
      expect(query).toBe('MATCH (p:Person) RETURN p.name SKIP 10 LIMIT 5');
    });

    it('should build complex query with all clauses', () => {
      const query = session.queryBuilder()
        .match('(p:Person)')
        .where('p.age > 25')
        .where('p.status = "active"')
        .return('p.name, p.age')
        .orderBy('p.age DESC')
        .skip(5)
        .limit(10)
        .build();
      
      expect(query).toBe(
        'MATCH (p:Person) WHERE p.age > 25 AND p.status = "active" RETURN p.name, p.age ORDER BY p.age DESC SKIP 5 LIMIT 10'
      );
    });
  });

  describe('execute()', () => {
    beforeEach(async () => {
      await session.execute("INSERT (:Person {name: 'Alice', age: 30})");
      await session.execute("INSERT (:Person {name: 'Bob', age: 25})");
      await session.execute("INSERT (:Person {name: 'Charlie', age: 35})");
    });

    it('should execute simple query', async () => {
      const result = await session.queryBuilder()
        .match('(p:Person)')
        .return('p.name')
        .execute();
      
      expect(result.rowCount).toBe(3);
    });

    it('should execute query with WHERE', async () => {
      const result = await session.queryBuilder()
        .match('(p:Person)')
        .where('p.age >= 30')
        .return('p.name')
        .execute();
      
      expect(result.rowCount).toBe(2);
    });

    it('should execute query with LIMIT', async () => {
      const result = await session.queryBuilder()
        .match('(p:Person)')
        .return('p.name')
        .limit(2)
        .execute();
      
      expect(result.rowCount).toBe(2);
    });
  });

  describe('chaining', () => {
    it('should support method chaining', () => {
      const builder = session.queryBuilder();
      
      expect(builder.match('(p:Person)')).toBe(builder);
      expect(builder.where('p.age > 25')).toBe(builder);
      expect(builder.return('p.name')).toBe(builder);
      expect(builder.orderBy('p.name')).toBe(builder);
      expect(builder.skip(10)).toBe(builder);
      expect(builder.limit(5)).toBe(builder);
    });
  });
});
