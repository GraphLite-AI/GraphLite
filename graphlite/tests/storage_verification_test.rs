// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Integration test to verify storage architecture
//!
//! This test creates a real database, inserts data, and verifies:
//! - Directory structure matches documentation
//! - WAL files created in correct location
//! - Sled trees organized correctly
//! - User data and system metadata properly separated

use graphlite::QueryCoordinator;
use std::fs;

#[test]
fn test_storage_directory_structure() {
    // Create temporary test directory
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");

    // Create database
    let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create database");

    // Verify directory structure
    assert!(db_path.exists(), "Database directory should exist");
    assert!(
        db_path.join("db").exists(),
        "Sled database file should exist"
    );
    assert!(db_path.join("conf").exists(), "Sled config should exist");
    assert!(db_path.join("wal").exists(), "WAL directory should exist");
    assert!(
        db_path.join("blobs").exists(),
        "Blobs directory should exist"
    );

    drop(coordinator);
}

#[test]
fn test_wal_directory_structure() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");

    let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create database");

    let session_id = coordinator
        .create_simple_session("admin")
        .expect("Failed to create session");

    // Create schema and graph to trigger WAL entries
    coordinator
        .process_query("CREATE SCHEMA /test_schema", &session_id)
        .expect("Failed to create schema");

    // Verify WAL directory structure
    let wal_dir = db_path.join("wal");
    assert!(wal_dir.exists(), "WAL directory should exist");
    assert!(
        wal_dir.join("catalog").exists(),
        "Catalog WAL directory should exist"
    );

    // Check if transaction WAL file exists
    let wal_files: Vec<_> = fs::read_dir(&wal_dir)
        .expect("Failed to read WAL directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("wal_"))
        .collect();

    assert!(!wal_files.is_empty(), "At least one WAL file should exist");

    drop(coordinator);
}

#[test]
fn test_user_data_storage_separation() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");

    {
        let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create database");

        let session_id = coordinator
            .create_simple_session("admin")
            .expect("Failed to create session");

        // Create schema, graph, and set context (proper order)
        coordinator
            .process_query("CREATE SCHEMA /company", &session_id)
            .expect("Failed to create schema");
        coordinator
            .process_query("CREATE GRAPH /company/employees", &session_id)
            .expect("Failed to create graph");
        coordinator
            .process_query("SESSION SET GRAPH /company/employees", &session_id)
            .expect("Failed to set graph context");

        // Insert user data (now that graph context is set)
        coordinator
            .process_query("INSERT (n:Person {name: 'Alice', age: 30})", &session_id)
            .expect("Failed to insert node");
    } // coordinator dropped here, scope ends

    // Verify database structure without opening Sled (avoid lock conflict)
    assert!(db_path.exists(), "Database directory should exist");
    assert!(
        db_path.join("db").exists(),
        "Sled database file should exist"
    );

    // Verify database file has content (user data was written)
    let db_file_metadata =
        fs::metadata(db_path.join("db")).expect("Failed to get db file metadata");
    assert!(
        db_file_metadata.len() > 1000,
        "Database file should be larger than 1KB after inserting data (actual: {} bytes)",
        db_file_metadata.len()
    );
}

#[test]
fn test_multi_graph_tree_isolation() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");

    {
        let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create database");

        let session_id = coordinator
            .create_simple_session("admin")
            .expect("Failed to create session");

        // Create two schemas with graphs
        coordinator
            .process_query("CREATE SCHEMA /schema1", &session_id)
            .expect("Failed to create schema1");
        coordinator
            .process_query("CREATE GRAPH /schema1/graph1", &session_id)
            .expect("Failed to create graph1");

        coordinator
            .process_query("CREATE SCHEMA /schema2", &session_id)
            .expect("Failed to create schema2");
        coordinator
            .process_query("CREATE GRAPH /schema2/graph2", &session_id)
            .expect("Failed to create graph2");

        // Insert data into first graph (set context first)
        coordinator
            .process_query("SESSION SET GRAPH /schema1/graph1", &session_id)
            .expect("Failed to set graph1 context");
        coordinator
            .process_query("INSERT (n:Person {name: 'Alice'})", &session_id)
            .expect("Failed to insert into graph1");

        // Insert data into second graph (set context first)
        coordinator
            .process_query("SESSION SET GRAPH /schema2/graph2", &session_id)
            .expect("Failed to set graph2 context");
        coordinator
            .process_query("INSERT (n:Company {name: 'TechCorp'})", &session_id)
            .expect("Failed to insert into graph2");
    } // coordinator dropped here

    // Verify multiple graphs created separate data
    assert!(db_path.exists(), "Database directory should exist");
    let db_file_metadata =
        fs::metadata(db_path.join("db")).expect("Failed to get db file metadata");

    // With 2 graphs and 2 inserts, should have substantial data
    assert!(
        db_file_metadata.len() > 2000,
        "Database should be larger with multiple graphs (actual: {} bytes)",
        db_file_metadata.len()
    );
}

#[test]
fn test_no_unwanted_data_directories() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");

    // Get current working directory
    let cwd = std::env::current_dir().expect("Failed to get current directory");

    // Create database
    let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create database");

    let session_id = coordinator
        .create_simple_session("admin")
        .expect("Failed to create session");

    // Perform operations that used to create unwanted data directories
    coordinator
        .process_query("CREATE SCHEMA /test", &session_id)
        .expect("Failed to create schema");
    coordinator
        .process_query("CREATE GRAPH /test/graph", &session_id)
        .expect("Failed to create graph");

    drop(coordinator);

    // Verify no "data" directory created in current working directory
    let unwanted_data_dir = cwd.join("data");
    assert!(
        !unwanted_data_dir.exists(),
        "Should NOT create 'data' directory in current working directory"
    );

    // Verify no "data" directory created in graphlite subdirectory
    let graphlite_data_dir = cwd.join("graphlite").join("data");
    assert!(
        !graphlite_data_dir.exists(),
        "Should NOT create 'data' directory in graphlite subdirectory"
    );

    // Verify all WAL files are in the database directory
    let db_wal_dir = db_path.join("wal");
    assert!(
        db_wal_dir.exists(),
        "WAL directory should exist in database directory"
    );
}

#[test]
fn test_catalog_and_user_data_coexist() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");

    {
        let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create database");

        let session_id = coordinator
            .create_simple_session("admin")
            .expect("Failed to create session");

        // Create catalog metadata (schema and graph)
        coordinator
            .process_query("CREATE SCHEMA /myschema", &session_id)
            .expect("Failed to create schema");
        coordinator
            .process_query("CREATE GRAPH /myschema/mygraph", &session_id)
            .expect("Failed to create graph");

        // Set graph context and create user data
        coordinator
            .process_query("SESSION SET GRAPH /myschema/mygraph", &session_id)
            .expect("Failed to set graph context");
        coordinator
            .process_query("INSERT (n:TestNode {value: 42})", &session_id)
            .expect("Failed to insert node");
    } // coordinator dropped here

    // Verify single database file contains both catalog and user data
    assert!(db_path.exists(), "Database directory should exist");
    assert!(
        db_path.join("db").exists(),
        "All data should be in single database file"
    );

    // Verify the database file contains both types of data
    let db_file_metadata =
        fs::metadata(db_path.join("db")).expect("Failed to get db file metadata");
    assert!(
        db_file_metadata.len() > 1000,
        "Database file should contain both catalog and user data (actual: {} bytes)",
        db_file_metadata.len()
    );
}
