// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Simple example demonstrating GraphLite embedding in Rust applications
//!
//! This example shows the recommended way to use GraphLite's public API.
//!
//! Run with: cargo run --example simple_usage

use graphlite::QueryCoordinator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== GraphLite Simple Usage Example ===\n");

    // Step 1: Initialize GraphLite with a database path
    // This single line handles all internal component setup
    println!("1. Initializing database...");
    let coordinator = QueryCoordinator::from_path("./example_db")
        .map_err(|e| format!("Failed to initialize database: {}", e))?;
    println!("   ✓ Database initialized\n");

    // Step 2: Create a session
    // Sessions track user context and graph/schema preferences
    println!("2. Creating session...");
    let session_id = coordinator.create_simple_session("example_user")
        .map_err(|e| format!("Failed to create session: {}", e))?;
    println!("   ✓ Session created: {}\n", session_id);

    // Step 3: Validate and analyze queries before execution
    println!("3. Validating and analyzing queries...");

    let test_query = "MATCH (n:Person) RETURN n.name";

    // Validate query syntax
    println!("   → Validating query...");
    match coordinator.validate_query(test_query) {
        Ok(_) => println!("     ✓ Query is valid"),
        Err(e) => println!("     ✗ Query validation failed: {}", e),
    }

    // Check if query is valid (convenience method)
    if coordinator.is_valid_query(test_query) {
        println!("     ✓ Query syntax check passed");
    }

    // Analyze query
    println!("   → Analyzing query...");
    match coordinator.analyze_query(test_query) {
        Ok(info) => {
            println!("     ✓ Query type: {:?}", info.query_type);
            println!("     ✓ Read-only: {}", info.is_read_only);
        }
        Err(e) => println!("     ✗ Query analysis failed: {}", e),
    }

    // Test with an invalid query
    println!("   → Testing invalid query...");
    let invalid_query = "MATCH (n RETURN n";
    if !coordinator.is_valid_query(invalid_query) {
        println!("     ✓ Correctly detected invalid query");
    }

    // Explain query execution plan
    println!("   → Explaining query execution plan...");
    match coordinator.explain_query(test_query) {
        Ok(plan) => {
            println!("     ✓ {}", plan.summary());
            println!("     Query Plan:\n{}", plan.format_tree());
        }
        Err(e) => println!("     ⚠ Query explain: {}", e),
    }

    // Step 4: Execute queries
    println!("\n4. Executing queries...");

    // Create a schema
    println!("   → Creating schema...");
    match coordinator.process_query("CREATE SCHEMA IF NOT EXISTS /example_schema", &session_id) {
        Ok(_) => println!("     ✓ Schema created"),
        Err(e) => println!("     ⚠ Schema creation: {}", e),
    }

    // Set the schema for this session
    println!("   → Setting schema...");
    coordinator.process_query("SESSION SET SCHEMA /example_schema", &session_id)?;
    println!("     ✓ Schema set");

    // Create a graph
    println!("   → Creating graph...");
    coordinator.process_query("CREATE GRAPH IF NOT EXISTS example_graph", &session_id)?;
    println!("     ✓ Graph created");

    // Set the graph for this session
    println!("   → Setting graph...");
    coordinator.process_query("SESSION SET GRAPH example_graph", &session_id)?;
    println!("     ✓ Graph set");

    // Insert some data
    println!("   → Inserting nodes...");
    coordinator.process_query(
        "INSERT (:Person {name: 'Alice', age: 30}), (:Person {name: 'Bob', age: 25})",
        &session_id
    )?;
    println!("     ✓ Nodes inserted");

    // Query the data
    println!("   → Querying data...");
    let result = coordinator.process_query(
        "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age",
        &session_id
    )?;
    println!("     ✓ Query executed");

    // Display results
    println!("\n5. Results:");
    println!("   Columns: {:?}", result.variables);
    println!("   Row count: {}", result.rows.len());
    for (i, row) in result.rows.iter().enumerate() {
        println!("   Row {}: {:?}", i + 1, row.values);
    }

    // Step 6: Clean up
    println!("\n6. Closing session...");
    coordinator.close_session(&session_id)?;
    println!("   ✓ Session closed");

    println!("\n=== Example Complete ===");
    println!("\nNote: Database files are stored in ./example_db/");
    println!("To clean up: rm -rf ./example_db/");

    Ok(())
}
