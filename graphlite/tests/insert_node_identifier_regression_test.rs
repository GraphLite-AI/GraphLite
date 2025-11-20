//! INSERT Node Identifier Regression Tests
//!
//! This test suite verifies the fix for INSERT node identifier reuse issue.
//! Previously, using INSERT with node identifier reuse like (n) -[r:KNOWS]-> (m)
//! was creating spurious nodes with NULL names.
//!
//! The fix implemented two-pass processing in InsertExecutor to handle identifier reuse correctly.

#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

/// Helper function to create test fixture for regression tests
fn create_regression_test_fixture() -> Result<TestFixture, Box<dyn std::error::Error>> {
    let fixture = TestFixture::empty()?;

    // Create a test graph in the schema
    let graph_name = format!(
        "regression_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
            % 1000000
    );
    fixture.query(&format!(
        "CREATE GRAPH /{}/{}",
        fixture.schema_name(),
        graph_name
    ))?;
    fixture.query(&format!(
        "SESSION SET GRAPH /{}/{}",
        fixture.schema_name(),
        graph_name
    ))?;

    Ok(fixture)
}

#[test]
fn test_insert_with_identifier_reuse_basic() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Test Case 1: INSERT with identifier reuse
    // This was previously creating spurious nodes
    fixture.query(r#"INSERT (n:Person {name: "Alice Smith"}), (m:Person {name: "Bob Johnson"}), (n) -[r:KNOWS]-> (m)"#)
        .expect("INSERT with identifier reuse should succeed");

    // Verify: Should only have 2 nodes, not 3 with a NULL node
    let result = fixture
        .query("MATCH (n) RETURN n.name ORDER BY n.name")
        .expect("Query should succeed");

    for row in &result.rows {}

    assert_eq!(result.rows.len(), 2, "Should have exactly 2 nodes");

    // Verify the names are correct
    let names: Vec<String> = result
        .rows
        .iter()
        .map(|row| row.values["n.name"].as_string().unwrap().to_string())
        .collect();

    assert!(
        names.contains(&"Alice Smith".to_string()),
        "Should contain Alice Smith"
    );
    assert!(
        names.contains(&"Bob Johnson".to_string()),
        "Should contain Bob Johnson"
    );
}

#[test]
fn test_edge_creation_with_identifier_reuse() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Create nodes and edge with identifier reuse
    fixture.query(r#"INSERT (n:Person {name: "Alice Smith"}), (m:Person {name: "Bob Johnson"}), (n) -[r:KNOWS]-> (m)"#)
        .expect("INSERT should succeed");

    // Verify the edge was created correctly
    let result = fixture.query(r#"MATCH (n:Person {name: "Alice Smith"}) -[r:KNOWS]-> (m:Person {name: "Bob Johnson"}) RETURN n.name as source, m.name as target"#)
        .expect("Edge query should succeed");

    assert_eq!(result.rows.len(), 1, "Should find exactly 1 edge");

    let row = &result.rows[0];
    assert_eq!(row.values["source"].as_string().unwrap(), "Alice Smith");
    assert_eq!(row.values["target"].as_string().unwrap(), "Bob Johnson");
}

#[test]
fn test_no_empty_labels() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Create test data
    fixture.query(r#"INSERT (n:Person {name: "Alice Smith"}), (m:Person {name: "Bob Johnson"}), (n) -[r:KNOWS]-> (m)"#)
        .expect("INSERT should succeed");

    // Check that no nodes have empty labels
    let result = fixture
        .query("MATCH (n) WHERE size(labels(n)) = 0 RETURN count(n) as empty_nodes")
        .expect("Empty labels query should succeed");

    for row in &result.rows {}

    assert_eq!(result.rows.len(), 1, "Should return 1 count row");

    let empty_count = result.rows[0].values["empty_nodes"].as_number().unwrap();
    assert_eq!(empty_count, 0.0, "Should have no nodes with empty labels");
}

#[test]
fn test_content_based_deduplication() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Initial INSERT
    fixture.query(r#"INSERT (n:Person {name: "Alice Smith"}), (m:Person {name: "Bob Johnson"}), (n) -[r:KNOWS]-> (m)"#)
        .expect("Initial INSERT should succeed");

    // Count initial nodes
    let result1 = fixture
        .query("MATCH (n) RETURN count(n) as total_nodes")
        .expect("Count query should succeed");
    let initial_count = result1.rows[0].values["total_nodes"].as_number().unwrap();

    // Try to insert duplicate
    fixture
        .query(r#"INSERT (n:Person {name: "Alice Smith"})"#)
        .expect("Duplicate INSERT should succeed");

    // Verify count didn't increase
    let result2 = fixture
        .query("MATCH (n) RETURN count(n) as total_nodes")
        .expect("Count query should succeed");
    let final_count = result2.rows[0].values["total_nodes"].as_number().unwrap();

    assert_eq!(
        initial_count, final_count,
        "Duplicate INSERT should not increase node count"
    );
}

#[test]
fn test_complex_pattern_with_multiple_identifier_reuses() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Complex pattern with multiple identifier reuses
    fixture.query(r#"INSERT (a:Person {name: "Eve"}), (b:Person {name: "Frank"}), (c:Person {name: "Grace"}), (a) -[:KNOWS]-> (b), (b) -[:KNOWS]-> (c), (c) -[:KNOWS]-> (a)"#)
        .expect("Complex INSERT should succeed");

    // Verify we have exactly 3 nodes
    let result = fixture
        .query("MATCH (n) RETURN count(n) as total_nodes")
        .expect("Count query should succeed");

    let node_count = result.rows[0].values["total_nodes"].as_number().unwrap();
    assert_eq!(node_count, 3.0, "Should have exactly 3 nodes");

    // Verify all nodes have names
    let names_result = fixture
        .query("MATCH (n:Person) RETURN n.name ORDER BY n.name")
        .expect("Names query should succeed");

    for row in &names_result.rows {}

    assert_eq!(
        names_result.rows.len(),
        3,
        "Should find exactly 3 Person nodes"
    );

    let names: Vec<String> = names_result
        .rows
        .iter()
        .map(|row| row.values["n.name"].as_string().unwrap().to_string())
        .collect();

    assert!(names.contains(&"Eve".to_string()), "Should contain Eve");
    assert!(names.contains(&"Frank".to_string()), "Should contain Frank");
    assert!(names.contains(&"Grace".to_string()), "Should contain Grace");
}

#[test]
fn test_anonymous_nodes_with_content() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Create anonymous nodes with content
    fixture.query(r#"INSERT (:Anonymous {type: "TypeA", value: 100}) -[:CONNECTS]-> (:Anonymous {type: "TypeB", value: 200})"#)
        .expect("Anonymous INSERT should succeed");

    // Verify we have 2 nodes
    let result = fixture
        .query("MATCH (n) RETURN count(n) as total_nodes")
        .expect("Count query should succeed");

    let node_count = result.rows[0].values["total_nodes"].as_number().unwrap();
    assert_eq!(node_count, 2.0, "Should have exactly 2 anonymous nodes");

    // Verify anonymous nodes were created correctly
    let anon_result = fixture
        .query("MATCH (n:Anonymous) RETURN n.type, n.value ORDER BY n.type")
        .expect("Anonymous nodes query should succeed");
    assert_eq!(
        anon_result.rows.len(),
        2,
        "Should find exactly 2 Anonymous nodes"
    );
}

#[test]
fn test_hub_and_spoke_pattern() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Pattern with same identifier used multiple times in edges
    fixture.query(r#"INSERT (hub:Hub {name: "Central"}), (s1:Spoke {name: "Spoke1"}), (s2:Spoke {name: "Spoke2"}), (s3:Spoke {name: "Spoke3"}), (hub) -[:CONNECTS]-> (s1), (hub) -[:CONNECTS]-> (s2), (hub) -[:CONNECTS]-> (s3)"#)
        .expect("Hub and spoke INSERT should succeed");

    // Verify we have 4 nodes
    let result = fixture
        .query("MATCH (n) RETURN count(n) as total_nodes")
        .expect("Count query should succeed");

    let node_count = result.rows[0].values["total_nodes"].as_number().unwrap();
    assert_eq!(
        node_count, 4.0,
        "Should have exactly 4 nodes (1 hub + 3 spokes)"
    );

    // Verify edges were created correctly with identifier reuse
    let edge_result = fixture.query(r#"MATCH (h:Hub {name: "Central"}) -[:CONNECTS]-> (s:Spoke) RETURN h.name as hub, s.name as spoke ORDER BY s.name"#)
        .expect("Edge verification query should succeed");

    assert_eq!(
        edge_result.rows.len(),
        3,
        "Should find exactly 3 hub-to-spoke edges"
    );

    let spokes: Vec<String> = edge_result
        .rows
        .iter()
        .map(|row| row.values["spoke"].as_string().unwrap().to_string())
        .collect();

    assert!(
        spokes.contains(&"Spoke1".to_string()),
        "Should connect to Spoke1"
    );
    assert!(
        spokes.contains(&"Spoke2".to_string()),
        "Should connect to Spoke2"
    );
    assert!(
        spokes.contains(&"Spoke3".to_string()),
        "Should connect to Spoke3"
    );
}

#[test]
fn test_no_spurious_nodes_comprehensive() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Create various patterns that previously caused issues
    fixture.query(r#"INSERT (n:Person {name: "Alice Smith"}), (m:Person {name: "Bob Johnson"}), (n) -[r:KNOWS]-> (m)"#)
        .expect("INSERT 1 should succeed");

    fixture.query(r#"INSERT (a:Person {name: "Charlie"}), (b:Person {name: "Diana"}), (a) -[:FRIENDS]-> (b), (b) -[:FRIENDS]-> (a)"#)
        .expect("INSERT 2 should succeed");

    fixture
        .query(r#"INSERT (:Anonymous {type: "Test"}) -[:LINKS]-> (:Anonymous {type: "Other"})"#)
        .expect("INSERT 3 should succeed");

    // Final verification: No spurious nodes
    let result = fixture.query(r#"MATCH (n) WHERE size(labels(n)) = 0 AND n.name IS NULL AND n.type IS NULL RETURN count(n) as spurious_nodes"#)
        .expect("Spurious nodes query should succeed");

    let spurious_count = result.rows[0].values["spurious_nodes"].as_number().unwrap();
    assert_eq!(spurious_count, 0.0, "Should have no spurious nodes");

    // Verify all created nodes are valid
    let total_result = fixture
        .query("MATCH (n) RETURN count(n) as total_nodes")
        .expect("Total count query should succeed");

    let total_count = total_result.rows[0].values["total_nodes"]
        .as_number()
        .unwrap();

    // Should have: 2 (Alice, Bob) + 2 (Charlie, Diana) + 2 (Anonymous) = 6 nodes
    assert_eq!(total_count, 6.0, "Should have exactly 6 valid nodes");
}

#[test]
fn test_person_nodes_all_have_names() {
    let fixture =
        create_regression_test_fixture().expect("Failed to create regression test fixture");

    // Create Person nodes through various patterns
    fixture
        .query(
            r#"INSERT (n:Person {name: "Alice"}), (m:Person {name: "Bob"}), (n) -[r:KNOWS]-> (m)"#,
        )
        .expect("INSERT should succeed");

    fixture.query(r#"INSERT (p:Person {name: "Charlie"}), (q:Person {name: "Diana"}), (p) -[:FRIENDS]-> (q)"#)
        .expect("INSERT should succeed");

    // Verify all Person nodes have non-NULL names
    let result = fixture
        .query("MATCH (n:Person) WHERE n.name IS NULL RETURN count(n) as null_names")
        .expect("NULL names query should succeed");

    let null_count = result.rows[0].values["null_names"].as_number().unwrap();
    assert_eq!(null_count, 0.0, "No Person nodes should have NULL names");

    // Verify we can retrieve all Person names
    let names_result = fixture
        .query("MATCH (n:Person) RETURN n.name ORDER BY n.name")
        .expect("Names query should succeed");

    for row in &names_result.rows {
        let name = row.values["n.name"].as_string().unwrap();
        assert!(!name.is_empty(), "Person name should not be empty");
    }

    assert_eq!(
        names_result.rows.len(),
        4,
        "Should have exactly 4 Person nodes"
    );
}
