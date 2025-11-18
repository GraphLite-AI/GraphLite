//! Test to verify MATCH SET transactional behavior with multiple properties

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;

#[test]
fn test_match_set_multiple_properties_rollback() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("match_set_rollback_test")
        .expect("Failed to setup graph");

    // Create multiple people
    fixture.query("INSERT (:Person {name: 'Alice', age: 30, city: 'NYC', status: 'active'})")
        .expect("Failed to create Alice");
    fixture.query("INSERT (:Person {name: 'Bob', age: 25, city: 'LA', status: 'active'})")
        .expect("Failed to create Bob");

    // Start transaction
    fixture.query("START TRANSACTION")
        .expect("Failed to start transaction");

    // Update multiple properties on multiple nodes with MATCH SET
    // Note: Using literal values since property references in SET values not yet supported
    fixture.query(
        "MATCH (p:Person) SET p.age = 99, p.city = 'UPDATED_CITY', p.status = 'updated'"
    ).expect("MATCH SET should succeed");

    // Verify changes were made
    let mid_result = fixture.query(
        "MATCH (p:Person) WHERE p.name = 'Alice' RETURN p.age as age, p.city as city, p.status as status"
    ).expect("Query should succeed");
    
    assert_eq!(mid_result.rows.len(), 1);
    let alice_row = &mid_result.rows[0].values;
    assert_eq!(alice_row.get("age"), Some(&Value::Number(99.0)), "Alice age should be 99");
    assert_eq!(alice_row.get("city"), Some(&Value::String("UPDATED_CITY".to_string())), "Alice city should be UPDATED_CITY");
    assert_eq!(alice_row.get("status"), Some(&Value::String("updated".to_string())), "Alice status should be updated");

    let mid_result2 = fixture.query(
        "MATCH (p:Person) WHERE p.name = 'Bob' RETURN p.age as age, p.city as city, p.status as status"
    ).expect("Query should succeed");

    assert_eq!(mid_result2.rows.len(), 1);
    let bob_row = &mid_result2.rows[0].values;
    assert_eq!(bob_row.get("age"), Some(&Value::Number(99.0)), "Bob age should be 99");
    assert_eq!(bob_row.get("city"), Some(&Value::String("UPDATED_CITY".to_string())), "Bob city should be UPDATED_CITY");
    assert_eq!(bob_row.get("status"), Some(&Value::String("updated".to_string())), "Bob status should be updated");

    // Rollback
    fixture.query("ROLLBACK")
        .expect("ROLLBACK should succeed");

    // Verify ALL changes on BOTH nodes were rolled back
    let after_alice = fixture.query(
        "MATCH (p:Person) WHERE p.name = 'Alice' RETURN p.age as age, p.city as city, p.status as status"
    ).expect("Query should succeed");
    
    assert_eq!(after_alice.rows.len(), 1);
    let alice_final = &after_alice.rows[0].values;
    assert_eq!(alice_final.get("age"), Some(&Value::Number(30.0)), "Alice age rolled back to 30");
    assert_eq!(alice_final.get("city"), Some(&Value::String("NYC".to_string())), "Alice city rolled back to NYC");
    assert_eq!(alice_final.get("status"), Some(&Value::String("active".to_string())), "Alice status rolled back to active");

    let after_bob = fixture.query(
        "MATCH (p:Person) WHERE p.name = 'Bob' RETURN p.age as age, p.city as city, p.status as status"
    ).expect("Query should succeed");
    
    assert_eq!(after_bob.rows.len(), 1);
    let bob_final = &after_bob.rows[0].values;
    assert_eq!(bob_final.get("age"), Some(&Value::Number(25.0)), "Bob age rolled back to 25");
    assert_eq!(bob_final.get("city"), Some(&Value::String("LA".to_string())), "Bob city rolled back to LA");
    assert_eq!(bob_final.get("status"), Some(&Value::String("active".to_string())), "Bob status rolled back to active");

}

#[test]
fn test_match_set_transactional_failure() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("match_set_transactional_fail_test")
        .expect("Failed to setup graph");

    // Create a person
    fixture.query("INSERT (:Person {name: 'Charlie', age: 28, city: 'SF'})")
        .expect("Failed to create Charlie");

    // Try MATCH SET with multiple properties where one will fail
    let result = fixture.query(
        "MATCH (p:Person {name: 'Charlie'}) SET p.age = 29, p.city = 'Seattle', p.birthday = datetime('1995-03-20')"
    );

    // Should fail because datetime needs time component
    assert!(result.is_err(), "MATCH SET should fail when one property evaluation fails");

    // Verify NO properties were changed (transactional guarantee)
    let after_result = fixture.query(
        "MATCH (p:Person {name: 'Charlie'}) RETURN p.age as age, p.city as city"
    ).expect("Query should succeed");
    
    assert_eq!(after_result.rows.len(), 1);
    let charlie = &after_result.rows[0].values;
    assert_eq!(charlie.get("age"), Some(&Value::Number(28.0)), "Age should still be 28");
    assert_eq!(charlie.get("city"), Some(&Value::String("SF".to_string())), "City should still be SF");

}

#[test]
fn test_match_set_single_property_rollback() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("match_set_single_prop_test")
        .expect("Failed to setup graph");

    // Create a person with multiple properties
    fixture.query("INSERT (:Person {name: 'Eve', age: 35, city: 'Boston', occupation: 'Engineer'})")
        .expect("Failed to create Eve");

    // Start transaction
    fixture.query("START TRANSACTION")
        .expect("Failed to start transaction");

    // Update just ONE property
    fixture.query("MATCH (p:Person {name: 'Eve'}) SET p.age = 36")
        .expect("SET should succeed");

    // Verify the change
    let mid_result = fixture.query(
        "MATCH (p:Person {name: 'Eve'}) RETURN p.age as age, p.city as city, p.occupation as occupation"
    ).expect("Query should succeed");
    
    assert_eq!(mid_result.rows.len(), 1);
    let mid_eve = &mid_result.rows[0].values;
    assert_eq!(mid_eve.get("age"), Some(&Value::Number(36.0)), "Age should be 36");
    assert_eq!(mid_eve.get("city"), Some(&Value::String("Boston".to_string())), "City should still be Boston");
    assert_eq!(mid_eve.get("occupation"), Some(&Value::String("Engineer".to_string())), "Occupation should still be Engineer");

    // Rollback
    fixture.query("ROLLBACK")
        .expect("ROLLBACK should succeed");

    // Verify age rolled back but OTHER properties remain
    let after_result = fixture.query(
        "MATCH (p:Person {name: 'Eve'}) RETURN p.age as age, p.city as city, p.occupation as occupation"
    ).expect("Query should succeed");
    
    assert_eq!(after_result.rows.len(), 1);
    let final_eve = &after_result.rows[0].values;
    assert_eq!(final_eve.get("age"), Some(&Value::Number(35.0)), "Age rolled back to 35");
    assert_eq!(final_eve.get("city"), Some(&Value::String("Boston".to_string())), "City should still be Boston");
    assert_eq!(final_eve.get("occupation"), Some(&Value::String("Engineer".to_string())), "Occupation should still be Engineer");

}
