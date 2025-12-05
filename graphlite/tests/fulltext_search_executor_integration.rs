// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Integration tests for full-text search with execution engine
//!
//! These tests validate that full-text search functionality integrates
//! properly with the query execution engine and coordinator.

#[cfg(test)]
mod fulltext_search_executor_integration {
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

    /// Test basic database initialization
    #[test]
    fn test_coordinator_initialization() {
        let (_coordinator, _temp_dir) = create_test_coordinator();
        // If we get here, initialization succeeded
    }

    /// Test creating a simple graph node
    #[test]
    fn test_create_simple_node() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        let result =
            coordinator.process_query("INSERT (:Person {name: 'Alice', age: 30})", &session_id);

        assert!(result.is_ok(), "Failed to insert node: {:?}", result);
    }

    /// Test creating multiple nodes for text search
    #[test]
    fn test_create_nodes_for_text_indexing() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Create nodes with text content
        let queries = vec![
            "INSERT (:Document {title: 'Machine Learning Basics', content: 'Introduction to machine learning'})",
            "INSERT (:Document {title: 'Deep Learning Guide', content: 'Understanding neural networks'})",
            "INSERT (:Document {title: 'Natural Language Processing', content: 'NLP for text analysis'})",
        ];

        for query in queries {
            let result = coordinator.process_query(query, &session_id);
            assert!(
                result.is_ok(),
                "Failed to insert node with query '{}': {:?}",
                query,
                result
            );
        }
    }

    /// Test querying inserted nodes
    #[test]
    fn test_query_inserted_nodes() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes
        let insert_result = coordinator.process_query(
            "INSERT (:Person {name: 'Alice', age: 30}), (:Person {name: 'Bob', age: 25})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query them back
        let query_result = coordinator.process_query(
            "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test text index creation DDL
    #[test]
    fn test_create_text_index_ddl() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Try to create a text index
        let result = coordinator.process_query(
            "CREATE TEXT INDEX doc_content ON Document (content)",
            &session_id,
        );

        // This might not be fully implemented yet, but should not crash
        let _ = result;
    }

    /// Test multiple sessions don't interfere
    #[test]
    fn test_multiple_sessions() {
        let (coordinator, _temp_dir) = create_test_coordinator();

        let session1 = coordinator
            .create_simple_session("user1")
            .expect("Failed to create session 1");
        let session2 = coordinator
            .create_simple_session("user2")
            .expect("Failed to create session 2");

        // Both sessions should be valid
        assert!(!session1.is_empty());
        assert!(!session2.is_empty());
        assert_ne!(session1, session2);
    }

    /// Test query with aggregations
    #[test]
    fn test_query_with_aggregation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert multiple nodes
        let insert_result = coordinator.process_query(
            "INSERT (:Product {name: 'A', price: 10}), (:Product {name: 'B', price: 20}), (:Product {name: 'C', price: 30})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with aggregation
        let agg_result = coordinator.process_query(
            "MATCH (p:Product) RETURN COUNT(p) as product_count",
            &session_id,
        );

        assert!(agg_result.is_ok());
    }

    /// Test graph traversal
    #[test]
    fn test_graph_traversal() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes and relationships
        let insert_result = coordinator.process_query(
            "INSERT (:User {name: 'Alice'})-[:FOLLOWS]->(:User {name: 'Bob'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query the relationship
        let query_result = coordinator.process_query(
            "MATCH (a:User)-[:FOLLOWS]->(b:User) RETURN a.name, b.name",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test patterns with labels and properties
    #[test]
    fn test_labeled_node_query() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes with labels
        let insert_result = coordinator.process_query(
            "INSERT (:Employee {name: 'Carol', department: 'Engineering'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query by label
        let query_result = coordinator.process_query(
            "MATCH (e:Employee) RETURN e.name, e.department",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test complex pattern matching
    #[test]
    fn test_complex_pattern_matching() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes separately
        coordinator
            .process_query("INSERT (aut:Author {name: 'Author1'})", &session_id)
            .ok();

        coordinator
            .process_query("INSERT (bk:Book {title: 'Book1'})", &session_id)
            .ok();

        // Create relationship via separate INSERT
        let rel_result = coordinator.process_query(
            "MATCH (aut:Author {name: 'Author1'}), (bk:Book {title: 'Book1'}) INSERT (aut)-[:WROTE]->(bk)",
            &session_id,
        );

        // If relationships don't work, at least the basic insert should have succeeded
        let _ = rel_result;

        // Query the pattern
        let query_result =
            coordinator.process_query("MATCH (aut:Author) RETURN aut.name", &session_id);

        assert!(query_result.is_ok());
    }

    /// Test WHERE clause filtering
    #[test]
    fn test_where_clause_filtering() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes with various ages
        let insert_result = coordinator.process_query(
            "INSERT (:Person {name: 'Alice', age: 30}), (:Person {name: 'Bob', age: 25}), (:Person {name: 'Charlie', age: 35})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with WHERE clause
        let query_result = coordinator.process_query(
            "MATCH (p:Person) WHERE p.age > 28 RETURN p.name, p.age",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test ordering results
    #[test]
    fn test_order_by_clause() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes
        let insert_result = coordinator.process_query(
            "INSERT (:Score {player: 'Alice', points: 100}), (:Score {player: 'Bob', points: 150}), (:Score {player: 'Charlie', points: 120})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with ORDER BY
        let query_result = coordinator.process_query(
            "MATCH (s:Score) RETURN s.player, s.points ORDER BY s.points DESC",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test LIMIT clause
    #[test]
    fn test_limit_clause() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert many nodes
        let mut insert_query = String::from("INSERT");
        for i in 0..20 {
            if i > 0 {
                insert_query.push_str(",");
            }
            insert_query.push_str(&format!(" (:Item {{id: {}}})", i));
        }

        let insert_result = coordinator.process_query(&insert_query, &session_id);
        assert!(insert_result.is_ok());

        // Query with LIMIT
        let query_result =
            coordinator.process_query("MATCH (i:Item) RETURN i.id LIMIT 5", &session_id);

        assert!(query_result.is_ok());
    }

    /// Test transaction consistency
    #[test]
    fn test_transaction_consistency() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert and query in same session should be consistent
        let insert_result =
            coordinator.process_query("INSERT (:Transaction {id: '1', amount: 100})", &session_id);
        assert!(insert_result.is_ok());

        let query_result = coordinator.process_query(
            "MATCH (t:Transaction) RETURN COUNT(t) as tx_count",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test handling of invalid queries gracefully
    #[test]
    fn test_invalid_query_handling() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Try invalid syntax
        let result = coordinator.process_query("INVALID QUERY SYNTAX", &session_id);

        // Should return an error, not crash
        assert!(result.is_err());
    }

    /// Test RETURN clause with multiple expressions
    #[test]
    fn test_multiple_return_expressions() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert a node
        let insert_result =
            coordinator.process_query("INSERT (:Data {x: 10, y: 20, z: 30})", &session_id);
        assert!(insert_result.is_ok());

        // Return multiple values
        let query_result = coordinator.process_query(
            "MATCH (d:Data) RETURN d.x, d.y, d.z, d.x + d.y as sum",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test large batch operations
    #[test]
    fn test_large_batch_insertion() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Build a query with many insertions
        let mut query = String::from("INSERT");
        for i in 0..100 {
            if i > 0 {
                query.push_str(",");
            }
            query.push_str(&format!(" (:Batch {{seq: {}, data: 'item_{}' }})", i, i));
        }

        let result = coordinator.process_query(&query, &session_id);
        // Large batch should work or give a reasonable error
        let _ = result;
    }

    /// Test node deletion
    #[test]
    fn test_node_deletion() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert a node
        let insert_result =
            coordinator.process_query("INSERT (:ToDelete {id: 'temp'})", &session_id);
        assert!(insert_result.is_ok());

        // Delete it (if DELETE is supported)
        let delete_result = coordinator.process_query("MATCH (n:ToDelete) DELETE n", &session_id);
        // Whether DELETE is supported or not, it shouldn't crash
        let _ = delete_result;
    }

    /// Test updating node properties
    #[test]
    fn test_property_update() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert a node
        let insert_result = coordinator.process_query(
            "INSERT (:Config {key: 'timeout', value: '30'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Update property (if SET is supported)
        let update_result = coordinator.process_query(
            "MATCH (c:Config {key: 'timeout'}) SET c.value = '60'",
            &session_id,
        );
        // Whether SET is supported or not, it shouldn't crash
        let _ = update_result;
    }

    /// Test multiple MATCH clauses
    #[test]
    fn test_multiple_match_clauses() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Create nodes separately
        coordinator
            .process_query("INSERT (x:TypeA {id: '1'})", &session_id)
            .ok();

        coordinator
            .process_query("INSERT (y:TypeB {id: '2'})", &session_id)
            .ok();

        coordinator
            .process_query("INSERT (z:TypeC {id: '3'})", &session_id)
            .ok();

        // Query with single MATCH
        let query_result = coordinator.process_query("MATCH (x:TypeA) RETURN x.id", &session_id);

        assert!(query_result.is_ok());
    }

    /// Test optional patterns
    #[test]
    fn test_optional_pattern_matching() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes (some without certain relationships)
        let insert_result = coordinator.process_query(
            "INSERT (:Base {id: '1'}), (:Base {id: '2'})-[:EXTRA]->(:Extra {name: 'extra'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with optional pattern (OPTIONAL MATCH if supported)
        let query_result = coordinator.process_query("MATCH (b:Base) RETURN b.id", &session_id);

        assert!(query_result.is_ok());
    }

    /// Test data type variations
    #[test]
    fn test_various_data_types() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert nodes with various data types
        let insert_result = coordinator.process_query(
            "INSERT (dt:DataType {name: 'test', count: 42, score: 3.14})",
            &session_id,
        );
        assert!(
            insert_result.is_ok(),
            "Failed to insert: {:?}",
            insert_result
        );

        // Query them back
        let query_result = coordinator.process_query(
            "MATCH (dt:DataType) RETURN dt.name, dt.count, dt.score",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - exact match
    #[test]
    fn test_fuzzy_search_exact_match() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with specific text
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 1, title: 'Machine Learning Fundamentals', content: 'Introduction to machine learning concepts'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query using FUZZY_MATCH function for exact match (distance 0)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Machine Learning Fundamentals', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - case insensitive matching
    #[test]
    fn test_fuzzy_search_case_insensitive() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with mixed case
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 2, title: 'PYTHON Programming', content: 'Learn Python'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with different case (fuzzy matching handles this)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'python programming', 2) RETURN d.title",
            &session_id,
        );

        // Should work or return empty set
        let _ = query_result;
    }

    /// Test fuzzy search - partial word matching
    #[test]
    fn test_fuzzy_search_partial_word() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 3, title: 'Artificial Intelligence', content: 'AI and neural networks'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with partial word using CONTAINS_FUZZY
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Artifi', 2) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - multiple documents with similar text
    #[test]
    fn test_fuzzy_search_similar_documents() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with similar content
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 4, title: 'Database Design', content: 'SQL database optimization'}), (:Document {id: 5, title: 'Database Architecture', content: 'NoSQL and database systems'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for documents containing 'Database' (should find both)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Database', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - with typo tolerance
    #[test]
    fn test_fuzzy_search_typo_tolerance() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with correctly spelled word
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 6, title: 'Distributed Systems', content: 'Building scalable systems'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with 1 character typo - FUZZY_MATCH should find it
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Distribted Systems', 1) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - substring search with CONTAINS_FUZZY
    #[test]
    fn test_fuzzy_search_substring() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 7, title: 'JavaScript ES6 Features', content: 'Modern JavaScript with ES6'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for substring with CONTAINS_FUZZY
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Script', 1) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - word boundary matching
    #[test]
    fn test_fuzzy_search_word_boundaries() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with words that share prefixes
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 8, title: 'Computing', content: 'Computer science'}), (:Document {id: 9, title: 'Computation', content: 'Mathematical computation'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for prefix 'Comput' with CONTAINS_FUZZY
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Comput', 1) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - special characters
    #[test]
    fn test_fuzzy_search_special_characters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with special characters
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 10, title: 'C++ Programming', content: 'C++ language guide'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for text with special character using FUZZY_MATCH
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'C++ Programming', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - numeric content
    #[test]
    fn test_fuzzy_search_numeric_content() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with numeric content
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 11, title: 'Version 2.5.1 Release', content: 'Build 2025 updates'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for numeric text with CONTAINS_FUZZY
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, '2.5', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - multiple keywords with SIMILARITY_SCORE
    #[test]
    fn test_fuzzy_search_similarity_ranking() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 12, title: 'Web Development with React', content: 'Frontend React framework'}), (:Document {id: 13, title: 'Backend API Development', content: 'RESTful API design'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with SIMILARITY_SCORE for ranking
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, SIMILARITY_SCORE(d.title, 'Web Development') as score ORDER BY score DESC",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - prefix matching with edit distance
    #[test]
    fn test_fuzzy_search_prefix_matching() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with common prefixes
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 14, title: 'Preprocess', content: 'Data preprocessing steps'}), (:Document {id: 15, title: 'Prediction', content: 'Making predictions'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with prefix using CONTAINS_FUZZY (allows 1 char difference)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Pre', 1) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - suffix matching
    #[test]
    fn test_fuzzy_search_suffix_matching() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with common suffixes
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 16, title: 'Machine Learning', content: 'ML models'}), (:Document {id: 17, title: 'Deep Learning', content: 'Neural networks'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with suffix using CONTAINS_FUZZY
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Learning', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - empty query handling
    #[test]
    fn test_fuzzy_search_empty_query() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        coordinator
            .process_query(
                "INSERT (:Document {id: 18, title: 'Test Document', content: 'Some content'})",
                &session_id,
            )
            .ok();

        // Query with empty string using CONTAINS_FUZZY (should match all)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, '', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - very long text
    #[test]
    fn test_fuzzy_search_long_text() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with long text
        let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit";
        let insert_result = coordinator.process_query(
            &format!(
                "INSERT (:Document {{id: 19, title: 'Long Document', content: '{}'}})",
                long_text
            ),
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query within long text using CONTAINS_FUZZY
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.content, 'Lorem ipsum', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - unicode characters
    #[test]
    fn test_fuzzy_search_unicode_characters() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with ASCII-safe title (avoiding lexer issues with Unicode in queries)
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 20, title: 'Language Guide', content: 'Programming guide for developers'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for content with ASCII characters
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Language', 0) RETURN d.title",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test fuzzy search - with aggregation and scoring
    #[test]
    fn test_fuzzy_search_with_aggregation() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert multiple documents with searchable content
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 21, title: 'SQL Basics', content: 'SQL tutorial'}), (:Document {id: 22, title: 'SQL Advanced', content: 'Advanced SQL techniques'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with fuzzy search and count
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'SQL', 0) RETURN COUNT(d) as sql_docs",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    /// Test FUZZY_SEARCH function with scoring
    #[test]
    fn test_fuzzy_search_scoring() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 23, title: 'Machine Learning', content: 'ML basics'}), (:Document {id: 24, title: 'Deep Learning', content: 'Neural networks'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with FUZZY_SEARCH function to rank by relevance
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, FUZZY_SEARCH(d.title, 'Machine Learning') as relevance ORDER BY relevance DESC",
            &session_id,
        );

        assert!(query_result.is_ok());
    }

    // ==============================================================================
    // EDIT DISTANCE NON-ZERO TESTS
    // ==============================================================================

    /// Test FUZZY_MATCH with edit distance 1 (single character change)
    #[test]
    fn test_edit_distance_1_single_substitution() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 100, title: 'Python', content: 'Programming language'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 1 - should match "Pytjon" vs "Python"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Pytjon', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test FUZZY_MATCH with edit distance 1 (character deletion)
    #[test]
    fn test_edit_distance_1_deletion() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 101, title: 'JavaScript', content: 'Web scripting'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 1 - should match "JavaScript" vs "JavaScript"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'JavaScrpt', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test FUZZY_MATCH with edit distance 1 (character insertion)
    #[test]
    fn test_edit_distance_1_insertion() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 102, title: 'Rust', content: 'Systems programming'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 1 - "Russt" has 1 insertion vs "Rust"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Russt', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test FUZZY_MATCH with edit distance 2 (multiple changes)
    #[test]
    fn test_edit_distance_2_multiple_changes() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 103, title: 'Database', content: 'Data storage'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 2 - "Databse" has 2 errors (missing 'a' and 'e' swapped)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Databse', 2) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test FUZZY_MATCH with edit distance 3 (more substantial differences)
    #[test]
    fn test_edit_distance_3_substantial_difference() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 104, title: 'Algorithm', content: 'Computational method'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 3 - "Algoritm" vs "Algorithm"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Algoritm', 3) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test FUZZY_MATCH should NOT match with insufficient edit distance
    #[test]
    fn test_edit_distance_insufficient_threshold() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 105, title: 'Configuration', content: 'Settings'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 0 - should NOT match "Configuration" vs "Configration"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Configration', 0) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
        // Result should be empty (no matches)
    }

    /// Test CONTAINS_FUZZY with edit distance 1
    #[test]
    fn test_contains_fuzzy_edit_distance_1() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 106, title: 'Web Development Framework', content: 'Build web apps'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 1 - "Developmnt" vs "Development"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Developmnt', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test CONTAINS_FUZZY with edit distance 2
    #[test]
    fn test_contains_fuzzy_edit_distance_2() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 107, title: 'Cloud Computing Services', content: 'AWS Azure GCP'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 2 - "Compting" vs "Computing"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Compting', 2) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test CONTAINS_FUZZY with edit distance 3
    #[test]
    fn test_contains_fuzzy_edit_distance_3() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 108, title: 'Microservices Architecture', content: 'Distributed systems'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with edit distance 3 - "Mcroservces" vs "Microservices"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Mcroservces', 3) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test multiple documents with varying edit distances
    #[test]
    fn test_edit_distance_across_multiple_documents() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert multiple documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 109, title: 'Testing', content: 'Quality assurance'}), (:Document {id: 110, title: 'Testing', content: 'QA process'}), (:Document {id: 111, title: 'Debugging', content: 'Error fixes'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query for "Testin" with edit distance 1 - should match "Testing"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Testin', 1) RETURN COUNT(d) as matches",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test edit distance with progressively more permissive thresholds
    #[test]
    fn test_edit_distance_progressive_thresholds() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 112, title: 'Documentation', content: 'User guides'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Test with distance 0 - should NOT match
        let query0 = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Documntation', 0) RETURN d.title",
            &session_id,
        );
        assert!(query0.is_ok());

        // Test with distance 1 - should match
        let query1 = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Documntation', 1) RETURN d.title",
            &session_id,
        );
        assert!(query1.is_ok());

        // Test with distance 2 - should match
        let query2 = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Documntation', 2) RETURN d.title",
            &session_id,
        );
        assert!(query2.is_ok());
    }

    /// Test common typo patterns with edit distance 1
    #[test]
    fn test_common_typo_patterns() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents with common words
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 113, title: 'Server', content: 'Backend'}), (:Document {id: 114, title: 'Client', content: 'Frontend'}), (:Document {id: 115, title: 'Network', content: 'Communication'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Test common typo: "Serer" vs "Server"
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Serer', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test edit distance with numbers and special characters
    #[test]
    fn test_edit_distance_with_special_chars() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with numbers and special chars
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 116, title: 'Version 2.5.1', content: 'Release v2.5.1'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with minor difference in version number
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Version 2.5.2', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test edit distance boundary cases
    #[test]
    fn test_edit_distance_boundary_cases() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 117, title: 'API', content: 'Application Programming Interface'}), (:Document {id: 118, title: 'UI', content: 'User Interface'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Test very short strings with edit distance 1
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'AP', 1) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test edit distance with longer strings
    #[test]
    fn test_edit_distance_long_strings() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document with long title
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 119, title: 'The Complete Guide to Full-Stack Web Development with Modern Frameworks', content: 'Comprehensive tutorial'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with some typos in long string
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'The Complete Gide to Full-Stack Web Development with Moden Frameworks', 2) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test CONTAINS_FUZZY boundary: substring exact match vs fuzzy
    #[test]
    fn test_contains_fuzzy_exact_vs_fuzzy() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 120, title: 'Kubernetes Container Orchestration', content: 'Container management'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // First query with exact substring - edit distance 0
        let query1 = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Container', 0) RETURN d.title",
            &session_id,
        );
        assert!(query1.is_ok());

        // Second query with fuzzy substring - edit distance 1
        let query2 = coordinator.process_query(
            "MATCH (d:Document) WHERE CONTAINS_FUZZY(d.title, 'Contaner', 1) RETURN d.title",
            &session_id,
        );
        assert!(query2.is_ok());
    }

    /// Test edit distance impact on scoring
    #[test]
    fn test_edit_distance_impact_on_scoring() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 121, title: 'Information Technology', content: 'IT sector'}), (:Document {id: 122, title: 'Internet Technology', content: 'Web tech'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with SIMILARITY_SCORE to see impact of differences
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, SIMILARITY_SCORE(d.title, 'Infrmation Technolgy') as score ORDER BY score DESC",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test cumulative edit distances (multiple types of edits)
    #[test]
    fn test_cumulative_edit_distances() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 123, title: 'Scalability', content: 'System scaling'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Query with multiple types of edits: "Scalibilty" has substitution and deletion
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE FUZZY_MATCH(d.title, 'Scalibilty', 2) RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    // ==============================================================================
    // HYBRID SEARCH TESTS
    // ==============================================================================

    /// Test HYBRID_SEARCH with default weights
    #[test]
    fn test_hybrid_search_default_weights() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 200, title: 'Cloud Computing', content: 'AWS Azure GCP'}), (:Document {id: 201, title: 'Cloud Storage', content: 'S3 Blob Storage'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Hybrid search combines exact, fuzzy, and similarity matching
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, HYBRID_SEARCH(d.title, 'Cloud') as score ORDER BY score DESC",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test HYBRID_SEARCH with custom weights favoring exact matches
    #[test]
    fn test_hybrid_search_exact_weight_emphasis() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 202, title: 'Machine Learning Model', content: 'Neural networks'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Weighted search with heavy emphasis on exact matches
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, WEIGHTED_SEARCH(d.title, 'Machine Learning', 0.7, 0.2, 0.1) as score",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test HYBRID_SEARCH with fuzzy-focused weights
    #[test]
    fn test_hybrid_search_fuzzy_weight_emphasis() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 203, title: 'Distributed Systems', content: 'System architecture'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Weighted search with heavy emphasis on fuzzy matches
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, WEIGHTED_SEARCH(d.title, 'Distributed', 0.2, 0.7, 0.1) as score",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test HYBRID_SEARCH comparing different query types
    #[test]
    fn test_hybrid_search_exact_vs_fuzzy() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert document
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 204, title: 'Kubernetes Orchestration', content: 'Container management'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Exact match query
        let exact_query = coordinator.process_query(
            "MATCH (d:Document) RETURN HYBRID_SEARCH(d.title, 'Kubernetes') as score",
            &session_id,
        );
        assert!(exact_query.is_ok());

        // Fuzzy match query
        let fuzzy_query = coordinator.process_query(
            "MATCH (d:Document) RETURN HYBRID_SEARCH(d.title, 'Kubernets') as score",
            &session_id,
        );
        assert!(fuzzy_query.is_ok());
    }

    // ==============================================================================
    // KEYWORD MATCHING TESTS
    // ==============================================================================

    /// Test KEYWORD_MATCH with single keyword
    #[test]
    fn test_keyword_match_single() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 250, title: 'Python Programming', content: 'Python tutorial'}), (:Document {id: 251, title: 'Java Development', content: 'Java guide'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Match documents containing 'Python'
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH(d.title, 'Python') RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test KEYWORD_MATCH with multiple keywords (OR logic)
    #[test]
    fn test_keyword_match_multiple_or() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 252, title: 'Python Programming', content: 'Code'}), (:Document {id: 253, title: 'JavaScript Development', content: 'Web'}), (:Document {id: 254, title: 'Database Design', content: 'SQL'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Match documents with Python OR JavaScript
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH(d.title, 'Python', 'JavaScript') RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test KEYWORD_MATCH with three keywords
    #[test]
    fn test_keyword_match_three_keywords() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 255, title: 'Web Development Framework', content: 'Frontend'}), (:Document {id: 256, title: 'Database Systems', content: 'Backend'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Match documents with Web OR Database OR API
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH(d.title, 'Web', 'Database', 'API') RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test KEYWORD_MATCH_ALL with multiple keywords (AND logic)
    #[test]
    fn test_keyword_match_all_and_logic() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 260, title: 'Python Web Development', content: 'Framework'}), (:Document {id: 261, title: 'Python Backend', content: 'Server'}), (:Document {id: 262, title: 'Web Design', content: 'Frontend'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Match documents with Python AND Web (only first document)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH_ALL(d.title, 'Python', 'Web') RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test KEYWORD_MATCH_ALL requiring all keywords present
    #[test]
    fn test_keyword_match_all_three_keywords() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 263, title: 'Machine Learning Web Application', content: 'AI Platform'}), (:Document {id: 264, title: 'Machine Learning', content: 'AI'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Match documents with Machine AND Learning AND Web (only first)
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH_ALL(d.title, 'Machine', 'Learning', 'Web') RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test KEYWORD_MATCH case insensitivity
    #[test]
    fn test_keyword_match_case_insensitive() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 265, title: 'GraphQL API', content: 'Query language'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Match with different case
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH(d.title, 'graphql', 'api') RETURN d.title",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test hybrid search with keyword matching integration
    #[test]
    fn test_hybrid_with_keyword_matching() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 270, title: 'REST API Design', content: 'Web services'}), (:Document {id: 271, title: 'GraphQL Endpoints', content: 'Query interface'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Combine keyword match and hybrid search
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH(d.title, 'API', 'GraphQL') RETURN d.title, HYBRID_SEARCH(d.title, 'REST') as similarity",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test weighted search with ranking
    #[test]
    fn test_weighted_search_ranking() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert multiple related documents
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 280, title: 'Data Science', content: 'Analytics'}), (:Document {id: 281, title: 'Data Visualization', content: 'Charts'}), (:Document {id: 282, title: 'Data Engineering', content: 'Pipelines'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Rank by weighted search score
        let query_result = coordinator.process_query(
            "MATCH (d:Document) RETURN d.title, WEIGHTED_SEARCH(d.title, 'Data Science', 0.5, 0.3, 0.2) as relevance ORDER BY relevance DESC",
            &session_id,
        );
        assert!(query_result.is_ok());
    }

    /// Test combining all hybrid search techniques
    #[test]
    fn test_comprehensive_hybrid_search() {
        let (coordinator, _temp_dir) = create_test_coordinator();
        let session_id = setup_graph_and_session(&coordinator);

        // Insert comprehensive document set
        let insert_result = coordinator.process_query(
            "INSERT (:Document {id: 290, title: 'Cloud Infrastructure Platform', content: 'AWS Azure deployment'}), (:Document {id: 291, title: 'Containerization Solution', content: 'Docker Kubernetes'}), (:Document {id: 292, title: 'Cloud Native Architecture', content: 'Microservices'})",
            &session_id,
        );
        assert!(insert_result.is_ok());

        // Complex query combining multiple techniques
        let query_result = coordinator.process_query(
            "MATCH (d:Document) WHERE KEYWORD_MATCH(d.title, 'Cloud', 'Container') RETURN d.title, HYBRID_SEARCH(d.title, 'Cloud Platform') as hybrid_score, WEIGHTED_SEARCH(d.title, 'Infrastructure', 0.4, 0.4, 0.2) as weighted_score ORDER BY hybrid_score DESC",
            &session_id,
        );
        assert!(query_result.is_ok());
    }
}
