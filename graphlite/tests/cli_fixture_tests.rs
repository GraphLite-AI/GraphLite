//! Tests for CliFixture infrastructure

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::cli_fixture::CliFixture;

#[test]
fn test_cli_fixture_initialization() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    assert!(fixture.db_path().exists());
}

#[test]
fn test_simple_create_schema() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema_name = fixture.schema_name();

    let result = fixture.assert_query_succeeds(&format!("CREATE SCHEMA /{};", schema_name));
    assert!(!result.is_empty());
}

#[test]
fn test_catalog_persistence() {
    let fixture = CliFixture::empty().expect("Failed to create CLI fixture");
    let schema_name = fixture.schema_name();

    // Create schema in command 1
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA /{};", schema_name));

    // Create graph in command 2 - this will FAIL if schema didn't persist
    let result = fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/test;", schema_name));
    assert!(!result.is_empty());

    // Verify we can reference the graph in command 3
    let result2 =
        fixture.assert_query_succeeds(&format!("SESSION SET GRAPH /{}/test;", schema_name));
    // SESSION SET GRAPH returns empty result on success
    assert!(result2.is_empty() || result2.len() == 0 || result2.rows.is_empty());
}
