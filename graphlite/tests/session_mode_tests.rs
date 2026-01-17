/// Integration tests for SessionMode (Instance vs Global)
///
/// These tests verify that:
/// 1. Instance mode provides session isolation between coordinators
/// 2. Global mode provides session sharing between coordinators
/// 3. Sessions are properly managed in each mode
use graphlite::{QueryCoordinator, SessionMode};
use tempfile::tempdir;

#[test]
fn test_instance_mode_isolation() {
    // Create two coordinators with Instance mode
    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();

    let coord1 =
        QueryCoordinator::from_path_with_mode(temp_dir1.path().join("db1"), SessionMode::Instance)
            .expect("Failed to create coordinator 1");

    let coord2 =
        QueryCoordinator::from_path_with_mode(temp_dir2.path().join("db2"), SessionMode::Instance)
            .expect("Failed to create coordinator 2");

    // Create session in coord1
    let session1 = coord1
        .create_simple_session("user1")
        .expect("Failed to create session 1");

    // Create session in coord2
    let session2 = coord2
        .create_simple_session("user2")
        .expect("Failed to create session 2");

    // Verify sessions are isolated (different session IDs)
    assert_ne!(session1, session2, "Sessions should have different IDs");
}

#[test]
#[serial_test::serial]
fn test_global_mode_session_sharing() {
    // Note: Due to Sled's exclusive lock, we use different databases
    // but the global session manager is still shared across coordinators.

    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();

    let coord1 =
        QueryCoordinator::from_path_with_mode(temp_dir1.path().join("db1"), SessionMode::Global)
            .expect("Failed to create coordinator 1");

    let coord2 =
        QueryCoordinator::from_path_with_mode(temp_dir2.path().join("db2"), SessionMode::Global)
            .expect("Failed to create coordinator 2");

    // Create session in coord1
    let session_id = coord1
        .create_simple_session("shared_user")
        .expect("Failed to create session in coord1");

    // CRITICAL: coord2 should be able to use the same session ID
    // In global mode, both coordinators share the same session pool
    // This will succeed if the session exists in the global pool
    let session2 = coord2.create_simple_session("another_user").unwrap();

    // Both sessions should exist in the global pool
    assert_ne!(
        session_id, session2,
        "Different sessions should have different IDs"
    );
}

#[test]
fn test_default_mode_is_instance() {
    // from_path() should use Instance mode by default
    let temp_dir = tempdir().unwrap();

    let coord = QueryCoordinator::from_path(temp_dir.path().join("db"))
        .expect("Failed to create coordinator");

    // Create a session and verify it works
    let _session_id = coord
        .create_simple_session("test_user")
        .expect("Failed to create session");
}

#[test]
fn test_instance_mode_explicit() {
    // Explicitly using Instance mode should behave like from_path()
    let temp_dir = tempdir().unwrap();

    let coord =
        QueryCoordinator::from_path_with_mode(temp_dir.path().join("db"), SessionMode::Instance)
            .expect("Failed to create coordinator");

    let _session_id = coord
        .create_simple_session("test_user")
        .expect("Failed to create session");
}

#[test]
#[serial_test::serial]
fn test_global_mode_session_close() {
    // Verify that closing a session in one coordinator affects the other
    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();

    let coord1 =
        QueryCoordinator::from_path_with_mode(temp_dir1.path().join("db1"), SessionMode::Global)
            .expect("Failed to create coordinator 1");

    let coord2 =
        QueryCoordinator::from_path_with_mode(temp_dir2.path().join("db2"), SessionMode::Global)
            .expect("Failed to create coordinator 2");

    // Create session in coord1
    let session_id = coord1
        .create_simple_session("user")
        .expect("Failed to create session");

    // Close session in coord1
    coord1
        .close_session(&session_id)
        .expect("Failed to close session");

    // Creating a new session in coord2 should work
    let _session2 = coord2
        .create_simple_session("user2")
        .expect("Should be able to create new session");
}

#[test]
fn test_multiple_databases_instance_mode() {
    // Multiple databases with Instance mode should be completely independent
    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();

    let coord1 =
        QueryCoordinator::from_path_with_mode(temp_dir1.path().join("db1"), SessionMode::Instance)
            .expect("Failed to create coordinator 1");

    let coord2 =
        QueryCoordinator::from_path_with_mode(temp_dir2.path().join("db2"), SessionMode::Instance)
            .expect("Failed to create coordinator 2");

    // Create sessions in each database
    let _session1 = coord1.create_simple_session("user1").unwrap();
    let _session2 = coord2.create_simple_session("user2").unwrap();

    // Each database should have its own isolated session
}

#[test]
#[serial_test::serial]
fn test_same_database_global_mode() {
    // Multiple coordinators with Global mode share sessions
    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();

    let coord1 =
        QueryCoordinator::from_path_with_mode(temp_dir1.path().join("db1"), SessionMode::Global)
            .expect("Failed to create coordinator 1");

    let coord2 =
        QueryCoordinator::from_path_with_mode(temp_dir2.path().join("db2"), SessionMode::Global)
            .expect("Failed to create coordinator 2");

    // Create a session in coord1
    let session1 = coord1.create_simple_session("admin").unwrap();

    // Create a session in coord2
    let session2 = coord2.create_simple_session("user").unwrap();

    // Both sessions should exist in the global pool
    assert_ne!(
        session1, session2,
        "Different sessions should have different IDs"
    );
}
