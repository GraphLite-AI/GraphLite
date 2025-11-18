//! Comprehensive integration tests for role management functionality
//!
//! Tests GRANT ROLE and REVOKE ROLE statements with positive and negative scenarios,
//! system role protection rules, and edge cases.

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;
use graphlite::Value;

#[test]
fn test_role_management_comprehensive() {
    let fixture = TestFixture::empty()
        .expect("Failed to create test fixture");

    log::debug!("=== Testing Comprehensive Role Management ===");

    // Step 1: Create some test roles first (default roles might not be visible in test environment)
    log::debug!("\n1. Testing role creation and basic functionality...");
    fixture.assert_query_succeeds("CREATE ROLE 'test_role_1'");
    fixture.assert_query_succeeds("CREATE ROLE 'test_role_2'");
    log::debug!("✓ Test roles created successfully");

    // Check if we can list roles (this might work differently in test environment)
    let roles_query_result = fixture.query("CALL gql.list_roles()");
    match roles_query_result {
        Ok(roles_result) => {
            log::debug!("✓ Role listing works, found {} roles", roles_result.rows.len());
        }
        Err(e) => {
            log::debug!("⚠ Role listing not available in test environment: {}", e);
            log::debug!("✓ Continuing with direct role operations testing");
        }
    }

    // Step 2: Create test users
    log::debug!("\n2. Testing user creation...");
    fixture.assert_query_succeeds("CREATE USER 'alice' PASSWORD 'password123'");
    fixture.assert_query_succeeds("CREATE USER 'bob' PASSWORD 'password456'");
    fixture.assert_query_succeeds("CREATE USER 'charlie' PASSWORD 'password789'");
    log::debug!("✓ Created test users: alice, bob, charlie");

    // Step 3: POSITIVE SCENARIOS - Grant roles to users
    log::debug!("\n3. Testing GRANT ROLE positive scenarios...");

    fixture.assert_query_succeeds("GRANT ROLE 'test_role_1' TO 'alice'");
    log::debug!("✓ Granted 'test_role_1' role to alice");

    fixture.assert_query_succeeds("GRANT ROLE 'test_role_2' TO 'bob'");
    log::debug!("✓ Granted 'test_role_2' role to bob");

    fixture.assert_query_succeeds("GRANT ROLE 'test_role_1' TO 'charlie'");
    fixture.assert_query_succeeds("GRANT ROLE 'test_role_2' TO 'charlie'");
    log::debug!("✓ Granted multiple roles to charlie");

    // Step 4: Test duplicate role grant (should succeed but indicate already has role)
    log::debug!("\n4. Testing duplicate GRANT ROLE...");
    let duplicate_result = fixture.assert_query_succeeds("GRANT ROLE 'test_role_1' TO 'alice'");

    // Check if the response indicates the user already has the role
    if let Some(row) = duplicate_result.rows.first() {
        if let Some(Value::String(message)) = row.values.get("status") {
            assert!(message.contains("already has"), "Should indicate user already has role: {}", message);
            log::debug!("✓ Correctly handled duplicate role grant: {}", message);
        }
    }

    // Step 5: POSITIVE SCENARIOS - Revoke roles
    log::debug!("\n5. Testing REVOKE ROLE positive scenarios...");

    fixture.assert_query_succeeds("REVOKE ROLE 'test_role_1' FROM 'alice'");
    log::debug!("✓ Revoked 'test_role_1' role from alice");

    fixture.assert_query_succeeds("REVOKE ROLE 'test_role_2' FROM 'charlie'");
    log::debug!("✓ Revoked 'test_role_2' role from charlie");

    log::debug!("✅ ALL POSITIVE SCENARIOS PASSED!");
}

#[test]
fn test_role_management_negative_scenarios() {
    let fixture = TestFixture::empty()
        .expect("Failed to create test fixture");

    log::debug!("=== Testing Role Management Negative Scenarios ===");

    // Create a test user for negative testing
    fixture.assert_query_succeeds("CREATE USER 'test_user' PASSWORD 'password'");

    // Test 1: System role protection - cannot revoke 'user' role
    log::debug!("\n1. Testing system role protection - 'user' role...");
    fixture.assert_query_fails(
        "REVOKE ROLE 'user' FROM 'test_user'",
        "Cannot revoke system role 'user'"
    );
    log::debug!("✓ Correctly prevented removal of 'user' role");

    // Test 2: System role protection - cannot revoke 'admin' role from 'admin' user
    log::debug!("\n2. Testing system role protection - 'admin' role from 'admin' user...");
    fixture.assert_query_fails(
        "REVOKE ROLE 'admin' FROM 'admin'",
        "Cannot revoke 'admin' role from 'admin' user"
    );
    log::debug!("✓ Correctly prevented removal of 'admin' role from 'admin' user");

    // Test 3: Grant non-existent role
    log::debug!("\n3. Testing grant of non-existent role...");
    fixture.assert_query_fails(
        "GRANT ROLE 'nonexistent_role' TO 'test_user'",
        "Role 'nonexistent_role' does not exist"
    );
    log::debug!("✓ Correctly failed to grant non-existent role");

    // Test 4: Grant role to non-existent user
    log::debug!("\n4. Testing grant to non-existent user...");
    fixture.assert_query_fails(
        "GRANT ROLE 'user' TO 'nonexistent_user'",
        "User 'nonexistent_user' does not exist"
    );
    log::debug!("✓ Correctly failed to grant role to non-existent user");

    // Test 5: Revoke non-existent role
    log::debug!("\n5. Testing revoke of non-existent role...");
    fixture.assert_query_fails(
        "REVOKE ROLE 'nonexistent_role' FROM 'test_user'",
        "Role 'nonexistent_role' does not exist"
    );
    log::debug!("✓ Correctly failed to revoke non-existent role");

    // Test 6: Revoke role from non-existent user (use a non-system role for this test)
    log::debug!("\n6. Testing revoke from non-existent user...");
    fixture.assert_query_succeeds("CREATE ROLE 'test_revoke_role'");
    fixture.assert_query_fails(
        "REVOKE ROLE 'test_revoke_role' FROM 'nonexistent_user'",
        "User 'nonexistent_user' does not exist"
    );
    log::debug!("✓ Correctly failed to revoke role from non-existent user");

    // Test 7: Revoke role user doesn't have (should succeed but indicate no change)
    log::debug!("\n7. Testing revoke of role user doesn't have...");
    let revoke_missing = fixture.assert_query_succeeds("REVOKE ROLE 'admin' FROM 'test_user'");

    if let Some(row) = revoke_missing.rows.first() {
        if let Some(Value::String(message)) = row.values.get("status") {
            assert!(message.contains("does not have"), "Should indicate user doesn't have role: {}", message);
            log::debug!("✓ Correctly handled missing role revocation: {}", message);
        }
    }

    log::debug!("✅ ALL NEGATIVE SCENARIOS PASSED!");
}

#[test]
fn test_role_assignment_integrity() {
    let fixture = TestFixture::empty()
        .expect("Failed to create test fixture");

    log::debug!("=== Testing Role Assignment Integrity ===");

    // Create test scenario with multiple users and roles
    fixture.assert_query_succeeds("CREATE ROLE 'test_role'");
    fixture.assert_query_succeeds("CREATE USER 'user1' PASSWORD 'pass1'");
    fixture.assert_query_succeeds("CREATE USER 'user2' PASSWORD 'pass2'");

    // Grant roles to multiple users
    fixture.assert_query_succeeds("GRANT ROLE 'test_role' TO 'user1'");
    fixture.assert_query_succeeds("GRANT ROLE 'test_role' TO 'user2'");

    // Verify both users have the role
    let users_result = fixture.assert_query_succeeds("CALL gql.list_users()");

    let mut users_with_test_role = 0;
    for row in &users_result.rows {
        if let Some(Value::String(username)) = row.values.get("username") {
            if username == "user1" || username == "user2" {
                if let Some(Value::String(roles_str)) = row.values.get("roles") {
                    if roles_str.contains("test_role") {
                        users_with_test_role += 1;
                    }
                }
            }
        }
    }

    assert_eq!(users_with_test_role, 2, "Both users should have test_role");
    log::debug!("✓ Multiple users can have the same role");

    // Revoke from one user and verify the other still has it
    fixture.assert_query_succeeds("REVOKE ROLE 'test_role' FROM 'user1'");

    let users_after_revoke = fixture.assert_query_succeeds("CALL gql.list_users()");

    let mut user1_has_role = false;
    let mut user2_has_role = false;

    for row in &users_after_revoke.rows {
        if let Some(Value::String(username)) = row.values.get("username") {
            if let Some(Value::String(roles_str)) = row.values.get("roles") {
                if username == "user1" && roles_str.contains("test_role") {
                    user1_has_role = true;
                }
                if username == "user2" && roles_str.contains("test_role") {
                    user2_has_role = true;
                }
            }
        }
    }

    assert!(!user1_has_role, "user1 should no longer have test_role");
    assert!(user2_has_role, "user2 should still have test_role");
    log::debug!("✓ Role revocation is user-specific");

    // Verify all users still have default 'user' role
    let mut all_users_have_user_role = true;
    for row in &users_after_revoke.rows {
        if let Some(Value::String(username)) = row.values.get("username") {
            if username == "user1" || username == "user2" {
                if let Some(Value::String(roles_str)) = row.values.get("roles") {
                    if !roles_str.contains("user") {
                        all_users_have_user_role = false;
                        break;
                    }
                }
            }
        }
    }

    assert!(all_users_have_user_role, "All users should retain default 'user' role");
    log::debug!("✓ Default 'user' role is preserved through role operations");

    log::debug!("✅ ALL INTEGRITY TESTS PASSED!");
}

#[test]
fn test_role_management_syntax_validation() {
    let fixture = TestFixture::empty()
        .expect("Failed to create test fixture");

    log::debug!("=== Testing Role Management Syntax Validation ===");

    // Create test data
    fixture.assert_query_succeeds("CREATE ROLE 'valid_role'");
    fixture.assert_query_succeeds("CREATE USER 'valid_user' PASSWORD 'password'");

    // Test valid syntax variations
    log::debug!("\n1. Testing valid syntax variations...");

    fixture.assert_query_succeeds("GRANT ROLE 'valid_role' TO 'valid_user'");
    log::debug!("✓ Basic GRANT ROLE syntax works");

    fixture.assert_query_succeeds("REVOKE ROLE 'valid_role' FROM 'valid_user'");
    log::debug!("✓ Basic REVOKE ROLE syntax works");

    // Test with different role/user name formats
    fixture.assert_query_succeeds("CREATE ROLE 'role_with_underscores'");
    fixture.assert_query_succeeds("CREATE USER 'user_with_underscores' PASSWORD 'password'");
    fixture.assert_query_succeeds("GRANT ROLE 'role_with_underscores' TO 'user_with_underscores'");
    log::debug!("✓ Names with underscores work");

    fixture.assert_query_succeeds("CREATE ROLE 'RoleWithMixedCase'");
    fixture.assert_query_succeeds("GRANT ROLE 'RoleWithMixedCase' TO 'valid_user'");
    log::debug!("✓ Mixed case names work");

    // Note: Invalid syntax tests would require testing the parser directly
    // since the query coordinator expects valid syntax

    log::debug!("✅ ALL SYNTAX VALIDATION TESTS PASSED!");
}

#[test]
fn test_role_management_edge_cases() {
    let fixture = TestFixture::empty()
        .expect("Failed to create test fixture");

    log::debug!("=== Testing Role Management Edge Cases ===");

    // Test with admin user (should exist from database installation)
    log::debug!("\n1. Testing operations with admin user...");

    // Create a test role and grant it to admin
    fixture.assert_query_succeeds("CREATE ROLE 'admin_test_role'");
    fixture.assert_query_succeeds("GRANT ROLE 'admin_test_role' TO 'admin'");
    log::debug!("✓ Can grant additional roles to admin user");

    // Can revoke non-admin roles from admin
    fixture.assert_query_succeeds("REVOKE ROLE 'admin_test_role' FROM 'admin'");
    log::debug!("✓ Can revoke non-admin roles from admin user");

    // But cannot revoke admin role from admin
    fixture.assert_query_fails(
        "REVOKE ROLE 'admin' FROM 'admin'",
        "Cannot revoke 'admin' role from 'admin' user"
    );
    log::debug!("✓ Cannot revoke admin role from admin user");

    // Test role operations across different users
    log::debug!("\n2. Testing cross-user role operations...");

    fixture.assert_query_succeeds("CREATE ROLE 'shared_role'");
    fixture.assert_query_succeeds("CREATE USER 'user_a' PASSWORD 'password'");
    fixture.assert_query_succeeds("CREATE USER 'user_b' PASSWORD 'password'");

    // Grant same role to multiple users
    fixture.assert_query_succeeds("GRANT ROLE 'shared_role' TO 'user_a'");
    fixture.assert_query_succeeds("GRANT ROLE 'shared_role' TO 'user_b'");

    // Verify both have the role
    let users_result = fixture.assert_query_succeeds("CALL gql.list_users()");
    let mut users_with_shared_role = 0;

    for row in &users_result.rows {
        if let Some(Value::String(username)) = row.values.get("username") {
            if username == "user_a" || username == "user_b" {
                if let Some(Value::String(roles_str)) = row.values.get("roles") {
                    if roles_str.contains("shared_role") {
                        users_with_shared_role += 1;
                    }
                }
            }
        }
    }

    assert_eq!(users_with_shared_role, 2, "Both users should have shared_role");
    log::debug!("✓ Multiple users can share the same role");

    // Test final system state
    log::debug!("\n3. Testing final system state...");

    let final_users = fixture.assert_query_succeeds("CALL gql.list_users()");
    let final_roles = fixture.assert_query_succeeds("CALL gql.list_roles()");

    log::debug!("Final system state:");
    log::debug!("  - Users: {}", final_users.rows.len());
    log::debug!("  - Roles: {}", final_roles.rows.len());

    // Verify system integrity - every user should have at least the 'user' role
    for row in &final_users.rows {
        if let Some(Value::String(username)) = row.values.get("username") {
            if let Some(Value::String(roles_str)) = row.values.get("roles") {
                assert!(roles_str.contains("user"), "User '{}' should have 'user' role: {}", username, roles_str);
            }
        }
    }

    log::debug!("✓ All users maintain required 'user' role");

    log::debug!("✅ ALL EDGE CASE TESTS PASSED!");
}
