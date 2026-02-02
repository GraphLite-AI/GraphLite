/**
 * Tests for Session
 */

import { GraphLite, Session, QueryError } from '../src';
import * as fs from 'fs';
import * as path from 'path';

const TEST_DB_PATH = path.join(__dirname, 'test_db_session');

describe('Session', () => {
  let db: GraphLite;
  let session: Session;

  beforeEach(() => {
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
    db = GraphLite.open(TEST_DB_PATH);
    session = db.session('admin');
  });

  afterEach(() => {
    db.close();
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
  });

  describe('metadata', () => {
    it('should return session id', () => {
      const id = session.id();
      expect(typeof id).toBe('string');
      expect(id.length).toBeGreaterThan(0);
    });

    it('should return username', () => {
      expect(session.username()).toBe('admin');
    });
  });

  describe('execute()', () => {
    it('should execute DDL statement', async () => {
      await expect(
        session.execute('CREATE SCHEMA IF NOT EXISTS test')
      ).resolves.not.toThrow();
    });

    it('should execute DML statement', async () => {
      await session.execute('CREATE SCHEMA IF NOT EXISTS test');
      await session.execute('SESSION SET SCHEMA test');
      await session.execute('CREATE GRAPH IF NOT EXISTS g');
      await session.execute('SESSION SET GRAPH g');
      
      await expect(
        session.execute("INSERT (:Person {name: 'Alice'})")
      ).resolves.not.toThrow();
    });
  });

  describe('query()', () => {
    beforeEach(async () => {
      await session.execute('CREATE SCHEMA IF NOT EXISTS test');
      await session.execute('SESSION SET SCHEMA test');
      await session.execute('CREATE GRAPH IF NOT EXISTS g');
      await session.execute('SESSION SET GRAPH g');
    });

    it('should execute query and return results', async () => {
      await session.execute("INSERT (:Person {name: 'Alice', age: 30})");
      
      const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');
      
      expect(result.rowCount).toBe(1);
      expect(result.rows[0]['p.name']).toBe('Alice');
      expect(result.rows[0]['p.age']).toBe(30);
    });

    it('should return empty result for no matches', async () => {
      const result = await session.query('MATCH (p:Person) RETURN p.name');
      expect(result.rowCount).toBe(0);
      expect(result.rows).toHaveLength(0);
    });
  });

  describe('transaction()', () => {
    it('should create a transaction', async () => {
      const tx = await session.transaction();
      expect(tx).toBeDefined();
      expect(tx.isActive()).toBe(true);
      await tx.rollback();
    });
  });

  describe('queryBuilder()', () => {
    it('should create a query builder', () => {
      const builder = session.queryBuilder();
      expect(builder).toBeDefined();
    });
  });
});
