//! WITH Clause Property Access Bug Investigation
//!
//! Focused test to identify why property access works in RETURN but fails in WITH

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_with_clause_property_access_bug() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");

    // Setup
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/bug_test", fixture.schema_name()));
    fixture.assert_query_succeeds(&format!(
        "SESSION SET GRAPH /{}/bug_test",
        fixture.schema_name()
    ));

    // Insert test node
    fixture.assert_query_succeeds(
        r#"
        INSERT (test:Node {
            id: 'test123',
            name: 'Test Node',
            data: [1.0, 2.0, 3.0],
            number: 42,
            text: 'hello'
        })
    "#,
    );

    // Test 1: Property access in RETURN (WORKS)
    let result1 = fixture.assert_query_succeeds(
        r#"
        MATCH (n:Node {id: 'test123'})
        RETURN n.data, n.number, n.text, n.name
    "#,
    );

    // Test 2: Property access in WITH (FAILS)
    let result2 = fixture.assert_query_succeeds(
        r#"
        MATCH (n:Node {id: 'test123'})
        WITH n.data as arr, n.number as num, n.text as txt, n.name as nm
        RETURN arr, num, txt, nm
    "#,
    );

    // Test 3: Compare node access vs property access
    let result3 = fixture.assert_query_succeeds(
        r#"
        MATCH (n:Node {id: 'test123'})
        WITH n as node, n.data as arr
        RETURN node, arr
    "#,
    );

    // Test 4: Workaround - pass node through WITH, access property in RETURN
    let result4 = fixture.assert_query_succeeds(
        r#"
        MATCH (n:Node {id: 'test123'})
        WITH n as node
        RETURN node.data, node.number, node.text, node.name
    "#,
    );
}

#[test]
fn test_with_clause_multiple_nodes_workaround() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");

    // Setup
    fixture.assert_query_succeeds(&format!(
        "CREATE GRAPH /{}/workaround",
        fixture.schema_name()
    ));
    fixture.assert_query_succeeds(&format!(
        "SESSION SET GRAPH /{}/workaround",
        fixture.schema_name()
    ));

    // Insert test documents
    fixture.assert_query_succeeds(
        r#"
        INSERT (doc1:Document {
            title: 'Doc1',
            score: 85
        })
    "#,
    );

    fixture.assert_query_succeeds(
        r#"
        INSERT (doc2:Document {
            title: 'Doc2',
            score: 92
        })
    "#,
    );

    // Workaround for multiple node property access using node passing
    let result = fixture.assert_query_succeeds(
        r#"
        MATCH (query_doc:Document {title: 'Doc1'}), (all_docs:Document)
        WITH query_doc, all_docs
        RETURN
            all_docs.title,
            query_doc.score as query_score,
            all_docs.score as doc_score
    "#,
    );

    // Verify the query returns results
    assert!(!result.rows.is_empty(), "Query should return results");
}
