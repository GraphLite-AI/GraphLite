//! Test that gql.* namespace is properly protected
//! Only gql.* prefix is allowed for system procedures

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_only_gql_prefix_works() {
    // Only gql.* prefix should work for system procedures
    let fixture = TestFixture::new()
        .expect("Should create test fixture");

    // Test with gql. prefix - should work
    let result = fixture.query("CALL gql.list_schemas()");
    assert!(result.is_ok(), "gql.list_schemas() should work, got: {:?}", result);

    let result = fixture.query("CALL gql.list_graphs()");
    assert!(result.is_ok(), "gql.list_graphs() should work, got: {:?}", result);

    let result = fixture.query("CALL gql.list_functions()");
    assert!(result.is_ok(), "gql.list_functions() should work, got: {:?}", result);

}

#[test]
fn test_system_prefix_rejected() {
    // system.* prefix should be rejected
    let fixture = TestFixture::new()
        .expect("Should create test fixture");

    let result = fixture.query("CALL system.list_schemas()");
    assert!(result.is_err(), "system.list_schemas() should be rejected");
    let err = result.unwrap_err();
    assert!(err.contains("Invalid procedure namespace") || err.contains("gql."),
        "Error should mention gql.* requirement, got: {}", err);

}

#[test]
fn test_no_prefix_rejected() {
    // Plain names without prefix should be rejected
    let fixture = TestFixture::new()
        .expect("Should create test fixture");

    let result = fixture.query("CALL list_schemas()");
    assert!(result.is_err(), "list_schemas() without prefix should be rejected");
    let err = result.unwrap_err();
    assert!(err.contains("Invalid procedure namespace") || err.contains("gql."),
        "Error should mention gql.* requirement, got: {}", err);

}

#[test]
#[ignore] // CREATE PROCEDURE syntax not fully implemented yet
fn test_cannot_create_gql_namespace_procedure() {
    // Users should not be able to create procedures in gql.* namespace
    // This test will be enabled once CREATE PROCEDURE parsing is fully implemented
    let fixture = TestFixture::new()
        .expect("Should create test fixture");

    let result = fixture.query("CREATE PROCEDURE gql.my_custom_proc() RETURN 1");
    assert!(result.is_err(), "Should not allow creating procedures in gql.* namespace");
    let err = result.unwrap_err();
    assert!(err.contains("gql.*") && err.contains("reserved"),
        "Error should mention gql.* is reserved, got: {}", err);

}

#[test]
#[ignore] // DROP PROCEDURE syntax not fully implemented yet
fn test_cannot_drop_gql_namespace_procedure() {
    // Users should not be able to drop procedures in gql.* namespace
    // This test will be enabled once DROP PROCEDURE parsing is fully implemented
    let fixture = TestFixture::new()
        .expect("Should create test fixture");

    let result = fixture.query("DROP PROCEDURE gql.list_schemas");
    assert!(result.is_err(), "Should not allow dropping procedures in gql.* namespace");
    let err = result.unwrap_err();
    assert!(err.contains("gql.*") && err.contains("reserved"),
        "Error should mention gql.* is reserved, got: {}", err);

}
