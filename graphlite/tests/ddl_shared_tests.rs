//! DDL Tests that can safely use a shared fixture
//! 
//! These tests perform operations within isolated schemas and don't interfere with each other.
//! They can run in parallel since each test uses its own schema namespace.

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;
use std::sync::OnceLock;

static SHARED_FIXTURE: OnceLock<TestFixture> = OnceLock::new();

fn get_shared_fixture() -> &'static TestFixture {
    SHARED_FIXTURE.get_or_init(|| {
        TestFixture::empty().expect("Failed to create shared fixture")
    })
}

#[test]
fn test_schema_ddl_operations() {
    let fixture = get_shared_fixture();
    
    // Create a schema within this test's isolated schema namespace
    let test_schema = "business_schema";
    
    // Test CREATE SCHEMA
    fixture.assert_query_succeeds(&format!(
        "CREATE SCHEMA IF NOT EXISTS {}", 
        test_schema
    ));
    
    // Test schema creation without IF NOT EXISTS
    fixture.assert_query_succeeds(
        "CREATE SCHEMA new_schema"
    );
    
    // Test duplicate schema creation should fail
    fixture.assert_query_fails(
        "CREATE SCHEMA new_schema",
        "already exists"
    );
    
    // Test IF NOT EXISTS prevents errors
    fixture.assert_query_succeeds(
        "CREATE SCHEMA IF NOT EXISTS new_schema"
    );
    
    // Test DROP SCHEMA
    fixture.assert_query_succeeds(
        "DROP SCHEMA IF EXISTS new_schema"
    );
    
    // Test dropping non-existent schema with IF EXISTS
    fixture.assert_query_succeeds(
        "DROP SCHEMA IF EXISTS nonexistent_schema"
    );
    
    // Test dropping non-existent schema without IF EXISTS should fail
    fixture.assert_query_fails(
        "DROP SCHEMA nonexistent_schema_2",
        "not found"
    );
}

#[test]
fn test_drop_schema_basic() {
    let fixture = get_shared_fixture();
    
    let test_schema = "drop_test_schema";
    
    // Test DROP SCHEMA on existing schema - should succeed
    fixture.assert_query_succeeds(&format!(
        "CREATE SCHEMA IF NOT EXISTS {}", 
        test_schema
    ));
    
    fixture.assert_query_succeeds(&format!(
        "DROP SCHEMA {}", 
        test_schema
    ));
    
    // Test DROP SCHEMA on non-existent schema - should fail
    fixture.assert_query_fails(
        "DROP SCHEMA nonexistent_schema",
        "not found"
    );
}

#[test] 
fn test_drop_schema_if_exists() {
    let fixture = get_shared_fixture();
    
    let test_schema = "if_exists_test_schema";
    
    // Test DROP SCHEMA IF EXISTS on existing schema - should succeed
    fixture.assert_query_succeeds(&format!(
        "CREATE SCHEMA IF NOT EXISTS {}", 
        test_schema
    ));
    
    fixture.assert_query_succeeds(&format!(
        "DROP SCHEMA IF EXISTS {}", 
        test_schema
    ));
    
    // Test DROP SCHEMA IF EXISTS on non-existent schema - should succeed
    fixture.assert_query_succeeds(
        "DROP SCHEMA IF EXISTS nonexistent_schema"
    );
}

#[test]
fn test_ddl_error_cases() {
    let fixture = get_shared_fixture();
    
    // Test invalid schema names - parser vs executor level validation
    // Different types of invalid names are caught at different levels
    
    // Names with spaces should be caught by parser or executor
    match fixture.query("CREATE SCHEMA \"invalid name with spaces\"") {
        Ok(_) => panic!("Expected schema name with spaces to fail"),
        Err(e) => {
            // Accept either parse error or validation error
            assert!(
                e.contains("Parse error") || e.contains("Invalid schema name"),
                "Expected parse error or validation error, got: {}",
                e
            );
        }
    }
    
    // Empty schema names - might be caught by parser or validator
    match fixture.query("CREATE SCHEMA \"\"") {
        Ok(_) => panic!("Expected empty schema name to fail"),
        Err(e) => {
            // Accept either parse error or validation error
            assert!(
                e.contains("Parse error") || e.contains("Invalid") || e.contains("Validation error"),
                "Expected parse error or validation error, got: {}",
                e
            );
        }
    }
    
    // Test invalid graph names
    fixture.assert_query_fails(
        "CREATE GRAPH invalid_schema/graph_name",
        "not found"
    );
    
    // Empty graph names - might be caught by parser or validator
    match fixture.query("CREATE GRAPH \"\"") {
        Ok(_) => panic!("Expected empty graph name to fail"),
        Err(e) => {
            // Accept either parse error or validation error
            assert!(
                e.contains("Parse error") || e.contains("Invalid") || e.contains("Validation error"),
                "Expected parse error or validation error, got: {}",
                e
            );
        }
    }
    
    // Test dropping non-existent resources without IF EXISTS
    fixture.assert_query_fails(
        "DROP SCHEMA non_existent_schema_12345",
        "not found"
    );
    
    fixture.assert_query_fails(
        "DROP GRAPH non_existent_schema/non_existent_graph",
        "not found"
    );
}

#[test]
fn test_session_schema_management() {
    let fixture = get_shared_fixture();
    
    let test_schema = "session_schema";
    
    // Create schema
    fixture.assert_query_succeeds(&format!(
        "CREATE SCHEMA {}",
        test_schema
    ));
    
    // Set session schema
    fixture.assert_query_succeeds(&format!(
        "SESSION SET SCHEMA {}",
        test_schema
    ));
    
    // Create graph with schema prefix since session schema context isn't implemented yet
    fixture.assert_query_succeeds(&format!("CREATE GRAPH {}/session_test_graph", test_schema));
    
    // Should be able to set this graph using full path
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH {}/session_test_graph", test_schema));
    
    // Insert data to verify we're in the right graph
    fixture.assert_query_succeeds(
        "INSERT (n:SessionTest {name: 'Schema Test'})"
    );
    
    // Verify data exists
    fixture.assert_first_value(
        "MATCH (n:SessionTest) RETURN count(n) as count",
        "count",
        Value::Number(1.0)
    );
    
    // Clean up
    fixture.assert_query_succeeds(&format!(
        "DROP SCHEMA {} CASCADE",
        test_schema
    ));
}
