//! Test to verify ROLLBACK works with batch undo operations from SET

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

#[test]
fn test_rollback_undoes_batch_set_operations() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture
        .setup_graph("rollback_batch_test")
        .expect("Failed to setup graph");

    // Create a person
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 28, city: 'LA', status: 'active'})")
        .expect("Failed to create person");

    // Start a transaction
    fixture
        .query("START TRANSACTION")
        .expect("Failed to start transaction");

    // Update multiple properties (creates batch undo operations)
    fixture.query(
        "MATCH (p:Person {name: 'Charlie'}) SET p.age = 29, p.city = 'SF', p.status = 'inactive'"
    ).expect("SET should succeed in transaction");

    // Verify changes were made
    let mid_result = fixture.query(
        "MATCH (p:Person {name: 'Charlie'}) RETURN p.age as age, p.city as city, p.status as status"
    ).expect("Query should succeed");

    let mid_row = &mid_result.rows[0].values;
    assert_eq!(
        mid_row.get("age"),
        Some(&Value::Number(29.0)),
        "Age should be 29"
    );
    assert_eq!(
        mid_row.get("city"),
        Some(&Value::String("SF".to_string())),
        "City should be SF"
    );
    assert_eq!(
        mid_row.get("status"),
        Some(&Value::String("inactive".to_string())),
        "Status should be inactive"
    );

    // Rollback the transaction
    fixture.query("ROLLBACK").expect("ROLLBACK should succeed");

    // Verify ALL changes were rolled back (batch undo worked!)
    let final_result = fixture.query(
        "MATCH (p:Person {name: 'Charlie'}) RETURN p.age as age, p.city as city, p.status as status"
    ).expect("Query should succeed");

    let final_row = &final_result.rows[0].values;
    assert_eq!(
        final_row.get("age"),
        Some(&Value::Number(28.0)),
        "Age should be rolled back to 28"
    );
    assert_eq!(
        final_row.get("city"),
        Some(&Value::String("LA".to_string())),
        "City should be rolled back to LA"
    );
    assert_eq!(
        final_row.get("status"),
        Some(&Value::String("active".to_string())),
        "Status should be rolled back to active"
    );
}
