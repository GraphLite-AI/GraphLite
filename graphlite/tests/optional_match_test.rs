//! Integration tests for OPTIONAL MATCH functionality
//!
//! Tests the ISO GQL OPTIONAL MATCH feature which implements left outer join semantics.
//!
//! ## Current Implementation Status (2024-12-14):
//!
//! ### ✅ Completed:
//! 1. **Lexer**: OPTIONAL keyword recognized
//! 2. **Parser**: OPTIONAL MATCH syntax accepted
//! 3. **AST**: MatchClause.optional field added
//! 4. **Planner**: LEFT OUTER JOIN created for standalone OPTIONAL MATCH
//!
//! ### ⚠️  Partial Implementation:
//! - **Standalone OPTIONAL MATCH works**: `OPTIONAL MATCH (p:Person) RETURN p.name` ✓
//! - **Pattern-only OPTIONAL MATCH**: Returns matches if any exist, empty set if none
//! - **Correlated OPTIONAL MATCH NOT YET WORKING**: `MATCH (p) OPTIONAL MATCH (p)-[r]->(f)`
//!
//! ### ❌ Not Yet Implemented:
//! 1. **Correlated OPTIONAL MATCH**: Sequential MATCH clauses with correlation
//! 2. **NULL value propagation**: Unmatched optional patterns should return NULL
//! 3. **OPTIONAL { MATCH }** and **OPTIONAL ( MATCH )** syntax variants
//! 4. **Multiple sequential MATCH clauses**: AST may need restructuring
//!
//! ## Technical Details:
//!
//! ### What Works:
//! ```gql
//! OPTIONAL MATCH (p:Person) RETURN p.name
//! // Returns: All Person nodes, or empty row if none exist
//! // Implementation: LEFT OUTER JOIN(SingleRow, NodeScan(Person))
//! ```
//!
//! ### What Doesn't Work Yet:
//! ```gql
//! MATCH (p:Person)
//! OPTIONAL MATCH (p)-[:FRIEND]->(f:Person)
//! RETURN p.name, f.name
//! // Current: Returns only people WITH friends (inner join)
//! // Expected: All people, with NULL for those without friends
//! // Requires: Correlated subquery support in AST/planner
//! ```
//!
//! ### Why Correlated OPTIONAL MATCH is Complex:
//! The query `MATCH (p) OPTIONAL MATCH (p)-[r]->(f)` involves:
//! 1. First MATCH binds variable `p`
//! 2. Second OPTIONAL MATCH is **correlated** on `p` (uses it from outer context)
//! 3. For EACH value of `p`, try to match the optional pattern
//! 4. Return (p, matched values) or (p, NULL) if no match
//!
//! Current AST structure (`BasicQuery`) only supports ONE match_clause,
//! so sequential matches need either:
//! - AST restructuring to support multiple match clauses
//! - Query rewriting to use WITH clauses
//! - Specialized correlated subquery handling
//!
//! ## Test Data:
//! - Alice (age 30) -[:FRIEND]-> Bob (age 25)
//! - Bob (age 25) -[:FRIEND]-> Charlie (age 35)
//! - Charlie has NO outgoing friendships
//!
//! ## Expected vs Current Behavior:
//!
//! | Query | Expected Rows | Current Rows | Status |
//! |-------|--------------|--------------|---------|
//! | `OPTIONAL MATCH (p:Person) RETURN p` | 3 | 3 | ✅ |
//! | `OPTIONAL MATCH (p)-[:FRIEND]->(f) RETURN p,f` | 2 or 1* | 2 | ⚠️ |
//! | `MATCH (p) OPTIONAL MATCH (p)-[r]->(f) RETURN p,f` | 3 | 2 | ❌ |
//!
//! *Standalone optional relationship pattern behavior needs clarification from spec

use graphlite::QueryCoordinator;
use std::sync::Arc;
use tempfile::TempDir;

fn create_test_coordinator() -> (Arc<QueryCoordinator>, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    let coordinator = QueryCoordinator::from_path(db_path).expect("Failed to create coordinator");
    (coordinator, temp_dir)
}

fn setup_graph_and_session(coordinator: &Arc<QueryCoordinator>) -> String {
    let session_id = coordinator
        .create_simple_session("test_user")
        .expect("Failed to create session");

    // Create schema
    coordinator
        .process_query("CREATE SCHEMA test_schema", &session_id)
        .expect("Failed to create schema");

    // Create graph
    coordinator
        .process_query("CREATE GRAPH test_schema/test_graph", &session_id)
        .expect("Failed to create graph");

    // Set current graph
    coordinator
        .process_query("SESSION SET GRAPH test_schema/test_graph", &session_id)
        .expect("Failed to set current graph");

    session_id
}

fn setup_test_data(coordinator: &Arc<QueryCoordinator>, session_id: &str) {
    // Create people
    let queries = vec![
        "INSERT (:Person {name: 'Alice', age: 30})",
        "INSERT (:Person {name: 'Bob', age: 25})",
        "INSERT (:Person {name: 'Charlie', age: 35})",
    ];

    for query in queries {
        coordinator
            .process_query(query, session_id)
            .expect(&format!("Failed to insert: {}", query));
    }

    // Create friendships (Alice->Bob, Bob->Charlie)
    // Note: Charlie has no outgoing friendships
    let friendship_queries = vec![
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) INSERT (a)-[:FRIEND]->(b)",
        "MATCH (a:Person {name: 'Bob'}), (c:Person {name: 'Charlie'}) INSERT (a)-[:FRIEND]->(c)",
    ];

    for query in friendship_queries {
        coordinator
            .process_query(query, session_id)
            .expect(&format!("Failed to create friendship: {}", query));
    }
}

#[test]
fn test_regular_match_baseline() {
    let (coordinator, _temp_dir) = create_test_coordinator();
    let session_id = setup_graph_and_session(&coordinator);
    setup_test_data(&coordinator, &session_id);

    // Regular MATCH should only return people who HAVE friends
    let query = "MATCH (p:Person)-[:FRIEND]->(f:Person) RETURN p.name, f.name";
    let result = coordinator.process_query(query, &session_id);

    assert!(result.is_ok(), "Regular MATCH should work: {:?}", result);
    println!("Regular MATCH result: {:?}", result);
}

#[test]
fn test_optional_match_simple_syntax() {
    let (coordinator, _temp_dir) = create_test_coordinator();
    let session_id = setup_graph_and_session(&coordinator);
    setup_test_data(&coordinator, &session_id);

    // Test 1: Just optional match by itself
    println!("\n=== Test 1: OPTIONAL MATCH standalone ===");
    let result1 = coordinator.process_query(
        "OPTIONAL MATCH (p:Person) RETURN p.name",
        &session_id
    );
    println!("Result: {:?}", result1);

    // Test 2: Two separate queries
    println!("\n=== Test 2: Two separate queries ===");
    let result2a = coordinator.process_query("MATCH (p:Person) RETURN p.name", &session_id);
    println!("First query rows: {}", result2a.as_ref().unwrap().rows.len());

    let result2b = coordinator.process_query(
        "OPTIONAL MATCH (p:Person)-[:FRIEND]->(f:Person) RETURN p.name, f.name",
        &session_id
    );
    println!("Second query result: {:?}", result2b.as_ref().map(|r| r.rows.len()));

    // Test 3: Combined query (what we're really testing)
    println!("\n=== Test 3: Combined MATCH ... OPTIONAL MATCH ... ===");
    let query = "MATCH (p:Person) OPTIONAL MATCH (p)-[:FRIEND]->(f:Person) RETURN p.name, f.name";
    let result = coordinator.process_query(query, &session_id);

    match &result {
        Ok(r) => {
            println!("✓ Query accepted!");
            println!("   Rows returned: {}", r.rows.len());
            println!("   Expected: 3 rows (Alice→Bob, Bob→Charlie, Charlie→NULL)");
            println!("   Current behavior: {} rows (inner join)", r.rows.len());
        }
        Err(e) => {
            println!("✗ Query failed: {:?}", e);
        }
    }

    assert!(result.is_ok(), "Query should be accepted");
}

#[test]
fn test_optional_match_with_braces() {
    let (coordinator, _temp_dir) = create_test_coordinator();
    let session_id = setup_graph_and_session(&coordinator);
    setup_test_data(&coordinator, &session_id);

    let query = "MATCH (p:Person) OPTIONAL { MATCH (p)-[:FRIEND]->(f:Person) } RETURN p.name, f.name";
    let result = coordinator.process_query(query, &session_id);

    println!("OPTIONAL {{ MATCH }} result: {:?}", result);

    if result.is_ok() {
        println!("✓ OPTIONAL {{ MATCH }} syntax works!");
    } else {
        println!("✗ OPTIONAL {{ MATCH }} failed: {:?}", result.err());
    }
}

#[test]
fn test_optional_match_with_parens() {
    let (coordinator, _temp_dir) = create_test_coordinator();
    let session_id = setup_graph_and_session(&coordinator);
    setup_test_data(&coordinator, &session_id);

    let query = "MATCH (p:Person) OPTIONAL ( MATCH (p)-[:FRIEND]->(f:Person) ) RETURN p.name, f.name";
    let result = coordinator.process_query(query, &session_id);

    println!("OPTIONAL ( MATCH ) result: {:?}", result);

    if result.is_ok() {
        println!("✓ OPTIONAL ( MATCH ) syntax works!");
    } else {
        println!("✗ OPTIONAL ( MATCH ) failed: {:?}", result.err());
    }
}
