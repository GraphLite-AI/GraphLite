//! Test identity-based set operations (UNION, INTERSECT, EXCEPT)
//!
//! This test verifies that set operations compare nodes by their identity
//! rather than by their projected property values.

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_intersect_with_identity_tracking() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Create a test graph
    fixture
        .query(&format!(
            "CREATE GRAPH /{}/identity_test",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "SESSION SET GRAPH /{}/identity_test",
            fixture.schema_name()
        ))
        .unwrap();

    // Insert test data - same people but we'll project different properties
    fixture
        .query(r#"INSERT (p1:Person {name: "Alice", age: 30, city: "NYC", salary: 100000})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p2:Person {name: "Bob", age: 25, city: "LA", salary: 80000})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p3:Person {name: "Charlie", age: 35, city: "Chicago", salary: 120000})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p4:Person {name: "Diana", age: 28, city: "NYC", salary: 95000})"#)
        .unwrap();

    // Test 1: INTERSECT with same nodes but different property projections
    log::debug!("\n=== Test 1: Identity-based INTERSECT ===");

    // Query 1: Get people in NYC, project name and age
    let query1 = r#"MATCH (p:Person) WHERE p.city = "NYC" RETURN p.name, p.age"#;
    let result1 = fixture.query(query1).unwrap();
    log::debug!("Query 1 (NYC, name+age): {} rows", result1.rows.len());
    for row in &result1.rows {
        log::debug!("  Row: {:?}", row.values);
        log::debug!("  Entities tracked: {:?}", row.source_entities);
    }

    // Query 2: Get high earners, project name and salary
    let query2 = r#"MATCH (p:Person) WHERE p.salary > 90000 RETURN p.name, p.salary"#;
    let result2 = fixture.query(query2).unwrap();
    log::debug!(
        "\nQuery 2 (salary>90000, name+salary): {} rows",
        result2.rows.len()
    );
    for row in &result2.rows {
        log::debug!("  Row: {:?}", row.values);
        log::debug!("  Entities tracked: {:?}", row.source_entities);
    }

    // INTERSECT: Should find nodes that appear in both (Alice and Diana)
    // Note: The projections are different (age vs salary) but it's the same nodes
    let intersect_query = format!("{} INTERSECT {}", query1, query2);
    log::debug!("\nRunning INTERSECT query...");
    let intersect_result = fixture.query(&intersect_query).unwrap();

    log::debug!("INTERSECT result: {} rows", intersect_result.rows.len());
    for row in &intersect_result.rows {
        log::debug!("  Row: {:?}", row.values);
        log::debug!("  Entities tracked: {:?}", row.source_entities);
    }

    // With identity-based comparison, we should get the common nodes
    // Alice is in NYC (query1) and has salary > 90000 (100000)
    // Diana is in NYC (query1) and has salary > 90000 (95000)
    // So we should get 2 rows
    assert_eq!(
        intersect_result.rows.len(),
        2,
        "INTERSECT should return 2 common nodes"
    );
}

#[test]
fn test_union_deduplicates_by_identity() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    fixture
        .query(&format!(
            "CREATE GRAPH /{}/union_identity_test",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "SESSION SET GRAPH /{}/union_identity_test",
            fixture.schema_name()
        ))
        .unwrap();

    // Insert test nodes
    fixture
        .query(r#"INSERT (p1:Person {name: "Alice", age: 30, dept: "Engineering"})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p2:Person {name: "Bob", age: 25, dept: "Sales"})"#)
        .unwrap();

    log::debug!("\n=== Test 2: Identity-based UNION deduplication ===");

    // Query same person with different projections
    let query1 = r#"MATCH (p:Person) WHERE p.name = "Alice" RETURN p.name, p.age"#;
    let query2 = r#"MATCH (p:Person) WHERE p.name = "Alice" RETURN p.name, p.dept"#;

    let result1 = fixture.query(query1).unwrap();
    log::debug!("Query 1 result: {} rows", result1.rows.len());
    for row in &result1.rows {
        log::debug!("  Values: {:?}", row.values);
        log::debug!("  Entities: {:?}", row.source_entities);
    }

    let result2 = fixture.query(query2).unwrap();
    log::debug!("\nQuery 2 result: {} rows", result2.rows.len());
    for row in &result2.rows {
        log::debug!("  Values: {:?}", row.values);
        log::debug!("  Entities: {:?}", row.source_entities);
    }

    // UNION should deduplicate based on node identity, not values
    let union_query = format!("{} UNION {}", query1, query2);
    log::debug!("\nRunning UNION query...");
    let union_result = fixture.query(&union_query).unwrap();

    log::debug!("UNION result: {} rows", union_result.rows.len());
    for row in &union_result.rows {
        log::debug!("  Values: {:?}", row.values);
        log::debug!("  Entities: {:?}", row.source_entities);
    }

    // With identity-based deduplication, we should get 1 row (same node)
    // Without it, we'd get 2 rows (different projections)
    assert_eq!(
        union_result.rows.len(),
        1,
        "UNION should deduplicate the same node"
    );
}

#[test]
fn test_except_removes_by_identity() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    fixture
        .query(&format!(
            "CREATE GRAPH /{}/except_identity_test",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "SESSION SET GRAPH /{}/except_identity_test",
            fixture.schema_name()
        ))
        .unwrap();

    // Insert test nodes
    fixture
        .query(r#"INSERT (p1:Person {name: "Alice", active: true, score: 100})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p2:Person {name: "Bob", active: true, score: 50})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p3:Person {name: "Charlie", active: false, score: 75})"#)
        .unwrap();

    log::debug!("\n=== Test 3: Identity-based EXCEPT ===");

    // Get all people, project name and active status
    let query1 = r#"MATCH (p:Person) RETURN p.name, p.active"#;
    // Get high scorers, project name and score
    let query2 = r#"MATCH (p:Person) WHERE p.score >= 100 RETURN p.name, p.score"#;

    let result1 = fixture.query(query1).unwrap();
    log::debug!("Query 1 (all people): {} rows", result1.rows.len());

    let result2 = fixture.query(query2).unwrap();
    log::debug!("Query 2 (high scorers): {} rows", result2.rows.len());

    // EXCEPT should remove nodes by identity
    let except_query = format!("{} EXCEPT {}", query1, query2);
    log::debug!("\nRunning EXCEPT query...");
    let except_result = fixture.query(&except_query).unwrap();

    log::debug!("EXCEPT result: {} rows", except_result.rows.len());
    for row in &except_result.rows {
        log::debug!("  Values: {:?}", row.values);
        log::debug!("  Entities: {:?}", row.source_entities);
    }

    // Should exclude Alice (score >= 100) but keep Bob and Charlie
    assert_eq!(
        except_result.rows.len(),
        2,
        "EXCEPT should exclude high scorers by identity"
    );
}

#[test]
fn test_identity_tracking_debug() {
    // Simple test to verify entity tracking is working
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    fixture
        .query(&format!(
            "CREATE GRAPH /{}/debug_test",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "SESSION SET GRAPH /{}/debug_test",
            fixture.schema_name()
        ))
        .unwrap();

    fixture
        .query(r#"INSERT (p:Person {name: "Test Person"})"#)
        .unwrap();

    // Simple query to check if entities are being tracked
    let result = fixture.query("MATCH (p:Person) RETURN p.name").unwrap();

    log::debug!("\n=== Debug: Entity Tracking ===");
    log::debug!("Query: MATCH (p:Person) RETURN p.name");
    log::debug!("Result rows: {}", result.rows.len());

    for (i, row) in result.rows.iter().enumerate() {
        log::debug!("Row {}:", i + 1);
        log::debug!("  Values: {:?}", row.values);
        log::debug!("  Source entities: {:?}", row.source_entities);
        log::debug!("  Has entities: {}", row.has_entities());

        if row.has_entities() {
            log::debug!("  ✅ Entity tracking is working!");
        } else {
            log::debug!("  ❌ No entities tracked - implementation may not be complete");
        }
    }

    assert!(
        result.rows[0].has_entities(),
        "Entities should be tracked for pattern-matched nodes"
    );
}
