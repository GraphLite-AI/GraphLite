//! Simple LET statement test
//!
//! Tests the basic LET + RETURN functionality

#![allow(unused_variables)]

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_simple_let_return() {
    let fixture = TestFixture::new().expect("Failed to create test fixture");
    fixture.setup_graph("let_test").expect("Failed to setup graph");



    let result = fixture.query("LET test_list = [1, 2, 3, 4, 5] RETURN test_list");

    match result {
        Ok(query_result) => {

            for (i, row) in query_result.rows.iter().enumerate() {
            }

            // Assert expected behavior
            assert_eq!(query_result.rows.len(), 1, "Should return exactly 1 row");
        }
        Err(e) => {
            panic!("Query failed: {:?}", e);
        }
    }
}
