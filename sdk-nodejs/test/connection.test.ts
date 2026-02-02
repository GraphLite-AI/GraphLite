/**
 * Tests for Connection (GraphLite class)
 */

import { GraphLite, ConnectionError, SessionError } from '../src';
import * as fs from 'fs';
import * as path from 'path';

const TEST_DB_PATH = path.join(__dirname, 'test_db_connection');

describe('GraphLite Connection', () => {
  beforeEach(() => {
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
  });

  afterEach(() => {
    if (fs.existsSync(TEST_DB_PATH)) {
      fs.rmSync(TEST_DB_PATH, { recursive: true, force: true });
    }
  });

  describe('open()', () => {
    it('should open a database with valid path', () => {
      const db = GraphLite.open(TEST_DB_PATH);
      expect(db).toBeInstanceOf(GraphLite);
      expect(db.isOpen()).toBe(true);
      db.close();
    });

    it('should create database directory if it does not exist', () => {
      const db = GraphLite.open(TEST_DB_PATH);
      expect(fs.existsSync(TEST_DB_PATH)).toBe(true);
      db.close();
    });
  });

  describe('session()', () => {
    it('should create a session with valid username', () => {
      const db = GraphLite.open(TEST_DB_PATH);
      const session = db.session('testuser');
      
      expect(session).toBeDefined();
      expect(session.username()).toBe('testuser');
      expect(session.id()).toBeTruthy();
      
      db.close();
    });

    it('should throw error when creating session on closed database', () => {
      const db = GraphLite.open(TEST_DB_PATH);
      db.close();
      
      expect(() => db.session('testuser')).toThrow(ConnectionError);
    });
  });

  describe('close()', () => {
    it('should close database', () => {
      const db = GraphLite.open(TEST_DB_PATH);
      expect(db.isOpen()).toBe(true);
      
      db.close();
      expect(db.isOpen()).toBe(false);
    });

    it('should be idempotent', () => {
      const db = GraphLite.open(TEST_DB_PATH);
      db.close();
      db.close(); // Should not throw
      
      expect(db.isOpen()).toBe(false);
    });
  });

  describe('version()', () => {
    it('should return version string', () => {
      const version = GraphLite.version();
      expect(typeof version).toBe('string');
      expect(version.length).toBeGreaterThan(0);
    });
  });
});
