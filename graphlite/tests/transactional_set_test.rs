//! Test to verify SET transactional behavior - all properties or rollback

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;

#[test]
fn test_set_transactional_all_or_nothing() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("transactional_set_test")
        .expect("Failed to setup graph");

    // Create a person
    fixture.query("INSERT (:Person {name: 'Alice', age: 30, city: 'NYC'})")
        .expect("Failed to create person");

    // Try to SET multiple properties where one will fail
    // datetime('1992-05-15') will fail because it needs time component
    let result = fixture.query(
        "MATCH (p:Person {name: 'Alice'}) SET p.age = 31, p.birthday = datetime('1992-05-15')"
    );

    // This should fail with an error (not a warning!)
    assert!(result.is_err(), "SET should fail when one property evaluation fails");

    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("Failed to evaluate") || err_msg.contains("datetime"), 
            "Error should mention datetime parsing failure: {}", err_msg);

    // Verify that NO properties were changed (transactional guarantee)
    let query_result = fixture.query("MATCH (p:Person {name: 'Alice'}) RETURN p.age as age")
        .expect("Query should succeed");

    assert_eq!(query_result.rows.len(), 1, "Should return 1 node");
    
    let age_value = &query_result.rows[0].values["age"];
    assert_eq!(age_value, &Value::Number(30.0), "Age should still be 30 (not changed to 31)");

}

#[test]
fn test_set_transactional_success_all_valid() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("transactional_set_success_test")
        .expect("Failed to setup graph");

    // Create a person
    fixture.query("INSERT (:Person {name: 'Bob', age: 25})")
        .expect("Failed to create person");

    // SET multiple properties - all valid (with correct datetime format)
    let result = fixture.query(
        "MATCH (p:Person {name: 'Bob'}) SET p.age = 26, p.city = upper('seattle'), p.birthday = datetime('1992-05-15T00:00:00Z')"
    );

    assert!(result.is_ok(), "SET should succeed when all properties are valid");

    // Verify ALL properties were changed
    let query_result = fixture.query(
        "MATCH (p:Person {name: 'Bob'}) RETURN p.age as age, p.city as city, p.birthday as birthday"
    ).expect("Query should succeed");

    assert_eq!(query_result.rows.len(), 1, "Should return 1 node");
    
    let row = &query_result.rows[0].values;
    assert_eq!(row.get("age"), Some(&Value::Number(26.0)), "Age should be updated to 26");
    assert_eq!(row.get("city"), Some(&Value::String("SEATTLE".to_string())), "City should be SEATTLE");
    assert!(row.get("birthday") != Some(&Value::Null), "Birthday should be set");

}
