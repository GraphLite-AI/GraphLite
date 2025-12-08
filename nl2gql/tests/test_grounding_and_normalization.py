from dataclasses import dataclass

from nl2gql.pipeline.generator import QueryGenerator
from nl2gql.pipeline.intent_linker import ground_links_to_schema
from nl2gql.pipeline.ir import IREdge, IRNode, IRReturn, ISOQueryIR
from nl2gql.pipeline.refiner import Refiner
from nl2gql.pipeline.runner import SyntaxResult
from nl2gql.pipeline.schema_graph import SchemaGraph


@dataclass
class _FakeRunner:
    def validate(self, query: str) -> SyntaxResult:
        return SyntaxResult(ok=True, rows=None)

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return False


def test_ground_links_to_schema_normalizes_labels_and_properties():
    schema = """
    Person: id, name
    Person-[:FRIEND]->Person
    """
    graph = SchemaGraph.from_text(schema)
    links = {
        "node_links": [{"alias": "A", "label": "person"}],
        "property_links": [{"alias": "A", "property": "Name"}],
        "rel_links": [{"left_alias": "A", "rel": "friend", "right_alias": "A"}],
    }

    grounded = ground_links_to_schema(links, graph)

    assert grounded["node_links"][0]["label"] == "Person"
    assert grounded["property_links"][0]["property"] == "name"
    assert grounded["rel_links"][0]["rel"] == "FRIEND"


def test_normalize_aliases_avoids_reserved_words_and_updates_refs():
    schema = """
    Person: id, name
    Person-[:KNOWS]->Person
    """
    graph = SchemaGraph.from_text(schema)
    generator = QueryGenerator(model="noop")
    refiner = Refiner(graph, generator=generator, runner=_FakeRunner(), max_loops=1)

    ir = ISOQueryIR(
        nodes={"Match": IRNode("Match"), "b": IRNode("b")},
        edges=[IREdge("Match", "KNOWS", "b")],
        returns=[IRReturn(expr="Match.id")],
    )

    mapping = refiner._normalize_aliases(ir)

    assert mapping["Match"] != "Match"
    assert "match" not in ir.nodes
    assert any(edge.left_alias == mapping["Match"] for edge in ir.edges)
    assert ir.returns[0].expr.startswith(mapping["Match"])


