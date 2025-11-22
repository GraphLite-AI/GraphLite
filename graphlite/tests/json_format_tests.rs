//! Tests for JSON format output
//!
//! This test suite validates that query results are correctly formatted as JSON
//! when using the CLI with --format json option.
//!
//! Note: Each CLI query runs in a separate process, so we use FROM clause
//! instead of SESSION SET GRAPH for graph context.

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::cli_fixture::CliFixture;
use serde_json::Value as JsonValue;

/// Helper macro to create schema and graph for tests
macro_rules! setup_test_graph {
    ($fixture:expr) => {{
        let schema_name = $fixture.schema_name();
        $fixture.assert_query_succeeds(&format!("CREATE SCHEMA /{};", schema_name));
        $fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/test;", schema_name));
        schema_name
    }};
}

/// Helper function to run query with graph context set
fn query_with_context(fixture: &CliFixture, schema: &str, query: &str) -> testutils::cli_fixture::CliQueryResult {
    // Prepend SESSION SET commands to the query
    let full_query = format!(
        "SESSION SET SCHEMA /{}; SESSION SET GRAPH /{}/test; {}",
        schema, schema, query
    );
    fixture.assert_query_succeeds(&full_query)
}

#[test]
fn test_json_format_basic_query() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert test data
    query_with_context(&fixture, &schema, "INSERT (:Person {name: 'Alice', age: 30});");

    // Query and verify JSON structure
    let result = query_with_context(&fixture, &schema, "MATCH (p:Person) RETURN p.name, p.age;");

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    // Verify we can access values from JSON
    assert!(row.values.contains_key("p.name"));
    assert!(row.values.contains_key("p.age"));
}

#[test]
fn test_json_format_with_null_values() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert data with some properties missing
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Bob'}}) FROM /{}/test;", schema
    ));

    // Query with missing property
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN p.name, p.age FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    // name should exist
    assert!(row.values.contains_key("p.name"));
}

#[test]
fn test_json_format_with_multiple_rows() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert multiple people
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Alice', age: 30}}), \
                (:Person {{name: 'Bob', age: 25}}), \
                (:Person {{name: 'Carol', age: 28}}) FROM /{}/test;", schema
    ));

    // Query all
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 3);

    // Verify all rows have the expected structure
    for row in &result.rows {
        assert!(row.values.contains_key("p.name"));
        assert!(row.values.contains_key("p.age"));
    }
}

#[test]
fn test_json_format_with_aggregation() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert test data
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Alice', city: 'NYC', age: 30}}), \
                (:Person {{name: 'Bob', city: 'NYC', age: 25}}), \
                (:Person {{name: 'Carol', city: 'SF', age: 28}}) FROM /{}/test;", schema
    ));

    // Query with aggregation
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN p.city, COUNT(p) AS count \
         GROUP BY p.city ORDER BY count DESC FROM /{}/test;", schema
    ));

    assert!(result.rows.len() > 0);

    for row in &result.rows {
        assert!(row.values.contains_key("p.city"));
        assert!(row.values.contains_key("count"));
    }
}

#[test]
fn test_json_format_with_relationships() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert people and relationship in one query
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Alice'}})-[:KNOWS {{since: '2020'}}]->(:Person {{name: 'Bob'}}) \
         FROM /{}/test;", schema
    ));

    // Query relationship
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name, b.name, r.since FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    assert!(row.values.contains_key("a.name"));
    assert!(row.values.contains_key("b.name"));
    assert!(row.values.contains_key("r.since"));
}

#[test]
fn test_json_format_with_string_functions() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert data
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'alice'}}) FROM /{}/test;", schema
    ));

    // Query with string function
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN UPPER(p.name) AS upper_name FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    assert!(row.values.contains_key("upper_name"));
}

#[test]
fn test_json_format_with_math_functions() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert data
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Number {{value: 16}}) FROM /{}/test;", schema
    ));

    // Query with math function
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (n:Number) RETURN n.value, SQRT(n.value) AS sqrt_value FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    assert!(row.values.contains_key("n.value"));
    assert!(row.values.contains_key("sqrt_value"));
}

#[test]
fn test_json_format_empty_result() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Query with no results
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN p.name FROM /{}/test;", schema
    ));

    // Should return empty rows array
    assert_eq!(result.rows.len(), 0);
}

#[test]
fn test_json_format_with_boolean_values() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert data with boolean
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Account {{active: true, verified: false}}) FROM /{}/test;", schema
    ));

    // Query boolean values
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (a:Account) RETURN a.active, a.verified FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    assert!(row.values.contains_key("a.active"));
    assert!(row.values.contains_key("a.verified"));
}

#[test]
fn test_json_format_with_multi_hop_query() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert people and relationships in one statement
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Alice'}})-[:KNOWS]->(:Person {{name: 'Bob'}})-[:KNOWS]->(:Person {{name: 'Carol'}}) \
         FROM /{}/test;", schema
    ));

    // Multi-hop query
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (a:Person {{name: 'Alice'}})-[:KNOWS]->(b)-[:KNOWS]->(c) \
         RETURN c.name AS friend_of_friend FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];

    assert!(row.values.contains_key("friend_of_friend"));
}

#[test]
fn test_json_format_with_limit() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert multiple records in one statement
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{id: 1}}), (:Person {{id: 2}}), (:Person {{id: 3}}), \
                (:Person {{id: 4}}), (:Person {{id: 5}}), (:Person {{id: 6}}), \
                (:Person {{id: 7}}), (:Person {{id: 8}}), (:Person {{id: 9}}), \
                (:Person {{id: 10}}) FROM /{}/test;", schema
    ));

    // Query with LIMIT
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN p.id LIMIT 3 FROM /{}/test;", schema
    ));

    // Should return exactly 3 rows
    assert_eq!(result.rows.len(), 3);
}

#[test]
fn test_json_format_with_order_by() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert data
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Charlie', age: 35}}), \
                (:Person {{name: 'Alice', age: 30}}), \
                (:Person {{name: 'Bob', age: 25}}) FROM /{}/test;", schema
    ));

    // Query with ORDER BY
    let result = fixture.assert_query_succeeds(&format!(
        "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age ASC FROM /{}/test;", schema
    ));

    assert_eq!(result.rows.len(), 3);

    // Results should be ordered by age
    for row in &result.rows {
        assert!(row.values.contains_key("p.name"));
        assert!(row.values.contains_key("p.age"));
    }
}

#[test]
fn test_json_format_raw_output_structure() {
    use std::process::Command;

    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema = setup_test_graph!(fixture);

    // Insert data
    fixture.assert_query_succeeds(&format!(
        "INSERT (:Person {{name: 'Alice', age: 30}}) FROM /{}/test;", schema
    ));

    // Execute query and get raw output
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--package", "graphlite-cli", "--bin", "graphlite", "--", "query"])
        .arg("--path").arg(fixture.db_path())
        .arg("--user").arg("admin")
        .arg("--password").arg("admin123")
        .arg("--format").arg("json")
        .arg(&format!("MATCH (p:Person) RETURN p.name, p.age FROM /{}/test;", schema))
        .env("RUST_LOG", "error")
        .output()
        .expect("Failed to execute query");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find JSON start
    let json_start = stdout.find('{').expect("Should have JSON output");
    let json_str = &stdout[json_start..];

    // Parse JSON
    let parsed: JsonValue = serde_json::from_str(json_str)
        .expect("Should be valid JSON");

    // Verify structure
    assert_eq!(parsed["status"], "success");
    assert!(parsed["columns"].is_array());
    assert!(parsed["rows"].is_array());
    assert!(parsed["rows_affected"].is_number());
    assert!(parsed["execution_time_ms"].is_number());

    // Verify columns
    let columns = parsed["columns"].as_array().unwrap();
    assert!(columns.len() >= 2);

    // Verify rows
    let rows = parsed["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);

    let first_row = &rows[0];
    assert!(first_row["p.name"].is_string());
    assert!(first_row["p.age"].is_number());
}
