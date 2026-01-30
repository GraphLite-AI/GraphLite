//! Test for ORDER BY DESC functionality
//!
//! This test verifies that ORDER BY with DESC direction correctly sorts results
//! in descending order.

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

#[test]
fn test_order_by_desc_simple() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup graph
    fixture
        .setup_graph("order_test")
        .expect("Failed to setup graph");

    // Insert test data with different ages
    fixture
        .query("INSERT (:Person {name: 'Alice', age: 30})")
        .expect("Failed to insert Alice");
    fixture
        .query("INSERT (:Person {name: 'Bob', age: 25})")
        .expect("Failed to insert Bob");
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35})")
        .expect("Failed to insert Charlie");

    // Query with ORDER BY DESC - should return Charlie (35), Alice (30), Bob (25)
    let result = fixture
        .query("MATCH (p:Person) RETURN p.name as name, p.age as age ORDER BY p.age DESC")
        .expect("Query with ORDER BY DESC should succeed");

    // Extract ages in order
    let ages: Vec<f64> = result
        .rows
        .iter()
        .filter_map(|row| row.values.get("age"))
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();

    println!("ORDER BY DESC - Ages in result order: {:?}", ages);
    println!("Expected: [35.0, 30.0, 25.0]");

    // With DESC order: Charlie (35) should be first, Alice (30) second, Bob (25) third
    assert_eq!(
        ages,
        vec![35.0, 30.0, 25.0],
        "ORDER BY p.age DESC should sort ages in descending order"
    );
}

#[test]
fn test_order_by_asc_simple() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup graph
    fixture
        .setup_graph("order_asc_test")
        .expect("Failed to setup graph");

    // Insert test data with different ages
    fixture
        .query("INSERT (:Person {name: 'Alice', age: 30})")
        .expect("Failed to insert Alice");
    fixture
        .query("INSERT (:Person {name: 'Bob', age: 25})")
        .expect("Failed to insert Bob");
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35})")
        .expect("Failed to insert Charlie");

    // Query with ORDER BY ASC - should return Bob (25), Alice (30), Charlie (35)
    let result = fixture
        .query("MATCH (p:Person) RETURN p.name as name, p.age as age ORDER BY p.age ASC")
        .expect("Query with ORDER BY ASC should succeed");

    // Extract ages in order
    let ages: Vec<f64> = result
        .rows
        .iter()
        .filter_map(|row| row.values.get("age"))
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();

    println!("ORDER BY ASC - Ages in result order: {:?}", ages);
    println!("Expected: [25.0, 30.0, 35.0]");

    // With ASC order: Bob (25) should be first, Alice (30) second, Charlie (35) third
    assert_eq!(
        ages,
        vec![25.0, 30.0, 35.0],
        "ORDER BY p.age ASC should sort ages in ascending order"
    );
}

#[test]
fn test_order_by_default_is_asc() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup graph
    fixture
        .setup_graph("order_default_test")
        .expect("Failed to setup graph");

    // Insert test data with different ages
    fixture
        .query("INSERT (:Person {name: 'Alice', age: 30})")
        .expect("Failed to insert Alice");
    fixture
        .query("INSERT (:Person {name: 'Bob', age: 25})")
        .expect("Failed to insert Bob");
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35})")
        .expect("Failed to insert Charlie");

    // Query with ORDER BY (no direction) - should default to ASC
    let result = fixture
        .query("MATCH (p:Person) RETURN p.name as name, p.age as age ORDER BY p.age")
        .expect("Query with ORDER BY (default) should succeed");

    // Extract ages in order
    let ages: Vec<f64> = result
        .rows
        .iter()
        .filter_map(|row| row.values.get("age"))
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();

    println!("ORDER BY (default) - Ages in result order: {:?}", ages);
    println!("Expected: [25.0, 30.0, 35.0]");

    // Default should be ASC: Bob (25) first, Alice (30) second, Charlie (35) third
    assert_eq!(
        ages,
        vec![25.0, 30.0, 35.0],
        "ORDER BY p.age (default) should sort ages in ascending order"
    );
}

#[test]
fn test_order_by_desc_with_where() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup graph
    fixture
        .setup_graph("order_where_test")
        .expect("Failed to setup graph");

    // Insert test data with different ages
    fixture
        .query("INSERT (:Person {name: 'Alice', age: 30})")
        .expect("Failed to insert Alice");
    fixture
        .query("INSERT (:Person {name: 'Bob', age: 25})")
        .expect("Failed to insert Bob");
    fixture
        .query("INSERT (:Person {name: 'Charlie', age: 35})")
        .expect("Failed to insert Charlie");

    // Query with WHERE and ORDER BY DESC - only ages > 25
    let result = fixture
        .query("MATCH (p:Person) WHERE p.age > 25 RETURN p.name as name, p.age as age ORDER BY p.age DESC")
        .expect("Query with WHERE and ORDER BY DESC should succeed");

    // Extract ages in order
    let ages: Vec<f64> = result
        .rows
        .iter()
        .filter_map(|row| row.values.get("age"))
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();

    println!("WHERE + ORDER BY DESC - Ages in result order: {:?}", ages);
    println!("Expected: [35.0, 30.0]");

    // With WHERE p.age > 25 and DESC: Charlie (35) first, Alice (30) second
    assert_eq!(
        ages,
        vec![35.0, 30.0],
        "WHERE p.age > 25 ORDER BY p.age DESC should return [35, 30]"
    );
}
