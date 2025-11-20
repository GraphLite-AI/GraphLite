#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_fraud_fixture_creation_debug() {
    // Try to create a regular fixture first
    let fixture = TestFixture::new().expect("Failed to create basic fixture");

    // Test CREATE GRAPH
    let create_result = fixture.query(&format!(
        "CREATE GRAPH /{}/fraud_graph",
        fixture.schema_name()
    ));
    if let Err(e) = create_result {
        panic!("✗ CREATE GRAPH failed: {}", e);
    }

    // Test SESSION SET
    let session_result = fixture.query(&format!(
        "SESSION SET GRAPH /{}/fraud_graph",
        fixture.schema_name()
    ));
    if let Err(e) = session_result {
        panic!("✗ SESSION SET failed: {}", e);
    }

    // Test Account INSERT
    let schema_id = fixture
        .schema_name()
        .replace("test_", "t")
        .replace("_", "x");
    let account_query = format!(
        "INSERT (a{}x1:Account {{id: 1, name: 'Account1', balance: 101.0, status: 'active', account_type: 'savings'}})",
        schema_id
    );
    if let Err(e) = fixture.query(&account_query) {
        panic!("✗ Account INSERT failed: {}", e);
    }

    // Test Merchant INSERT
    let merchant_query = format!(
        "INSERT (m{}x1:Merchant {{id: 1, name: 'Merchant1', category: 'retail'}})",
        schema_id
    );
    if let Err(e) = fixture.query(&merchant_query) {
        panic!("✗ Merchant INSERT failed: {}", e);
    }

    // Test MATCH + INSERT
    let match_query = format!(
        "MATCH (a:Account {{id: 1}}), (m:Merchant {{id: 1}}) INSERT (a)-[:Transaction {{id: '{}x1', amount: 2.5, timestamp: '2024-01-01T00:00:00Z'}}]->(m)",
        schema_id
    );
    if let Err(e) = fixture.query(&match_query) {
        panic!("✗ MATCH + INSERT failed: {}", e);
    }

    // Now test the full fraud fixture
    let full_fixture = TestFixture::with_fraud_data();
    match full_fixture {
        Ok(_) => {}
        Err(e) => panic!("❌ Full fraud fixture failed: {}", e),
    }
}
