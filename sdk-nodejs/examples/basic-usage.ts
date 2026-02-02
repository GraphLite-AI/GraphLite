/**
 * Basic usage example for GraphLite SDK
 * 
 * This example demonstrates the core features of the GraphLite Node.js SDK:
 * - Opening a database
 * - Creating sessions
 * - Executing queries
 * - Using transactions
 * - Query builder API
 * - Typed result deserialization
 */

import { GraphLite, TypedResult } from '../src';
import * as fs from 'fs';
import * as path from 'path';

// Define a Person class for typed deserialization
class Person {
  constructor(
    public name: string,
    public age: number,
    public city: string
  ) {}
}

async function main() {
  console.log('=== GraphLite SDK Basic Usage Example ===\n');

  // Use temporary directory for demo
  const dbPath = path.join(__dirname, '../test_db_example');
  
  // Clean up if exists
  if (fs.existsSync(dbPath)) {
    fs.rmSync(dbPath, { recursive: true, force: true });
  }

  try {
    // 1. Open a database
    console.log('1. Opening database...');
    const db = GraphLite.open(dbPath);
    console.log(`   ✓ Database opened at ${dbPath}\n`);

    // 2. Create a session
    console.log('2. Creating session...');
    const session = db.session('admin');
    console.log(`   ✓ Session created for user '${session.username()}'\n`);

    // 3. Execute DDL statements
    console.log('3. Creating schema and graph...');
    await session.execute('CREATE SCHEMA IF NOT EXISTS example');
    await session.execute('SESSION SET SCHEMA example');
    await session.execute('CREATE GRAPH IF NOT EXISTS social');
    await session.execute('SESSION SET GRAPH social');
    console.log('   ✓ Schema and graph created\n');

    // 4. Insert data using transactions
    console.log('4. Inserting data with transaction...');
    const tx = await session.transaction();
    try {
      await tx.execute("INSERT (:Person {name: 'Alice', age: 30, city: 'New York'})");
      await tx.execute("INSERT (:Person {name: 'Bob', age: 25, city: 'San Francisco'})");
      await tx.execute("INSERT (:Person {name: 'Charlie', age: 35, city: 'New York'})");
      await tx.execute("INSERT (:Person {name: 'Diana', age: 28, city: 'Boston'})");
      await tx.commit();
      console.log('   ✓ Inserted 4 persons\n');
    } catch (error) {
      await tx.rollback();
      throw error;
    }

    // 5. Query data directly
    console.log('5. Querying data...');
    const result1 = await session.query('MATCH (p:Person) RETURN p.name, p.age, p.city ORDER BY p.age');
    console.log(`   Found ${result1.rowCount} persons:`);
    for (const row of result1.rows) {
      console.log(`   - ${row['p.name']}, age ${row['p.age']}, from ${row['p.city']}`);
    }
    console.log();

    // 6. Use query builder
    console.log('6. Using query builder...');
    const result2 = await session.queryBuilder()
      .match('(p:Person)')
      .where('p.age > 25')
      .return('p.name, p.age, p.city')
      .orderBy('p.age DESC')
      .execute();
    console.log(`   Found ${result2.rowCount} persons over 25:`);
    for (const row of result2.rows) {
      console.log(`   - ${row['p.name']}, age ${row['p.age']}, from ${row['p.city']}`);
    }
    console.log();

    // 7. Typed deserialization
    console.log('7. Using typed deserialization...');
    const result3 = await session.query('MATCH (p:Person) RETURN p.name, p.age, p.city ORDER BY p.name');
    const typed = new TypedResult(result3);
    const people = typed.deserializeRows(Person);
    console.log(`   Deserialized ${people.length} persons:`);
    for (const person of people) {
      console.log(`   - ${person.name}, age ${person.age}, from ${person.city}`);
    }
    console.log();

    // 8. Scalar query (count)
    console.log('8. Getting person count...');
    const countResult = await session.query('MATCH (p:Person) RETURN count(p) as count');
    const countTyped = new TypedResult(countResult);
    const count = countTyped.scalar();
    console.log(`   Total persons: ${count}\n`);

    // 9. Query with LIMIT
    console.log('9. Using query builder with pagination...');
    const result4 = await session.queryBuilder()
      .match('(p:Person)')
      .return('p.name, p.age')
      .orderBy('p.name')
      .skip(1)
      .limit(2)
      .execute();
    console.log(`   Found ${result4.rowCount} persons (skip 1, limit 2):`);
    for (const row of result4.rows) {
      console.log(`   - ${row['p.name']}, age ${row['p.age']}`);
    }
    console.log();

    // 10. Close database
    console.log('10. Closing database...');
    db.close();
    console.log('   ✓ Database closed\n');

    console.log('=== Example completed successfully ===');
    return 0;

  } catch (error) {
    console.error('\n[ERROR]', error);
    return 1;
  } finally {
    // Cleanup
    if (fs.existsSync(dbPath)) {
      fs.rmSync(dbPath, { recursive: true, force: true });
    }
  }
}

// Run example
main().then(code => process.exit(code));
