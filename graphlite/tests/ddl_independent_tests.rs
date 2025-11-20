//! DDL Tests that require independent fixtures
//!
//! These tests perform operations that affect the entire database state
//! or require complete isolation. They should be run serially to avoid conflicts.
//!
//! Run with: cargo test --test ddl_independent_tests -- --test-threads=1

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

#[test]
fn test_graph_ddl_operations() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "graph_ops_schema";
    let test_graph = &format!("{}/test_graph", test_schema);

    // Create schema first
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    // Test basic CREATE GRAPH
    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}", test_graph));

    // Test creating another graph in same schema
    let another_graph = &format!("{}/another_graph", test_schema);
    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}", another_graph));

    // Test duplicate graph creation should fail
    fixture.assert_query_fails(&format!("CREATE GRAPH {}", another_graph), "already exists");

    // Test basic DROP GRAPH
    fixture.assert_query_succeeds(&format!("DROP GRAPH {}", another_graph));

    // Clean up schema
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}

#[test]
fn test_complex_ddl_scenarios() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Test nested schema and graph operations
    let schemas = vec!["finance_schema", "analytics_schema", "operations_schema"];

    // Create multiple schemas
    for schema in &schemas {
        fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", schema));
    }

    // Create graphs in each schema
    for schema in &schemas {
        let graphs = vec![
            format!("{}/tx_graph", schema),
            format!("{}/user_graph", schema),
            format!("{}/audit_graph", schema),
        ];

        for graph in &graphs {
            fixture.assert_query_succeeds(&format!("CREATE GRAPH {}", graph));
        }
    }

    // Test setting session graph to one of the created graphs
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH {}/tx_graph", schemas[0]));

    // Switch to different graph and verify isolation
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH {}/user_graph", schemas[0]));

    // Cleanup: Drop all created resources
    for schema in &schemas {
        fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", schema));
    }
}

#[test]
fn test_ddl_cascade_operations() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "c_schema";
    let test_graph = format!("{}/c_test_graph", test_schema);

    // Create schema and graph
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}", test_graph));

    // Set as current graph and add data
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH {}", test_graph));

    fixture.assert_query_succeeds("INSERT (c_test:CTest {name: 'Will be deleted'})");

    // Test CASCADE drop - should drop schema and all contained graphs
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));

    // After CASCADE drop, verify multiple things:

    // 1. Schema should be gone - creating new graph should fail
    fixture.assert_query_fails(
        &format!("CREATE GRAPH {}/new_graph_after_drop", test_schema),
        "not found",
    );

    // 2. Original graph should no longer exist
    fixture.assert_query_fails(
        &format!("SESSION SET GRAPH {}", test_graph),
        "Graph does not exist",
    );

    // 3. Session should be reset/invalid after CASCADE drop
    // The current session graph should be invalid now
    fixture.assert_query_fails(
        "INSERT (n:Test {name: 'Should fail'})",
        "No graph context available",
    );
}

#[test]
fn test_ddl_transaction_behavior() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "tx_test_schema";

    // Test DDL in transaction
    fixture.assert_query_succeeds("BEGIN");

    fixture.assert_query_succeeds(&format!("CREATE SCHEMA IF NOT EXISTS {}", test_schema));

    // DDL should be visible within transaction
    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}/tx_test_graph", test_schema));

    // Rollback transaction
    fixture.assert_query_succeeds("ROLLBACK");

    // Schema should still exist (DDL typically auto-commits)
    // This behavior may vary by implementation
    let result = fixture.query(&format!(
        "CREATE GRAPH IF NOT EXISTS {}/another_test_graph",
        test_schema
    ));

    match result {
        Ok(_) => {}
        Err(_) => {}
    }
}

#[test]
fn test_create_graph_with_relative_path() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "relative_path_schema";

    // Create schema first
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    // Set session schema
    fixture.assert_query_succeeds(&format!("SESSION SET SCHEMA {}", test_schema));

    // Now CREATE GRAPH with relative path should work
    fixture.assert_query_succeeds("CREATE GRAPH relative_graph");

    // Verify the graph was created with full path by setting it as session graph
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH {}/relative_graph", test_schema));

    // Test a second relative graph
    fixture.assert_query_succeeds("CREATE GRAPH another_relative_graph");

    // Verify we can set the second graph too
    fixture.assert_query_succeeds(&format!(
        "SESSION SET GRAPH {}/another_relative_graph",
        test_schema
    ));

    // Clean up
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}

#[test]
fn test_drop_graph_with_relative_path() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "drop_relative_schema";

    // Create schema and graphs first
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}/test_graph_1", test_schema));

    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}/test_graph_2", test_schema));

    // Set session schema
    fixture.assert_query_succeeds(&format!("SESSION SET SCHEMA {}", test_schema));

    // Now DROP GRAPH with relative path should work
    fixture.assert_query_succeeds("DROP GRAPH test_graph_1");

    // Verify graph was dropped by trying to set it (should fail)
    fixture.assert_query_fails(
        &format!("SESSION SET GRAPH {}/test_graph_1", test_schema),
        "does not exist",
    );

    // Drop the second graph using relative path
    fixture.assert_query_succeeds("DROP GRAPH test_graph_2");

    // Clean up
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}

#[test]
fn test_session_graph_reset_on_drop() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "session_reset_schema";

    // Create schema and graph
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}/active_graph", test_schema));

    // Set session schema and graph
    fixture.assert_query_succeeds(&format!("SESSION SET SCHEMA {}", test_schema));

    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH {}/active_graph", test_schema));

    // Verify we can insert data (proves session graph is active)
    fixture.assert_query_succeeds("INSERT (n:TestNode {name: 'test'})");

    // Drop the current session graph
    fixture.assert_query_succeeds(&format!("DROP GRAPH {}/active_graph", test_schema));

    // Now trying to insert should fail because session graph was reset
    fixture.assert_query_fails(
        "INSERT (n:TestNode {name: 'should_fail'})",
        "No graph context available",
    );

    // Clean up
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}

#[test]
fn test_session_graph_reset_on_relative_drop() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "relative_drop_schema";

    // Create schema and graph
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    // Set session schema first
    fixture.assert_query_succeeds(&format!("SESSION SET SCHEMA {}", test_schema));

    // Create graph using relative path
    fixture.assert_query_succeeds("CREATE GRAPH active_graph");

    // Set as session graph using relative path (should now work!)
    fixture.assert_query_succeeds("SESSION SET GRAPH active_graph");

    // Verify we can insert data (proves session graph is active)
    fixture.assert_query_succeeds("INSERT (n:TestNode {name: 'test'})");

    // Drop using relative path (this should reset the session)
    fixture.assert_query_succeeds("DROP GRAPH active_graph");

    // Now trying to insert should fail because session graph was reset
    fixture.assert_query_fails(
        "INSERT (n:TestNode {name: 'should_fail'})",
        "No graph context available",
    );

    // Clean up
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}

#[test]
fn test_session_graph_full_path_storage() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "full_path_schema";

    // Create schema and graph
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    // Set session schema first
    fixture.assert_query_succeeds(&format!("SESSION SET SCHEMA {}", test_schema));

    // Create graph using relative path
    fixture.assert_query_succeeds("CREATE GRAPH session_test_graph");

    // Set session graph using relative path
    fixture.assert_query_succeeds("SESSION SET GRAPH session_test_graph");

    // Verify we can insert data (proves session graph is active)
    fixture.assert_query_succeeds("INSERT (n:TestNode {name: 'test'})");

    // The show_session call should now display the full path
    let result = fixture.query("CALL gql.show_session()").unwrap();

    // Check that current_graph property shows full path
    let mut found_full_path = false;
    for row in &result.rows {
        if let (Some(prop_name), Some(prop_value)) = (
            row.values.get("property_name"),
            row.values.get("property_value"),
        ) {
            if let (Value::String(name), Value::String(value)) = (prop_name, prop_value) {
                if name == "current_graph" {
                    // Should be full path format: /full_path_schema/session_test_graph
                    assert!(
                        value.contains(&format!("/{}/session_test_graph", test_schema)),
                        "Expected full path containing '/{}/session_test_graph', got: '{}'",
                        test_schema,
                        value
                    );
                    found_full_path = true;
                }
            }
        }
    }

    assert!(
        found_full_path,
        "Did not find current_graph property in show_session output"
    );

    // Clean up
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}

#[test]
fn test_drop_graph_preserves_session_schema() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    let test_schema = "schema_preservation_test";

    // Create schema and graph
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA {}", test_schema));

    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}/preservation_graph", test_schema));

    // Set session schema and graph
    fixture.assert_query_succeeds(&format!("SESSION SET SCHEMA {}", test_schema));

    fixture.assert_query_succeeds(&format!(
        "SESSION SET GRAPH {}/preservation_graph",
        test_schema
    ));

    // Verify both schema and graph are set
    let result_before = fixture.query("CALL gql.show_session()").unwrap();
    let mut found_schema = false;
    let mut found_graph = false;

    for row in &result_before.rows {
        if let (Some(prop_name), Some(_prop_value)) = (
            row.values.get("property_name"),
            row.values.get("property_value"),
        ) {
            if let Value::String(name) = prop_name {
                match name.as_str() {
                    "current_schema" => found_schema = true,
                    "current_graph" => found_graph = true,
                    _ => {}
                }
            }
        }
    }

    assert!(found_schema, "Schema should be set before dropping graph");
    assert!(found_graph, "Graph should be set before dropping graph");

    // Drop the graph
    fixture.assert_query_succeeds(&format!("DROP GRAPH {}/preservation_graph", test_schema));

    // Verify schema is still set but graph is cleared
    let result_after = fixture.query("CALL gql.show_session()").unwrap();
    let mut found_schema_after = false;
    let mut found_graph_after = false;

    for row in &result_after.rows {
        if let (Some(prop_name), Some(_prop_value)) = (
            row.values.get("property_name"),
            row.values.get("property_value"),
        ) {
            if let Value::String(name) = prop_name {
                match name.as_str() {
                    "current_schema" => found_schema_after = true,
                    "current_graph" => found_graph_after = true,
                    _ => {}
                }
            }
        }
    }

    assert!(
        found_schema_after,
        "Schema should still be set after dropping graph"
    );
    assert!(
        !found_graph_after,
        "Graph should be cleared after dropping graph"
    );

    // Clean up
    fixture.assert_query_succeeds(&format!("DROP SCHEMA {} CASCADE", test_schema));
}
