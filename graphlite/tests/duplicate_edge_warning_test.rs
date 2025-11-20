// Test to verify that duplicate edge insertion produces a warning
// This test verifies the fix for: https://github.com/GraphLite-AI/GraphLite/issues/XXX

#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_duplicate_edge_shows_warning() {
    // Create a fresh test fixture
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup graph context
    fixture
        .setup_graph("duplicate_edge_test")
        .expect("Failed to setup graph");

    // Setup: Create two people
    fixture
        .query("INSERT (:Person {name: 'Charlie'}), (:Person {name: 'Diana'})")
        .expect("Node creation should succeed");

    // First edge insertion - should succeed without warnings
    let result1 = fixture
        .query(
            "MATCH (c:Person {name: 'Charlie'}), (d:Person {name: 'Diana'})
         INSERT (c)-[:KNOWS {since: 2021, strength: 'weak'}]->(d)",
        )
        .expect("First edge insertion should succeed");

    assert_eq!(
        result1.rows_affected, 1,
        "First insertion should affect 1 row"
    );
    assert!(
        result1.warnings.is_empty(),
        "First insertion should have no warnings"
    );

    // Second edge insertion - should detect duplicate and show warning
    let result2 = fixture
        .query(
            "MATCH (c:Person {name: 'Charlie'}), (d:Person {name: 'Diana'})
         INSERT (c)-[:KNOWS {since: 2021, strength: 'weak'}]->(d)",
        )
        .expect("Second edge insertion should succeed (but skip duplicate)");

    // Verify warning exists
    assert!(
        !result2.warnings.is_empty(),
        "Expected warning for duplicate edge insertion"
    );

    assert!(
        result2.warnings[0].contains("Duplicate edge detected"),
        "Warning should mention 'Duplicate edge detected', got: {}",
        result2.warnings[0]
    );

    // Verify rows_affected is 0 for duplicate insert
    assert_eq!(
        result2.rows_affected, 0,
        "Duplicate edge insertion should not affect any rows"
    );

    // Verify only one edge exists
    let count_result = fixture
        .query("MATCH ()-[r:KNOWS]->() RETURN COUNT(r) AS edge_count")
        .expect("Count query should succeed");

    let edge_count = count_result.rows[0].values["edge_count"]
        .as_number()
        .expect("Should get a number");

    assert_eq!(
        edge_count, 1.0,
        "Should have exactly 1 edge after duplicate insertion attempt, found: {}",
        edge_count
    );
}

#[test]
fn test_duplicate_edge_with_regular_insert() {
    // Test that duplicate edge detection works with inline node creation
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup graph context
    fixture
        .setup_graph("regular_insert_test")
        .expect("Failed to setup graph");

    // First insertion with inline nodes - should succeed
    let result1 = fixture
        .query("INSERT (:User {id: 'u1'})-[:FOLLOWS {since: '2020-01-01'}]->(:User {id: 'u2'})")
        .expect("First insertion should succeed");

    assert!(result1.rows_affected >= 1, "Should create nodes and edge");
    assert!(result1.warnings.is_empty());

    // Second insertion - nodes already exist, so only edge insertion is attempted
    // Since nodes are duplicates, they won't be re-created, but we'll try to create the edge again
    let result2 = fixture
        .query("INSERT (:User {id: 'u1'})-[:FOLLOWS {since: '2020-01-01'}]->(:User {id: 'u2'})")
        .expect("Second insertion should succeed");

    // Should have warnings - at minimum for duplicate nodes
    // Edge warning appears only if nodes already existed and edge is duplicate
    assert!(
        !result2.warnings.is_empty(),
        "Should have warnings for duplicates"
    );

    // Verify we have node duplicate warnings
    let has_node_warnings = result2
        .warnings
        .iter()
        .any(|w| w.contains("Duplicate node detected"));

    assert!(
        has_node_warnings,
        "Should have 'Duplicate node detected' warnings, got: {:?}",
        result2.warnings
    );
}
