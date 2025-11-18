//! Test the fixed TYPE and SIZE utility functions

#[path = "testutils/mod.rs"]
mod testutils;

use testutils::test_fixture::TestFixture;

#[test]
fn test_type_function_fixed() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");

    // Setup
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/type_test", fixture.schema_name()));
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH /{}/type_test", fixture.schema_name()));

    fixture.assert_query_succeeds(r#"
        INSERT (doc:Document {
            title: 'Test Doc',
            tags: ['tag1', 'tag2', 'tag3'],
            count: 42,
            flag: true
        })
    "#);


    // Test with array
    let result1 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.tags as arr
        RETURN TYPE(arr) as array_type
    "#);

    // Test with string
    let result2 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.title as title_str
        RETURN TYPE(title_str) as string_type
    "#);

    // Test with number
    let result3 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.count as num
        RETURN TYPE(num) as number_type
    "#);

    // Test with boolean
    let result4 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.flag as bool_val
        RETURN TYPE(bool_val) as bool_type
    "#);

    // Test with node
    let result5 = fixture.query(r#"
        MATCH (d:Document)
        RETURN TYPE(d) as node_type
    "#);
}

#[test]
fn test_size_function_fixed() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");

    // Setup
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/size_test", fixture.schema_name()));
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH /{}/size_test", fixture.schema_name()));

    fixture.assert_query_succeeds(r#"
        INSERT (doc:Document {
            title: 'Test Document',
            numbers: [1, 2, 3, 4, 5],
            list_data: ['a', 'b', 'c']
        })
    "#);


    // Test with number list
    let result1 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.numbers as nums
        RETURN SIZE(nums) as numbers_size
    "#);

    // Test with string
    let result2 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.title as title_str
        RETURN SIZE(title_str) as string_size
    "#);

    // Test with list
    let result3 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.list_data as list_val
        RETURN SIZE(list_val) as list_size
    "#);

    // Test with literal values
    let result4 = fixture.query(r#"
        RETURN
            SIZE([1, 2, 3, 4]) as literal_list_size,
            SIZE('hello world') as literal_string_size,
            SIZE([10, 20, 30]) as literal_numbers_size
    "#);
}

#[test]
fn test_combined_type_and_size() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");

    // Setup
    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/combined_test", fixture.schema_name()));
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH /{}/combined_test", fixture.schema_name()));

    fixture.assert_query_succeeds(r#"
        INSERT (doc:Document {
            title: 'Test Document',
            data: [0.1, 0.2, 0.3, 0.4, 0.5]
        })
    "#);


    // Test combined usage
    let result = fixture.query(r#"
        MATCH (d:Document)
        WITH d.data as arr
        RETURN
            arr,
            TYPE(arr) as data_type,
            SIZE(arr) as data_size
    "#);

    // Validate results
    match result {
        Ok(query_result) => {
            if let Some(row) = query_result.rows.first() {
            }
        }
        Err(e) => {
        }
    }
}

#[test]
fn debug_properties_function() {
    let fixture = TestFixture::empty().expect("Failed to create fixture");

    fixture.assert_query_succeeds(&format!("CREATE GRAPH /{}/props_debug", fixture.schema_name()));
    fixture.assert_query_succeeds(&format!("SESSION SET GRAPH /{}/props_debug", fixture.schema_name()));

    fixture.assert_query_succeeds(r#"
        INSERT (doc:Document {
            title: 'Test Doc',
            data: [1.0, 2.0, 3.0],
            count: 42
        })
    "#);


    // Test what properties() actually returns
    let result1 = fixture.query(r#"
        MATCH (d:Document)
        RETURN properties(d) as props
    "#);

    // Test direct property access vs properties function
    let result2 = fixture.query(r#"
        MATCH (d:Document)
        RETURN d.title, d.data, d.count, properties(d)
    "#);

    // Test dot notation property access
    let result3 = fixture.query(r#"
        MATCH (d:Document)
        WITH d.title as title_from_with, d.data as data_from_with
        RETURN title_from_with, data_from_with
    "#);
}
