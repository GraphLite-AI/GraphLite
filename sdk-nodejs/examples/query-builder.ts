/**
 * Query builder examples for GraphLite SDK
 * 
 * This example demonstrates the fluent query builder API:
 * - Building queries with method chaining
 * - Complex query patterns
 * - Query reuse and composition
 */

import { GraphLite } from '../src';
import * as fs from 'fs';
import * as path from 'path';

async function main() {
  console.log('=== GraphLite SDK Query Builder Examples ===\n');

  const dbPath = path.join(__dirname, '../test_db_qb');
  
  if (fs.existsSync(dbPath)) {
    fs.rmSync(dbPath, { recursive: true, force: true });
  }

  try {
    const db = GraphLite.open(dbPath);
    const session = db.session('admin');

    // Setup
    await session.execute('CREATE SCHEMA IF NOT EXISTS test');
    await session.execute('SESSION SET SCHEMA test');
    await session.execute('CREATE GRAPH IF NOT EXISTS social');
    await session.execute('SESSION SET GRAPH social');

    // Insert test data
    const tx = await session.transaction();
    try {
      await tx.execute("INSERT (:Person {name: 'Alice', age: 30, city: 'New York', status: 'active'})");
      await tx.execute("INSERT (:Person {name: 'Bob', age: 25, city: 'San Francisco', status: 'active'})");
      await tx.execute("INSERT (:Person {name: 'Charlie', age: 35, city: 'New York', status: 'inactive'})");
      await tx.execute("INSERT (:Person {name: 'Diana', age: 28, city: 'Boston', status: 'active'})");
      await tx.execute("INSERT (:Person {name: 'Eve', age: 40, city: 'New York', status: 'active'})");
      await tx.execute("INSERT (:Person {name: 'Frank', age: 22, city: 'San Francisco', status: 'inactive'})");
      await tx.commit();
      console.log('Inserted test data\n');
    } catch (error) {
      await tx.rollback();
      throw error;
    }

    // Example 1: Simple query
    console.log('1. Simple query with WHERE...');
    const result1 = await session.queryBuilder()
      .match('(p:Person)')
      .where('p.age >= 30')
      .return('p.name, p.age')
      .orderBy('p.age')
      .execute();
    console.log(`   Found ${result1.rowCount} persons aged 30+:`);
    for (const row of result1.rows) {
      console.log(`   - ${row['p.name']}: ${row['p.age']}`);
    }
    console.log();

    // Example 2: Multiple WHERE clauses
    console.log('2. Multiple WHERE clauses (AND logic)...');
    const result2 = await session.queryBuilder()
      .match('(p:Person)')
      .where('p.age > 25')
      .where('p.city = "New York"')
      .where('p.status = "active"')
      .return('p.name, p.age, p.city')
      .execute();
    console.log(`   Found ${result2.rowCount} active New Yorkers over 25:`);
    for (const row of result2.rows) {
      console.log(`   - ${row['p.name']}: ${row['p.age']} (${row['p.city']})`);
    }
    console.log();

    // Example 3: Pagination
    console.log('3. Pagination (SKIP and LIMIT)...');
    const result3 = await session.queryBuilder()
      .match('(p:Person)')
      .return('p.name, p.age')
      .orderBy('p.name')
      .skip(2)
      .limit(3)
      .execute();
    console.log(`   Persons 3-5 (skip 2, limit 3):`);
    for (const row of result3.rows) {
      console.log(`   - ${row['p.name']}: ${row['p.age']}`);
    }
    console.log();

    // Example 4: Build query without executing
    console.log('4. Building query string without executing...');
    const query = session.queryBuilder()
      .match('(p:Person)')
      .where('p.age >= 30')
      .where('p.status = "active"')
      .return('p.name, p.age, p.city')
      .orderBy('p.age DESC')
      .limit(5)
      .build();
    console.log(`   Built query: ${query}\n`);

    // Example 5: Complex ordering
    console.log('5. Complex ORDER BY...');
    const result5 = await session.queryBuilder()
      .match('(p:Person)')
      .return('p.name, p.age, p.city')
      .orderBy('p.city ASC, p.age DESC')
      .execute();
    console.log(`   Persons ordered by city (ASC), then age (DESC):`);
    for (const row of result5.rows) {
      console.log(`   - ${row['p.name']}: ${row['p.age']} (${row['p.city']})`);
    }
    console.log();

    // Example 6: Aggregation with GROUP BY (using raw query)
    console.log('6. Aggregation by city...');
    const result6 = await session.query(`
      MATCH (p:Person)
      RETURN p.city, count(p) as count, avg(p.age) as avgAge
      GROUP BY p.city
      ORDER BY count DESC
    `);
    console.log(`   Persons by city:`);
    for (const row of result6.rows) {
      console.log(`   - ${row['p.city']}: ${row['count']} persons, avg age ${Math.round(row['avgAge'] as number)}`);
    }
    console.log();

    // Example 7: Reusable base query
    console.log('7. Reusable query patterns...');
    const activePeople = session.queryBuilder()
      .match('(p:Person)')
      .where('p.status = "active"');
    
    // Extend with different conditions
    const activeInNY = activePeople
      .where('p.city = "New York"')
      .return('p.name, p.city')
      .execute();
    
    console.log(`   Active people in New York:`);
    const activeNYResult = await activeInNY;
    for (const row of activeNYResult.rows) {
      console.log(`   - ${row['p.name']} (${row['p.city']})`);
    }
    console.log();

    db.close();
    console.log('=== Query builder examples completed successfully ===');
    return 0;

  } catch (error) {
    console.error('\n[ERROR]', error);
    return 1;
  } finally {
    if (fs.existsSync(dbPath)) {
      fs.rmSync(dbPath, { recursive: true, force: true });
    }
  }
}

main().then(code => process.exit(code));
