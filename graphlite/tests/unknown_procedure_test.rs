//! Test that unknown procedures give proper error messages

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_unknown_gql_procedure_error() {
    let fixture = TestFixture::new().expect("Should create test fixture");

    // Try to call a non-existent gql.* procedure
    let result = fixture.query("CALL gql.nonexistent_procedure();");

    assert!(result.is_err(), "Should fail for non-existent procedure");

    let err = result.unwrap_err();

    // Should get "procedure not found" error, NOT "No graph context" error
    assert!(
        err.contains("procedure not found") || err.contains("not supported"),
        "Error should mention procedure not found, got: {}",
        err
    );

    assert!(
        !err.contains("No graph context"),
        "Should not get graph context error for unknown procedure, got: {}",
        err
    );

    // Should say "Available system procedures" not "ISO GQL procedures"
    assert!(
        err.contains("Available system procedures"),
        "Error should say 'Available system procedures', got: {}",
        err
    );
}

#[test]
fn test_list_available_procedures_in_error() {
    let fixture = TestFixture::new().expect("Should create test fixture");

    let result = fixture.query("CALL gql.bad_procedure_name();");

    assert!(result.is_err(), "Should fail for non-existent procedure");

    let err = result.unwrap_err();

    // Error should list available procedures to help the user
    assert!(
        err.contains("list_schemas") || err.contains("list_graphs"),
        "Error should list available procedures to help user, got: {}",
        err
    );
}
