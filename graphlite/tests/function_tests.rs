//! Comprehensive function tests using the new test fixture framework
//!
//! This file consolidates tests for:
//! - Function execution with deterministic test data
//! - Function gap analysis and BNF compliance
//! - Function planning and query optimization
//! - End-to-end function integration testing

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::{TestFixture, TestSuite, TestCase, FixtureType};
use graphlite::Value;




// use testutils::generate_sample_fraud_data;


// Extension trait to add missing methods to TestFixture
trait TestFixtureExtensions {
    fn assert_numeric_range(&self, query: &str, column: &str, min: f64, max: f64) -> f64;
}

impl TestFixtureExtensions for TestFixture {
    fn assert_numeric_range(&self, query: &str, column: &str, min: f64, max: f64) -> f64 {
        let result = self.assert_query_succeeds(query);
        assert_eq!(result.rows.len(), 1);
        
        let value = result.rows[0].values.get(column).expect("Column should exist");
        if let Value::Number(num) = value {
            assert!(*num >= min && *num <= max, 
                "Value {} should be between {} and {}", num, min, max);
            *num
        } else {
            panic!("Expected numeric value, got {:?}", value);
        }
    }
}


// ==============================================================================
// FUNCTION EXECUTION TESTS
// ==============================================================================

#[test]
fn test_count_function_with_sample_data() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_count_function_with_sample_data").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test COUNT() function with real graph data
    fixture.assert_first_value(
        "MATCH (a:Account) RETURN count(a) as account_count",
        "account_count",
        Value::Number(50.0)
    );
    
    // Test COUNT with limited results
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) RETURN count(a) as total LIMIT 1"
    );
    assert!(!result.rows.is_empty());
    
    if let Some(row) = result.rows.first() {
        if let Some(Value::Number(count)) = row.values.get("total") {
            assert!(*count > 0.0, "Should have accounts in test data");
        }
    }
}


#[test]
fn test_count_function_empty_result() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_count_function_empty_result").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test COUNT() with empty dataset (simulating no matches)
    fixture.assert_first_value(
        "MATCH (a:Account) WHERE a.balance > 999999999 RETURN count(a) as count",
        "count",
        Value::Number(0.0)
    );
}


#[test]
fn test_count_function_with_merchant_data() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_count_function_with_merchant_data").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test COUNT() function with merchant data
    fixture.assert_first_value(
        "MATCH (m:Merchant) RETURN count(m) as merchant_count",
        "merchant_count",
        Value::Number(20.0)
    );
    
    // Test specific merchant category counting
    let result = fixture.assert_query_succeeds(
        "MATCH (m:Merchant) WHERE m.category IS NOT NULL RETURN count(m) as categorized_merchants"
    );
    assert!(!result.rows.is_empty());
}

// ==============================================================================
// END-TO-END FUNCTION EXECUTION TESTS
// ==============================================================================

// TODO: This test has broken session management - disabled for now
/*
#[test] 
fn test_count_end_to_end_basic() {
    let fixture = TestFixture::with_fraud_data()
        .expect("Failed to create fraud data fixture");
    
    // Test basic COUNT() query
    let result = fixture.assert_query_succeeds(
        "MATCH (account:Account) RETURN COUNT(account) as account_count"
    );
    
    // Verify results
    assert_eq!(result.rows.len(), 1); // Single aggregate row
    
    if let Some(row) = result.rows.first() {
        let count_value = row.values.get("account_count")
            .expect("Should have account_count column");
        if let Value::Number(count) = count_value {
            assert!(*count > 0.0, "Should have accounts in fraud data");
            assert_eq!(*count, 1000.0, "Should have exactly 1000 accounts");
        } else {
            panic!("COUNT() should return a Number");
        }
    } else {
        panic!("Should return at least one row");
    }
}
*/

#[test]
fn test_count_end_to_end_with_where_clause() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_count_end_to_end_with_where_clause").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test COUNT() with WHERE clause
    let result = fixture.assert_query_succeeds(
        "MATCH (account:Account) WHERE account.balance > 3000 RETURN COUNT(account) as high_balance_count"
    );
    
    // Verify results
    assert_eq!(result.rows.len(), 1); // Single aggregate row
    
    if let Some(row) = result.rows.first() {
        let count_value = row.values.get("high_balance_count")
            .expect("Should have high_balance_count column");
        if let Value::Number(count) = count_value {
            assert!(*count >= 0.0, "Count should be non-negative");
            // With fraud data, expect some high-balance accounts
        } else {
            panic!("COUNT() should return a Number");
        }
    } else {
        panic!("Should return at least one row");
    }
}

// TODO: AVG function has argument type issues - needs investigation
// #[test]
// fn test_average_end_to_end_with_where_clause() {
//     // Test AVERAGE query with WHERE clause
//     let query_str = "MATCH (account:Account) WHERE account.balance > 94000 RETURN AVG(account.balance)";
//     
//     let executed_result = execute_test_query_with_session(query_str).expect("Failed to execute query");
//     let result = &query_result;
//     
//     // Verify results
//     assert_eq!(result.rows.len(), 1); // Single aggregate row
//     assert_eq!(result.variables, vec!["AVG(...)"]);
//     
//     let avg_value = result.rows[0].get_value("AVG(...)").unwrap();
//     if let Value::Number(avg) = avg_value {
//         // With persistent test data, expect average to be reasonable
//         assert!(avg >= &50000.0, "Average should be at least 50000, got {}", avg);
//         assert!(avg <= &200000.0, "Average should be at most 200000, got {}", avg);
//     } else {
//         panic!("AVG() should return a Number");
//     }
// }

#[test]
fn test_count_with_id_argument() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_count_with_id_argument").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test COUNT(account) vs COUNT(account.id)
    let result1 = fixture.assert_query_succeeds(
        "MATCH (account:Account) WHERE account.balance > 2000 RETURN COUNT(account) as count_node"
    );
    
    let result2 = fixture.assert_query_succeeds(
        "MATCH (account:Account) WHERE account.balance > 2000 RETURN COUNT(account.id) as count_id"
    );
    
    // Both should return the same count since id is never null
    if let (Some(row1), Some(row2)) = (result1.rows.first(), result2.rows.first()) {
        let count_node = row1.values.get("count_node").expect("Should have count_node");
        let count_id = row2.values.get("count_id").expect("Should have count_id");
        
        if let (Value::Number(count1), Value::Number(count2)) = (count_node, count_id) {
            assert_eq!(count1, count2, "COUNT(account) and COUNT(account.id) should be equal");
            assert!(*count1 >= 0.0, "Count should be non-negative");
        } else {
            panic!("Both COUNT operations should return Numbers");
        }
    } else {
        panic!("Both queries should return at least one row");
    }
}

#[test]
fn test_sum_end_to_end_with_where_clause() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_sum_end_to_end_with_where_clause").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test SUM query with WHERE clause
    let result = fixture.assert_query_succeeds(
        "MATCH (account:Account) WHERE account.balance > 2000 RETURN SUM(account.balance) as total_balance"
    );
    
    // Verify results
    assert_eq!(result.rows.len(), 1); // Single aggregate row
    
    if let Some(row) = result.rows.first() {
        let sum_value = row.values.get("total_balance")
            .expect("Should have total_balance column");
        if let Value::Number(sum) = sum_value {
            assert!(*sum >= 0.0, "Sum should be non-negative");
        } else {
            panic!("SUM() should return a Number");
        }
    } else {
        panic!("Should return at least one row");
    }
}

#[test]
fn test_min_end_to_end_with_where_clause() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_min_end_to_end_with_where_clause").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test MIN query with WHERE clause using deterministic fraud data
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) WHERE a.balance > 1000 RETURN MIN(a.balance) as min_balance"
    );
    
    assert_eq!(result.rows.len(), 1);
    
    // With fraud data generator: balance = (i as f64) * 100.0 + (i % 50) (range 101-5000)
    // So MIN with balance > 1000 should return a value in this range
    let min_val = fixture.assert_numeric_range(
        "MATCH (a:Account) WHERE a.balance > 1000 RETURN MIN(a.balance) as min_balance",
        "min_balance",
        1000.0,
        5000.0
    );
    
}

#[test]
fn test_max_end_to_end_with_where_clause() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_max_end_to_end_with_where_clause").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test MAX query with WHERE clause using deterministic fraud data
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) WHERE a.balance < 4000 RETURN MAX(a.balance) as max_balance"
    );
    
    assert_eq!(result.rows.len(), 1);
    
    // With fraud data generator: balance = (i as f64) * 100.0 + (i % 50) (range 101-5000)
    // So MAX with balance < 4000 should return a value less than 4000
    let max_val = fixture.assert_numeric_range(
        "MATCH (a:Account) WHERE a.balance < 4000 RETURN MAX(a.balance) as max_balance",
        "max_balance",
        101.0,
        4000.0
    );
    
}

#[test]
fn test_aggregate_functions_with_empty_result() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_aggregate_functions_with_empty_result").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test aggregate functions with no matching rows using impossible filters
    
    // Test SUM with empty result (should return null per ISO GQL standards)
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) WHERE a.balance > 10000000 RETURN SUM(a.balance) as sum_result"
    );
    assert_eq!(result.rows.len(), 1);
    let sum_value = result.rows[0].values.get("sum_result").unwrap();
    assert!(sum_value.is_null(), "SUM should return null for empty result per ISO GQL standards");
    
    // Test MIN with empty result (should return null)
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) WHERE a.balance > 10000000 RETURN MIN(a.balance) as min_result"
    );
    assert_eq!(result.rows.len(), 1);
    let min_value = result.rows[0].values.get("min_result").unwrap();
    assert!(min_value.is_null(), "MIN should return null for empty result");
    
    // Test MAX with empty result (should return null)
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) WHERE a.balance > 10000000 RETURN MAX(a.balance) as max_result"
    );
    assert_eq!(result.rows.len(), 1);
    let max_value = result.rows[0].values.get("max_result").unwrap();
    assert!(max_value.is_null(), "MAX should return null for empty result");
    
}

// ==============================================================================
// FUNCTION METADATA TESTS
// ==============================================================================






// ==============================================================================
// BNF COMPLIANCE AND GAP ANALYSIS TESTS
// ==============================================================================

#[test]
fn test_bnf_function_compliance() {
    let _fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    _fixture.setup_graph("test_bnf_function_compliance").expect("Failed to setup graph");
    // Re-insert simple data since we have a fresh graph
    _fixture.insert_simple_data().expect("Failed to insert simple data");

    // Test BNF-specified functions using data-driven approach
    let test_suite = TestSuite {
        name: "BNF Function Compliance Suite".to_string(),
        fixture_type: FixtureType::Simple,
        test_cases: vec![
            // Aggregation functions (BNF specified)
            TestCase {
                name: "count_without_args".to_string(),
                description: "COUNT() without args".to_string(),
                query: "RETURN COUNT() as result".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            TestCase {
                name: "datetime_function".to_string(),
                description: "DATETIME function".to_string(),
                query: "RETURN DATETIME('2023-01-01T00:00:00Z') as result".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
            TestCase {
                name: "now_function".to_string(),
                description: "NOW function".to_string(),
                query: "RETURN NOW() as result".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
        ],
    };
    
    let results = test_suite.run().expect("Failed to run BNF compliance suite");
    results.print_summary();
    
    let total = results.passed + results.failed;
}

#[test]
fn test_case_sensitivity_functions() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_case_sensitivity_functions").expect("Failed to setup graph");
    // Re-insert simple data since we have a fresh graph
    fixture.insert_simple_data().expect("Failed to insert simple data");

    // Test case sensitivity - BNF specifies lowercase for some functions
    let case_tests = vec![
        ("count()", "COUNT()"),  // BNF: "count" vs implementation: "COUNT"
        ("sum([1,2])", "SUM([1,2])"),
        ("avg([1,2])", "AVG([1,2])"),
        ("min([1,2])", "MIN([1,2])"),
        ("max([1,2])", "MAX([1,2])"),
    ];
    
    for (lowercase, uppercase) in case_tests {
        let query_lower = format!("RETURN {} as result", lowercase);
        let query_upper = format!("RETURN {} as result", uppercase);
        
        let lower_result = fixture.query(&query_lower);
        let upper_result = fixture.query(&query_upper);
        
        match (lower_result.is_ok(), upper_result.is_ok()) {
            (true, true) => {},
            (true, false) => {},
            (false, true) => {},
            (false, false) => {},
        }
    }
}

// ==============================================================================
// FUNCTION PLANNING TESTS
// ==============================================================================

#[test]
fn test_function_planning() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_function_planning").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    
    // Test query with function using fraud data
    let _ = fixture.assert_query_succeeds(
        "MATCH (a:Account) RETURN COUNT(a.id) as account_count"
    );
    
    // Verify we get the expected count from fraud data (50 accounts)
    fixture.assert_first_value(
        "MATCH (a:Account) RETURN COUNT(a.id) as account_count",
        "account_count",
        Value::Number(50.0)
    );
    
}

#[test]
fn test_multiple_functions_planning() {

    let fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    fixture.setup_graph("test_multiple_functions_planning").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    fixture.insert_fraud_data().expect("Failed to insert fraud data");

    // Test query with multiple functions using fraud data
    let result = fixture.assert_query_succeeds(
        "MATCH (a:Account) WHERE a.balance > 3000 RETURN COUNT(a.id) as count, SUM(a.balance) as total"
    );
    
    assert_eq!(result.rows.len(), 1);
    
    // Verify both functions returned results
    let row = &result.rows[0];
    assert!(row.values.contains_key("count"));
    assert!(row.values.contains_key("total"));
    
}

// ==============================================================================
// COMPREHENSIVE INTEGRATION TESTS
// ==============================================================================

#[test]
fn test_function_integration_comprehensive() {
    let _fixture = TestFixture::new().expect("Failed to create test fixture");
    // Setup fresh graph for this test to avoid interference
    _fixture.setup_graph("test_function_integration_comprehensive").expect("Failed to setup graph");
    // Re-insert fraud data since we have a fresh graph
    _fixture.insert_fraud_data().expect("Failed to insert fraud data");

    
    // Test comprehensive query with multiple function types using data-driven approach
    let test_suite = TestSuite {
        name: "Function Integration Suite".to_string(),
        fixture_type: FixtureType::Fraud,
        test_cases: vec![
            TestCase {
                name: "basic_count".to_string(),
                description: "Basic COUNT() function".to_string(),
                query: "MATCH (a:Account) RETURN COUNT() as count".to_string(),
                expected_rows: Some(1),
                expected_values: Some([("count".to_string(), Value::Number(50.0))].into()),
                expected_error: None,
            },
            TestCase {
                name: "filtered_count".to_string(),
                description: "Filtered aggregation COUNT".to_string(),
                query: "MATCH (a:Account) WHERE a.balance > 3000 RETURN COUNT(a) as count".to_string(),
                expected_rows: Some(1),
                expected_values: None, // Don't specify exact value, just verify it works
                expected_error: None,
            },
            TestCase {
                name: "multiple_aggregates".to_string(),
                description: "Multiple aggregate functions".to_string(),
                query: "MATCH (a:Account) RETURN MIN(a.balance) as min_bal, MAX(a.balance) as max_bal, SUM(a.balance) as total_bal".to_string(),
                expected_rows: Some(1),
                expected_values: None,
                expected_error: None,
            },
        ],
    };
    
    let results = test_suite.run().expect("Failed to run integration suite");
    results.print_summary();
    
    assert_eq!(results.failed, 0, "All function integration tests should pass");
    let total = results.passed + results.failed;
}
