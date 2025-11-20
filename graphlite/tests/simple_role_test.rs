//! Simple test to verify GRANT/REVOKE role functionality works
//! This test focuses on the core functionality without complex dependencies

#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_grant_revoke_basic_functionality() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Create test role and users
    fixture.assert_query_succeeds("CREATE ROLE 'test_role'");
    fixture.assert_query_succeeds("CREATE USER 'test_user' PASSWORD 'password'");

    // Test GRANT ROLE
    let grant_result = fixture.assert_query_succeeds("GRANT ROLE 'test_role' TO 'test_user'");

    // Print the result to see what we get

    // Test duplicate GRANT (should succeed but indicate already has role)
    let duplicate_grant = fixture.assert_query_succeeds("GRANT ROLE 'test_role' TO 'test_user'");

    // Test REVOKE ROLE
    let revoke_result = fixture.assert_query_succeeds("REVOKE ROLE 'test_role' FROM 'test_user'");

    // Test revoking role user doesn't have (should succeed but indicate no change)
    let revoke_missing = fixture.assert_query_succeeds("REVOKE ROLE 'test_role' FROM 'test_user'");
}

#[test]
fn test_role_error_scenarios() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Set up test data
    fixture.assert_query_succeeds("CREATE ROLE 'existing_role'");
    fixture.assert_query_succeeds("CREATE USER 'existing_user' PASSWORD 'password'");

    // Test 1: Grant non-existent role
    fixture.assert_query_fails(
        "GRANT ROLE 'nonexistent_role' TO 'existing_user'",
        "does not exist",
    );

    // Test 2: Grant role to non-existent user
    fixture.assert_query_fails(
        "GRANT ROLE 'existing_role' TO 'nonexistent_user'",
        "does not exist",
    );

    // Test 3: Revoke non-existent role
    fixture.assert_query_fails(
        "REVOKE ROLE 'nonexistent_role' FROM 'existing_user'",
        "does not exist",
    );

    // Test 4: Revoke role from non-existent user
    fixture.assert_query_fails(
        "REVOKE ROLE 'existing_role' FROM 'nonexistent_user'",
        "does not exist",
    );
}

#[test]
fn test_system_role_protection() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Create a test user
    fixture.assert_query_succeeds("CREATE USER 'protected_user' PASSWORD 'password'");

    // Test 1: Try to revoke 'user' role (should fail)
    fixture.assert_query_fails(
        "REVOKE ROLE 'user' FROM 'protected_user'",
        "Cannot revoke system role 'user'",
    );

    // Test 2: Try to revoke 'admin' role from 'admin' user (should fail)
    fixture.assert_query_fails(
        "REVOKE ROLE 'admin' FROM 'admin'",
        "Cannot revoke 'admin' role from 'admin' user",
    );
}
