//! Test all examples from README.md to ensure they are correct and functional
//!
//! These tests use instance-based session management which provides complete
//! isolation between test cases. They can now safely run in parallel.
//!
//! Run with: cargo test --test readme_examples_test

#[path = "testutils/mod.rs"]
mod testutils;

use graphlite::Value;
use testutils::test_fixture::TestFixture;

// ============================================================================
// Pattern Matching Examples Tests
// ============================================================================

#[test]
fn test_readme_pattern_matching_setup() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_pattern_test")
        .expect("Failed to setup graph");

    // Create people
    fixture
        .query(
            "INSERT (:Person {name: 'Alice', age: 30, city: 'NYC'}),
                (:Person {name: 'Bob', age: 25, city: 'NYC'}),
                (:Person {name: 'Carol', age: 28, city: 'SF'}),
                (:Person {name: 'Dave', age: 35, city: 'NYC'})",
        )
        .expect("Failed to insert people");

    // Create companies
    fixture
        .query(
            "INSERT (:Company {name: 'TechCorp', founded: '2010-01-01'}),
                (:Company {name: 'DataInc', founded: '2015-06-15'})",
        )
        .expect("Failed to insert companies");

    // Create KNOWS relationships
    fixture
        .query(
            "MATCH (alice:Person {name: 'Alice'}), (bob:Person {name: 'Bob'})
         INSERT (alice)-[:KNOWS {since: '2020-01-01'}]->(bob)",
        )
        .expect("Failed to create Alice-Bob KNOWS");

    fixture
        .query(
            "MATCH (bob:Person {name: 'Bob'}), (carol:Person {name: 'Carol'})
         INSERT (bob)-[:KNOWS {since: '2021-03-15'}]->(carol)",
        )
        .expect("Failed to create Bob-Carol KNOWS");

    fixture
        .query(
            "MATCH (alice:Person {name: 'Alice'}), (dave:Person {name: 'Dave'})
         INSERT (alice)-[:KNOWS {since: '2019-05-20'}]->(dave)",
        )
        .expect("Failed to create Alice-Dave KNOWS");

    fixture
        .query(
            "MATCH (carol:Person {name: 'Carol'}), (dave:Person {name: 'Dave'})
         INSERT (carol)-[:KNOWS {since: '2022-01-10'}]->(dave)",
        )
        .expect("Failed to create Carol-Dave KNOWS");

    // Create WORKS_AT relationships
    fixture
        .query(
            "MATCH (alice:Person {name: 'Alice'}), (tech:Company {name: 'TechCorp'})
         INSERT (alice)-[:WORKS_AT {role: 'Engineer', since: '2020-01-01'}]->(tech)",
        )
        .expect("Failed to create Alice WORKS_AT TechCorp");

    fixture
        .query(
            "MATCH (bob:Person {name: 'Bob'}), (tech:Company {name: 'TechCorp'})
         INSERT (bob)-[:WORKS_AT {role: 'Designer', since: '2021-01-01'}]->(tech)",
        )
        .expect("Failed to create Bob WORKS_AT TechCorp");

    fixture
        .query(
            "MATCH (carol:Person {name: 'Carol'}), (data:Company {name: 'DataInc'})
         INSERT (carol)-[:WORKS_AT {role: 'Analyst', since: '2022-01-01'}]->(data)",
        )
        .expect("Failed to create Carol WORKS_AT DataInc");
}

#[test]
fn test_readme_friends_of_friends() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_fof_test")
        .expect("Failed to setup graph");

    // Setup data
    setup_pattern_matching_data(&fixture);

    // Find friends of friends
    let result = fixture
        .query(
            "MATCH (person:Person)-[:KNOWS]->(friend)-[:KNOWS]->(fof)
         WHERE person.name = 'Alice'
         RETURN fof.name",
        )
        .expect("Friends of friends query should succeed");

    // Should return Carol and Dave
    assert!(
        !result.rows.is_empty(),
        "Should find at least one friend of friend"
    );

    let names: Vec<String> = result
        .rows
        .iter()
        .filter_map(|row| {
            if let Some(Value::String(name)) = row.values.get("fof.name") {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        names.contains(&"Carol".to_string()) || names.contains(&"Dave".to_string()),
        "Should find Carol or Dave as friends of friends"
    );
}

#[test]
#[ignore] // Variable-length path syntax not yet implemented
fn test_readme_variable_length_paths() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_varlen_test")
        .expect("Failed to setup graph");

    setup_pattern_matching_data(&fixture);

    // Variable-length paths - syntax not yet implemented
    let result = fixture
        .query(
            "MATCH (start:Person)-[:KNOWS{1,3}]->(end:Person)
         RETURN start.name, end.name",
        )
        .expect("Variable-length paths query should succeed");

    assert!(!result.rows.is_empty(), "Should find paths");
}

#[test]
fn test_readme_multiple_patterns_coworkers() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_coworkers_test")
        .expect("Failed to setup graph");

    setup_pattern_matching_data(&fixture);

    // Multiple patterns - find coworkers who know each other
    let result = fixture
        .query(
            "MATCH (a:Person)-[:WORKS_AT]->(company:Company),
               (a)-[:KNOWS]->(b:Person)-[:WORKS_AT]->(company)
         RETURN a.name, b.name, company.name",
        )
        .expect("Coworkers query should succeed");

    // Should find Alice and Bob at TechCorp
    assert!(
        !result.rows.is_empty(),
        "Should find coworkers who know each other"
    );
}

// ============================================================================
// Data Modification Examples Tests
// ============================================================================

#[test]
fn test_readme_insert_simple_node() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_insert_test")
        .expect("Failed to setup graph");

    // Insert nodes and relationships
    fixture
        .query("INSERT (n:Person {name: 'Charlie', age: 35})")
        .expect("Simple INSERT should succeed");

    // Verify
    let result = fixture
        .query("MATCH (p:Person {name: 'Charlie'}) RETURN p.age")
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("p.age"),
        Some(&Value::Number(35.0))
    );
}

#[test]
fn test_readme_insert_with_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_func_insert_test")
        .expect("Failed to setup graph");

    // Insert with function expressions
    fixture
        .query(
            "INSERT (:Product {
            name: upper('laptop'),
            price: abs(-1299.99),
            warranty: duration('P2Y')
        })",
        )
        .expect("INSERT with functions should succeed");

    // Verify
    let result = fixture
        .query("MATCH (p:Product) RETURN p.name, p.price, p.warranty")
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("p.name"),
        Some(&Value::String("LAPTOP".to_string()))
    );
    assert_eq!(
        result.rows[0].values.get("p.price"),
        Some(&Value::Number(1299.99))
    );
    // warranty is duration in seconds: P2Y = 2 years ≈ 63072000 seconds
    assert!(result.rows[0].values.get("p.warranty").is_some());
}

#[test]
fn test_readme_insert_edge_with_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_edge_func_test")
        .expect("Failed to setup graph");

    // Create nodes first
    fixture
        .query("INSERT (:Person {name: 'Alice'}), (:Person {name: 'Bob'})")
        .expect("Insert nodes should succeed");

    // Insert edges with function expressions
    fixture
        .query(
            "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
         INSERT (a)-[:KNOWS {
             since: duration('P5Y'),
             strength: round(0.857)
         }]->(b)",
        )
        .expect("INSERT edge with functions should succeed");

    // Verify
    let result = fixture
        .query(
            "MATCH (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'})
         RETURN r.since, r.strength",
        )
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("r.strength"),
        Some(&Value::Number(1.0))
    );
}

#[test]
fn test_readme_nested_functions_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_nested_insert_test")
        .expect("Failed to setup graph");

    // Nested function calls in INSERT
    fixture
        .query(
            "INSERT (:Data {
            value: round(abs(-42.7)),
            text: upper(lower('MiXeD CaSe'))
        })",
        )
        .expect("INSERT with nested functions should succeed");

    // Verify
    let result = fixture
        .query("MATCH (d:Data) RETURN d.value, d.text")
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("d.value"),
        Some(&Value::Number(43.0))
    );
    assert_eq!(
        result.rows[0].values.get("d.text"),
        Some(&Value::String("MIXED CASE".to_string()))
    );
}

#[test]
fn test_readme_set_node_with_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_set_node_test")
        .expect("Failed to setup graph");

    // Create node
    fixture
        .query("INSERT (:Person {name: 'Alice', age: 30})")
        .expect("Insert should succeed");

    // SET node properties with functions
    fixture
        .query(
            "MATCH (p:Person {name: 'Alice'})
         SET p.age = 31,
             p.name_upper = upper('alice'),
             p.account_age = duration('P5Y')",
        )
        .expect("SET with functions should succeed");

    // Verify
    let result = fixture
        .query(
            "MATCH (p:Person {name: 'Alice'})
         RETURN p.age, p.name_upper, p.account_age",
        )
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("p.age"),
        Some(&Value::Number(31.0))
    );
    assert_eq!(
        result.rows[0].values.get("p.name_upper"),
        Some(&Value::String("ALICE".to_string()))
    );
}

#[test]
fn test_readme_set_edge_with_functions() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_set_edge_test")
        .expect("Failed to setup graph");

    // Create nodes and edge
    fixture
        .query("INSERT (:Person {name: 'Alice'}), (:Person {name: 'Bob'})")
        .expect("Insert nodes should succeed");

    fixture
        .query(
            "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
         INSERT (a)-[:KNOWS]->(b)",
        )
        .expect("Insert edge should succeed");

    // SET edge properties with functions
    fixture
        .query(
            "MATCH (a:Person)-[r:KNOWS]->(b:Person)
         SET r.duration = duration('P5Y'),
             r.strength = abs(-0.9),
             r.label = upper('relationship')",
        )
        .expect("SET edge with functions should succeed");

    // Verify
    let result = fixture
        .query(
            "MATCH (a:Person)-[r:KNOWS]->(b:Person)
         RETURN r.strength, r.label",
        )
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("r.strength"),
        Some(&Value::Number(0.9))
    );
    assert_eq!(
        result.rows[0].values.get("r.label"),
        Some(&Value::String("RELATIONSHIP".to_string()))
    );
}

#[test]
fn test_readme_nested_functions_set() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_nested_set_test")
        .expect("Failed to setup graph");

    // Create node
    fixture
        .query("INSERT (:Product {name: 'Widget', price: 100.0})")
        .expect("Insert should succeed");

    // Nested functions in SET
    fixture
        .query(
            "MATCH (p:Product)
         SET p.price = round(abs(-99.99)),
             p.name = upper(lower('MiXeD'))",
        )
        .expect("SET with nested functions should succeed");

    // Verify
    let result = fixture
        .query("MATCH (p:Product) RETURN p.price, p.name")
        .expect("Query should succeed");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values.get("p.price"),
        Some(&Value::Number(100.0))
    );
    assert_eq!(
        result.rows[0].values.get("p.name"),
        Some(&Value::String("MIXED".to_string()))
    );
}

#[test]
fn test_readme_delete_node() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_delete_test")
        .expect("Failed to setup graph");

    // Create nodes
    fixture
        .query("INSERT (:Person {name: 'Bob'}), (:Person {name: 'Alice'})")
        .expect("Insert should succeed");

    // Delete nodes
    fixture
        .query("MATCH (p:Person {name: 'Bob'}) DELETE p")
        .expect("DELETE should succeed");

    // Verify Bob is deleted
    let result = fixture
        .query("MATCH (p:Person {name: 'Bob'}) RETURN p")
        .expect("Query should succeed");
    assert_eq!(result.rows.len(), 0, "Bob should be deleted");

    // Verify Alice still exists
    let result = fixture
        .query("MATCH (p:Person {name: 'Alice'}) RETURN p")
        .expect("Query should succeed");
    assert_eq!(result.rows.len(), 1, "Alice should still exist");
}

// ============================================================================
// Aggregation Examples Tests
// ============================================================================

#[test]
fn test_readme_count_relationships() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_count_test")
        .expect("Failed to setup graph");

    setup_pattern_matching_data(&fixture);

    // Count relationships per person
    let result = fixture
        .query(
            "MATCH (p:Person)-[r:KNOWS]->()
         RETURN p.name, COUNT(r) AS friend_count",
        )
        .expect("COUNT query should succeed");

    assert!(!result.rows.is_empty(), "Should have results");

    // Verify Alice has 2 friends
    let alice_row = result
        .rows
        .iter()
        .find(|row| row.values.get("p.name") == Some(&Value::String("Alice".to_string())));

    if let Some(row) = alice_row {
        assert_eq!(
            row.values.get("friend_count"),
            Some(&Value::Number(2.0)),
            "Alice should have 2 KNOWS relationships"
        );
    }
}

#[test]
fn test_readme_group_by_aggregate() {
    let fixture = TestFixture::new().expect("Failed to create fixture");
    fixture
        .setup_graph("readme_groupby_test")
        .expect("Failed to setup graph");

    setup_pattern_matching_data(&fixture);

    // Group by city and calculate statistics
    let result = fixture
        .query(
            "MATCH (p:Person)
         RETURN p.city, AVG(p.age) AS avg_age, COUNT(*) AS population
         GROUP BY p.city",
        )
        .expect("GROUP BY query should succeed");

    assert!(result.rows.len() >= 2, "Should have at least 2 cities");

    // Find NYC row
    let nyc_row = result
        .rows
        .iter()
        .find(|row| row.values.get("p.city") == Some(&Value::String("NYC".to_string())));

    if let Some(row) = nyc_row {
        // NYC has Alice (30), Bob (25), Dave (35) = avg 30.0, count 3
        assert_eq!(
            row.values.get("population"),
            Some(&Value::Number(3.0)),
            "NYC should have 3 people"
        );
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Setup the standard pattern matching test data
fn setup_pattern_matching_data(fixture: &TestFixture) {
    // Create people
    fixture
        .query(
            "INSERT (:Person {name: 'Alice', age: 30, city: 'NYC'}),
                (:Person {name: 'Bob', age: 25, city: 'NYC'}),
                (:Person {name: 'Carol', age: 28, city: 'SF'}),
                (:Person {name: 'Dave', age: 35, city: 'NYC'})",
        )
        .expect("Failed to insert people");

    // Create companies
    fixture
        .query(
            "INSERT (:Company {name: 'TechCorp', founded: '2010-01-01'}),
                (:Company {name: 'DataInc', founded: '2015-06-15'})",
        )
        .expect("Failed to insert companies");

    // Create KNOWS relationships
    fixture
        .query(
            "MATCH (alice:Person {name: 'Alice'}), (bob:Person {name: 'Bob'})
         INSERT (alice)-[:KNOWS {since: '2020-01-01'}]->(bob)",
        )
        .expect("Failed to create Alice-Bob KNOWS");

    fixture
        .query(
            "MATCH (bob:Person {name: 'Bob'}), (carol:Person {name: 'Carol'})
         INSERT (bob)-[:KNOWS {since: '2021-03-15'}]->(carol)",
        )
        .expect("Failed to create Bob-Carol KNOWS");

    fixture
        .query(
            "MATCH (alice:Person {name: 'Alice'}), (dave:Person {name: 'Dave'})
         INSERT (alice)-[:KNOWS {since: '2019-05-20'}]->(dave)",
        )
        .expect("Failed to create Alice-Dave KNOWS");

    fixture
        .query(
            "MATCH (carol:Person {name: 'Carol'}), (dave:Person {name: 'Dave'})
         INSERT (carol)-[:KNOWS {since: '2022-01-10'}]->(dave)",
        )
        .expect("Failed to create Carol-Dave KNOWS");

    // Create WORKS_AT relationships
    fixture
        .query(
            "MATCH (alice:Person {name: 'Alice'}), (tech:Company {name: 'TechCorp'})
         INSERT (alice)-[:WORKS_AT {role: 'Engineer', since: '2020-01-01'}]->(tech)",
        )
        .expect("Failed to create Alice WORKS_AT TechCorp");

    fixture
        .query(
            "MATCH (bob:Person {name: 'Bob'}), (tech:Company {name: 'TechCorp'})
         INSERT (bob)-[:WORKS_AT {role: 'Designer', since: '2021-01-01'}]->(tech)",
        )
        .expect("Failed to create Bob WORKS_AT TechCorp");

    fixture
        .query(
            "MATCH (carol:Person {name: 'Carol'}), (data:Company {name: 'DataInc'})
         INSERT (carol)-[:WORKS_AT {role: 'Analyst', since: '2022-01-01'}]->(data)",
        )
        .expect("Failed to create Carol WORKS_AT DataInc");
}
