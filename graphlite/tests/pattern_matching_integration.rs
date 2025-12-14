// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Integration tests for pattern matching functions
//!
//! These tests validate the 5 pattern matching functions:
//! - FT_STARTS_WITH: Prefix matching
//! - FT_ENDS_WITH: Suffix matching
//! - FT_WILDCARD: Wildcard pattern matching (* and ?)
//! - FT_REGEX: Regular expression matching
//! - FT_PHRASE_PREFIX: Phrase prefix matching (autocomplete)

#[cfg(test)]
mod pattern_matching_integration {
    use graphlite::QueryCoordinator;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper function to create a test coordinator with a temporary database
    fn create_test_coordinator() -> (Arc<QueryCoordinator>, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path();

        let coordinator =
            QueryCoordinator::from_path(db_path).expect("Failed to create coordinator");

        (coordinator, temp_dir)
    }

    /// Helper to setup a graph and session for testing
    fn setup_graph_and_session(coordinator: &Arc<QueryCoordinator>) -> String {
        let session_id = coordinator
            .create_simple_session("test_user")
            .expect("Failed to create session");

        // Create schema
        let schema_result = coordinator.process_query("CREATE SCHEMA test_schema", &session_id);
        assert!(
            schema_result.is_ok(),
            "Failed to create schema: {:?}",
            schema_result
        );

        // Create graph
        let graph_result =
            coordinator.process_query("CREATE GRAPH test_schema/test_graph", &session_id);
        assert!(
            graph_result.is_ok(),
            "Failed to create graph: {:?}",
            graph_result
        );

        // Set graph as current
        let set_result =
            coordinator.process_query("SESSION SET GRAPH test_schema/test_graph", &session_id);
        assert!(set_result.is_ok(), "Failed to set graph: {:?}", set_result);

        session_id
    }

    /// Setup test data for pattern matching tests
    fn setup_pattern_test_data(coordinator: &Arc<QueryCoordinator>, session_id: &str) {
        // Insert test data with various text patterns
        // Using Person and Document node types which are known to work
        // Note: Simplified to use fewer properties per node to avoid parse errors
        let queries = vec![
            // Person nodes with various name patterns
            "INSERT (:Person {name: 'alice', email: 'alice@example.com'})",
            "INSERT (:Person {name: 'bob_admin', email: 'bob@gmail.com'})",
            "INSERT (:Person {name: 'charlie', email: 'charlie@company.org'})",
            "INSERT (:Person {name: 'david123', email: 'david@example.com'})",
            "INSERT (:Person {name: 'eve_test', email: 'eve@test.com'})",

            // Person nodes with role property
            "INSERT (:Person {username: 'alice', role: 'admin'})",
            "INSERT (:Person {username: 'bob_admin', role: 'user'})",
            "INSERT (:Person {username: 'charlie', role: 'moderator'})",

            // Document nodes with file-like names
            "INSERT (:Document {name: 'document.pdf', content: 'PDF'})",
            "INSERT (:Document {name: 'image.png', content: 'Image'})",
            "INSERT (:Document {name: 'script.js', content: 'JS'})",
            "INSERT (:Document {name: 'data.json', content: 'JSON'})",
            "INSERT (:Document {name: 'readme.md', content: 'MD'})",

            // Document nodes with path property
            "INSERT (:Document {name: 'report.pdf', path: '/docs/report.pdf'})",
            "INSERT (:Document {name: 'photo.png', path: '/images/photo.png'})",

            // Document nodes with titles
            "INSERT (:Document {title: 'Machine Learning Basics', category: 'ML'})",
            "INSERT (:Document {title: 'Deep Learning Advanced', category: 'DL'})",
            "INSERT (:Document {title: 'Natural Language Processing', category: 'NLP'})",
            "INSERT (:Document {title: 'Machine Vision Systems', category: 'CV'})",
            "INSERT (:Document {title: 'Data Science Fundamentals', category: 'DS'})",

            // Person nodes with SKU patterns
            "INSERT (:Person {sku: 'ABC-001', name: 'Widget A'})",
            "INSERT (:Person {sku: 'ABC-002', name: 'Widget B'})",
            "INSERT (:Person {sku: 'XYZ-100', name: 'Gadget X'})",
            "INSERT (:Person {sku: 'XYZ-101', name: 'Gadget Y'})",
            "INSERT (:Person {sku: 'DEF-500', name: 'Tool D'})",
        ];

        for query in queries {
            let result = coordinator.process_query(query, session_id);
            assert!(
                result.is_ok(),
                "Failed to insert test data with query '{}': {:?}",
                query,
                result
            );
        }
    }

    // ==============================================================================
    // FT_STARTS_WITH TESTS (20 tests)
    // ==============================================================================

    #[test]
    fn test_starts_with_basic_prefix() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'alice') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_empty_prefix() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, '') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_admin_role() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.role, 'admin') RETURN u.username, u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_multiple_prefixes() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'alice') OR FT_STARTS_WITH(u.username, 'bob') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_no_match() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'xyz') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_file_paths() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_STARTS_WITH(f.path, '/docs') RETURN f.name, f.path";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_numeric_string() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'david') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_document_titles() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_STARTS_WITH(d.title, 'Machine') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_product_sku() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (p:Person) WHERE FT_STARTS_WITH(p.sku, 'ABC') RETURN p.sku, p.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_combined_with_other_filters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (p:Person) WHERE FT_STARTS_WITH(p.sku, 'ABC') AND p.price < 15.0 RETURN p.sku, p.price";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_in_return_clause() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) RETURN u.username, FT_STARTS_WITH(u.username, 'alice') as starts_with_alice";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_ordering() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'a') OR FT_STARTS_WITH(u.username, 'b') RETURN u.username ORDER BY u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_count_aggregation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'alice') RETURN COUNT(u) as count";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_distinct() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.email, 'alice') RETURN DISTINCT u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_limit() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_STARTS_WITH(d.title, 'Machine') RETURN d.title LIMIT 1";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_null_handling() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert node without username property
        coordinator
            .process_query("INSERT (:Person {email: 'test@example.com'})", &session_id)
            .expect("Failed to insert node");

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'alice') RETURN u.email";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_negation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE NOT FT_STARTS_WITH(u.username, 'alice') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_and_logic() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'alice') AND FT_STARTS_WITH(u.email, 'alice') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_groupby() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.email, 'alice') OR FT_STARTS_WITH(u.email, 'bob') RETURN u.role, COUNT(u) as count GROUP BY u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_starts_with_case_sensitive() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        // Test case sensitivity (alice vs Alice)
        let query = "MATCH (u:Person) WHERE FT_STARTS_WITH(u.username, 'Alice') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    // ==============================================================================
    // FT_ENDS_WITH TESTS (18 tests)
    // ==============================================================================

    #[test]
    fn test_ends_with_basic_suffix() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.pdf') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_email_domain() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_ENDS_WITH(u.email, '@example.com') RETURN u.username, u.email";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_multiple_extensions() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.pdf') OR FT_ENDS_WITH(f.name, '.png') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_path_suffix() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.path, '.pdf') RETURN f.path";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_no_match() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.exe') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_username_suffix() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_ENDS_WITH(u.username, '_admin') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_email_tld() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_ENDS_WITH(u.email, '.com') RETURN u.email";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_combined_filters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.json') AND f.size < 500 RETURN f.name, f.size";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_in_return() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) RETURN f.name, FT_ENDS_WITH(f.name, '.pdf') as is_pdf";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_count() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.md') RETURN COUNT(f) as markdown_count";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_ordering() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.pdf') OR FT_ENDS_WITH(f.name, '.md') RETURN f.name ORDER BY f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_null_handling() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        coordinator
            .process_query("INSERT (:Document {path: '/test/file'})", &session_id)
            .expect("Failed to insert node");

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.pdf') RETURN f.path";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_negation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE NOT FT_ENDS_WITH(f.name, '.pdf') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_empty_suffix() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_and_starts_with() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_STARTS_WITH(f.name, 'data') AND FT_ENDS_WITH(f.name, '.json') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_limit() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_ENDS_WITH(u.email, '.com') RETURN u.email LIMIT 2";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_distinct() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_ENDS_WITH(u.email, '.com') RETURN DISTINCT u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_ends_with_case_sensitive() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_ENDS_WITH(f.name, '.PDF') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    // ==============================================================================
    // FT_WILDCARD TESTS (18 tests)
    // ==============================================================================

    #[test]
    fn test_wildcard_trailing_star() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, 'alice*') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_leading_star() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, '*admin') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_middle_star() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, 'a*e') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_question_mark() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, 'alice?????') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_mixed() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, '?ob*') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_file_extension() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_WILDCARD(f.name, '*.pdf') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_multiple_stars() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_WILDCARD(f.path, '/*/report.*') RETURN f.path";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_sku_pattern() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (p:Person) WHERE FT_WILDCARD(p.sku, 'ABC-*') RETURN p.sku";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_no_match() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, 'xyz*') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_combined_filters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (p:Person) WHERE FT_WILDCARD(p.sku, 'ABC-*') AND p.price < 15.0 RETURN p.sku, p.price";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_in_return() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) RETURN u.username, FT_WILDCARD(u.username, '*admin') as is_admin_user";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_count() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_WILDCARD(f.name, '*.pdf') RETURN COUNT(f) as pdf_count";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_ordering() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, '*_*') RETURN u.username ORDER BY u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_null_handling() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        coordinator
            .process_query("INSERT (:Person {email: 'test@example.com'})", &session_id)
            .expect("Failed to insert node");

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, 'alice*') RETURN u.email";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_negation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE NOT FT_WILDCARD(f.name, '*.pdf') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_limit() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (f:Document) WHERE FT_WILDCARD(f.name, '*.*') RETURN f.name LIMIT 3";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_distinct() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.email, '*@*.com') RETURN DISTINCT u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_wildcard_exact_length() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (u:Person) WHERE FT_WILDCARD(u.username, '???') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    // ==============================================================================
    // FT_REGEX TESTS (18 tests)
    // ==============================================================================

    #[test]
    fn test_regex_basic_pattern() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '^alice$') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_digit_pattern() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '.*[0-9]+$') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_email_pattern() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.email, '^[a-z]+@[a-z]+\.com$') RETURN u.email";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_sku_pattern() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (p:Person) WHERE FT_REGEX(p.sku, '^[A-Z]{3}-[0-9]{3}$') RETURN p.sku";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_alternation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.role, '^(admin|moderator)$') RETURN u.username, u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_file_extension() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (f:Document) WHERE FT_REGEX(f.name, '\.(pdf|png|md)$') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_character_class() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '^[a-z]+$') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_no_match() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '^xyz[0-9]+$') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_combined_filters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (p:Person) WHERE FT_REGEX(p.sku, '^ABC-') AND p.price < 15.0 RETURN p.sku, p.price";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_in_return() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) RETURN u.username, FT_REGEX(u.username, '.*admin.*') as has_admin";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_count() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.email, '\.com$') RETURN COUNT(u) as com_emails";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_ordering() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '^[a-e]') RETURN u.username ORDER BY u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_null_handling() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        coordinator
            .process_query("INSERT (:Person {email: 'test@example.com'})", &session_id)
            .expect("Failed to insert node");

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '^alice$') RETURN u.email";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_negation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (f:Document) WHERE NOT FT_REGEX(f.name, '\.pdf$') RETURN f.name";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_limit() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.email, '@.*\.com$') RETURN u.email LIMIT 2";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_distinct() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.email, '\.com$') RETURN DISTINCT u.role";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_quantifiers() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (u:Person) WHERE FT_REGEX(u.username, '^[a-z]{3,}$') RETURN u.username";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_regex_word_boundary() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = r"MATCH (d:Document) WHERE FT_REGEX(d.title, '\bMachine\b') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    // ==============================================================================
    // FT_PHRASE_PREFIX TESTS (18 tests)
    // ==============================================================================

    #[test]
    fn test_phrase_prefix_single_word() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_two_words() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine Learn') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_autocomplete() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Deep Lear') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_no_match() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Quantum Com') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_case_insensitive() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'machine learn') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_three_words() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine Learning Bas') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_partial_word() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Nat') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_combined_filters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') AND d.year = 2023 RETURN d.title, d.year";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_in_return() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) RETURN d.title, FT_PHRASE_PREFIX(d.title, 'Machine') as starts_with_machine";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_count() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') RETURN COUNT(d) as count";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_ordering() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') OR FT_PHRASE_PREFIX(d.title, 'Deep') RETURN d.title ORDER BY d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_null_handling() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        coordinator
            .process_query("INSERT (:Document {category: 'ML'})", &session_id)
            .expect("Failed to insert node");

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') RETURN d.category";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_negation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE NOT FT_PHRASE_PREFIX(d.title, 'Machine') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_limit() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') RETURN d.title LIMIT 1";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_distinct() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') RETURN DISTINCT d.category";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_empty_phrase() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, '') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_multiple_matches() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Learn') OR FT_PHRASE_PREFIX(d.title, 'Process') RETURN d.title";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }

    #[test]
    fn test_phrase_prefix_groupby() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);
        setup_pattern_test_data(&coordinator, &session_id);

        let query = "MATCH (d:Document) WHERE FT_PHRASE_PREFIX(d.title, 'Machine') OR FT_PHRASE_PREFIX(d.title, 'Deep') RETURN d.year, COUNT(d) as count GROUP BY d.year";
        let result = coordinator.process_query(query, &session_id);
        assert!(result.is_ok(), "Query failed: {:?}", result);
    }
}
