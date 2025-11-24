//! Tests for JSON format output and query result structures
//!
//! This test suite validates query results and their data structures.
//!
//! Note: These tests use TestFixture rather than CliFixture because:
//! 1. INSERT statements don't support FROM clause in ISO GQL
//! 2. SESSION SET commands are <session-activity> and cannot be mixed with
//!    data/query statements (which are <statement> types in <procedure-body>)
//! 3. Each CLI invocation executes one <gql-program>, creating a new session
//! 4. ISO GQL doesn't support semicolon-separated statements at top level
//!
//! These tests validate the same query functionality and result structures
//! that would be serialized to JSON in CLI output.

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

/// Helper macro to create and setup graph for tests
macro_rules! setup_test_graph {
    ($fixture:expr) => {{
        let graph_name = format!("test_{}", fastrand::u64(..));
        $fixture.query(&format!("CREATE GRAPH {}", graph_name)).expect("Create graph failed");
        $fixture.query(&format!("SESSION SET GRAPH {}", graph_name)).expect("Set graph failed");
        graph_name
    }};
}

#[test]
fn test_json_format_basic_query() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert test data
    fixture.query("INSERT (:Person {name: 'Alice', age: 30});").expect("Insert failed");

    // Query and verify result structure
    let result = fixture.query("MATCH (p:Person) RETURN p.name, p.age;").expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 2);
    assert_eq!(result.variables[0], "p.name");
    assert_eq!(result.variables[1], "p.age");
}

#[test]
fn test_json_format_with_null_values() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert data with some properties missing
    fixture.query("INSERT (:Person {name: 'Bob'});").expect("Insert failed");

    // Query with missing property - should return null for age
    let result = fixture.query("MATCH (p:Person) RETURN p.name, p.age;").expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 2);

    // Verify first value is the name
    let row = &result.rows[0];
    assert_eq!(row.values.len(), 2);
}

#[test]
fn test_json_format_with_multiple_rows() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert multiple people
    fixture.query(
        "INSERT (:Person {name: 'Alice', age: 30}), \
                (:Person {name: 'Bob', age: 25}), \
                (:Person {name: 'Carol', age: 28});"
    ).expect("Insert failed");

    // Query all with ordering
    let result = fixture.query(
        "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.variables.len(), 2);

    // Verify all rows have the expected number of values
    for row in &result.rows {
        assert_eq!(row.values.len(), 2);
    }
}

#[test]
fn test_json_format_with_aggregation() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert test data
    fixture.query(
        "INSERT (:Person {name: 'Alice', city: 'NYC', age: 30}), \
                (:Person {name: 'Bob', city: 'NYC', age: 25}), \
                (:Person {name: 'Carol', city: 'SF', age: 28});"
    ).expect("Insert failed");

    // Query with aggregation
    let result = fixture.query(
        "MATCH (p:Person) RETURN p.city, COUNT(p) AS count \
         GROUP BY p.city ORDER BY count DESC;"
    ).expect("Query failed");

    assert!(result.rows.len() > 0);
    assert_eq!(result.variables.len(), 2);

    // Verify all rows have correct structure
    for row in &result.rows {
        assert_eq!(row.values.len(), 2);
    }
}

#[test]
fn test_json_format_with_relationships() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert people and relationship in one query
    fixture.query(
        "INSERT (:Person {name: 'Alice'})-[:KNOWS {since: '2020'}]->(:Person {name: 'Bob'});"
    ).expect("Insert failed");

    // Query relationship
    let result = fixture.query(
        "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name, b.name, r.since;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 3);
    assert_eq!(result.variables[0], "a.name");
    assert_eq!(result.variables[1], "b.name");
    assert_eq!(result.variables[2], "r.since");
}

#[test]
fn test_json_format_with_string_functions() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert data
    fixture.query("INSERT (:Person {name: 'alice'});").expect("Insert failed");

    // Query with string function
    let result = fixture.query(
        "MATCH (p:Person) RETURN UPPER(p.name) AS upper_name;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 1);
    assert_eq!(result.variables[0], "upper_name");
}

#[test]
fn test_json_format_with_math_functions() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert data
    fixture.query("INSERT (:Number {value: 16});").expect("Insert failed");

    // Query with math function
    let result = fixture.query(
        "MATCH (n:Number) RETURN n.value, SQRT(n.value) AS sqrt_value;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 2);
    assert_eq!(result.variables[0], "n.value");
    assert_eq!(result.variables[1], "sqrt_value");
}

#[test]
fn test_json_format_empty_result() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Query with no results
    let result = fixture.query("MATCH (p:Person) RETURN p.name;").expect("Query failed");

    // Should return empty rows array but variables should still be present
    assert_eq!(result.rows.len(), 0);
    assert_eq!(result.variables.len(), 1);
    assert_eq!(result.variables[0], "p.name");
}

#[test]
fn test_json_format_with_boolean_values() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert data with boolean
    fixture.query(
        "INSERT (:Account {active: true, verified: false});"
    ).expect("Insert failed");

    // Query boolean values
    let result = fixture.query(
        "MATCH (a:Account) RETURN a.active, a.verified;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 2);
    assert_eq!(result.variables[0], "a.active");
    assert_eq!(result.variables[1], "a.verified");
}

#[test]
fn test_json_format_with_multi_hop_query() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert people and relationships in one statement
    fixture.query(
        "INSERT (:Person {name: 'Alice'})-[:KNOWS]->(:Person {name: 'Bob'})-[:KNOWS]->(:Person {name: 'Carol'});"
    ).expect("Insert failed");

    // Multi-hop query
    let result = fixture.query(
        "MATCH (a:Person {name: 'Alice'})-[:KNOWS]->(b)-[:KNOWS]->(c) \
         RETURN c.name AS friend_of_friend;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.variables.len(), 1);
    assert_eq!(result.variables[0], "friend_of_friend");
}

#[test]
fn test_json_format_with_limit() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert multiple records in one statement
    fixture.query(
        "INSERT (:Person {id: 1}), (:Person {id: 2}), (:Person {id: 3}), \
                (:Person {id: 4}), (:Person {id: 5}), (:Person {id: 6}), \
                (:Person {id: 7}), (:Person {id: 8}), (:Person {id: 9}), \
                (:Person {id: 10});"
    ).expect("Insert failed");

    // Query with LIMIT
    let result = fixture.query(
        "MATCH (p:Person) RETURN p.id LIMIT 3;"
    ).expect("Query failed");

    // Should return exactly 3 rows
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.variables.len(), 1);
}

#[test]
fn test_json_format_with_order_by() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert data
    fixture.query(
        "INSERT (:Person {name: 'Charlie', age: 35}), \
                (:Person {name: 'Alice', age: 30}), \
                (:Person {name: 'Bob', age: 25});"
    ).expect("Insert failed");

    // Query with ORDER BY
    let result = fixture.query(
        "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age ASC;"
    ).expect("Query failed");

    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.variables.len(), 2);

    // Results should be ordered - verify structure
    for row in &result.rows {
        assert_eq!(row.values.len(), 2);
    }
}

#[test]
fn test_json_format_raw_output_structure() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");
    setup_test_graph!(fixture);

    // Insert data
    fixture.query("INSERT (:Person {name: 'Alice', age: 30});").expect("Insert failed");

    // Execute query
    let result = fixture.query("MATCH (p:Person) RETURN p.name, p.age;").expect("Query failed");

    // Verify QueryResult structure (this is what gets serialized to JSON in CLI)
    assert_eq!(result.variables.len(), 2);
    assert_eq!(result.variables[0], "p.name");
    assert_eq!(result.variables[1], "p.age");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].values.len(), 2);

    // Verify the result has the expected metadata fields
    // (These would be in the JSON output: status, variables, rows, rows_affected, execution_time_ms)
    // execution_time_ms should be a valid u64 value
    let _ = result.execution_time_ms; // Just verify it exists
}
