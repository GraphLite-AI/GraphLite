from nl2gql.pipeline.preprocess import Preprocessor
from nl2gql.pipeline.schema_graph import SchemaGraph


def test_preprocessor_builds_filtered_schema_and_hints():
    schema = """
    Person: id, name, age
    Person-[:FRIEND]->Person
    """
    graph = SchemaGraph.from_text(schema)
    pre = Preprocessor(graph)

    result = pre.run("Find friends of a person", feedback=[])

    assert "Person" in result.filtered_schema.nodes
    assert any(edge.rel == "FRIEND" for edge in result.filtered_schema.edges)
    assert result.structural_hints, "expected structural hints to be populated"


