//! Basic usage example for GraphLite SDK
//!
//! This example demonstrates the core features of the GraphLite Rust SDK:
//! - Opening a database
//! - Creating sessions
//! - Executing queries
//! - Using transactions
//! - Query builder API
//! - Typed result deserialization
//!
//! Run with: cargo run --example basic_usage

use graphlite_sdk::{GraphLite, Error};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Person {
    name: String,
    age: f64,
}

fn main() -> Result<(), Error> {
    println!("=== GraphLite SDK Basic Usage Example ===\n");

    // 1. Open a database
    println!("1. Opening database...");
    let db_path = "/tmp/graphlite_sdk_example";
    let db = GraphLite::open(db_path)?;
    println!("   ✓ Database opened at {}\n", db_path);

    // 2. Create a session
    println!("2. Creating session...");
    let session = db.session("admin")?;
    println!("   ✓ Session created for user 'admin'\n");

    // 3. Execute DDL statements
    println!("3. Creating schema and graph...");
    session.execute("CREATE SCHEMA IF NOT EXISTS example")?;
    session.execute("USE SCHEMA example")?;
    session.execute("CREATE GRAPH IF NOT EXISTS social")?;
    session.execute("USE GRAPH social")?;
    println!("   ✓ Schema and graph created\n");

    // 4. Insert data using transactions
    println!("4. Inserting data with transaction...");
    {
        let mut tx = session.transaction()?;
        tx.execute("CREATE (p:Person {name: 'Alice', age: 30})")?;
        tx.execute("CREATE (p:Person {name: 'Bob', age: 25})")?;
        tx.execute("CREATE (p:Person {name: 'Charlie', age: 35})")?;
        tx.commit()?;
        println!("   ✓ Inserted 3 persons\n");
    }

    // 5. Query data directly
    println!("5. Querying data...");
    let result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")?;
    println!("   Found {} persons:", result.rows.len());
    for row in &result.rows {
        if let (Some(name), Some(age)) = (row.get_value("name"), row.get_value("age")) {
            println!("   - Name: {:?}, Age: {:?}", name, age);
        }
    }
    println!();

    // 6. Use query builder
    println!("6. Using query builder...");
    let result = session.query_builder()
        .match_pattern("(p:Person)")
        .where_clause("p.age > 25")
        .return_clause("p.name as name, p.age as age")
        .order_by("p.age DESC")
        .execute()?;
    println!("   Found {} persons over 25:", result.rows.len());
    for row in &result.rows {
        if let (Some(name), Some(age)) = (row.get_value("name"), row.get_value("age")) {
            println!("   - Name: {:?}, Age: {:?}", name, age);
        }
    }
    println!();

    // 7. Typed deserialization
    println!("7. Using typed deserialization...");
    let result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")?;
    let typed = graphlite_sdk::TypedResult::from(result);
    let people: Vec<Person> = typed.deserialize_rows()?;
    println!("   Deserialized {} persons:", people.len());
    for person in &people {
        println!("   - {:?}", person);
    }
    println!();

    // 8. Transaction with rollback
    println!("8. Demonstrating transaction rollback...");
    {
        let mut tx = session.transaction()?;
        tx.execute("CREATE (p:Person {name: 'David', age: 40})")?;
        println!("   Created person 'David' in transaction");
        // Transaction is dropped without commit - automatically rolls back
    }
    println!("   Transaction rolled back (David not persisted)\n");

    // 9. Verify rollback
    let result = session.query("MATCH (p:Person) RETURN count(p) as count")?;
    if let Some(row) = result.rows.first() {
        if let Some(count) = row.get_value("count") {
            println!("   Person count after rollback: {:?}\n", count);
        }
    }

    println!("=== Example completed successfully ===");
    Ok(())
}
