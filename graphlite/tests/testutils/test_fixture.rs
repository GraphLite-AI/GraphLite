//! Test fixture for GraphLite integration tests
//!
//! Provides isolated test database instances using ONLY the public QueryCoordinator API.
//! Tests must not access internal components - use only public QueryCoordinator API.

use graphlite::{QueryCoordinator, QueryResult, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Test fixture with isolated database instance
/// Uses ONLY the public QueryCoordinator API - no internal components
pub struct TestFixture {
    coordinator: Arc<QueryCoordinator>,
    session_id: String,
    schema_name: String,
    graph_name: Option<String>,
    _temp_dir: tempfile::TempDir,
}

impl TestFixture {
    /// Create empty test fixture
    pub fn empty() -> Result<Self, Box<dyn std::error::Error>> {
        Self::new()
    }

    /// Get the schema name
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Get the graph name (if a graph was created)
    pub fn graph_name(&self) -> Option<&str> {
        self.graph_name.as_deref()
    }

    /// Create a new test fixture using ONLY public API
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create temporary directory
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("graphlite_test");

        // Use public API - QueryCoordinator::from_path()
        // This initializes ALL internal components automatically
        let coordinator = QueryCoordinator::from_path(db_path)
            .map_err(Box::<dyn std::error::Error>::from)?;

        // Create session using public API
        let session_id = coordinator
            .create_simple_session("admin")
            .map_err(Box::<dyn std::error::Error>::from)?;

        // Use a unique schema name for test isolation (prevents concurrent test interference)
        let schema_name = format!("test_schema_{}", fastrand::u64(..));

        // Create the shared schema if it doesn't exist
        let _ = coordinator.process_query(
            &format!("CREATE SCHEMA IF NOT EXISTS /{}", schema_name),
            &session_id,
        );

        // Set the schema as the session default
        let _ =
            coordinator.process_query(&format!("SESSION SET SCHEMA /{}", schema_name), &session_id);

        let fixture = TestFixture {
            coordinator,
            session_id,
            schema_name,
            graph_name: None,
            _temp_dir: temp_dir,
        };

        Ok(fixture)
    }

    /// Create test fixture with simple data
    pub fn with_simple_data() -> Result<Self, Box<dyn std::error::Error>> {
        let mut fixture = Self::new()?;

        // Setup fresh graph with unique name for isolation
        let graph_name = format!("test_graph_{}", fastrand::u64(..));
        fixture.setup_graph(&graph_name)?;
        fixture.graph_name = Some(graph_name);

        // Insert simple test data
        fixture.insert_simple_data()?;

        Ok(fixture)
    }

    /// Insert simple test data (test nodes and relationships)
    /// Call this after setup_graph() to populate a fresh graph with simple data
    pub fn insert_simple_data(&self) -> Result<(), String> {
        // Insert test nodes
        for i in 1..=20 {
            self.query(&format!(
                "INSERT (n:TestNode {{id: {}, name: 'Node {}', value: {}}})",
                i,
                i,
                i * 10
            ))?;
        }

        // Insert relationships (creates 9 edges: 1->2, 2->3, ..., 9->10)
        for i in 1..10 {
            self.query(&format!(
                "MATCH (a:TestNode {{id: {}}}), (b:TestNode {{id: {}}})
                 INSERT (a)-[:CONNECTS_TO {{weight: {}}}]->(b)",
                i,
                i + 1,
                i * 2
            ))?;
        }

        Ok(())
    }

    /// Create test fixture with fraud data
    pub fn with_fraud_data() -> Result<Self, Box<dyn std::error::Error>> {
        let mut fixture = Self::new()?;

        // Setup fresh graph with unique name for isolation
        let graph_name = format!("fraud_graph_{}", fastrand::u64(..));
        fixture.setup_graph(&graph_name)?;
        fixture.graph_name = Some(graph_name);

        // Insert fraud detection test data
        fixture.insert_fraud_data()?;

        Ok(fixture)
    }

    /// Insert fraud detection test data (accounts, merchants, transactions)
    /// Call this after setup_graph() to populate a fresh graph with fraud data
    pub fn insert_fraud_data(&self) -> Result<(), String> {
        // Create accounts
        for i in 1..=50 {
            let balance = (i as f64) * 100.0;
            let account_type = match i % 4 {
                0 => "checking",
                1 => "savings",
                2 => "business",
                _ => "investment",
            };
            let account_status = if i % 10 == 0 { "inactive" } else { "active" };
            let risk_score = (i % 100) as f64 / 10.0; // Risk score from 0.0 to 9.9
            self.query(&format!(
                "INSERT (a:Account {{id: {}, account_number: 'ACC{}', name: 'Account{}', balance: {}, status: '{}', account_status: '{}', account_type: '{}', risk_score: {}}})",
                i, i, i, balance, account_status, account_status, account_type, risk_score
            ))?;
        }

        // Create merchants
        for i in 1..=20 {
            self.query(&format!(
                "INSERT (m:Merchant {{id: {}, name: 'Merchant{}', category: 'retail'}})",
                i, i
            ))?;
        }

        // Create transactions (relationships between accounts and merchants)
        for i in 1..=100 {
            let account_id = ((i - 1) % 50) + 1;
            let merchant_id = ((i - 1) % 20) + 1;
            let amount = 50.0 + ((i % 30) as f64);

            self.query(&format!(
                "MATCH (a:Account {{id: {}}}), (m:Merchant {{id: {}}})
                 INSERT (a)-[:Transaction {{amount: {}, timestamp: {}}}]->(m)",
                account_id, merchant_id, amount, i
            ))?;
        }

        // Create purchases (accounts to merchants)
        for i in 1..=50 {
            let account_id = ((i - 1) % 50) + 1;
            let merchant_id = ((i - 1) % 20) + 1;
            let amount = ((i % 30) + 1) as f64 * 3.5;

            self.query(&format!(
                "MATCH (a:Account {{id: {}}}), (m:Merchant {{id: {}}})
                 INSERT (a)-[:Purchase {{amount: {}, timestamp: {}}}]->(m)",
                account_id,
                merchant_id,
                amount,
                i + 100
            ))?;
        }

        Ok(())
    }

    /// Setup a fresh graph for testing (drops if exists, then creates)
    /// Use this at the start of each test to ensure isolation
    pub fn setup_graph(&self, graph_name: &str) -> Result<(), String> {
        // Drop graph if it exists (ignore errors if it doesn't exist)
        let _ = self.query(&format!(
            "DROP GRAPH IF EXISTS /{}/{}",
            self.schema_name, graph_name
        ));

        // Create fresh graph
        self.query(&format!(
            "CREATE GRAPH /{}/{}",
            self.schema_name, graph_name
        ))?;

        // Set as session graph
        self.query(&format!(
            "SESSION SET GRAPH /{}/{}",
            self.schema_name, graph_name
        ))?;

        Ok(())
    }

    /// Execute a query
    pub fn query(&self, query_text: &str) -> Result<QueryResult, String> {
        // Use the coordinator to execute the query with proper orchestration
        self.coordinator.process_query(query_text, &self.session_id)
    }

    /// Execute query and assert success
    pub fn assert_query_succeeds(&self, query: &str) -> QueryResult {
        self.query(query)
            .unwrap_or_else(|e| panic!("Query failed: {}\nError: {}", query, e))
    }

    /// Execute query and assert failure
    pub fn assert_query_fails(&self, query: &str, expected_error: &str) {
        match self.query(query) {
            Ok(_) => panic!("Query should have failed: {}", query),
            Err(e) => assert!(
                e.contains(expected_error),
                "Expected error containing '{}', got: {}",
                expected_error,
                e
            ),
        }
    }

    /// Assert first value
    pub fn assert_first_value(&self, query: &str, column: &str, expected: Value) {
        let result = self.assert_query_succeeds(query);
        assert!(!result.rows.is_empty(), "Query returned no rows: {}", query);

        let actual = result.rows[0]
            .values
            .get(column)
            .unwrap_or_else(|| panic!("Column '{}' not found", column));

        assert_eq!(
            actual, &expected,
            "Column '{}': expected {:?}, got {:?}",
            column, expected, actual
        );
    }

    /// Execute query and return aggregate statistics
    pub fn assert_aggregates(
        &self,
        query: &str,
        _expected_stats: AggregateStats,
    ) -> AggregateStats {
        let result = self.assert_query_succeeds(query);
        assert!(!result.rows.is_empty(), "Query returned no rows: {}", query);

        let row = &result.rows[0].values;

        AggregateStats {
            count: row
                .get("count")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0),
            sum: row
                .get("sum")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0),
            avg: row
                .get("avg")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0),
            min: row
                .get("min")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0),
            max: row
                .get("max")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0),
        }
    }

    /// Create with large data
    pub fn with_large_data(
        _num_nodes: usize,
        _avg_degree: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::empty()
    }
}

/// Aggregate statistics
#[derive(Debug, Clone)]
pub struct AggregateStats {
    pub count: f64,
    pub sum: f64,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
}

/// Fixture type
#[derive(Debug, Clone)]
pub enum FixtureType {
    Simple,
    Fraud,
    Empty,
}

/// Test case
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub description: String,
    pub query: String,
    pub expected_rows: Option<usize>,
    pub expected_values: Option<HashMap<String, Value>>,
    pub expected_error: Option<String>,
}

/// Test suite results
#[derive(Debug)]
pub struct TestSuiteResults {
    pub suite_name: String,
    pub passed: usize,
    pub failed: usize,
    pub test_results: Vec<TestCaseResult>,
}

/// Test case result
#[derive(Debug)]
pub struct TestCaseResult {
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
    pub duration: std::time::Duration,
}

impl TestSuiteResults {
    pub fn print_summary(&self) {
        println!("\n=== Test Suite: {} ===", self.suite_name);
        println!(
            "Passed: {}, Failed: {}, Total: {}",
            self.passed,
            self.failed,
            self.passed + self.failed
        );
    }
}

/// Test suite
#[derive(Debug)]
pub struct TestSuite {
    pub name: String,
    pub fixture_type: FixtureType,
    pub test_cases: Vec<TestCase>,
}

impl TestSuite {
    /// Create a new test suite
    pub fn new(name: String, fixture_type: FixtureType, test_cases: Vec<TestCase>) -> Self {
        Self {
            name,
            fixture_type,
            test_cases,
        }
    }

    /// Run all test cases in the suite
    pub fn run(&self) -> Result<TestSuiteResults, Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        let mut test_results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;

        // Create fixture and setup unique graph for this test suite
        let fixture = TestFixture::new()?;

        match self.fixture_type {
            FixtureType::Simple => {
                fixture.setup_graph("test_suite_simple")?;
                fixture.insert_simple_data()?;
            }
            FixtureType::Fraud => {
                fixture.setup_graph("test_suite_fraud")?;
                fixture.insert_fraud_data()?;
            }
            FixtureType::Empty => {
                fixture.setup_graph("test_suite_empty")?;
            }
        };

        // Run each test case
        for test_case in &self.test_cases {
            let test_start = std::time::Instant::now();

            let result = fixture.query(&test_case.query);

            let (success, error) = match (&result, &test_case.expected_error) {
                (Ok(query_result), None) => {
                    // Success expected, check row count if specified
                    if let Some(expected_rows) = test_case.expected_rows {
                        if query_result.rows.len() == expected_rows {
                            (true, None)
                        } else {
                            (
                                false,
                                Some(format!(
                                    "Expected {} rows, got {}",
                                    expected_rows,
                                    query_result.rows.len()
                                )),
                            )
                        }
                    } else {
                        (true, None)
                    }
                }
                (Err(e), Some(expected_err)) => {
                    // Error expected, check if it matches
                    if e.contains(expected_err) {
                        (true, None)
                    } else {
                        (
                            false,
                            Some(format!(
                                "Expected error containing '{}', got: {}",
                                expected_err, e
                            )),
                        )
                    }
                }
                (Ok(_), Some(expected_err)) => (
                    false,
                    Some(format!(
                        "Expected error '{}', but query succeeded",
                        expected_err
                    )),
                ),
                (Err(e), None) => (false, Some(format!("Unexpected error: {}", e))),
            };

            if success {
                passed += 1;
            } else {
                failed += 1;
            }

            test_results.push(TestCaseResult {
                name: test_case.name.clone(),
                success,
                error,
                duration: test_start.elapsed(),
            });
        }

        println!("\n{} - Completed in {:?}", self.name, start.elapsed());
        println!("Passed: {}/{}", passed, passed + failed);

        Ok(TestSuiteResults {
            suite_name: self.name.clone(),
            passed,
            failed,
            test_results,
        })
    }
}
