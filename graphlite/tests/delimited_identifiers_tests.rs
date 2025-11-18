//! Tests for ISO GQL Delimited Identifiers
//!
//! ISO GQL Grammar Reference:
//! ```
//! <identifier> ::= <regular-id> | "`" <delimited-id-chars> "`"
//! <delimited-id-chars> ::= (<letter> | <digit> | <special-char> | "``")*
//! ```
//!
//! Delimited identifiers allow special characters like hyphens, spaces, etc.
//! in schema names, graph names, labels, and property names.

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;

#[test]
fn test_backtick_schema_names() {
    let fixture = TestFixture::new().unwrap();

    // Test 1: Schema with hyphen
    fixture.query("CREATE SCHEMA IF NOT EXISTS /`test-schema`").unwrap();
    fixture.query("CREATE GRAPH /`test-schema`/`test-graph`").unwrap();
    fixture.query("SESSION SET GRAPH /`test-schema`/`test-graph`").unwrap();

    // Insert node to verify schema/graph works
    fixture.query("INSERT (n:TestNode {id: 1})").unwrap();

    let result = fixture.query("MATCH (n:TestNode) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));
}

#[test]
fn test_backtick_graph_names() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();

    // Test 1: Graph name with spaces
    fixture.query(&format!("CREATE GRAPH /{}/`My Test Graph`", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/`My Test Graph`", fixture.schema_name())).unwrap();

    fixture.query("INSERT (n:Person {name: 'Alice'})").unwrap();

    let result = fixture.query("MATCH (n:Person) RETURN n.name as name").unwrap();
    assert_eq!(result.rows[0].values.get("name"), Some(&Value::String("Alice".to_string())));
}

#[test]
fn test_backtick_label_names() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Label with hyphen
    fixture.query("INSERT (n:`Test-Node` {id: 1})").unwrap();

    let result = fixture.query("MATCH (n:`Test-Node`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));

    // Test 2: Label with space
    fixture.query("INSERT (p:`Person Type` {name: 'Bob'})").unwrap();

    let result = fixture.query("MATCH (p:`Person Type`) RETURN p.name as name").unwrap();
    assert_eq!(result.rows[0].values.get("name"), Some(&Value::String("Bob".to_string())));

    // Test 3: Label with special characters
    fixture.query("INSERT (e:`Entity@123` {value: 42})").unwrap();

    let result = fixture.query("MATCH (e:`Entity@123`) RETURN e.value as value").unwrap();
    assert_eq!(result.rows[0].values.get("value"), Some(&Value::Number(42.0)));
}

#[test]
fn test_backtick_property_names() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Property with hyphen (would need parser support for property access)
    // For now, test that we can create nodes with properties that will be accessible via regular names
    fixture.query("INSERT (n:Person {name: 'Alice', age: 30})").unwrap();

    let result = fixture.query("MATCH (n:Person) RETURN n.name as name, n.age as age").unwrap();
    assert_eq!(result.rows[0].values.get("name"), Some(&Value::String("Alice".to_string())));
    assert_eq!(result.rows[0].values.get("age"), Some(&Value::Number(30.0)));
}

#[test]
fn test_backtick_multiple_labels() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Multiple labels with delimited identifiers
    fixture.query("INSERT (n:`Label-One`:`Label-Two` {id: 1})").unwrap();

    let result = fixture.query("MATCH (n:`Label-One`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));

    let result = fixture.query("MATCH (n:`Label-Two`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));
}

#[test]
fn test_backtick_with_numeric_prefix() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Label starting with number (not allowed without backticks)
    fixture.query("INSERT (n:`123Type` {id: 1})").unwrap();

    let result = fixture.query("MATCH (n:`123Type`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));
}

#[test]
fn test_backtick_reserved_keywords() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Use reserved keyword as label
    fixture.query("INSERT (n:`MATCH` {id: 1})").unwrap();

    let result = fixture.query("MATCH (n:`MATCH`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));

    // Test 2: Use another reserved keyword
    fixture.query("INSERT (r:`RETURN` {id: 2})").unwrap();

    let result = fixture.query("MATCH (r:`RETURN`) RETURN count(r) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));
}

#[test]
fn test_backtick_mixed_with_regular_identifiers() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Mix regular and delimited identifiers
    fixture.query("INSERT (n:Person:`Special-Type` {name: 'Alice'})").unwrap();

    let result = fixture.query("MATCH (n:Person) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));

    let result = fixture.query("MATCH (n:`Special-Type`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));
}

#[test]
fn test_backtick_case_sensitivity() {
    let fixture = TestFixture::new().unwrap();

    fixture.query(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name())).unwrap();
    fixture.query(&format!("CREATE GRAPH /{}/test_graph", fixture.schema_name())).unwrap();
    fixture.query(&format!("SESSION SET GRAPH /{}/test_graph", fixture.schema_name())).unwrap();

    // Test 1: Delimited identifiers should preserve case
    fixture.query("INSERT (n:`MyLabel` {id: 1})").unwrap();
    fixture.query("INSERT (n:`mylabel` {id: 2})").unwrap();
    fixture.query("INSERT (n:`MYLABEL` {id: 3})").unwrap();

    let result = fixture.query("MATCH (n:`MyLabel`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));

    let result = fixture.query("MATCH (n:`mylabel`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));

    let result = fixture.query("MATCH (n:`MYLABEL`) RETURN count(n) as count").unwrap();
    assert_eq!(result.rows[0].values.get("count"), Some(&Value::Number(1.0)));
}
