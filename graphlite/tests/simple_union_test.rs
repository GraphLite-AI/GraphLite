#[path = "testutils/mod.rs"]
mod testutils;
use testutils::test_fixture::TestFixture;
use graphlite::Value;

#[test]
fn test_simple_union_without_relationships() {
    let fixture = TestFixture::empty().expect("Failed to create test fixture");

    // Create schema (required before creating graphs - ISO GQL compliant)
    fixture.assert_query_succeeds(&format!("CREATE SCHEMA IF NOT EXISTS /{}", fixture.schema_name()));

    // Create a test graph
    let graph_name = "simple_union_test";
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/{}", fixture.schema_name(), graph_name));
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH /{}/{}", fixture.schema_name(), graph_name));

    // Insert simple test data
    fixture.assert_query_succeeds("INSERT (p1:Person {name: 'Alice', age: 25})");
    fixture.assert_query_succeeds("INSERT (p2:Person {name: 'Bob', age: 35})");
    fixture.assert_query_succeeds("INSERT (p3:Person {name: 'Charlie', age: 45})");

    // Test UNION operation
    let query = "MATCH (p:Person) WHERE p.age < 30 RETURN p.name UNION MATCH (p:Person) WHERE p.age > 40 RETURN p.name";

    let result = fixture.assert_query_succeeds(query);

    // Should find Alice (age 25) and Charlie (age 45)
    assert_eq!(result.rows.len(), 2, "UNION should return 2 rows");

    let names: Vec<String> = result.rows.iter()
        .filter_map(|row| {
            if let Some(Value::String(name)) = row.values.get("p.name") {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(names.contains(&"Alice".to_string()));
    assert!(names.contains(&"Charlie".to_string()));
}
