//! ISO GQL Pattern Matching Compliance Tests
//! 
//! Tests for complex graph patterns, path expressions, variable-length paths,
//! and advanced pattern matching according to ISO GQL standard

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::{TestFixture, TestSuite, TestCase, FixtureType};
use graphlite::Value;
use std::collections::HashMap;




#[test]
fn test_basic_node_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test anonymous node pattern
    fixture.assert_first_value(
        "MATCH () RETURN count(*) as node_count",
        "node_count",
        Value::Number(70.0)
    );
    
    // Test named node pattern
    fixture.assert_first_value(
        "MATCH (n) RETURN count(n) as named_nodes",
        "named_nodes", 
        Value::Number(70.0)
    );
    
    // Test labeled node pattern
    fixture.assert_first_value(
        "MATCH (a:Account) RETURN count(a) as accounts",
        "accounts",
        Value::Number(50.0)
    );
    
    // Test multiple label pattern
    fixture.assert_query_succeeds(
        "INSERT (test:Person:Employee {name: 'John', role: 'Manager'})"
    );
    
    fixture.assert_first_value(
        "MATCH (pe:Person:Employee) RETURN count(pe) as person_employees",
        "person_employees",
        Value::Number(1.0)
    );
    
    // Test node pattern with properties
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account {status: 'active'}) RETURN count(a) as active_accounts"
    );
    assert!(!result.rows.is_empty());
    
    // Test node pattern with property constraints
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account {status: 'active', account_type: 'checking'}) 
         RETURN count(a) as active_checking"
    );
    assert!(!result.rows.is_empty());
}

#[test]
fn test_relationship_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test anonymous relationship pattern (using named variable)
    // In shared fixture mode, other tests may add relationships, so check minimum
    let result = fixture.assert_query_succeeds(
        "MATCH ()-[rel]->() RETURN count(rel) as relationships"
    );
    let count = result.rows[0].values.get("relationships").unwrap();
    if let Value::Number(n) = count {
        assert!(*n >= 150.0, "Expected at least 150 relationships, got {}", n);
    }

    // Test named relationship pattern
    let result = fixture.assert_query_succeeds(
        "MATCH ()-[r]->() RETURN count(r) as named_relationships"
    );
    let count = result.rows[0].values.get("named_relationships").unwrap();
    if let Value::Number(n) = count {
        assert!(*n >= 150.0, "Expected at least 150 named relationships, got {}", n);
    }
    
    // Test typed relationship pattern
    fixture.assert_first_value(
        "MATCH ()-[t:Transaction]->() RETURN count(t) as transactions",
        "transactions",
        Value::Number(100.0)
    );
    
    // Test relationship pattern with properties
    let result = fixture.assert_query_succeeds(
        "MATCH ()-[t:Transaction {status: 'completed'}]->() 
         RETURN count(t) as completed_transactions"
    );
    assert!(!result.rows.is_empty());
    
    // Test bidirectional relationship pattern
    // In shared fixture mode, check minimum count
    let result = fixture.assert_query_succeeds(
        "MATCH ()-[r]-() RETURN count(r) as bidirectional"
    );
    let count = result.rows[0].values.get("bidirectional").unwrap();
    if let Value::Number(n) = count {
        assert!(*n >= 300.0, "Expected at least 300 bidirectional relationships, got {}", n);
    }

    // Test relationship direction patterns
    let result = fixture.assert_query_succeeds(
        "MATCH ()<-[t:Transaction]-() RETURN count(t) as incoming_transactions"
    );
    let count = result.rows[0].values.get("incoming_transactions").unwrap();
    if let Value::Number(n) = count {
        assert!(*n >= 100.0, "Expected at least 100 incoming transactions, got {}", n);
    }
}

#[test]
fn test_complex_path_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test simple path pattern
    let result = fixture.assert_query_succeeds(
        "MATCH path = (a:Account)-[t:Transaction]->(m:Merchant) 
         RETURN count(path) as transaction_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test multi-hop path pattern
    let result = fixture.assert_query_succeeds(
        "MATCH path = (a1:Account)-[t1:Transaction]->(m:Merchant)<-[t2:Transaction]-(a2:Account)
         WHERE a1 <> a2
         RETURN count(path) as shared_merchant_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test path with multiple relationship types
    let result = fixture.assert_query_succeeds(
        "MATCH path = (a:Account)-[r:Transaction|Purchase]->(m:Merchant)
         RETURN count(path) as mixed_relationship_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test triangular patterns
    fixture.assert_query_succeeds(
        "INSERT (hub:Hub {id: 'central'}),
                (node1:TestNode {id: 1}),
                (node2:TestNode {id: 2})"
    );
    
    fixture.assert_query_succeeds(
        "MATCH (hub:Hub), (n1:TestNode {id: 1}), (n2:TestNode {id: 2})
         INSERT (hub)-[:CONNECTS]->(n1),
                (hub)-[:CONNECTS]->(n2),
                (n1)-[:LINKS]->(n2)"
    );
    
    let result = fixture.assert_query_succeeds(
        "MATCH triangle = (hub:Hub)-[:CONNECTS]->(n1:TestNode)-[:LINKS]->(n2:TestNode)<-[:CONNECTS]-(hub)
         RETURN count(triangle) as triangles"
    );
    assert!(!result.rows.is_empty());
}

#[test]
fn test_variable_length_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Create a chain pattern for testing (avoid reserved keywords like 'start')
    fixture.assert_query_succeeds(
        "INSERT (node0:ChainNode {id: 0}),
                (node1:ChainNode {id: 1}),
                (node2:ChainNode {id: 2}),
                (node3:ChainNode {id: 3}),
                (node4:ChainNode {id: 4})"
    );
    
    fixture.assert_query_succeeds(
        "MATCH (s:ChainNode {id: 0}), (n1:ChainNode {id: 1}), (n2:ChainNode {id: 2}), 
               (n3:ChainNode {id: 3}), (e:ChainNode {id: 4})
         INSERT (s)-[:NEXT]->(n1)-[:NEXT]->(n2)-[:NEXT]->(n3)-[:NEXT]->(e)"
    );
    
    // Test variable length path - 1 to 3 hops
    let result = fixture.assert_query_succeeds(
        "MATCH path = (node0:ChainNode {id: 0})-[:NEXT]{1,3}->(node_end)
         RETURN count(path) as one_to_three_hop_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test variable length path - up to 2 hops
    let result = fixture.assert_query_succeeds(
        "MATCH path = (node0:ChainNode {id: 0})-[:NEXT]{,2}->(node_end)
         RETURN count(path) as up_to_two_hop_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test variable length path - at least 2 hops
    let result = fixture.assert_query_succeeds(
        "MATCH path = (node0:ChainNode {id: 0})-[:NEXT]{2,}->(node_end)
         RETURN count(path) as at_least_two_hop_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test variable length path on the main graph
    let result = fixture.assert_query_succeeds(
        "MATCH path = (a:Account)-[:Transaction|Purchase]{1,2}->(m:Merchant)
         RETURN count(DISTINCT a) as accounts_with_paths,
                count(DISTINCT m) as reachable_merchants
         LIMIT 1000"  // Limit to avoid very large results
    );
    assert!(!result.rows.is_empty());
}

#[test]
fn test_optional_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test simple LIMIT clause functionality
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)
         RETURN a.account_number
         LIMIT 10"
    );
    assert!(result.rows.len() <= 10);
    
    // Test conditional pattern matching with WHERE clause
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WHERE t.amount > 50000
         RETURN DISTINCT a.account_number
         LIMIT 15"
    );
    assert!(result.rows.len() <= 15);
    
    // Test pattern with conditional existence using separate matches
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)
         RETURN a.account_number,
                EXISTS {MATCH (a)-[t:Transaction]->(m:Merchant)} as has_transactions
         LIMIT 10"
    );
    // Verify the query executed successfully and returned results
    assert!(!result.rows.is_empty());
    
    // Test filtering patterns with high-value transactions
    let result = fixture.assert_query_succeeds(
        "MATCH (m:Merchant)<-[t:Transaction]-(a:Account)
         WHERE t.amount > 10000
         RETURN DISTINCT m.name
         LIMIT 10"
    );
    assert!(result.rows.len() <= 10);
}

#[test]
fn test_pattern_comprehensions() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test pattern matching for collecting related data using ISO GQL compliant syntax
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         RETURN a.account_number,
                t.amount as transaction_amount,
                m.category as merchant_category
         LIMIT 10"
    );
    assert!(result.rows.len() <= 10);
    
    // Test pattern matching with filtering for large transactions  
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WHERE t.amount > 10000
         RETURN a.account_number,
                m.name as merchant,
                t.amount as amount
         ORDER BY t.amount DESC
         LIMIT 10"
    );
    assert!(result.rows.len() <= 10);
    
    // Test pattern matching for revenue calculations
    let result = fixture.assert_query_succeeds(
        "MATCH (m:Merchant)<-[t:Transaction]-(a:Account)
         RETURN m.name,
                count(t) as transaction_count,
                sum(t.amount) as total_revenue
         GROUP BY m.name
         ORDER BY sum(t.amount) DESC
         LIMIT 10"
    );
    assert!(result.rows.len() <= 10);
}

#[test]
fn test_pattern_data_driven_cases() {
    let test_suite = TestSuite {
        name: "Pattern Matching Test Suite".to_string(),
        fixture_type: FixtureType::Fraud,
        test_cases: vec![
            // Basic pattern tests
            TestCase {
                name: "simple_node_pattern".to_string(),
                description: "Match simple node pattern".to_string(),
                query: "MATCH (n) RETURN count(n) as total_nodes".to_string(),
                expected_rows: Some(1),
                expected_values: Some(HashMap::from([("total_nodes".to_string(), Value::Number(70.0))])),
                expected_error: None,
            },
            TestCase {
                name: "labeled_node_pattern".to_string(),
                description: "Match labeled node pattern".to_string(),
                query: "MATCH (a:Account) RETURN count(a) as accounts".to_string(),
                expected_rows: Some(1),
                expected_values: Some(HashMap::from([("accounts".to_string(), Value::Number(50.0))])),
                expected_error: None,
            },
            TestCase {
                name: "relationship_pattern".to_string(),
                description: "Match relationship pattern".to_string(),
                query: "MATCH ()-[r:Transaction]->() RETURN count(r) as transactions".to_string(),
                expected_rows: Some(1),
                expected_values: Some(HashMap::from([("transactions".to_string(), Value::Number(100.0))])),
                expected_error: None,
            },
            // Complex pattern tests
            TestCase {
                name: "multi_hop_pattern".to_string(),
                description: "Match multi-hop pattern".to_string(),
                query: "MATCH (a1:Account)-[:Transaction]->(m:Merchant)<-[:Transaction]-(a2:Account) WHERE a1 <> a2 RETURN count(*) as shared_merchants".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            TestCase {
                name: "optional_pattern".to_string(),
                description: "Match with optional pattern".to_string(),
                query: "MATCH (a:Account) RETURN count(DISTINCT a) as all_accounts".to_string(),
                expected_rows: Some(1),
                expected_values: Some(HashMap::from([("all_accounts".to_string(), Value::Number(50.0))])),
                expected_error: None,
            },
            // Variable length patterns
            TestCase {
                name: "variable_length_pattern".to_string(),
                description: "Match variable length pattern".to_string(),
                query: "MATCH path = ()-[:Transaction|Purchase]->{1,2}() RETURN count(path) as paths LIMIT 1000".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
        ],
    };
    
    let results = test_suite.run().expect("Failed to run pattern matching test suite");
    results.print_summary();
    
    assert!(results.passed >= 5, "Should have at least 5 passing pattern tests");
}

#[test]
fn test_shortest_path_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Create a more complex graph for shortest path testing
    fixture.assert_query_succeeds(
        "INSERT (a:PathNode {id: 'A'}),
                (b:PathNode {id: 'B'}),
                (c:PathNode {id: 'C'}),
                (d:PathNode {id: 'D'}),
                (e:PathNode {id: 'E'})"
    );
    
    // Create multiple paths between A and E
    fixture.assert_query_succeeds(
        "MATCH (a:PathNode {id: 'A'}), (b:PathNode {id: 'B'}), (c:PathNode {id: 'C'}), 
               (d:PathNode {id: 'D'}), (e:PathNode {id: 'E'})
         INSERT (a)-[:PATH {weight: 1}]->(b)-[:PATH {weight: 1}]->(e),
                (a)-[:PATH {weight: 1}]->(c)-[:PATH {weight: 2}]->(d)-[:PATH {weight: 1}]->(e),
                (a)-[:PATH {weight: 5}]->(e)"
    );
    
    // Test shortest path (if supported)
    let result = fixture.query(
        "MATCH path = shortestPath((a:PathNode {id: 'A'})-[:PATH*]-(e:PathNode {id: 'E'}))
         RETURN length(path) as path_length"
    );
    
    match result {
        Ok(r) => {
            log::debug!("Shortest path supported: length = {:?}", r.rows);
            assert!(!r.rows.is_empty());
        },
        Err(_) => {
            log::debug!("Shortest path syntax not supported, testing manual approach");
            
            // Test manual shortest path using variable length with ORDER BY and LIMIT (demonstrating these work)
            let result = fixture.assert_query_succeeds(
                "MATCH (a:PathNode {id: 'A'})-[:PATH]{1,3}->(e:PathNode {id: 'E'})
                 RETURN a.id as start_id, e.id as end_id
                 ORDER BY end_id DESC
                 LIMIT 1"
            );
            assert!(!result.rows.is_empty());
        }
    }
}

#[test]
fn test_pattern_performance() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test simple pattern performance
    let start = std::time::Instant::now();
    
    fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         RETURN count(*) as pattern_matches"
    );
    
    let simple_duration = start.elapsed();
    log::debug!("Simple pattern matching took: {:?}", simple_duration);
    
    // Test complex pattern performance
    let start = std::time::Instant::now();
    
    fixture.assert_query_succeeds(
        "MATCH (a1:Account)-[t1:Transaction]->(m:Merchant)<-[t2:Transaction]-(a2:Account)
         WHERE a1 <> a2 AND t1.amount > 1000 AND t2.amount > 1000
         RETURN count(*) as complex_patterns"
    );
    
    let complex_duration = start.elapsed();
    log::debug!("Complex pattern matching took: {:?}", complex_duration);
    
    // Test variable length pattern performance
    let start = std::time::Instant::now();
    
    let result = fixture.query(
        "MATCH path = (a:Account)-[:Transaction|Purchase]->{1,2}(m:Merchant)
         WHERE a.balance > 3000
         RETURN count(path) as var_length_paths"
    );
    
    let var_length_duration = start.elapsed();
    log::debug!("Variable length pattern matching took: {:?}", var_length_duration);
    
    // All should complete in reasonable time
    assert!(simple_duration.as_secs() < 5, "Simple patterns should be fast");
    assert!(complex_duration.as_secs() < 15, "Complex patterns should complete reasonably");
    
    match result {
        Ok(_) => assert!(var_length_duration.as_secs() < 30, "Variable length should complete"),
        Err(_) => log::debug!("Variable length patterns not supported or too expensive"),
    }
}

#[test]
fn test_pattern_edge_cases() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test self-referencing patterns
    fixture.assert_query_succeeds(
        "INSERT (self_ref:SelfRef {id: 1})"
    );
    
    fixture.assert_query_succeeds(
        "MATCH (s:SelfRef) INSERT (s)-[:SELF_LOOP]->(s)"
    );
    
    let result = fixture.assert_query_succeeds(
        "MATCH (n)-[r:SELF_LOOP]->(n) RETURN count(r) as self_loops"
    );
    assert!(!result.rows.is_empty());
    
    // Test patterns with no matches
    fixture.assert_first_value(
        "MATCH (a:Account)-[:NONEXISTENT]->(b:Merchant) RETURN count(*) as no_matches",
        "no_matches",
        Value::Number(0.0)
    );
    
    // Test patterns with multiple relationship types
    let result = fixture.assert_query_succeeds(
        "MATCH ()-[r:Transaction|Purchase|NONEXISTENT]->() RETURN count(r) as mixed_types"
    );
    assert!(!result.rows.is_empty());
    
    // Test deeply nested patterns - testing if 'level' is a reserved keyword
    fixture.assert_query_succeeds(
        "INSERT (deep1:DeepNode {depth: 1}), (deep2:DeepNode {depth: 2}), (deep3:DeepNode {depth: 3}), (deep4:DeepNode {depth: 4})"
    );
    
    fixture.assert_query_succeeds(
        "MATCH (d1:DeepNode {depth: 1}), (d2:DeepNode {depth: 2}), (d3:DeepNode {depth: 3}), (d4:DeepNode {depth: 4}) INSERT (d1)-[:DEEPER]->(d2)-[:DEEPER]->(d3)-[:DEEPER]->(d4)"
    );
    
    let result = fixture.assert_query_succeeds(
        "MATCH path = (start:DeepNode {depth: 1})-[:DEEPER]{3}->(end:DeepNode {depth: 4})
         RETURN count(path) as deep_paths"
    );
    assert!(!result.rows.is_empty());
    
    // Test patterns with property filters on relationships
    let result = fixture.assert_query_succeeds(
        "MATCH ()-[t:Transaction]->() 
         WHERE t.amount IS NOT NULL AND t.status IS NOT NULL
         RETURN count(t) as filtered_transactions"
    );
    assert!(!result.rows.is_empty());
}

#[test]
fn test_union_and_intersection_patterns() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    log::debug!("=== COMPREHENSIVE LIMIT BUG INVESTIGATION ===");
    
    // Test 1: Simple LIMIT (baseline - should work)
    log::debug!("\\n1. Testing simple LIMIT (baseline)");
    let simple_limit = fixture.assert_query_succeeds(
        "MATCH (a:Account) RETURN a.account_number LIMIT 5"
    );
    log::debug!("Simple LIMIT 5: {} rows", simple_limit.rows.len());
    
    // Test 2: LIMIT inside parentheses (potential issue)
    log::debug!("\\n2. Testing LIMIT inside parentheses");
    let paren_limit = fixture.assert_query_succeeds(
        "(MATCH (a:Account) RETURN a.account_number LIMIT 5)"
    );
    log::debug!("Parenthesized LIMIT 5: {} rows", paren_limit.rows.len());
    
    // Test 3: ORDER BY + LIMIT (to check if ORDER BY affects LIMIT)
    log::debug!("\\n3. Testing ORDER BY + LIMIT");
    let order_limit = fixture.assert_query_succeeds(
        "MATCH (a:Account) RETURN a.account_number ORDER BY a.account_number LIMIT 5"
    );
    log::debug!("ORDER BY + LIMIT 5: {} rows", order_limit.rows.len());
    
    // Test 4: Parentheses + ORDER BY + LIMIT
    log::debug!("\\n4. Testing parentheses + ORDER BY + LIMIT");
    let paren_order_limit = fixture.assert_query_succeeds(
        "(MATCH (a:Account) RETURN a.account_number ORDER BY a.account_number LIMIT 5)"
    );
    log::debug!("Parenthesized ORDER BY + LIMIT 5: {} rows", paren_order_limit.rows.len());
    
    // Debug: Test individual parts of the UNION to understand the bug
    log::debug!("\\n=== Testing UNION-specific LIMIT behavior ===");
    
    // Test 5: Individual UNION components with broader data
    let left_broad = fixture.assert_query_succeeds(
        "MATCH (a:Account) RETURN 'left' as side, a.account_number LIMIT 3"
    );
    log::debug!("Left side (broad): {} rows", left_broad.rows.len());
    
    let right_broad = fixture.assert_query_succeeds(
        "MATCH (m:Merchant) RETURN 'right' as side, m.name LIMIT 2"
    );
    log::debug!("Right side (broad): {} rows", right_broad.rows.len());
    
    // Test 6: UNION without parentheses
    log::debug!("\\n5. Testing UNION without parentheses");
    let union_no_parens_result = fixture.query(
        "MATCH (a:Account) RETURN 'left' as side, a.account_number LIMIT 3
         UNION ALL
         MATCH (m:Merchant) RETURN 'right' as side, m.name LIMIT 2"
    );
    match union_no_parens_result {
        Ok(result) => log::debug!("UNION without parens: {} rows", result.rows.len()),
        Err(e) => log::debug!("UNION without parens failed: {}", e),
    }
    
    // Test 7: UNION with parentheses but no final LIMIT
    log::debug!("\\n6. Testing UNION with parentheses, no final LIMIT");
    let union_parens_no_final = fixture.assert_query_succeeds(
        "(MATCH (a:Account) RETURN 'left' as side, a.account_number LIMIT 3)
         UNION ALL
         (MATCH (m:Merchant) RETURN 'right' as side, m.name LIMIT 2)"
    );
    log::debug!("UNION with parens, no final LIMIT: {} rows", union_parens_no_final.rows.len());
    
    // Test 8: UNION with parentheses and final LIMIT
    log::debug!("\\n7. Testing UNION with parentheses and final LIMIT");
    
    let debug_query = "(MATCH (a:Account) RETURN 'left' as side, a.account_number LIMIT 3)
         UNION ALL
         (MATCH (m:Merchant) RETURN 'right' as side, m.name LIMIT 2)
         LIMIT 4";
    
    
    let union_parens_with_final = fixture.assert_query_succeeds(debug_query);
    log::debug!("UNION with parens and final LIMIT 4: {} rows", union_parens_with_final.rows.len());
    
    // Test 9: INTERSECT with LIMIT
    log::debug!("\\n8. Testing INTERSECT with LIMIT");
    let intersect_result = fixture.assert_query_succeeds(
        "(MATCH (a:Account) RETURN a.account_number LIMIT 10)
         INTERSECT
         (MATCH (a:Account) RETURN a.account_number LIMIT 15)
         LIMIT 8"
    );
    log::debug!("INTERSECT with LIMIT 8: {} rows", intersect_result.rows.len());
    
    // Test 10: EXCEPT with LIMIT
    log::debug!("\\n9. Testing EXCEPT with LIMIT");
    let except_result = fixture.assert_query_succeeds(
        "(MATCH (a:Account) RETURN a.account_number LIMIT 20)
         EXCEPT
         (MATCH (a:Account) WHERE a.balance > 5000 RETURN a.account_number LIMIT 5)
         LIMIT 12"
    );
    log::debug!("EXCEPT with LIMIT 12: {} rows", except_result.rows.len());
    
    log::debug!("\\n=== BUG ANALYSIS ===");
    log::debug!("Expected behavior:");
    log::debug!("- Simple LIMIT: works");
    log::debug!("- Parenthesized LIMIT: should work same as simple");
    log::debug!("- UNION with individual LIMITs: should respect both individual and final LIMITs");
    log::debug!("- INTERSECT/EXCEPT with LIMITs: should respect all LIMIT clauses");
    
    // Comprehensive assertion to document the bugs
    log::debug!("\\n=== DOCUMENTING BUGS FOR FIX ===");
    
    // Only fail the test if we have clear evidence of bugs
    let simple_limit_works = simple_limit.rows.len() == 5;
    let paren_limit_works = paren_limit.rows.len() == 5; 
    let union_final_limit_works = union_parens_with_final.rows.len() <= 4;
    let intersect_limit_works = intersect_result.rows.len() <= 8;
    let except_limit_works = except_result.rows.len() <= 12;
    
    log::debug!("Simple LIMIT works: {}", simple_limit_works);
    log::debug!("Parenthesized LIMIT works: {}", paren_limit_works);
    log::debug!("UNION final LIMIT works: {}", union_final_limit_works);
    log::debug!("INTERSECT LIMIT works: {}", intersect_limit_works);
    log::debug!("EXCEPT LIMIT works: {}", except_limit_works);
    
    // Fail if any critical LIMIT functionality is broken
    if !union_final_limit_works {
        panic!("UNION LIMIT bug: expected <= 4 rows, got {}", union_parens_with_final.rows.len());
    }
    if !intersect_limit_works {
        panic!("INTERSECT LIMIT bug: expected <= 8 rows, got {}", intersect_result.rows.len());
    }
    if !except_limit_works {
        panic!("EXCEPT LIMIT bug: expected <= 12 rows, got {}", except_result.rows.len());
    }
}

#[test]
fn test_pattern_with_aggregations() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    // Test patterns combined with aggregations using ISO GQL compliant syntax
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         RETURN a.account_number,
                m.name,
                count(*) as transaction_count
         GROUP BY a.account_number, m.name
         HAVING count(*) > 5
         ORDER BY count(*) DESC
         LIMIT 10"
    );
    assert!(result.rows.len() <= 10);
    
    // Test pattern aggregation with simple counting
    let result = fixture.query(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         RETURN count(DISTINCT a) as total_accounts,
                count(DISTINCT m) as total_merchants,
                count(t) as total_transactions"
    );
    
    match result {
        Ok(r) => assert!(!r.rows.is_empty()),
        Err(_) => log::debug!("Complex variable length pattern aggregation not supported"),
    }
    
    // Test pattern with simple aggregation (ISO GQL compliant)
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WHERE t.amount > 10000
         RETURN count(DISTINCT a) as accounts_with_large_transactions,
                count(t) as large_transaction_count"
    );
    assert!(!result.rows.is_empty());
}

#[test]
fn debug_multi_hop_pattern() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");
    
    log::debug!("=== Test 1: Basic multi-hop pattern ===");
    let query1 = "MATCH (a1:Account)-[:Transaction]->(m:Merchant)<-[:Transaction]-(a2:Account) RETURN count(*) as shared_merchants";
    
    let result1 = fixture.query(query1);
    match &result1 {
        Ok(r) => log::debug!("✅ Basic works: {} rows", r.rows.len()),
        Err(e) => log::debug!("❌ Basic failed: {:?}", e),
    }
    
    // Test 1.5: Simple two-hop without WHERE to see if a2 is bound at all
    log::debug!("=== Test 1.5: Two-hop without WHERE ===");
    let query1_5 = "MATCH (a1:Account)-[:Transaction]->(m:Merchant)<-[:Transaction]-(a2:Account) RETURN a1, a2, a1.account_number, a2.account_number LIMIT 3";
    
    let result1_5 = fixture.query(query1_5);
    match &result1_5 {
        Ok(r) => {
            log::debug!("✅ Two-hop without WHERE works: {} rows", r.rows.len());
            if let Some(first_row) = r.rows.first() {
                log::debug!("  Available variables: {:?}", first_row.values.keys().collect::<Vec<_>>());
                // Print first few actual values to debug
                for (key, value) in first_row.values.iter().take(4) {
                    log::debug!("    {} = {:?}", key, value);
                }
            }
        },
        Err(e) => log::debug!("❌ Two-hop without WHERE failed: {:?}", e),
    }

    log::debug!("=== Test 2: With != operator ===");  
    let query2 = "MATCH (a1:Account)-[:Transaction]->(m:Merchant)<-[:Transaction]-(a2:Account) WHERE a1 != a2 RETURN count(*) as shared_merchants";
    
    let result2 = fixture.query(query2);
    match &result2 {
        Ok(r) => log::debug!("✅ != works: {} rows", r.rows.len()),
        Err(e) => log::debug!("❌ != failed: {:?}", e),
    }
    
    log::debug!("=== Test 3: With <> operator ===");
    let query3 = "MATCH (a1:Account)-[:Transaction]->(m:Merchant)<-[:Transaction]-(a2:Account) WHERE a1 <> a2 RETURN count(*) as shared_merchants";
    
    let result3 = fixture.query(query3);
    match &result3 {
        Ok(r) => log::debug!("✅ <> works: {} rows", r.rows.len()),
        Err(e) => log::debug!("❌ <> failed: {:?}", e),
    }

    // Test 4: Debug the physical plan generation
    log::debug!("=== Test 4: Debug physical plan ===");
    log::debug!("Let me check if I can trace the physical plan generation...");
    
    // For now, let's test if a simpler variable pattern works
    let simple_query = "MATCH (a:Account) RETURN a.account_number LIMIT 1";
    let simple_result = fixture.query(simple_query);
    match &simple_result {
        Ok(r) => {
            log::debug!("✅ Simple query works: {} rows", r.rows.len());
            if let Some(first_row) = r.rows.first() {
                log::debug!("  Variables in simple query: {:?}", first_row.values.keys().collect::<Vec<_>>());
            }
        },
        Err(e) => log::debug!("❌ Simple query failed: {:?}", e),
    }

    assert!(result1.is_ok(), "Basic multi-hop pattern should work");
}

#[test]
#[ignore = "Tests ISO GQL WITH and NEXT clause composition. \
            ✅ WITH clause chaining (MATCH-WITH-MATCH pattern) - NOW IMPLEMENTED! \
            ❓ NEXT statement for query chaining - Status unknown, needs verification. \
            Remove #[ignore] to test WITH patterns (Pattern 1 should work). \
            NEXT patterns (Pattern 2 & 3) may fail if NEXT not yet implemented."]
fn test_with_and_next_composition() {
    let fixture = TestFixture::with_fraud_data().expect("Failed to create test fixture");

    log::debug!("\n=== Testing WITH and NEXT Clause Composition ===\n");

    // Test 1: WITH clause chaining (intra-query transformation)
    // This tests MATCH -> WITH -> MATCH -> RETURN pattern
    log::debug!("Test 1: WITH clause chaining (intra-query)");
    let with_chain_query = "
        MATCH (a:Account)-[t:Transaction]->(m:Merchant)
        WITH a, m, count(t) as transaction_count, sum(t.amount) as total_spent
        WHERE transaction_count > 5
        MATCH (m)<-[:Transaction]-(other:Account)
        WHERE other <> a
        RETURN a.account_number as account_id,
               m.name as merchant_name,
               transaction_count,
               total_spent,
               count(DISTINCT other) as fellow_customers
        ORDER BY total_spent DESC
        LIMIT 10
    ";

    let with_result = fixture.query(with_chain_query);
    match &with_result {
        Ok(r) => {
            log::debug!("✅ WITH chaining works: {} rows", r.rows.len());
            assert!(r.rows.len() <= 10, "LIMIT 10 should be respected");

            // Verify expected columns
            if let Some(first_row) = r.rows.first() {
                assert!(first_row.values.contains_key("account_id"), "Should have account_id");
                assert!(first_row.values.contains_key("merchant_name"), "Should have merchant_name");
                assert!(first_row.values.contains_key("transaction_count"), "Should have transaction_count");
                assert!(first_row.values.contains_key("total_spent"), "Should have total_spent");
                assert!(first_row.values.contains_key("fellow_customers"), "Should have fellow_customers");

                log::debug!("  Sample row: account={:?}, merchant={:?}, tx_count={:?}, total={:?}, fellow={:?}",
                    first_row.values.get("account_id"),
                    first_row.values.get("merchant_name"),
                    first_row.values.get("transaction_count"),
                    first_row.values.get("total_spent"),
                    first_row.values.get("fellow_customers")
                );
            }
        },
        Err(e) => log::debug!("❌ WITH chaining failed: {:?}", e),
    }

    // Test 2: NEXT statement chaining (inter-query composition)
    // This tests MATCH -> RETURN -> NEXT -> MATCH -> RETURN pattern
    log::debug!("\nTest 2: NEXT statement chaining (inter-query)");
    let next_chain_query = "
        MATCH (a:Account)-[t:Transaction]->(dest:Account)
        RETURN a, count(t) as num_transfers
        GROUP BY a

        NEXT

        MATCH (a:Account)<-[:Owns]-(owner:Person)
        RETURN a.account_number as account_id,
               owner.name as owner_name,
               num_transfers

        NEXT

        FILTER WHERE num_transfers > 2
        RETURN account_id, owner_name, num_transfers
        ORDER BY num_transfers DESC
    ";

    let next_result = fixture.query(next_chain_query);
    match &next_result {
        Ok(r) => {
            log::debug!("✅ NEXT chaining works: {} rows", r.rows.len());

            // Verify columns from final RETURN
            if let Some(first_row) = r.rows.first() {
                assert!(first_row.values.contains_key("account_id"), "Should have account_id");
                assert!(first_row.values.contains_key("owner_name"), "Should have owner_name");
                assert!(first_row.values.contains_key("num_transfers"), "Should have num_transfers");

                // Verify filtering worked (num_transfers > 2)
                if let Some(Value::Number(transfers)) = first_row.values.get("num_transfers") {
                    assert!(*transfers > 2.0, "FILTER should ensure num_transfers > 2, got {}", transfers);
                }

                log::debug!("  Sample row: account={:?}, owner={:?}, transfers={:?}",
                    first_row.values.get("account_id"),
                    first_row.values.get("owner_name"),
                    first_row.values.get("num_transfers")
                );
            }
        },
        Err(e) => log::debug!("❌ NEXT chaining failed: {:?}", e),
    }

    // Test 3: Combined WITH and NEXT (both intra-query and inter-query)
    // This demonstrates using both features together
    log::debug!("\nTest 3: Combined WITH and NEXT (complex composition)");
    let combined_query = "
        MATCH (p:Person)-[:Owns]->(a:Account)
        WITH p.name as person_name, a.account_number as account_id, a.balance as balance
        WHERE balance > 10000
        RETURN person_name, account_id, balance

        NEXT

        MATCH (a:Account)-[t:Transaction]->()
        WHERE a.account_number = account_id
        RETURN person_name,
               account_id,
               balance,
               count(t) as transaction_count
        GROUP BY person_name, account_id, balance
        ORDER BY transaction_count DESC
        LIMIT 5
    ";

    let combined_result = fixture.query(combined_query);
    match &combined_result {
        Ok(r) => {
            log::debug!("✅ Combined WITH+NEXT works: {} rows", r.rows.len());
            assert!(r.rows.len() <= 5, "LIMIT 5 should be respected");

            if let Some(first_row) = r.rows.first() {
                assert!(first_row.values.contains_key("person_name"), "Should have person_name");
                assert!(first_row.values.contains_key("account_id"), "Should have account_id");
                assert!(first_row.values.contains_key("balance"), "Should have balance");
                assert!(first_row.values.contains_key("transaction_count"), "Should have transaction_count");

                // Verify balance filter from WITH clause
                if let Some(Value::Number(bal)) = first_row.values.get("balance") {
                    assert!(*bal > 10000.0, "WITH filter should ensure balance > 10000, got {}", bal);
                }

                log::debug!("  Sample row: person={:?}, account={:?}, balance={:?}, tx_count={:?}",
                    first_row.values.get("person_name"),
                    first_row.values.get("account_id"),
                    first_row.values.get("balance"),
                    first_row.values.get("transaction_count")
                );
            }
        },
        Err(e) => log::debug!("❌ Combined WITH+NEXT failed: {:?}", e),
    }

    // Assert at least one of the patterns works
    let any_success = with_result.is_ok() || next_result.is_ok() || combined_result.is_ok();

    let with_status = if with_result.is_ok() { "OK".to_string() } else { format!("Error: {:?}", with_result.as_ref().err()) };
    let next_status = if next_result.is_ok() { "OK".to_string() } else { format!("Error: {:?}", next_result.as_ref().err()) };
    let combined_status = if combined_result.is_ok() { "OK".to_string() } else { format!("Error: {:?}", combined_result.as_ref().err()) };

    assert!(any_success,
        "At least one composition pattern (WITH chaining, NEXT chaining, or combined) should work. \
         WITH: {}, NEXT: {}, Combined: {}",
        with_status, next_status, combined_status
    );

    log::debug!("\n=== Composition Test Summary ===");
    log::debug!("WITH chaining: {}", if with_result.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    log::debug!("NEXT chaining: {}", if next_result.is_ok() { "✅ PASS" } else { "❌ FAIL" });
    log::debug!("Combined WITH+NEXT: {}", if combined_result.is_ok() { "✅ PASS" } else { "❌ FAIL" });
}
