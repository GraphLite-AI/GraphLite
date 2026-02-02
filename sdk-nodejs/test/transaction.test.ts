/**
 * Tests for Transaction
 */

import { GraphLite, Session, Transaction, TransactionError, TransactionState } from '../src';
import * as fs from 'fs';
import * as path from 'path';

const TEST_DB_PATH = path.join(__dirname, 'test_db_transaction');

describe('Transaction', () => {
  let db: GraphLite;
  let session: Session;

  beforeEach(async () => {
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
    db = GraphLite.open(TEST_DB_PATH);
    session = db.session('admin');
    
    // Setup schema and graph
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

  describe('begin()', () => {
    it('should create an active transaction', async () => {
      const tx = await session.transaction();
      expect(tx).toBeInstanceOf(Transaction);
      expect(tx.isActive()).toBe(true);
      expect(tx.state()).toBe(TransactionState.Active);
      await tx.rollback();
    });
  });

  describe('execute()', () => {
    it('should execute statement in transaction', async () => {
      const tx = await session.transaction();
      await expect(
        tx.execute("INSERT (:Person {name: 'Alice'})")
      ).resolves.not.toThrow();
      await tx.rollback();
    });

    it('should throw error if transaction is committed', async () => {
      const tx = await session.transaction();
      await tx.commit();
      
      await expect(
        tx.execute("INSERT (:Person {name: 'Bob'})")
      ).rejects.toThrow(TransactionError);
    });

    it('should throw error if transaction is rolled back', async () => {
      const tx = await session.transaction();
      await tx.rollback();
      
      await expect(
        tx.execute("INSERT (:Person {name: 'Bob'})")
      ).rejects.toThrow(TransactionError);
    });
  });

  describe('query()', () => {
    it('should execute query in transaction', async () => {
      const tx = await session.transaction();
      await tx.execute("INSERT (:Person {name: 'Alice', age: 30})");
      
      const result = await tx.query('MATCH (p:Person) RETURN p.name, p.age');
      expect(result.rowCount).toBe(1);
      
      await tx.rollback();
    });
  });

  describe('commit()', () => {
    it('should commit transaction and persist changes', async () => {
      const tx = await session.transaction();
      await tx.execute("INSERT (:Person {name: 'Alice'})");
      await tx.commit();
      
      const result = await session.query('MATCH (p:Person) RETURN count(p) as count');
      expect(result.rows[0]['count']).toBe(1);
    });

    it('should change state to committed', async () => {
      const tx = await session.transaction();
      await tx.commit();
      
      expect(tx.state()).toBe(TransactionState.Committed);
      expect(tx.isActive()).toBe(false);
    });

    it('should throw error if already committed', async () => {
      const tx = await session.transaction();
      await tx.commit();
      
      await expect(tx.commit()).rejects.toThrow(TransactionError);
    });
  });

  describe('rollback()', () => {
    it('should rollback transaction and discard changes', async () => {
      const tx = await session.transaction();
      await tx.execute("INSERT (:Person {name: 'Alice'})");
      await tx.rollback();
      
      const result = await session.query('MATCH (p:Person) RETURN count(p) as count');
      expect(result.rows[0]['count']).toBe(0);
    });

    it('should change state to rolled back', async () => {
      const tx = await session.transaction();
      await tx.rollback();
      
      expect(tx.state()).toBe(TransactionState.RolledBack);
      expect(tx.isActive()).toBe(false);
    });

    it('should be idempotent', async () => {
      const tx = await session.transaction();
      await tx.rollback();
      await tx.rollback(); // Should not throw
      
      expect(tx.state()).toBe(TransactionState.RolledBack);
    });
  });

  describe('auto-rollback', () => {
    it('should auto-rollback on disposal if not committed', async () => {
      const tx = await session.transaction();
      await tx.execute("INSERT (:Person {name: 'Alice'})");
      
      // Dispose without committing
      await tx[Symbol.asyncDispose]();
      
      const result = await session.query('MATCH (p:Person) RETURN count(p) as count');
      expect(result.rows[0]['count']).toBe(0);
    });

    it('should not rollback if already committed', async () => {
      const tx = await session.transaction();
      await tx.execute("INSERT (:Person {name: 'Alice'})");
      await tx.commit();
      
      // Dispose after commit
      await tx[Symbol.asyncDispose]();
      
      const result = await session.query('MATCH (p:Person) RETURN count(p) as count');
      expect(result.rows[0]['count']).toBe(1);
    });
  });
});
