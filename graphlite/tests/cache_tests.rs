//! Tests for cache clearing system procedures and data persistence
//!
//! Tests cache clearing (gql.clear_cache) and cache statistics (gql.cache_stats)
//! Also tests data persistence across cache clears and sessions
//!
//! Note: All tests use isolated fixtures to avoid race conditions when running in parallel

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

#[test]
fn test_cache_clearing_procedure() {
    log::debug!("🧪 Testing cache clearing procedure");

    // Use isolated fixture instead of shared one to avoid cache clearing interference
    // when tests run in parallel
    let fixture = TestFixture::new().expect("Failed to create test fixture");

    // Create test graph - use a completely separate schema to avoid conflicts
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_schema = format!("cache_test_schema_{}", unique_id);
    let test_graph = format!("{}/cache_test_graph", test_schema);

    fixture
        .query(&format!("CREATE SCHEMA IF NOT EXISTS {}", test_schema))
        .unwrap();
    fixture
        .query(&format!("SESSION SET SCHEMA {}", test_schema))
        .unwrap();
    fixture
        .query(&format!("CREATE GRAPH {}", test_graph))
        .unwrap();
    fixture
        .query(&format!("SESSION SET GRAPH {}", test_graph))
        .unwrap();

    log::debug!("  📊 Setting up test data...");

    // Insert test data
    fixture.assert_query_succeeds("INSERT (:TestNode {id: 1, name: 'cache_test_node1'})");
    fixture.assert_query_succeeds("INSERT (:TestNode {id: 2, name: 'cache_test_node2'})");
    fixture.assert_query_succeeds(
        "MATCH (n1:TestNode {id: 1}), (n2:TestNode {id: 2}) INSERT (n1)-[:TEST_EDGE {type: 'cache_test'}]->(n2)"
    );

    // Query data to ensure it gets cached
    log::debug!("  📋 Querying data to populate cache...");
    fixture.assert_query_succeeds("MATCH (n:TestNode) RETURN n.id, n.name ORDER BY n.id");
    fixture.assert_query_succeeds(
        "MATCH (n1:TestNode)-[r:TEST_EDGE]->(n2:TestNode) RETURN n1.name, n2.name",
    );

    // Check cache stats before clearing
    log::debug!("  📈 Checking cache stats before clearing...");
    let cache_stats_before = fixture.query("CALL gql.cache_stats()").unwrap();
    log::debug!(
        "    Cache stats before: {} rows",
        cache_stats_before.rows.len()
    );
    assert!(
        !cache_stats_before.rows.is_empty(),
        "Should have cache stats"
    );

    // Find storage cache entry
    let mut storage_cache_found = false;
    for row in &cache_stats_before.rows {
        if let Some(Value::String(cache_type)) = row.values.get("cache_type") {
            if cache_type == "storage_cache" {
                storage_cache_found = true;
                if let Some(Value::Number(entries)) = row.values.get("entries") {
                    log::debug!("    Storage cache entries before clear: {}", entries);
                    assert!(
                        *entries >= 0.0,
                        "Storage cache should have non-negative entries"
                    );
                }
            }
        }
    }
    assert!(storage_cache_found, "Should find storage_cache in stats");

    // Clear all caches
    log::debug!("  🧹 Clearing caches...");
    let clear_result = fixture.query("CALL gql.clear_cache()").unwrap();
    log::debug!("    Clear result: {} rows", clear_result.rows.len());
    assert!(!clear_result.rows.is_empty(), "Should have clear result");

    // Verify clear result
    let clear_row = &clear_result.rows[0];
    if let Some(Value::String(status)) = clear_row.values.get("status") {
        log::debug!("    Clear status: {}", status);
        assert!(
            status == "success" || status == "partial",
            "Clear should succeed"
        );
    }

    if let Some(Value::String(cleared_caches)) = clear_row.values.get("cleared_caches") {
        log::debug!("    Cleared caches: {}", cleared_caches);
        assert!(
            cleared_caches.contains("storage_cache"),
            "Should clear storage_cache"
        );
    }

    // Query data again after clearing cache - this should still work (from storage)
    log::debug!("  🔄 Verifying data persistence after cache clear...");
    let result_after_clear = fixture
        .query("MATCH (n:TestNode) RETURN n.id, n.name ORDER BY n.id")
        .unwrap();

    assert_eq!(
        result_after_clear.rows.len(),
        2,
        "Should still find 2 nodes after cache clear"
    );

    // Verify the data is correct
    let first_node = &result_after_clear.rows[0];
    if let (Some(Value::Number(id)), Some(Value::String(name))) = (
        first_node.values.get("n.id"),
        first_node.values.get("n.name"),
    ) {
        assert_eq!(*id, 1.0);
        assert_eq!(name, "cache_test_node1");
    } else {
        panic!(
            "First node should have id=1 and name='cache_test_node1', got: {:?}",
            first_node.values
        );
    }

    // Query relationships after cache clear
    let rel_result_after_clear = fixture
        .query("MATCH (n1:TestNode)-[r:TEST_EDGE]->(n2:TestNode) RETURN n1.name, n2.name")
        .unwrap();

    assert_eq!(
        rel_result_after_clear.rows.len(),
        1,
        "Should still find 1 relationship after cache clear"
    );

    // Check cache stats after clearing and re-querying
    log::debug!("  📈 Checking cache stats after clearing and re-querying...");
    let cache_stats_after = fixture.query("CALL gql.cache_stats()").unwrap();
    assert!(
        !cache_stats_after.rows.is_empty(),
        "Should have cache stats after clear"
    );

    log::debug!("  ✅ Cache clearing procedure test passed!");
}

#[test]
fn test_data_persistence_across_sessions() {
    log::debug!("🔄 Testing data persistence across sessions");

    // Use isolated fixture to avoid interference from parallel tests
    // Session 1: Create data
    log::debug!("  📝 Session 1: Creating data...");
    let fixture1 = TestFixture::new().expect("Failed to create test fixture");

    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Create graph and data in session 1 - use separate schema
    let test_schema = format!("persistence_test_schema_{}", unique_id);
    let test_graph = format!("{}/persistence_test_graph", test_schema);

    fixture1
        .query(&format!("CREATE SCHEMA IF NOT EXISTS {}", test_schema))
        .unwrap();
    fixture1
        .query(&format!("SESSION SET SCHEMA {}", test_schema))
        .unwrap();
    fixture1
        .query(&format!("CREATE GRAPH {}", test_graph))
        .unwrap();
    fixture1
        .query(&format!("SESSION SET GRAPH {}", test_graph))
        .unwrap();

    // Insert test data step by step
    fixture1.assert_query_succeeds("INSERT (:Person {id: 1, name: 'Alice', age: 30})");
    fixture1.assert_query_succeeds("INSERT (:Person {id: 2, name: 'Bob', age: 25})");
    fixture1.assert_query_succeeds("INSERT (:Company {name: 'TechCorp', industry: 'Technology'})");
    fixture1.assert_query_succeeds(
        "MATCH (person1:Person {name: 'Alice'}), (company:Company {name: 'TechCorp'}) 
         INSERT (person1)-[:WORKS_FOR {position: 'Engineer', salary: 85000}]->(company)",
    );
    fixture1.assert_query_succeeds(
        "MATCH (person2:Person {name: 'Bob'}), (company:Company {name: 'TechCorp'}) 
         INSERT (person2)-[:WORKS_FOR {position: 'Designer', salary: 70000}]->(company)",
    );

    // Verify data exists in session 1
    let session1_result = fixture1
        .query("MATCH (p:Person) RETURN p.id, p.name, p.age ORDER BY p.id")
        .unwrap();
    assert_eq!(
        session1_result.rows.len(),
        2,
        "Session 1 should have 2 people"
    );

    // Clear cache in session 1
    log::debug!("  🧹 Session 1: Clearing cache...");
    fixture1.query("CALL gql.clear_cache()").unwrap();

    // Verify data still exists in session 1 after cache clear
    let session1_after_clear = fixture1
        .query("MATCH (p:Person) RETURN p.id, p.name ORDER BY p.id")
        .unwrap();
    assert_eq!(
        session1_after_clear.rows.len(),
        2,
        "Session 1 should still have 2 people after cache clear"
    );

    log::debug!("  📊 Session 1 complete - data persisted through cache clear");

    // Session 2: Access same data (simulating new session by setting graph again)
    // Note: Using same fixture (not truly separate session, but tests persistence within same instance)
    log::debug!("  🔄 Session 2: Accessing persisted data...");

    // Set same graph in session 2 (data should be persisted)
    fixture1
        .query(&format!("SESSION SET SCHEMA {}", test_schema))
        .unwrap();
    fixture1
        .query(&format!("SESSION SET GRAPH {}", test_graph))
        .unwrap();

    // Query data in session 2 - should find persisted data
    let session2_result = fixture1
        .query("MATCH (p:Person) RETURN p.id, p.name, p.age ORDER BY p.id")
        .unwrap();

    assert_eq!(
        session2_result.rows.len(),
        2,
        "Session 2 should find 2 persisted people"
    );

    // Verify the actual data values are correct
    let person1 = &session2_result.rows[0];
    let person2 = &session2_result.rows[1];

    if let (Some(Value::Number(id1)), Some(Value::String(name1))) =
        (person1.values.get("p.id"), person1.values.get("p.name"))
    {
        assert_eq!(*id1, 1.0);
        assert_eq!(name1, "Alice");
    } else {
        panic!(
            "Person 1 data not found correctly in session 2, got: {:?}",
            person1.values
        );
    }

    if let (Some(Value::Number(id2)), Some(Value::String(name2))) =
        (person2.values.get("p.id"), person2.values.get("p.name"))
    {
        assert_eq!(*id2, 2.0);
        assert_eq!(name2, "Bob");
    } else {
        panic!(
            "Person 2 data not found correctly in session 2, got: {:?}",
            person2.values
        );
    }

    // Test relationships persist across sessions
    let session2_rel_result = fixture1
        .query(
            "MATCH (p:Person)-[w:WORKS_FOR]->(c:Company)
         RETURN p.name, w.position, w.salary, c.name
         ORDER BY p.name",
        )
        .unwrap();

    assert_eq!(
        session2_rel_result.rows.len(),
        2,
        "Session 2 should find 2 work relationships"
    );

    // Verify relationship data
    let alice_work = &session2_rel_result.rows[0];
    if let (Some(Value::String(name)), Some(Value::String(position))) = (
        alice_work.values.get("p.name"),
        alice_work.values.get("w.position"),
    ) {
        assert_eq!(name, "Alice");
        assert_eq!(position, "Engineer");
    }

    // Clear cache in session 2
    log::debug!("  🧹 Session 2: Clearing cache...");
    fixture1.query("CALL gql.clear_cache()").unwrap();

    // Add new data in session 2
    log::debug!("  ➕ Session 2: Adding new data...");
    fixture1.assert_query_succeeds("INSERT (:Person {id: 3, name: 'Charlie', age: 28})");
    fixture1.assert_query_succeeds(
        "MATCH (person3:Person {name: 'Charlie'}), (company:Company {name: 'TechCorp'})
         INSERT (person3)-[:WORKS_FOR {position: 'Manager', salary: 95000}]->(company)",
    );

    // Verify all 3 people exist in session 2
    let session2_final = fixture1
        .query("MATCH (p:Person) RETURN p.id, p.name ORDER BY p.id")
        .unwrap();
    assert_eq!(
        session2_final.rows.len(),
        3,
        "Session 2 should now have 3 people total"
    );

    log::debug!("  ✅ Cross-session persistence test passed!");

    // Session 3: Verify session 2's additions persisted
    log::debug!("  🔍 Session 3: Verifying session 2's additions persisted...");

    fixture1
        .query(&format!("SESSION SET SCHEMA {}", test_schema))
        .unwrap();
    fixture1
        .query(&format!("SESSION SET GRAPH {}", test_graph))
        .unwrap();

    let session3_result = fixture1
        .query("MATCH (p:Person) RETURN p.id, p.name ORDER BY p.id")
        .unwrap();

    assert_eq!(
        session3_result.rows.len(),
        3,
        "Session 3 should find all 3 people from previous sessions"
    );

    // Verify Charlie from session 2 is there
    let charlie = &session3_result.rows[2];
    if let (Some(Value::Number(id)), Some(Value::String(name))) =
        (charlie.values.get("p.id"), charlie.values.get("p.name"))
    {
        assert_eq!(*id, 3.0);
        assert_eq!(name, "Charlie");
    } else {
        panic!(
            "Charlie (added in session 2) not found in session 3, got: {:?}",
            charlie.values
        );
    }

    log::debug!("  ✅ Multi-session persistence test passed!");
}

#[test]
fn test_cache_stats_procedure() {
    log::debug!("📊 Testing cache stats procedure");

    // Use isolated fixture to avoid interference from cache clearing in parallel tests
    let fixture = TestFixture::new().expect("Failed to create test fixture");

    // Create test graph - use separate schema to avoid conflicts
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_schema = format!("cache_stats_schema_{}", unique_id);
    let test_graph = format!("{}/cache_stats_test", test_schema);

    fixture
        .query(&format!("CREATE SCHEMA IF NOT EXISTS {}", test_schema))
        .unwrap();
    fixture
        .query(&format!("SESSION SET SCHEMA {}", test_schema))
        .unwrap();
    fixture
        .query(&format!("CREATE GRAPH {}", test_graph))
        .unwrap();
    fixture
        .query(&format!("SESSION SET GRAPH {}", test_graph))
        .unwrap();

    // Insert some data
    fixture.assert_query_succeeds("INSERT (:StatsTestNode {id: 1, data: 'test1'})");
    fixture.assert_query_succeeds("INSERT (:StatsTestNode {id: 2, data: 'test2'})");

    // Query to populate cache
    fixture.assert_query_succeeds("MATCH (n:StatsTestNode) RETURN n.id, n.data");

    // Test cache stats procedure
    let stats_result = fixture.query("CALL gql.cache_stats()").unwrap();

    log::debug!("  📈 Cache stats returned {} rows", stats_result.rows.len());
    assert!(
        !stats_result.rows.is_empty(),
        "Cache stats should return at least one row"
    );

    // Verify expected columns are present
    assert!(
        stats_result.variables.contains(&"cache_type".to_string()),
        "Should have cache_type column"
    );
    assert!(
        stats_result.variables.contains(&"entries".to_string()),
        "Should have entries column"
    );

    // Look for storage cache row
    let mut found_storage_cache = false;
    for row in &stats_result.rows {
        if let Some(Value::String(cache_type)) = row.values.get("cache_type") {
            log::debug!("    Found cache type: {}", cache_type);

            if cache_type == "storage_cache" {
                found_storage_cache = true;

                // Verify storage cache has some entries (should have at least our test graph)
                if let Some(Value::Number(entries)) = row.values.get("entries") {
                    log::debug!("      Storage cache entries: {}", entries);
                    assert!(
                        *entries >= 0.0,
                        "Storage cache entries should be non-negative"
                    );
                } else {
                    panic!("Storage cache should have entries field");
                }

                // Check other fields exist (may be N/A for storage cache)
                assert!(
                    row.values.contains_key("hit_rate"),
                    "Should have hit_rate field"
                );
                assert!(
                    row.values.contains_key("memory_bytes"),
                    "Should have memory_bytes field"
                );
            }
        }
    }

    assert!(
        found_storage_cache,
        "Should find storage_cache in cache stats"
    );

    // Test stats consistency after cache operations
    fixture.query("CALL gql.clear_cache()").unwrap();

    let stats_after_clear = fixture.query("CALL gql.cache_stats()").unwrap();
    assert!(
        !stats_after_clear.rows.is_empty(),
        "Cache stats should work after clear"
    );

    log::debug!("  ✅ Cache stats procedure test passed!");
}

#[test]
fn test_is_valid_procedure() {
    log::debug!("🔍 Testing system procedure validation");

    // Use isolated fixture for consistency (this test doesn't need fraud data)
    let fixture = TestFixture::new().expect("Failed to create test fixture");

    // Test all valid procedures - they should all execute without "not found" errors
    let valid_procedures = vec![
        "gql.list_schemas",
        "gql.list_graphs",
        "gql.list_graph_types",
        "gql.list_functions",
        "gql.list_roles",
        "gql.list_users",
        "gql.show_session",
        "gql.cache_stats",
        "gql.clear_cache",
    ];

    log::debug!("  ✅ Testing {} valid procedures", valid_procedures.len());

    for proc_name in &valid_procedures {
        log::debug!("    Testing: {}", proc_name);

        // Each procedure should execute without "not found" error
        // Some may fail with other errors (like missing arguments), but they should be recognized
        let result = fixture.query(&format!("CALL {}()", proc_name));

        // Check that we don't get "not found" or "not supported" error
        if let Err(err_msg) = result {
            let err_str = err_msg.to_string();
            assert!(
                !err_str.contains("not found") && !err_str.contains("not supported"),
                "Procedure {} should be recognized as valid, but got error: {}",
                proc_name,
                err_str
            );
            // It's OK if it fails for other reasons (like validation errors)
            log::debug!(
                "      ✓ {} recognized (may have validation errors, which is OK)",
                proc_name
            );
        } else {
            log::debug!("      ✓ {} executed successfully", proc_name);
        }
    }

    // Test invalid procedures - should get error (may be "not found" or other execution error)
    let invalid_procedures = vec![
        "gql.invalid_procedure",
        "gql.drop_database", // Dangerous operation not exposed
        "gql.shutdown",      // System operation not exposed
        "system.invalid",
    ];

    log::debug!(
        "  ❌ Testing {} invalid procedures (should fail)",
        invalid_procedures.len()
    );

    for proc_name in &invalid_procedures {
        log::debug!("    Testing: {}", proc_name);

        let result = fixture.query(&format!("CALL {}()", proc_name));

        // Should get an error (parser may accept it but execution should fail)
        assert!(
            result.is_err(),
            "Invalid procedure {} should fail",
            proc_name
        );

        let err_msg = result.unwrap_err().to_string();

        // Error may be "not found", "not supported", or other execution errors
        // The key is that it fails - we're verifying the procedure doesn't accidentally work
        log::debug!(
            "      ✓ {} correctly rejected with error: {}",
            proc_name,
            err_msg.lines().next().unwrap_or(&err_msg)
        );
    }

    // Test system.* prefix mapping to gql.* (should work for valid procedures)
    log::debug!("  🔄 Testing system.* prefix mapping");

    let result = fixture.query("CALL system.cache_stats()");
    // system.cache_stats should be recognized (mapped to gql.cache_stats)
    if let Err(err_msg) = result {
        let err_str = err_msg.to_string();
        assert!(
            !err_str.contains("not found") && !err_str.contains("not supported"),
            "system.cache_stats should be recognized (mapped to gql.cache_stats), got: {}",
            err_str
        );
    }
    log::debug!("    ✓ system.cache_stats correctly mapped to gql.cache_stats");

    log::debug!("  ✅ System procedure validation test passed!");
    log::debug!(
        "     - {} valid procedures recognized",
        valid_procedures.len()
    );
    log::debug!(
        "     - {} invalid procedures rejected",
        invalid_procedures.len()
    );
    log::debug!("     - system.* prefix mapping works");
}
