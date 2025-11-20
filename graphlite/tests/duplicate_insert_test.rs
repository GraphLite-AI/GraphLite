#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_duplicate_insert_creates_two_nodes() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup a fresh graph for this test
    fixture
        .setup_graph("duplicate_test_graph")
        .expect("Failed to setup graph");

    // Execute FIRST INSERT
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35, city: 'Seattle'})")
        .expect("First INSERT should succeed");

    // Execute SECOND INSERT (identical)
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35, city: 'Seattle'})")
        .expect("Second INSERT should succeed");

    // Query to count how many Person nodes exist
    let count_result = fixture
        .query("MATCH (p:Person) RETURN count(p) as total")
        .expect("Count query should succeed");

    // Verify the count
    assert_eq!(
        count_result.rows.len(),
        1,
        "Should return exactly one row for count query"
    );

    let count_value = count_result.rows[0]
        .values
        .get("total")
        .expect("Should have 'total' field in result");

    let actual_count = match count_value {
        graphlite::Value::Number(n) => *n,
        _ => panic!("Count should be a number, got: {:?}", count_value),
    };

    // Assert: GraphLite's hash-based deduplication means only 1 node is created
    assert_eq!(
        actual_count, 1.0,
        "Expected 1 node (hash-based deduplication), but found {} nodes",
        actual_count
    );

    // Verify that the second INSERT returned a warning about the duplicate
    // Let's re-run the INSERT to capture the warning
    let duplicate_insert_result = fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35, city: 'Seattle'})")
        .expect("Duplicate INSERT should succeed with warning");

    // Verify warning was generated
    assert!(
        !duplicate_insert_result.warnings.is_empty(),
        "Expected warning for duplicate INSERT, but got none"
    );

    // Verify warning message content
    let warning = &duplicate_insert_result.warnings[0];
    assert!(
        warning.contains("Duplicate node detected"),
        "Warning should mention 'Duplicate node detected', got: {}",
        warning
    );

    // Verify rows_affected is 0 for duplicate insert
    assert_eq!(
        duplicate_insert_result.rows_affected, 0,
        "Expected rows_affected = 0 for duplicate INSERT, got {}",
        duplicate_insert_result.rows_affected
    );

    // Query to see all Person nodes with their IDs
    let all_persons = fixture
        .query("MATCH (p:Person) RETURN p.name, p.age, p.city, ID(p) as node_id")
        .expect("Query should succeed");

    // Verify only one Person node exists
    assert_eq!(
        all_persons.rows.len(),
        1,
        "Expected exactly 1 Person node after duplicate INSERT, found {}",
        all_persons.rows.len()
    );

    // Verify the node has correct properties
    let person = &all_persons.rows[0];
    assert_eq!(
        person.values.get("p.name"),
        Some(&graphlite::Value::String("Charlie".to_string()))
    );
    assert_eq!(
        person.values.get("p.age"),
        Some(&graphlite::Value::Number(35.0))
    );
    assert_eq!(
        person.values.get("p.city"),
        Some(&graphlite::Value::String("Seattle".to_string()))
    );
}

#[test]
fn test_insert_with_same_content_node_ids() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup a fresh graph for this test
    fixture
        .setup_graph("node_id_test_graph")
        .expect("Failed to setup graph");

    // Insert first node
    fixture
        .query("INSERT (n:Person {name: 'Alice', age: 30})")
        .expect("First INSERT should succeed");

    // Insert second identical node
    fixture
        .query("INSERT (n:Person {name: 'Alice', age: 30})")
        .expect("Second INSERT should succeed");

    // Get all node IDs
    let result = fixture
        .query("MATCH (p:Person) RETURN p.name, ID(p) as node_id")
        .expect("Query should succeed");

    // Verify only one node was created
    assert_eq!(
        result.rows.len(),
        1,
        "Expected exactly 1 Person node (hash-based deduplication), found {}",
        result.rows.len()
    );

    // Verify the node has correct name
    let person = &result.rows[0];
    assert_eq!(
        person.values.get("p.name"),
        Some(&graphlite::Value::String("Alice".to_string())),
        "Person node should have name 'Alice'"
    );

    // Get the node ID (should be the same for both inserts due to hash-based deduplication)
    let node_id = person
        .values
        .get("node_id")
        .expect("Should have 'node_id' field");

    // Re-run the second INSERT to verify it returns a warning
    let duplicate_result = fixture
        .query("INSERT (n:Person {name: 'Alice', age: 30})")
        .expect("Duplicate INSERT should succeed with warning");

    // Verify warning exists
    assert!(
        !duplicate_result.warnings.is_empty(),
        "Expected warning for duplicate INSERT"
    );
    assert!(
        duplicate_result.warnings[0].contains("Duplicate node detected"),
        "Warning should mention duplicate detection, got: {}",
        duplicate_result.warnings[0]
    );

    // Verify rows_affected is 0
    assert_eq!(
        duplicate_result.rows_affected, 0,
        "Expected rows_affected = 0 for duplicate INSERT, got {}",
        duplicate_result.rows_affected
    );
}

#[test]
fn test_multiple_inserts_in_sequence() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup a fresh graph for this test
    fixture
        .setup_graph("sequence_test_graph")
        .expect("Failed to setup graph");

    // Execute 3 identical inserts
    for i in 1..=3 {
        fixture
            .query("INSERT (:Person {name: 'Bob', age: 25})")
            .expect(&format!("INSERT {} should succeed", i));
    }

    // Check total count
    let count_result = fixture
        .query("MATCH (p:Person {name: 'Bob'}) RETURN count(p) as total")
        .expect("Count query should succeed");

    // Verify the count
    assert_eq!(
        count_result.rows.len(),
        1,
        "Should return exactly one row for count query"
    );

    let count_value = count_result.rows[0]
        .values
        .get("total")
        .expect("Should have 'total' field in result");

    let actual_count = match count_value {
        graphlite::Value::Number(n) => *n,
        _ => panic!("Count should be a number, got: {:?}", count_value),
    };

    // Assert: With hash-based deduplication, 3 identical INSERTs create only 1 node
    assert_eq!(
        actual_count, 1.0,
        "Expected 1 node after 3 identical INSERTs (hash-based deduplication), but found {} nodes",
        actual_count
    );

    // Re-run an INSERT to verify it returns a warning
    let duplicate_result = fixture
        .query("INSERT (:Person {name: 'Bob', age: 25})")
        .expect("Duplicate INSERT should succeed with warning");

    // Verify warning exists
    assert!(
        !duplicate_result.warnings.is_empty(),
        "Expected warning for duplicate INSERT"
    );
    assert!(
        duplicate_result.warnings[0].contains("Duplicate node detected"),
        "Warning should mention duplicate detection"
    );

    // Verify rows_affected is 0
    assert_eq!(
        duplicate_result.rows_affected, 0,
        "Expected rows_affected = 0 for duplicate INSERT"
    );
}
