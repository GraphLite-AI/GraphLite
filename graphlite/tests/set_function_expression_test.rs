#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;

// ============================================================================
// Node Property SET Tests
// ============================================================================

#[test]
fn test_set_with_string_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("set_function_test")
        .expect("Failed to setup graph");

    // Insert a person
    fixture.query("INSERT (:Person {name: 'Alice', age: 30})")
        .expect("INSERT should succeed");

    // SET with string function expressions
    fixture.query("MATCH (p:Person {name: 'Alice'}) SET p.name_upper = upper('alice'), p.name_lower = lower('ALICE')")
        .expect("SET should succeed");

    // Query back to verify
    let check_result = fixture.query("MATCH (p:Person {name: 'Alice'}) RETURN p.name_upper as upper, p.name_lower as lower")
        .expect("Query should succeed");

    let name_upper = &check_result.rows[0].values["upper"];
    let name_lower = &check_result.rows[0].values["lower"];

    assert_eq!(name_upper, &Value::String("ALICE".to_string()), "upper() should work");
    assert_eq!(name_lower, &Value::String("alice".to_string()), "lower() should work");

}

#[test]
fn test_set_with_math_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("set_math_test")
        .expect("Failed to setup graph");

    // Insert a product
    fixture.query("INSERT (:Product {name: 'Widget'})")
        .expect("INSERT should succeed");

    // SET with math function expressions
    fixture.query("MATCH (p:Product) SET p.price = abs(-99.99), p.quantity = floor(42.7), p.rating = ceil(4.3)")
        .expect("SET should succeed");

    // Query back
    let check = fixture.query("MATCH (p:Product) RETURN p.price as price, p.quantity as qty, p.rating as rating")
        .expect("Query should succeed");

    assert_eq!(check.rows[0].values["price"], Value::Number(99.99));
    assert_eq!(check.rows[0].values["qty"], Value::Number(42.0));
    assert_eq!(check.rows[0].values["rating"], Value::Number(5.0));

}

#[test]
fn test_set_with_duration() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("set_duration_test")
        .expect("Failed to setup graph");

    // Insert an event
    fixture.query("INSERT (:Event {name: 'Conference'})")
        .expect("INSERT should succeed");

    // SET with duration function
    fixture.query("MATCH (e:Event) SET e.length = duration('P2D')")
        .expect("SET should succeed");

    // Query back
    let check = fixture.query("MATCH (e:Event) RETURN e.length as length")
        .expect("Query should succeed");

    let length = &check.rows[0].values["length"];
    assert!(length != &Value::Null, "duration() should return a value");

}

// ============================================================================
// Edge Property MATCH SET Tests
// ============================================================================

#[test]
fn test_match_set_edge_properties_with_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("set_edge_test")
        .expect("Failed to setup graph");

    // Create two nodes and a relationship
    fixture.query("INSERT (:Person {name: 'Alice'})")
        .expect("INSERT should succeed");

    fixture.query("INSERT (:Person {name: 'Bob'})")
        .expect("INSERT should succeed");

    fixture.query("MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) INSERT (a)-[:KNOWS {since: 2020}]->(b)")
        .expect("INSERT edge should succeed");

    // Try to SET edge properties with function expressions using MATCH SET
    let result = fixture.query(
        "MATCH (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'}) \
         SET r.duration = duration('P5Y'), r.strength = abs(-0.9), r.label_upper = upper('knows')"
    );


    if let Err(e) = &result {
        panic!("MATCH SET edge properties should succeed, got error: {}", e);
    }

    // Query back to verify
    let check = fixture.query(
        "MATCH (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'}) \
         RETURN r.duration as duration, r.strength as strength, r.label_upper as label_upper"
    ).expect("Query should succeed");


    let duration = &check.rows[0].values["duration"];
    let strength = &check.rows[0].values["strength"];
    let label_upper = &check.rows[0].values["label_upper"];


    assert!(duration != &Value::Null, "duration should not be null");
    assert_eq!(strength, &Value::Number(0.9), "abs() should work");
    assert_eq!(label_upper, &Value::String("KNOWS".to_string()), "upper() should work");

}

#[test]
fn test_match_set_edge_with_temporal_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("temporal_edge_test")
        .expect("Failed to setup graph");

    // Create nodes and edge
    fixture.query("INSERT (:User {id: 1}), (:User {id: 2})")
        .expect("INSERT nodes should succeed");

    fixture.query("MATCH (a:User {id: 1}), (b:User {id: 2}) INSERT (a)-[:MESSAGED]->(b)")
        .expect("INSERT edge should succeed");

    // SET edge property with duration function
    fixture.query(
        "MATCH (a:User {id: 1})-[r:MESSAGED]->(b:User {id: 2}) \
         SET r.response_time = duration('PT30M')"
    ).expect("SET should succeed");

    // Verify
    let check = fixture.query(
        "MATCH (a:User {id: 1})-[r:MESSAGED]->(b:User {id: 2}) \
         RETURN r.response_time as response_time"
    ).expect("Query should succeed");

    let response_time = &check.rows[0].values["response_time"];

    assert!(response_time != &Value::Null, "response_time should not be null");
    // PT30M = 30 minutes = 1800 seconds
    assert_eq!(response_time, &Value::Number(1800.0), "duration('PT30M') should be 1800 seconds");

}

#[test]
fn test_match_set_edge_with_nested_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("nested_edge_test")
        .expect("Failed to setup graph");

    // Create nodes and edge
    fixture.query("INSERT (:Item {id: 1}), (:Item {id: 2})")
        .expect("INSERT nodes should succeed");

    fixture.query("MATCH (a:Item {id: 1}), (b:Item {id: 2}) INSERT (a)-[:RELATED]->(b)")
        .expect("INSERT edge should succeed");

    // SET edge property with nested function calls
    fixture.query(
        "MATCH (a:Item {id: 1})-[r:RELATED]->(b:Item {id: 2}) \
         SET r.score = round(abs(-0.857)), r.label = upper(lower('ReLaTeD'))"
    ).expect("SET with nested functions should succeed");

    // Verify
    let check = fixture.query(
        "MATCH (a:Item {id: 1})-[r:RELATED]->(b:Item {id: 2}) \
         RETURN r.score as score, r.label as label"
    ).expect("Query should succeed");

    let score = &check.rows[0].values["score"];
    let label = &check.rows[0].values["label"];


    assert_eq!(score, &Value::Number(1.0), "round(abs(-0.857)) should be 1.0");
    assert_eq!(label, &Value::String("RELATED".to_string()), "upper(lower('ReLaTeD')) should be 'RELATED'");

}
