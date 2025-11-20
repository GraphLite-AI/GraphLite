//! Debug test for INTERSECT operation returning empty results

#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

#[test]
fn test_intersect_identical_rows() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Create a test graph with sample data
    fixture
        .query(&format!(
            "CREATE GRAPH /{}/intersect_debug",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "SESSION SET GRAPH /{}/intersect_debug",
            fixture.schema_name()
        ))
        .unwrap();

    // Insert test data
    fixture
        .query(r#"INSERT (p1:Person {name: "Alice Smith", age: 32, salary: 85000})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p2:Person {name: "Eve Davis", age: 41, salary: 110000})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p3:Person {name: "Charlie Brown", age: 35, salary: 95000})"#)
        .unwrap();
    fixture
        .query(r#"INSERT (p4:Person {name: "Grace Chen", age: 33, salary: 88000})"#)
        .unwrap();

    // Test the individual queries first
    let result1 = fixture
        .query("MATCH (p:Person) WHERE p.age > 30 RETURN p.name, p.age, p.salary")
        .unwrap();
    for (i, row) in result1.rows.iter().enumerate() {}

    let result2 = fixture
        .query("MATCH (p:Person) WHERE p.salary > 80000 RETURN p.name, p.age, p.salary")
        .unwrap();
    for (i, row) in result2.rows.iter().enumerate() {}

    // Test the INTERSECT query
    let intersect_query = "
        MATCH (p:Person) WHERE p.age > 30 RETURN p.name, p.age, p.salary
        INTERSECT
        MATCH (p:Person) WHERE p.salary > 80000 RETURN p.name, p.age, p.salary
    ";
    let intersect_result = match fixture.query(intersect_query) {
        Ok(result) => result,
        Err(e) => {
            panic!("INTERSECT query failed");
        }
    };

    for (i, row) in intersect_result.rows.iter().enumerate() {
        // Check for node identities
        for (key, value) in &row.values {
            if let Value::Node(node) = value {}
        }
    }

    // The INTERSECT should return 4 rows since both queries return the same 4 rows
    assert_eq!(
        intersect_result.rows.len(),
        4,
        "INTERSECT should return 4 rows when both sides return identical rows"
    );
}
