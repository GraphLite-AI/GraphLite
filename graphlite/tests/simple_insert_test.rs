#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_simple_insert() {
    let fixture = TestFixture::new().expect("Failed to create fixture");

    // Setup fresh graph for this test
    fixture
        .setup_graph("test_graph")
        .expect("Failed to setup graph");

    // Try simplest possible INSERT
    fixture
        .query("INSERT (n:TestNode {id: 1})")
        .expect("Failed to insert simple node");

    // Test each property type individually
    fixture
        .query("INSERT (a1:Account {id: 1})")
        .expect("Failed with just id");

    fixture
        .query("INSERT (a2:Account {name: 'Account1'})")
        .expect("Failed with name property");

    fixture
        .query("INSERT (a3:Account {balance: 100.0})")
        .expect("Failed with balance property");

    // Try with quoted property name
    fixture
        .query("INSERT (a4:Account {`status`: 'active'})")
        .expect("Failed with quoted status property");

    fixture
        .query("INSERT (a5:Account {account_type: 'savings'})")
        .expect("Failed with account_type property");

    // Try full Account node like fraud fixture uses
    fixture.query("INSERT (a:Account {id: 1, name: 'Account1', balance: 100.0, status: 'active', account_type: 'savings'})")
        .expect("Failed to insert full Account node");
}
