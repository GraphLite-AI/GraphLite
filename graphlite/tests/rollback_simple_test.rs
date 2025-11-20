//! Simple test to debug ROLLBACK behavior

#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

#[test]
fn test_simple_rollback_debug() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .setup_graph("rollback_debug_test")
        .expect("Failed to setup graph");

    // Create a person
    let insert_result = fixture
        .query("INSERT (:Person {name: 'Dave', age: 40})")
        .expect("Failed to create person");

    // Verify person exists before transaction
    let before_result = fixture
        .query("MATCH (p:Person {name: 'Dave'}) RETURN p.age as age")
        .expect("Query before transaction should succeed");
    assert_eq!(before_result.rows.len(), 1);
    assert_eq!(
        before_result.rows[0].values.get("age"),
        Some(&Value::Number(40.0))
    );

    // Start a transaction
    let txn_result = fixture.query("START TRANSACTION");
    assert!(txn_result.is_ok(), "START TRANSACTION should succeed");

    // Update age in transaction
    let update_result = fixture.query("MATCH (p:Person {name: 'Dave'}) SET p.age = 41");
    assert!(update_result.is_ok(), "SET should succeed in transaction");

    // Verify change was made
    let mid_result = fixture
        .query("MATCH (p:Person {name: 'Dave'}) RETURN p.age as age")
        .expect("Query during transaction should succeed");
    assert_eq!(mid_result.rows.len(), 1);
    assert_eq!(
        mid_result.rows[0].values.get("age"),
        Some(&Value::Number(41.0))
    );

    // Rollback
    let rollback_result = fixture.query("ROLLBACK");
    assert!(rollback_result.is_ok(), "ROLLBACK should succeed");

    // Verify change was rolled back
    let after_result =
        fixture.query("MATCH (p:Person {name: 'Dave'}) RETURN p.age as age, p.name as name");

    match after_result {
        Ok(result) => {
            if result.rows.len() > 0 {
                assert_eq!(
                    result.rows[0].values.get("age"),
                    Some(&Value::Number(40.0)),
                    "Age should be rolled back to 40"
                );
            } else {
                panic!("No rows returned after ROLLBACK - node was deleted!");
            }
        }
        Err(e) => {
            panic!("Query after ROLLBACK failed: {:?}", e);
        }
    }
}
