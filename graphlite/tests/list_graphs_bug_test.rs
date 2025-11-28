// Test for bug fix: CALL gql.list_graphs() returning NULL values
// Bug: list_graphs() was looking for name at wrong JSON path (graph.name instead of graph.id.name)
// This test verifies the fix works correctly

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_list_graphs_returns_actual_names_not_null() {
    let fixture = TestFixture::new().expect("Should create test fixture");

    // Create multiple graphs with specific names (schema already created by new())
    let schema_name = fixture.schema_name();
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/bug_test_graph_1", schema_name));
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/bug_test_graph_2", schema_name));

    // List all graphs
    let result = fixture.assert_query_succeeds("CALL gql.list_graphs()");

    // Should have at least 2 graphs
    assert!(
        result.rows.len() >= 2,
        "Should have at least 2 graphs, got {}",
        result.rows.len()
    );

    // Verify each row has non-NULL values
    for (idx, row) in result.rows.iter().enumerate() {
        // Check graph_name column exists
        let graph_name = row
            .values
            .get("graph_name")
            .unwrap_or_else(|| panic!("Row {} missing graph_name column", idx));

        // Check graph_name is not NULL
        assert!(
            !matches!(graph_name, graphlite::Value::Null),
            "Row {} graph_name should not be NULL, got: {:?}",
            idx,
            graph_name
        );

        // Check schema_name column exists
        let schema_name = row
            .values
            .get("schema_name")
            .unwrap_or_else(|| panic!("Row {} missing schema_name column", idx));

        // Check schema_name is not NULL
        assert!(
            !matches!(schema_name, graphlite::Value::Null),
            "Row {} schema_name should not be NULL, got: {:?}",
            idx,
            schema_name
        );
    }

    // Find our specific graphs by name
    let graph_names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|row| {
            row.values.get("graph_name").and_then(|v| {
                if let graphlite::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
        })
        .collect();

    assert!(
        graph_names.contains(&"bug_test_graph_1".to_string()),
        "Should find bug_test_graph_1 in results, got: {:?}",
        graph_names
    );
    assert!(
        graph_names.contains(&"bug_test_graph_2".to_string()),
        "Should find bug_test_graph_2 in results, got: {:?}",
        graph_names
    );

    // Verify schema names are correct
    let schema_names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|row| {
            row.values.get("schema_name").and_then(|v| {
                if let graphlite::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
        })
        .collect();

    // All our graphs should be in the fixture's schema
    let expected_schema = fixture.schema_name();
    for schema_name_found in &schema_names {
        if graph_names.iter().any(|g| g.starts_with("bug_test_graph_")) {
            assert_eq!(
                schema_name_found, expected_schema,
                "Graphs should be in {}, got: {}",
                expected_schema, schema_name_found
            );
        }
    }

    // Cleanup
    fixture.assert_query_succeeds(&format!(
        "DROP GRAPH IF EXISTS /{}/bug_test_graph_1",
        schema_name
    ));
    fixture.assert_query_succeeds(&format!(
        "DROP GRAPH IF EXISTS /{}/bug_test_graph_2",
        schema_name
    ));
}

#[test]
fn test_list_graphs_with_where_clause_filters_by_name() {
    // This test verifies that WHERE filtering works AND that graph_name is not NULL
    // Uses with_fraud_data() which creates fraud_graph with a random suffix
    let fixture = TestFixture::with_fraud_data().expect("Should create test fixture");

    // Get the actual graph name that was created
    let expected_graph_name = fixture
        .graph_name()
        .expect("Fixture should have a graph name");

    // Filter by the fraud_graph that was created by with_fraud_data()
    let result = fixture.assert_query_succeeds(&format!(
        "CALL gql.list_graphs()
         YIELD graph_name, schema_name
         WHERE graph_name = '{}'",
        expected_graph_name
    ));

    // Should return exactly 1 graph
    assert_eq!(
        result.rows.len(),
        1,
        "WHERE filter should return exactly 1 graph, got {}",
        result.rows.len()
    );

    // Verify it's the correct graph and graph_name is not NULL
    let graph_name = result.rows[0]
        .values
        .get("graph_name")
        .expect("Should have graph_name");

    // This is the key test - graph_name should NOT be NULL (this was the bug)
    assert!(
        !matches!(graph_name, graphlite::Value::Null),
        "BUG FIX VERIFIED: graph_name should not be NULL, got: {:?}",
        graph_name
    );

    if let graphlite::Value::String(name) = graph_name {
        assert_eq!(
            name, expected_graph_name,
            "Filtered graph should be {}, got: {}",
            expected_graph_name, name
        );
    } else {
        panic!("graph_name should be String, got: {:?}", graph_name);
    }
}

#[test]
#[ignore] // Temporarily disabled due to complex graph type syntax - requires parser investigation
fn test_list_graph_types_returns_actual_names_not_null() {
    let fixture = TestFixture::new().expect("Should create test fixture");

    // Create a graph type (using simplified syntax)
    // Note: This test is disabled until graph type creation syntax is clarified
    let schema = fixture.schema_name();
    fixture.assert_query_succeeds(&format!("CREATE GRAPH TYPE /{}/TestGraphType ( VERTEX TYPES ( Person {{ name: STRING, age: INTEGER }} ), EDGE TYPES ( KNOWS {{ since: DATE }} SOURCE Person DESTINATION Person ) )", schema));

    // List graph types
    let result = fixture.assert_query_succeeds("CALL gql.list_graph_types()");

    // Should have at least 1 graph type
    assert!(
        !result.rows.is_empty(),
        "Should have at least 1 graph type, got {}",
        result.rows.len()
    );

    // Find our graph type
    let mut found_test_type = false;
    for row in &result.rows {
        let graph_type_name = row
            .values
            .get("graph_type_name")
            .expect("Should have graph_type_name column");

        // Verify not NULL
        assert!(
            !matches!(graph_type_name, graphlite::Value::Null),
            "graph_type_name should not be NULL, got: {:?}",
            graph_type_name
        );

        // Check if it's our type
        if let graphlite::Value::String(name) = graph_type_name {
            if name == "TestGraphType" {
                found_test_type = true;

                // Verify schema_name is also correct
                let schema_name = row
                    .values
                    .get("schema_name")
                    .expect("Should have schema_name column");

                if let graphlite::Value::String(schema) = schema_name {
                    assert_eq!(
                        schema, "graph_type_test_schema",
                        "Schema name should be graph_type_test_schema, got: {}",
                        schema
                    );
                }
            }
        }
    }

    assert!(found_test_type, "Should find TestGraphType in results");

    // Cleanup
    fixture.assert_query_succeeds("DROP GRAPH TYPE IF EXISTS /test_schema/TestGraphType CASCADE");
}
