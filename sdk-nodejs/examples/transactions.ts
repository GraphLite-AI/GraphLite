/**
 * Transaction examples for GraphLite SDK
 * 
 * This example demonstrates various transaction patterns:
 * - Explicit commit/rollback
 * - Automatic rollback on error
 * - Transaction state management
 */

import { GraphLite } from '../src';
import * as fs from 'fs';
import * as path from 'path';

async function main() {
  console.log('=== GraphLite SDK Transaction Examples ===\n');

  const dbPath = path.join(__dirname, '../test_db_tx');
  
  if (fs.existsSync(dbPath)) {
    fs.rmSync(dbPath, { recursive: true, force: true });
  }

  try {
    const db = GraphLite.open(dbPath);
    const session = db.session('admin');

    // Setup
    await session.execute('CREATE SCHEMA IF NOT EXISTS test');
    await session.execute('SESSION SET SCHEMA test');
    await session.execute('CREATE GRAPH IF NOT EXISTS g');
    await session.execute('SESSION SET GRAPH g');

    // Example 1: Explicit commit
    console.log('1. Transaction with explicit commit...');
    const tx1 = await session.transaction();
    try {
      await tx1.execute("INSERT (:Person {name: 'Alice', age: 30})");
      await tx1.execute("INSERT (:Person {name: 'Bob', age: 25})");
      await tx1.commit();
      console.log('   ✓ Transaction committed\n');
    } catch (error) {
      await tx1.rollback();
      throw error;
    }

    // Verify
    let result = await session.query('MATCH (p:Person) RETURN count(p) as count');
    console.log(`   Persons after commit: ${result.rows[0]['count']}\n`);

    // Example 2: Automatic rollback on error
    console.log('2. Transaction with automatic rollback on error...');
    const tx2 = await session.transaction();
    try {
      await tx2.execute("INSERT (:Person {name: 'Charlie', age: 35})");
      console.log('   Inserted Charlie');
      
      // Simulate error
      throw new Error('Simulated error!');
      
      // This line won't execute
      await tx2.commit();
    } catch (error) {
      console.log(`   Error occurred: ${(error as Error).message}`);
      await tx2.rollback();
      console.log('   ✓ Transaction rolled back\n');
    }

    // Verify Charlie wasn't persisted
    result = await session.query('MATCH (p:Person) RETURN count(p) as count');
    console.log(`   Persons after rollback: ${result.rows[0]['count']}\n`);

    // Example 3: Query within transaction
    console.log('3. Query within transaction...');
    const tx3 = await session.transaction();
    try {
      await tx3.execute("INSERT (:Person {name: 'Diana', age: 28})");
      
      // Query within same transaction
      const txResult = await tx3.query('MATCH (p:Person) RETURN p.name, p.age ORDER BY p.name');
      console.log(`   Found ${txResult.rowCount} persons in transaction:`);
      for (const row of txResult.rows) {
        console.log(`   - ${row['p.name']}, age ${row['p.age']}`);
      }
      
      await tx3.commit();
      console.log('   ✓ Transaction committed\n');
    } catch (error) {
      await tx3.rollback();
      throw error;
    }

    // Example 4: Transaction state tracking
    console.log('4. Transaction state tracking...');
    const tx4 = await session.transaction();
    console.log(`   Initial state: ${tx4.state()}`);
    console.log(`   Is active: ${tx4.isActive()}`);
    
    await tx4.execute("INSERT (:Person {name: 'Eve', age: 23})");
    console.log(`   After execute state: ${tx4.state()}`);
    
    await tx4.commit();
    console.log(`   After commit state: ${tx4.state()}`);
    console.log(`   Is active: ${tx4.isActive()}\n`);

    // Final count
    result = await session.query('MATCH (p:Person) RETURN count(p) as count');
    console.log(`Final person count: ${result.rows[0]['count']}\n`);

    db.close();
    console.log('=== Transaction examples completed successfully ===');
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
