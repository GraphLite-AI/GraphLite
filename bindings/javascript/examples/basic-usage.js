/**
 * Basic usage example for GraphLite JavaScript bindings
 * 
 * Demonstrates:
 * - Opening a database
 * - Creating sessions
 * - Setting up schema and graph
 * - Inserting data
 * - Querying data
 * - Proper cleanup
 */

const { GraphLite } = require('..');

async function main() {
  console.log('GraphLite Basic Usage Example');
  console.log('=============================\n');

  // Open database
  console.log('Opening database...');
  const db = new GraphLite('./test_db');

  try {
    // Create session
    console.log('Creating session...');
    const session = db.createSession('admin');
    console.log(`Session created: ${session}\n`);

    // Set up schema
    console.log('Setting up schema...');
    await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS example');
    await db.execute(session, 'SESSION SET SCHEMA example');

    // Set up graph
    console.log('Setting up graph...');
    await db.execute(session, 'CREATE GRAPH IF NOT EXISTS social');
    await db.execute(session, 'SESSION SET GRAPH social');

    // Insert data
    console.log('Inserting data...');
    await db.execute(session, `
      INSERT (:Person {name: 'Alice', age: 30, city: 'New York'}),
             (:Person {name: 'Bob', age: 25, city: 'San Francisco'}),
             (:Person {name: 'Charlie', age: 35, city: 'New York'}),
             (:Person {name: 'Diana', age: 28, city: 'Boston'})
    `);
    console.log('Data inserted!\n');

    // Query all people
    console.log('Querying all people...');
    const allPeople = await db.query(session, `
      MATCH (p:Person)
      RETURN p.name, p.age, p.city
      ORDER BY p.age
    `);

    console.log(`Found ${allPeople.rowCount} people:`);
    for (const row of allPeople.rows) {
      console.log(`  - ${row['p.name']}, age ${row['p.age']}, from ${row['p.city']}`);
    }
    console.log();

    // Query with WHERE clause
    console.log('Querying people from New York...');
    const nyPeople = await db.query(session, `
      MATCH (p:Person)
      WHERE p.city = 'New York'
      RETURN p.name, p.age
    `);

    console.log(`Found ${nyPeople.rowCount} people in New York:`);
    for (const row of nyPeople.rows) {
      console.log(`  - ${row['p.name']}, age ${row['p.age']}`);
    }
    console.log();

    // Aggregation
    console.log('Computing statistics...');
    const stats = await db.query(session, `
      MATCH (p:Person)
      RETURN count(p) as total, avg(p.age) as avg_age, min(p.age) as min_age, max(p.age) as max_age
    `);

    const statsRow = stats.first();
    console.log(`Statistics:`);
    console.log(`  - Total people: ${statsRow.total}`);
    console.log(`  - Average age: ${statsRow.avg_age.toFixed(1)}`);
    console.log(`  - Min age: ${statsRow.min_age}`);
    console.log(`  - Max age: ${statsRow.max_age}`);
    console.log();

    // Using QueryResult methods
    console.log('Using QueryResult methods...');
    const names = allPeople.column('p.name');
    console.log(`All names: ${names.join(', ')}`);

    const adults = allPeople.filter(row => row['p.age'] >= 30);
    console.log(`Adults (30+): ${adults.length} people`);

    const firstPerson = allPeople.first();
    console.log(`First person: ${firstPerson['p.name']}`);
    console.log();

    // Close session
    console.log('Closing session...');
    await db.closeSession(session);

  } catch (error) {
    console.error('Error:', error.message);
    throw error;
  } finally {
    // Close database
    console.log('Closing database...');
    db.close();
    console.log('Done!');
  }
}

// Run the example
if (require.main === module) {
  main().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

module.exports = main;
