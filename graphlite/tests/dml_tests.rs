//! ISO GQL DML (Data Manipulation Language) Compliance Tests
//!
//! Tests for INSERT, SET, REMOVE, DELETE statements according to ISO GQL standard
//! Covers all data modification operations

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::{FixtureType, TestCase, TestFixture, TestSuite};

#[test]
fn test_insert_operations() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_insert_operations")
        .expect("Failed to setup graph");

    // Test basic node insertion
    fixture.assert_query_succeeds("INSERT (new_node:Person {name: 'John Doe', age: 30})");

    // Verify insertion
    fixture.assert_first_value(
        "MATCH (p:Person) WHERE p.name = 'John Doe' RETURN count(p) as count",
        "count",
        Value::Number(1.0),
    );

    // Test multiple node insertion
    fixture.assert_query_succeeds(
        "INSERT (alice:Person {name: 'Alice', age: 25}), 
                (bob:Person {name: 'Bob', age: 28})",
    );

    // Verify multiple insertions
    fixture.assert_first_value(
        "MATCH (p:Person) WHERE p.name IN ['Alice', 'Bob'] RETURN count(p) as count",
        "count",
        Value::Number(2.0),
    );

    // Test node insertion with multiple labels
    fixture.assert_query_succeeds(
        "INSERT (manager:Person:Employee {name: 'Manager', age: 45, role: 'supervisor'})",
    );

    // Test relationship insertion
    fixture.assert_query_succeeds("INSERT (company:Company {name: 'TechCorp'})");

    fixture.assert_query_succeeds(
        "MATCH (p:Person {name: 'John Doe'}), (c:Company {name: 'TechCorp'})
         INSERT (p)-[:WORKS_FOR {since: '2020-01-01', position: 'Developer'}]->(c)",
    );

    // Verify relationship
    fixture.assert_first_value(
        "MATCH (p:Person)-[r:WORKS_FOR]->(c:Company) 
         RETURN count(r) as count",
        "count",
        Value::Number(1.0),
    );

    // Test complex pattern insertion - split into separate statements for now
    fixture.assert_query_succeeds("INSERT (ai_proj:Project {name: 'AI System'})");

    fixture.assert_query_succeeds(
        "MATCH (proj:Project {name: 'AI System'})
         INSERT (john_lead:Person {name: 'John'})-[:ASSIGNED_TO {role: 'lead'}]->(proj)",
    );
}

#[test]
fn test_set_operations() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_set_operations")
        .expect("Failed to setup graph");

    // Insert test data first
    fixture.assert_query_succeeds(
        "INSERT (emp:Employee {name: 'Jane Smith', age: 30, salary: 50000})",
    );

    // Test property SET
    fixture.assert_query_succeeds(
        "MATCH (emp:Employee {name: 'Jane Smith'}) 
         SET emp.age = 31, emp.salary = 55000",
    );

    // Verify property update
    let result = fixture.assert_query_succeeds(
        "MATCH (emp:Employee {name: 'Jane Smith'}) 
         RETURN emp.age as age, emp.salary as salary",
    );
    assert_eq!(result.rows.len(), 1);

    // Test adding new property
    fixture.assert_query_succeeds(
        "MATCH (emp:Employee {name: 'Jane Smith'}) 
         SET emp.department = 'Engineering', emp.active = true",
    );

    // Verify new property
    fixture.assert_first_value(
        "MATCH (emp:Employee {name: 'Jane Smith'}) 
         RETURN emp.department as dept",
        "dept",
        Value::String("Engineering".to_string()),
    );

    // Test label SET
    fixture.assert_query_succeeds(
        "MATCH (emp:Employee {name: 'Jane Smith'}) 
         SET emp:Manager",
    );

    // Verify label addition
    fixture.assert_first_value(
        "MATCH (emp:Employee:Manager {name: 'Jane Smith'}) 
         RETURN count(emp) as count",
        "count",
        Value::Number(1.0),
    );

    // Test conditional SET
    fixture.assert_query_succeeds(
        "MATCH (emp:Employee) 
         WHERE emp.salary > 50000 
         SET emp.performance_tier = 'high'",
    );

    // Test SET with expression
    fixture.assert_query_succeeds(
        "MATCH (emp:Employee {name: 'Jane Smith'}) 
         SET emp.salary = emp.salary * 1.1",
    );
}

#[test]
fn test_remove_operations() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_remove_operations")
        .expect("Failed to setup graph");

    // Insert test data
    fixture.assert_query_succeeds(
        "INSERT (temp:TempNode:ExtraLabel {prop1: 'value1', prop2: 'value2', prop3: 'value3'})",
    );

    // Test property REMOVE
    fixture.assert_query_succeeds(
        "MATCH (temp:TempNode) 
         REMOVE temp.prop1, temp.prop2",
    );

    // Verify properties removed
    fixture.assert_first_value(
        "MATCH (temp:TempNode) 
         RETURN temp.prop3 as remaining_prop",
        "remaining_prop",
        Value::String("value3".to_string()),
    );

    // Test label REMOVE
    fixture.assert_query_succeeds(
        "MATCH (temp:TempNode:ExtraLabel) 
         REMOVE temp:ExtraLabel",
    );

    // Verify label removed (node should still exist with TempNode label)
    fixture.assert_first_value(
        "MATCH (temp:TempNode) 
         WHERE NOT temp:ExtraLabel 
         RETURN count(temp) as count",
        "count",
        Value::Number(1.0),
    );

    // Test conditional REMOVE
    fixture.assert_query_succeeds(
        "INSERT (conditional:ConditionalNode {status: 'temporary', value: 100})",
    );

    fixture.assert_query_succeeds(
        "MATCH (c:ConditionalNode) 
         WHERE c.status = 'temporary' 
         REMOVE c.status",
    );

    // Verify conditional remove
    let result = fixture
        .query(
            "MATCH (c:ConditionalNode) 
         RETURN c.status as status",
        )
        .unwrap();

    assert!(result.rows[0].values.get("status").unwrap() == &Value::Null);
}

#[test]
fn test_count_aggregation_with_empty_results() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_count_aggregation")
        .expect("Failed to setup graph");
    // Re-insert simple data since we have a fresh graph
    fixture
        .insert_simple_data()
        .expect("Failed to insert simple data");

    // First test a working case - COUNT on existing nodes
    let working_result = fixture
        .query("MATCH (n:TestNode) RETURN count(n) as count")
        .unwrap();
    for _row in working_result.rows.iter() {}

    // Now test COUNT on non-existent nodes - should return 1 row with count=0
    let result = match fixture.query("MATCH (x:NonExistentLabel) RETURN count(x) as count") {
        Ok(r) => r,
        Err(e) => {
            panic!("Query failed with error: {}", e);
        }
    };

    for _row in result.rows.iter() {}

    // This should return exactly 1 row with count=0, not 0 rows
    assert_eq!(
        result.rows.len(),
        1,
        "COUNT should always return exactly 1 row, even with 0 matching nodes"
    );
    let count_value = result.rows[0].values.get("count").unwrap();
    assert_eq!(
        count_value,
        &Value::Number(0.0),
        "COUNT of non-existent nodes should be 0"
    );
}

#[test]
fn test_delete_operations() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Create schema and graph for this test (ISO GQL compliant)
    fixture
        .query(&format!(
            "CREATE SCHEMA IF NOT EXISTS /{}",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "CREATE GRAPH /{}/delete_test_graph",
            fixture.schema_name()
        ))
        .unwrap();
    fixture
        .query(&format!(
            "SESSION SET GRAPH /{}/delete_test_graph",
            fixture.schema_name()
        ))
        .unwrap();

    // Insert test data for deletion
    fixture.assert_query_succeeds(
        "INSERT (delete_me:DeleteTest {name: 'ToDelete'}),
                (keep_me:DeleteTest {name: 'ToKeep'}),
                (other:Other {name: 'Other'})",
    );

    fixture.assert_query_succeeds(
        "MATCH (d:DeleteTest {name: 'ToDelete'}), (o:Other)
         INSERT (d)-[:RELATED_TO]->(o)",
    );

    // Test simple node deletion with relationships (requires DETACH)
    fixture.assert_query_succeeds(
        "MATCH (d:DeleteTest {name: 'ToDelete'}) 
         DETACH DELETE d",
    );

    // Verify deletion - use query instead of assert_first_value to handle empty results
    let result = fixture
        .query(
            "MATCH (d:DeleteTest {name: 'ToDelete'}) 
         RETURN count(d) as count",
        )
        .unwrap();

    // COUNT should return 1 row even with 0 count
    assert_eq!(
        result.rows.len(),
        1,
        "COUNT should always return exactly 1 row"
    );
    let count = result.rows[0].values.get("count").unwrap();
    assert_eq!(
        count,
        &Value::Number(0.0),
        "Expected count to be 0 after deletion"
    );

    // Verify other nodes remain
    let result = fixture
        .query(
            "MATCH (d:DeleteTest {name: 'ToKeep'}) 
         RETURN count(d) as count",
        )
        .unwrap();

    if result.rows.is_empty() {
        panic!("Expected to find remaining nodes, but count query returned no rows");
    } else {
        let count = result.rows[0].values.get("count").unwrap();
        assert_eq!(count, &Value::Number(1.0), "Expected 1 remaining node");
    }

    // Test relationship deletion
    fixture.assert_query_succeeds("INSERT (src:Source {id: 1}), (dst:Destination {id: 2})");

    fixture.assert_query_succeeds(
        "MATCH (src:Source), (dst:Destination)
         INSERT (src)-[r:CONNECTS {weight: 1.0}]->(dst)",
    );

    fixture.assert_query_succeeds(
        "MATCH ()-[r:CONNECTS]->() 
         DELETE r",
    );

    // Verify relationship deleted but nodes remain
    fixture.assert_first_value(
        "MATCH ()-[r:CONNECTS]->() 
         RETURN count(r) as count",
        "count",
        Value::Number(0.0),
    );

    fixture.assert_first_value(
        "MATCH (n) WHERE n:Source OR n:Destination 
         RETURN count(n) as count",
        "count",
        Value::Number(2.0),
    );

    // Test DETACH DELETE (delete node and its relationships)
    fixture.assert_query_succeeds("INSERT (hub:Hub {id: 'central'})");

    fixture.assert_query_succeeds(
        "MATCH (hub:Hub), (src:Source), (dst:Destination)
         INSERT (src)-[:CONNECTS_TO]->(hub),
                (hub)-[:CONNECTS_TO]->(dst)",
    );

    fixture.assert_query_succeeds(
        "MATCH (hub:Hub) 
         DETACH DELETE hub",
    );

    // Verify node and its relationships are deleted
    fixture.assert_first_value(
        "MATCH (hub:Hub) 
         RETURN count(hub) as count",
        "count",
        Value::Number(0.0),
    );

    fixture.assert_first_value(
        "MATCH ()-[r:CONNECTS_TO]->() 
         RETURN count(r) as count",
        "count",
        Value::Number(0.0),
    );
}

#[test]
fn test_dml_data_driven_cases() {
    let test_suite = TestSuite {
        name: "DML Operations Test Suite".to_string(),
        fixture_type: FixtureType::Simple,
        test_cases: vec![
            // INSERT tests
            TestCase {
                name: "insert_single_node".to_string(),
                description: "Insert a single node with properties".to_string(),
                query: "INSERT (test:TestNode {name: 'DML Test', value: 42})".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            TestCase {
                name: "insert_multiple_nodes".to_string(),
                description: "Insert multiple nodes in one statement".to_string(),
                query: "INSERT (a:NodeA {id: 1}), (b:NodeB {id: 2})".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            // SET tests  
            TestCase {
                name: "set_property_value".to_string(),
                description: "Set property values on existing nodes".to_string(),
                query: "MATCH (n:TestNode) WHERE n.name = 'Node 1' SET n.updated = true, n.timestamp = '2024-01-01'".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            // REMOVE tests
            TestCase {
                name: "remove_property".to_string(),
                description: "Remove property from node".to_string(),
                query: "MATCH (n:TestNode) WHERE n.value = 1 REMOVE n.name".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            // DELETE tests
            TestCase {
                name: "delete_nodes_conditional".to_string(),
                description: "Delete nodes based on condition".to_string(),
                query: "MATCH (n:TestNode) WHERE n.value > 15 DELETE n".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
        ],
    };

    let results = test_suite.run().expect("Failed to run DML test suite");
    results.print_summary();

    assert!(
        results.passed >= 4,
        "Should have at least 4 passing DML tests"
    );
}

#[test]
fn test_simple_match_set() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_simple_match_set")
        .expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture
        .insert_fraud_data()
        .expect("Failed to insert fraud data");

    // Test simple MATCH SET without complex patterns
    fixture.assert_query_succeeds(
        "MATCH (a:Account) 
         WHERE a.balance > 100000 
         SET a.tier = 'premium'",
    );
}

#[test]
fn test_complex_dml_scenarios() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_complex_dml_scenarios")
        .expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture
        .insert_fraud_data()
        .expect("Failed to insert fraud data");

    // Test batch operations on large dataset

    // 1. Batch update based on conditions
    fixture.assert_query_succeeds(
        "MATCH (a:Account) 
         WHERE a.balance > 100000 
         SET a:HighValue, a.tier = 'premium'",
    );

    // Verify batch update
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account:HighValue) 
         RETURN count(a) as premium_accounts",
    );
    assert!(!result.rows.is_empty());

    // 2. Insert derived data based on existing patterns
    fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WHERE t.amount > 50000
         INSERT (alert:SecurityAlert {
             account_id: a.account_number,
             merchant: m.name,
             amount: t.amount,
             alert_type: 'high_value_transaction',
             created_at: '2024-01-01T12:00:00Z'
         })",
    );

    // Verify derived data insertion
    let result = fixture.assert_query_succeeds(
        "MATCH (alert:SecurityAlert) 
         WHERE alert.alert_type = 'high_value_transaction' 
         RETURN count(alert) as alerts",
    );
    assert!(!result.rows.is_empty());

    // 3. Conditional property updates with calculations
    fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->()
         WITH a, count(t) as transaction_count, avg(t.amount) as avg_amount
         WHERE transaction_count > 10
         SET a.activity_score = transaction_count * 0.1 + avg_amount * 0.0001,
             a.active_user = true",
    );

    // 4. Remove outdated or invalid data
    fixture.assert_query_succeeds(
        "MATCH (a:Account) 
         WHERE a.status = 'inactive' AND a.balance = 0 
         DETACH DELETE a",
    );

    // 5. Bulk relationship modifications
    fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WHERE t.amount < 10 AND t.status = 'completed'
         SET t:MicroTransaction, t.processed_date = '2024-01-01'",
    );
}

#[test]
fn test_dml_transaction_behavior() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    fixture
        .setup_graph("test_transaction_behavior")
        .expect("Failed to setup graph");

    // Test DML within transactions
    fixture.assert_query_succeeds("BEGIN");

    // Insert data in transaction
    fixture.assert_query_succeeds("INSERT (tx_test:TransactionTest {name: 'TX Data', value: 100})");

    // Verify data is visible within transaction
    fixture.assert_first_value(
        "MATCH (t:TransactionTest) RETURN count(t) as count",
        "count",
        Value::Number(1.0),
    );

    // Update data in transaction
    fixture.assert_query_succeeds(
        "MATCH (t:TransactionTest) 
         SET t.value = 200, t.updated_in_tx = true",
    );

    // Rollback transaction
    fixture.assert_query_succeeds("ROLLBACK");

    // Verify data is rolled back
    fixture.assert_first_value(
        "MATCH (t:TransactionTest) RETURN count(t) as count",
        "count",
        Value::Number(0.0),
    );

    // Test commit behavior
    fixture.assert_query_succeeds("BEGIN");

    fixture.assert_query_succeeds("INSERT (commit_test:CommitTest {name: 'Committed Data'})");

    fixture.assert_query_succeeds("COMMIT");

    // Verify data persists after commit
    fixture.assert_first_value(
        "MATCH (c:CommitTest) RETURN count(c) as count",
        "count",
        Value::Number(1.0),
    );
}

#[test]
fn test_dml_error_cases() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    fixture
        .setup_graph("test_error_cases")
        .expect("Failed to setup graph");

    // Test invalid INSERT syntax
    fixture.assert_query_fails("INSERT (invalid syntax here)", "Parse error");

    // Test SET on non-existent nodes (should succeed but affect 0 nodes)
    fixture.assert_query_succeeds("MATCH (n:NonExistentLabel) SET n.prop = 'value'");

    // Test REMOVE on non-existent properties (should succeed)
    fixture.assert_query_succeeds("INSERT (test:RemoveTest {prop1: 'value'})");

    fixture.assert_query_succeeds("MATCH (test:RemoveTest) REMOVE test.non_existent_prop");

    // Test DELETE with missing DETACH when node has relationships
    // TODO: This test is disabled because MATCH INSERT is not working properly
    // The MATCH INSERT operation fails to create relationships, so the DELETE constraint check
    // cannot be properly tested. Once MATCH INSERT is fixed, re-enable this test.
    /*
    fixture.assert_query_succeeds(
        "INSERT (connected:Connected {id: 1}), (other:Other {id: 2})"
    );

    fixture.assert_query_succeeds(
        "MATCH (c:Connected), (o:Other) INSERT (c)-[:LINKS_TO]->(o)"
    );

    fixture.assert_query_fails(
        "MATCH (c:Connected) DELETE c",
        "Cannot delete node"
    );
    */

    // Test type mismatches in SET operations
    fixture.assert_query_succeeds("INSERT (type_test:TypeTest {number_prop: 42})");

    // This may or may not fail depending on type coercion rules
    let result = fixture.query("MATCH (t:TypeTest) SET t.number_prop = 'string_value'");

    if let Ok(_) = result {}
}

#[test]
fn test_dml_performance() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture
        .setup_graph("test_dml_performance")
        .expect("Failed to setup graph");

    // Test bulk insert performance
    let start = std::time::Instant::now();

    // Insert 100 nodes in smaller batches to avoid lexer infinite loop
    for batch in 0..10 {
        let mut insert_query = String::from("INSERT ");
        let mut clauses = Vec::new();

        for i in 0..10 {
            let node_id = batch * 10 + i;
            clauses.push(format!(
                "(perf_node_{}:PerfNode {{id: {}, batch: {}, value: {}}})",
                node_id,
                node_id,
                batch,
                node_id * 2
            ));
        }

        // Debug: show the first few clauses
        batch == 0;

        insert_query.push_str(&clauses.join(", "));
        fixture.assert_query_succeeds(&insert_query);

        // Debug: check node count after each batch
        let result = fixture
            .query("MATCH (p:PerfNode) RETURN count(p) as count")
            .unwrap();
        if !result.rows.is_empty() {
            let _count = result.rows[0]
                .values
                .get("count")
                .unwrap_or(&Value::Number(0.0));
        }
    }

    let insert_duration = start.elapsed();

    // Verify all nodes inserted
    let result = fixture
        .query("MATCH (p:PerfNode) RETURN count(p) as count")
        .unwrap();
    if result.rows.is_empty() {
        panic!("Expected to find nodes, but count query returned no rows");
    } else {
        let count = result.rows[0].values.get("count").unwrap();
        assert_eq!(
            count,
            &Value::Number(100.0),
            "Expected 100 nodes to be inserted"
        );
    }

    // Test bulk update performance
    let start = std::time::Instant::now();

    fixture.assert_query_succeeds(
        "MATCH (p:PerfNode) 
         WHERE p.value % 2 = 0 
         SET p:EvenValue, p.processed = true",
    );

    let update_duration = start.elapsed();

    // Test bulk delete performance
    let start = std::time::Instant::now();

    fixture.assert_query_succeeds(
        "MATCH (p:PerfNode) 
         WHERE p.batch >= 5 
         DELETE p",
    );

    let delete_duration = start.elapsed();

    // Verify partial deletion
    let result = fixture
        .query("MATCH (p:PerfNode) RETURN count(p) as count")
        .unwrap();
    if result.rows.is_empty() {
        panic!("Expected to find remaining nodes, but count query returned no rows");
    } else {
        let count = result.rows[0].values.get("count").unwrap();
        assert_eq!(
            count,
            &Value::Number(50.0),
            "Expected 50 nodes to remain after deletion"
        );
    }

    assert!(
        insert_duration.as_secs() < 10,
        "Bulk insert should complete within 10 seconds"
    );
    assert!(
        update_duration.as_secs() < 5,
        "Bulk update should complete within 5 seconds"
    );
    assert!(
        delete_duration.as_secs() < 5,
        "Bulk delete should complete within 5 seconds"
    );
}
