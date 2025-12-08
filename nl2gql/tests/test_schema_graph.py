from nl2gql.pipeline.schema_graph import SchemaGraph


def test_schema_graph_parses_nodes_and_edges():
    schema = """
    Person: id, name
    Company: id, name
    Person-[:FRIEND]->Person
    Person-[:WORKS_AT]->Company
    """
    graph = SchemaGraph.from_text(schema)

    assert graph.has_node("Person")
    assert graph.has_property("Person", "name")
    assert graph.edge_exists("Person", "FRIEND", "Person")
    assert graph.edge_exists("Person", "WORKS_AT", "Company")

    description = graph.describe_full()
    assert "Person" in description and "Company" in description


