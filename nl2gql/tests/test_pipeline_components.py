import types
import unittest
from pathlib import Path

from nl2gql.pipeline import (
    CandidateQuery,
    FilteredSchema,
    IRFilter,
    IREdge,
    IRNode,
    IROrder,
    IRReturn,
    ISOQueryIR,
    IntentLinkGuidance,
    PreprocessResult,
    Preprocessor,
    Refiner,
    SchemaGraph,
    SyntaxResult,
)

SCHEMA_TEXT = (Path(__file__).resolve().parents[1] / "sample_schema.txt").read_text(encoding="utf-8")


class SchemaGraphTests(unittest.TestCase):
    def test_parses_entities_and_edges(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)
        self.assertIn("Person", graph.nodes)
        self.assertIn("Company", graph.nodes)
        self.assertTrue(graph.edge_exists("Person", "WORKS_AT", "Company"))
        self.assertIn("name", graph.list_properties("City"))
        paths = graph.shortest_paths({"Person"}, {"City"})
        self.assertTrue(any(path for path in paths if path[0].rel in {"LIVES_IN", "WORKS_AT"}))


class PreprocessorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.graph = SchemaGraph.from_text(SCHEMA_TEXT)
        self.preprocessor = Preprocessor(self.graph)

    def test_filters_schema_and_hints(self):
        nl = "List Person name and Company industry for each WORKS_AT relationship"
        result = self.preprocessor.run(nl, feedback=[])

        self.assertIn("Person", result.filtered_schema.nodes)
        self.assertIn("Company", result.filtered_schema.nodes)
        edge_descriptors = [e.descriptor() for e in result.filtered_schema.edges]
        self.assertIn("(Person)-[:WORKS_AT]->(Company)", edge_descriptors)
        self.assertTrue(any("WORKS_AT" in hint for hint in result.structural_hints))
        self.assertEqual(result.normalized_nl, nl)


class ISOQueryIRTests(unittest.TestCase):
    def test_parse_render_roundtrip(self):
        raw = (
            "MATCH (p:Person)-[:WORKS_AT]->(c:Company)\n"
            "WHERE p.age > 30\n"
            "RETURN c.name, count(p) AS headcount\n"
            "ORDER BY headcount DESC\n"
            "LIMIT 5"
        )
        ir, errors = ISOQueryIR.parse(raw)
        self.assertIsNotNone(ir)
        self.assertFalse(errors)
        self.assertEqual(len(ir.nodes), 2)
        self.assertEqual(len(ir.edges), 1)
        self.assertEqual(ir.limit, 5)
        rendered = ir.render()
        self.assertIn("MATCH (p:Person)-[:WORKS_AT]->(c:Company)", rendered)
        self.assertIn("RETURN c.name, count(p) AS headcount", rendered)
        self.assertEqual(ir.validate_bindings(), [])


class RefinerIntegrationTests(unittest.TestCase):
    def test_refiner_accepts_deterministic_candidate(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)
        filtered = FilteredSchema(
            nodes=graph.nodes,
            edges=graph.edges,
            strategy_hits={"exact": ["Person", "Company"], "ner_masked": [], "semantic": []},
            path_hints=["Person -[:WORKS_AT]-> Company"],
        )

        class FakePreprocessor:
            def run(self, nl, feedback):
                return PreprocessResult(
                    raw_nl=nl,
                    normalized_nl=nl,
                    phrases=[],
                    filtered_schema=filtered,
                    structural_hints=filtered.path_hints,
                )

        class FakeIntentLinker:
            def run(self, _nl, _pre, _feedback):
                frame = {"path_hints": ["Person -[:WORKS_AT]-> Company"]}
                links = {
                    "node_links": [{"alias": "p", "label": "Person"}, {"alias": "c", "label": "Company"}],
                    "property_links": [],
                    "rel_links": [{"left_alias": "p", "rel": "WORKS_AT", "right_alias": "c"}],
                    "canonical_aliases": {},
                }
                return IntentLinkGuidance(frame=frame, links=links)

        ir = ISOQueryIR(
            nodes={"p": IRNode(alias="p", label="Person"), "c": IRNode(alias="c", label="Company")},
            edges=[IREdge(left_alias="p", rel="WORKS_AT", right_alias="c")],
            filters=[IRFilter(alias="p", prop="age", op=">", value=25)],
            returns=[IRReturn(expr="p.name"), IRReturn(expr="c.name", alias="company")],
            order_by=[IROrder(expr="p.name", direction="ASC")],
            limit=10,
        )

        class FakeGenerator:
            def generate(self, _pre, _failures, _guidance):
                return [CandidateQuery(query=ir.render(), reason="deterministic stub")]

        class DummyRunner:
            def __enter__(self):
                return self

            def __exit__(self, exc_type, exc, tb):
                return None

            def validate(self, _query: str):
                return SyntaxResult(ok=True, rows=[])

        logic_validator = types.SimpleNamespace(validate=lambda _nl, _schema, _query, _hints: (True, None))

        refiner = Refiner(
            graph,
            generator=FakeGenerator(),  # type: ignore[arg-type]
            logic_validator=logic_validator,  # type: ignore[arg-type]
            runner=DummyRunner(),  # type: ignore[arg-type]
            max_loops=1,
        )

        final_query, _timeline = refiner.run(
            nl="names and companies",
            preprocessor=FakePreprocessor(),
            intent_linker=FakeIntentLinker(),
            spinner=None,
        )

        self.assertIn("MATCH", final_query)
        self.assertIn("RETURN", final_query)


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
