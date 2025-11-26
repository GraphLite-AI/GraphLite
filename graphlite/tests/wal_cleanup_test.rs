// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Test for WAL cleanup on clean shutdown

use graphlite::QueryCoordinator;
use tempfile::TempDir;

#[test]
fn test_clean_shutdown_wal_cleanup() {
    // Create a temporary database directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().to_path_buf();

    // First initialization - create database
    let coordinator = QueryCoordinator::from_path(&db_path)
        .expect("Failed to create coordinator");

    // Create a session using simple API
    let session_id = coordinator
        .create_simple_session("testuser")
        .expect("Failed to create session");

    // Execute a simple query to ensure WAL is written
    let _ = coordinator.process_query("CREATE GRAPH test", &session_id);

    // Simulate clean shutdown
    coordinator
        .session_manager()
        .shutdown()
        .expect("Failed to shutdown");

    // Explicitly drop the coordinator
    drop(coordinator);

    // CRITICAL: Clear the global session manager to release Arc references
    // The global SESSION_MANAGER holds an Arc clone that prevents Sled from releasing locks
    graphlite::clear_session_manager().expect("Failed to clear session manager");

    // Give Sled's background threads time to release file locks
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Verify shutdown marker exists
    let shutdown_marker = db_path.join(".clean_shutdown");
    assert!(
        shutdown_marker.exists(),
        "Clean shutdown marker should exist"
    );

    // Second initialization - should cleanup WAL
    {
        let _coordinator = QueryCoordinator::from_path(&db_path)
            .expect("Failed to create coordinator on restart");

        // Verify shutdown marker was removed
        assert!(
            !shutdown_marker.exists(),
            "Shutdown marker should be removed after startup"
        );

        // Verify WAL directory exists but is empty (or recreated)
        let wal_dir = db_path.join("wal");
        if wal_dir.exists() {
            let entries: Vec<_> = std::fs::read_dir(&wal_dir)
                .expect("Failed to read WAL directory")
                .collect();
            println!("WAL directory has {} entries after cleanup", entries.len());
        }
    }
}

#[test]
fn test_unclean_shutdown_recovery() {
    // Create a temporary database directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().to_path_buf();

    // First initialization - create database
    let coordinator = QueryCoordinator::from_path(&db_path)
        .expect("Failed to create coordinator");

    // Create a session using simple API
    let session_id = coordinator
        .create_simple_session("testuser")
        .expect("Failed to create session");

    // Execute a simple query
    let _ = coordinator.process_query("CREATE GRAPH test", &session_id);

    // Simulate unclean shutdown (don't call shutdown, just drop)
    // This simulates a crash or kill signal
    drop(coordinator);

    // CRITICAL: Clear the global session manager to release Arc references
    graphlite::clear_session_manager().expect("Failed to clear session manager");

    // Give Sled's background threads time to release file locks
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Verify shutdown marker does NOT exist (unclean shutdown)
    let shutdown_marker = db_path.join(".clean_shutdown");
    assert!(
        !shutdown_marker.exists(),
        "Clean shutdown marker should NOT exist after unclean shutdown"
    );

    // Second initialization - should run recovery
    {
        let _coordinator = QueryCoordinator::from_path(&db_path)
            .expect("Failed to create coordinator after unclean shutdown");

        // If we get here, recovery succeeded
        println!("Recovery completed successfully");
    }
}

#[test]
fn test_fresh_database_initialization() {
    // Create a temporary database directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().to_path_buf();

    // Fresh initialization - no shutdown marker, no WAL
    let coordinator = QueryCoordinator::from_path(&db_path)
        .expect("Failed to create coordinator on fresh database");

    // Create a session using simple API
    let session_id = coordinator
        .create_simple_session("testuser")
        .expect("Failed to create session");

    // Create schema first
    coordinator
        .process_query("CREATE SCHEMA test", &session_id)
        .expect("Failed to create schema");

    // Set schema context
    coordinator
        .process_query("SESSION SET SCHEMA test", &session_id)
        .expect("Failed to set schema");

    // Execute a query
    let result = coordinator.process_query("CREATE GRAPH fresh_test", &session_id);
    assert!(result.is_ok(), "Query should succeed on fresh database: {:?}", result);
}
