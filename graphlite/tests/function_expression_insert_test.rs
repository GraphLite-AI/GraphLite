//! Tests for function expressions in INSERT statements
//!
//! This test suite verifies that any registered function can be used in property values
//! for both node and edge INSERT statements, including nested function calls.
//!
//! Covers:
//! - String functions (upper, lower, substring)
//! - Mathematical functions (abs, floor, ceil, round)
//! - Temporal functions (duration, datetime, now)
//! - Nested function calls
//! - Both simple INSERT and MATCH INSERT statements

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;

// ============================================================================
// TEMPORAL FUNCTIONS
// ============================================================================

#[test]
fn test_duration_function_in_node_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("duration_node_test")
        .expect("Failed to setup graph");

    // Execute INSERT with duration() function
    let result = fixture.query("INSERT (:Example {age: duration('P30Y')})")
        .expect("INSERT with duration() should succeed");

    assert_eq!(result.rows_affected, 1, "Should have inserted 1 node");

    // Query the node back to verify the duration was stored
    let query_result = fixture.query("MATCH (e:Example) RETURN e.age as age")
        .expect("Query should succeed");

    assert_eq!(query_result.rows.len(), 1, "Should return 1 node");

    let age_value = &query_result.rows[0].values["age"];
    // Duration function should return a numeric value (seconds)
    assert!(age_value != &Value::Null, "Duration should not be null");
    assert!(matches!(age_value, Value::Number(_)), "Duration should be a number");

}

#[test]
fn test_duration_function_in_edge_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("duration_edge_test")
        .expect("Failed to setup graph");

    // Create two nodes first
    fixture.query("INSERT (:Person {name: 'Alice'})")
        .expect("Failed to create first node");
    fixture.query("INSERT (:Person {name: 'Bob'})")
        .expect("Failed to create second node");

    // Execute INSERT with edge containing duration() function
    let result = fixture.query(
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) \
         INSERT (a)-[:KNOWS {duration: duration('P5Y'), since: 2020}]->(b)"
    ).expect("INSERT edge with duration() should succeed");

    assert_eq!(result.rows_affected, 1, "Should have inserted 1 edge");

    // Query the edge back to verify the duration was stored
    let query_result = fixture.query(
        "MATCH (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'}) \
         RETURN r.duration as duration, r.since as since"
    ).expect("Query should succeed");

    assert_eq!(query_result.rows.len(), 1, "Should return 1 edge");

    let duration_value = &query_result.rows[0].values["duration"];
    assert!(duration_value != &Value::Null, "Duration should not be null");
    assert!(matches!(duration_value, Value::Number(_)), "Duration should be a number");

    let since_value = &query_result.rows[0].values["since"];
    assert_eq!(since_value, &Value::Number(2020.0), "Since should be 2020");

}

// ============================================================================
// MULTIPLE FUNCTION TYPES IN NODE INSERT
// ============================================================================

#[test]
fn test_multiple_function_types_in_node_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("multi_function_node_test")
        .expect("Failed to setup graph");

    // Test upper() function
    fixture.query("INSERT (:StringTest {value: upper('hello')})")
        .expect("INSERT with upper() should succeed");

    // Test lower() function
    fixture.query("INSERT (:StringTest {value: lower('WORLD')})")
        .expect("INSERT with lower() should succeed");

    // Test abs() function
    fixture.query("INSERT (:MathTest {value: abs(-42.5)})")
        .expect("INSERT with abs() should succeed");

    // Test floor() function
    fixture.query("INSERT (:MathTest {value: floor(42.7)})")
        .expect("INSERT with floor() should succeed");

    // Test ceil() function
    fixture.query("INSERT (:MathTest {value: ceil(4.3)})")
        .expect("INSERT with ceil() should succeed");

    // Test round() function
    fixture.query("INSERT (:MathTest {value: round(3.14159)})")
        .expect("INSERT with round() should succeed");

    // Verify upper() worked
    let upper_result = fixture.query("MATCH (s:StringTest {value: 'HELLO'}) RETURN s.value as value")
        .expect("Query should succeed");
    assert_eq!(upper_result.rows.len(), 1, "upper() should create 'HELLO'");

    // Verify lower() worked
    let lower_result = fixture.query("MATCH (s:StringTest {value: 'world'}) RETURN s.value as value")
        .expect("Query should succeed");
    assert_eq!(lower_result.rows.len(), 1, "lower() should create 'world'");

}

// ============================================================================
// MULTIPLE FUNCTION TYPES IN EDGE INSERT
// ============================================================================

#[test]
fn test_multiple_function_types_in_edge_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("multi_function_edge_test")
        .expect("Failed to setup graph");

    // Create two nodes
    fixture.query("INSERT (:User {id: 1, name: 'Alice'})")
        .expect("Failed to create first node");
    fixture.query("INSERT (:User {id: 2, name: 'Bob'})")
        .expect("Failed to create second node");

    // Insert edge with multiple function calls
    let result = fixture.query(
        "MATCH (a:User {id: 1}), (b:User {id: 2}) \
         INSERT (a)-[:SENT_MESSAGE { \
            subject: upper('hello'), \
            preview: substring('This is a preview of the message', 0, 10), \
            priority: abs(-5), \
            sent_at: duration('P0DT1H30M'), \
            word_count: round(123.7) \
         }]->(b)"
    ).expect("INSERT edge with multiple functions should succeed");

    assert_eq!(result.rows_affected, 1, "Should have inserted 1 edge");

    // Query back to verify
    let query_result = fixture.query(
        "MATCH (a:User {id: 1})-[r:SENT_MESSAGE]->(b:User {id: 2}) \
         RETURN r.subject as subject, r.preview as preview, r.priority as priority, \
         r.sent_at as sent_at, r.word_count as word_count"
    ).expect("Query should succeed");

    assert_eq!(query_result.rows.len(), 1, "Should return 1 edge");
    let row = &query_result.rows[0].values;

    // Verify each function worked
    assert_eq!(row.get("subject"), Some(&Value::String("HELLO".to_string())), "upper() should work in edge");
    assert_eq!(row.get("preview"), Some(&Value::String("This is a ".to_string())), "substring() should work in edge");
    assert_eq!(row.get("priority"), Some(&Value::Number(5.0)), "abs() should work in edge");
    assert!(row.get("sent_at") != Some(&Value::Null), "duration() should work in edge");
    assert_eq!(row.get("word_count"), Some(&Value::Number(124.0)), "round() should work in edge");

}

// ============================================================================
// NESTED FUNCTION CALLS
// ============================================================================

#[test]
fn test_nested_function_calls_in_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("nested_function_test")
        .expect("Failed to setup graph");

    // Test nested function calls
    let result = fixture.query(
        "INSERT (:Data { \
            value: round(abs(-42.7)), \
            text: upper(lower('MiXeD CaSe')) \
         })"
    ).expect("INSERT with nested functions should succeed");

    assert_eq!(result.rows_affected, 1, "Should have inserted 1 node");

    // Query back
    let query_result = fixture.query("MATCH (d:Data) RETURN d.value as value, d.text as text")
        .expect("Query should succeed");

    let row = &query_result.rows[0].values;

    // round(abs(-42.7)) should give 43.0
    assert_eq!(row.get("value"), Some(&Value::Number(43.0)), "Nested round(abs()) should work");

    // upper(lower('MiXeD CaSe')) should give 'MIXED CASE'
    assert_eq!(row.get("text"), Some(&Value::String("MIXED CASE".to_string())), "Nested upper(lower()) should work");

}

// ============================================================================
// COMPREHENSIVE FUNCTION COVERAGE
// ============================================================================

#[test]
fn test_comprehensive_function_coverage() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    fixture.setup_graph("comprehensive_test")
        .expect("Failed to setup graph");

    // Test various function categories in a single comprehensive test

    // String functions
    fixture.query("INSERT (:Test {str1: upper('test'), str2: lower('TEST')})")
        .expect("String functions should work");

    // Mathematical functions
    fixture.query("INSERT (:Test {math1: abs(-100), math2: floor(99.9), math3: ceil(0.1)})")
        .expect("Math functions should work");

    // Temporal functions
    fixture.query("INSERT (:Test {temp1: duration('P1Y')})")
        .expect("Temporal functions should work");

    // Query to verify
    let string_result = fixture.query("MATCH (t:Test) WHERE t.str1 IS NOT NULL RETURN t.str1 as upper, t.str2 as lower")
        .expect("Query should succeed");

    assert_eq!(string_result.rows.len(), 1);
    assert_eq!(string_result.rows[0].values.get("upper"), Some(&Value::String("TEST".to_string())));
    assert_eq!(string_result.rows[0].values.get("lower"), Some(&Value::String("test".to_string())));

    let math_result = fixture.query("MATCH (t:Test) WHERE t.math1 IS NOT NULL RETURN t.math1 as abs, t.math2 as floor, t.math3 as ceil")
        .expect("Query should succeed");

    assert_eq!(math_result.rows.len(), 1);
    assert_eq!(math_result.rows[0].values.get("abs"), Some(&Value::Number(100.0)));
    assert_eq!(math_result.rows[0].values.get("floor"), Some(&Value::Number(99.0)));
    assert_eq!(math_result.rows[0].values.get("ceil"), Some(&Value::Number(1.0)));

}
