// Test for bug fix: CALL gql.list_graphs() returning NULL values
// Simplified version that avoids complex syntax

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_list_graphs_bug_fix_simple() {
    // Use existing fixture with fraud data which already has graphs
    let fixture = TestFixture::with_fraud_data()
        .expect("Should create fixture with fraud data");

    // List all graphs
    let result = fixture.query("CALL gql.list_graphs()").unwrap();

    // Should have at least one graph
    assert!(!result.rows.is_empty(),
        "Should have at least one graph");

    // Verify that first row has non-NULL values
    let graph_name = result.rows[0].values.get("graph_name")
        .expect("Should have graph_name column");

    // This was the bug - graph_name was NULL
    assert!(!matches!(graph_name, graphlite::Value::Null),
        "BUG FIXED: graph_name should not be NULL, got: {:?}", graph_name);

    let schema_name = result.rows[0].values.get("schema_name")
        .expect("Should have schema_name column");

    assert!(!matches!(schema_name, graphlite::Value::Null),
        "BUG FIXED: schema_name should not be NULL, got: {:?}", schema_name);

    // Print the values to show they're real
    if let graphlite::Value::String(name) = graph_name {
    }
    if let graphlite::Value::String(schema) = schema_name {
    }
}
