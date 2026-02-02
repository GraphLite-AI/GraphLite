/**
 * Complex queries example for GraphLite JavaScript bindings
 * 
 * Demonstrates:
 * - Pattern matching
 * - Relationships/edges
 * - Complex WHERE clauses
 * - Aggregations
 * - Ordering and limiting
 * - Graph traversal
 */

const { GraphLite } = require('..');

async function setupSocialNetwork(db, session) {
  console.log('Setting up social network...');
  
  await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS social_demo');
  await db.execute(session, 'SESSION SET SCHEMA social_demo');
  await db.execute(session, 'CREATE GRAPH IF NOT EXISTS network');
  await db.execute(session, 'SESSION SET GRAPH network');

  // Insert people
  await db.execute(session, `
    INSERT 
      (:Person {id: 1, name: 'Alice', age: 30, city: 'New York'}),
      (:Person {id: 2, name: 'Bob', age: 25, city: 'San Francisco'}),
      (:Person {id: 3, name: 'Charlie', age: 35, city: 'New York'}),
      (:Person {id: 4, name: 'Diana', age: 28, city: 'Boston'}),
      (:Person {id: 5, name: 'Eve', age: 32, city: 'Seattle'})
  `);

  console.log('Social network created!\n');
}

async function complexQueries() {
  console.log('GraphLite Complex Queries Example');
  console.log('==================================\n');

  const db = new GraphLite('./test_db');

  try {
    const session = db.createSession('admin');

    // Setup
    await setupSocialNetwork(db, session);

    // Query 1: Pattern matching with WHERE
    console.log('1. Pattern matching with WHERE clause:');
    const youngPeople = await db.query(session, `
      MATCH (p:Person)
      WHERE p.age < 30
      RETURN p.name, p.age, p.city
      ORDER BY p.age
    `);
    
    console.log(`   Found ${youngPeople.rowCount} people under 30:`);
    for (const row of youngPeople) {
      console.log(`   - ${row['p.name']}, ${row['p.age']} years old, ${row['p.city']}`);
    }
    console.log();

    // Query 2: Aggregation by group
    console.log('2. Aggregation - people per city:');
    const byCity = await db.query(session, `
      MATCH (p:Person)
      RETURN p.city as city, count(p) as count
      ORDER BY count DESC
    `);
    
    for (const row of byCity) {
      console.log(`   - ${row.city}: ${row.count} people`);
    }
    console.log();

    // Query 3: Complex WHERE conditions
    console.log('3. Complex WHERE conditions:');
    const filtered = await db.query(session, `
      MATCH (p:Person)
      WHERE p.age >= 28 AND p.age <= 32 AND p.city IN ['New York', 'Seattle']
      RETURN p.name, p.age, p.city
      ORDER BY p.age DESC
    `);
    
    console.log(`   People aged 28-32 in New York or Seattle:`);
    for (const row of filtered) {
      console.log(`   - ${row['p.name']}, ${row['p.age']}, ${row['p.city']}`);
    }
    console.log();

    // Query 4: Statistics
    console.log('4. Statistical aggregations:');
    const stats = await db.query(session, `
      MATCH (p:Person)
      RETURN 
        count(p) as total,
        avg(p.age) as avg_age,
        min(p.age) as min_age,
        max(p.age) as max_age,
        sum(p.age) as total_age
    `);
    
    const s = stats.first();
    console.log(`   Statistics:`);
    console.log(`   - Total people: ${s.total}`);
    console.log(`   - Average age: ${s.avg_age.toFixed(2)}`);
    console.log(`   - Age range: ${s.min_age} - ${s.max_age}`);
    console.log(`   - Total age sum: ${s.total_age}`);
    console.log();

    // Query 5: LIMIT and OFFSET
    console.log('5. Pagination with LIMIT:');
    const page1 = await db.query(session, `
      MATCH (p:Person)
      RETURN p.name, p.age
      ORDER BY p.name
      LIMIT 3
    `);
    
    console.log(`   First page (3 results):`);
    for (const row of page1) {
      console.log(`   - ${row['p.name']}, ${row['p.age']}`);
    }
    console.log();

    // Query 6: Using QueryResult methods
    console.log('6. Using QueryResult utility methods:');
    const allPeople = await db.query(session, `
      MATCH (p:Person)
      RETURN p.name, p.age, p.city
    `);
    
    // Get specific column
    const names = allPeople.column('p.name');
    console.log(`   All names: ${names.join(', ')}`);
    
    // Filter results
    const seniors = allPeople.filter(row => row['p.age'] >= 30);
    console.log(`   People 30+: ${seniors.length}`);
    
    // Map results
    const nameCityPairs = allPeople.map(row => `${row['p.name']} (${row['p.city']})`);
    console.log(`   Name-City pairs:`);
    nameCityPairs.forEach(pair => console.log(`   - ${pair}`));
    
    // Select specific columns
    const subset = allPeople.select('p.name', 'p.city');
    console.log(`   Selected columns (first):`, subset[0]);
    console.log();

    // Query 7: DISTINCT values
    console.log('7. Distinct cities:');
    const cities = await db.query(session, `
      MATCH (p:Person)
      RETURN DISTINCT p.city as city
      ORDER BY city
    `);
    
    console.log(`   Cities with people: ${cities.column('city').join(', ')}`);
    console.log();

    await db.closeSession(session);

  } catch (error) {
    console.error('Error:', error.message);
    throw error;
  } finally {
    db.close();
  }

  console.log('Complex queries example completed!');
}

// Run the example
if (require.main === module) {
  complexQueries().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

module.exports = complexQueries;
