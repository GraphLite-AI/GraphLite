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
from nl2gql.pipeline.intent_judge import IntentJudgeResult
from nl2gql.pipeline.requirements import RequirementContract, coverage_violations

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

class PlanParsingTests(unittest.TestCase):
    def test_plan_rejects_truncated_match(self):
        from nl2gql.pipeline.generator import Plan

        plan = Plan.from_raw(
            {
                "match": ["(n1:Library)-[:"],
                "return": ["n1.name"],
            }
        )
        self.assertIsNone(plan)


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
        intent_stub = types.SimpleNamespace(
            evaluate=lambda *_args, **_kwargs: IntentJudgeResult(valid=True, reasons=[], missing_requirements=[])
        )

        refiner = Refiner(
            graph,
            generator=FakeGenerator(),  # type: ignore[arg-type]
            logic_validator=logic_validator,  # type: ignore[arg-type]
            runner=DummyRunner(),  # type: ignore[arg-type]
            intent_judge=intent_stub,  # type: ignore[arg-type]
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


class RefinerGroupingTests(unittest.TestCase):
    def test_grouping_rewrites_without_alias_of_alias(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)

        dummy_runner = types.SimpleNamespace(validate=lambda _query: SyntaxResult(ok=True, rows=[]))
        dummy_logic = types.SimpleNamespace(validate=lambda *_args, **_kwargs: (True, None))
        intent_stub = types.SimpleNamespace(
            evaluate=lambda *_args, **_kwargs: IntentJudgeResult(valid=True, reasons=[], missing_requirements=[])
        )
        refiner = Refiner(
            graph,
            generator=types.SimpleNamespace(),
            logic_validator=dummy_logic,
            runner=dummy_runner,
            intent_judge=intent_stub,
        )

        ir = ISOQueryIR(
            nodes={
                "p": IRNode(alias="p", label="Person"),
                "c": IRNode(alias="c", label="Company"),
                "ct": IRNode(alias="ct", label="City"),
            },
            edges=[
                IREdge(left_alias="p", rel="WORKS_AT", right_alias="c"),
                IREdge(left_alias="c", rel="LOCATED_IN", right_alias="ct"),
            ],
            filters=[
                IRFilter(alias="p", prop="age", op=">", value=30),
                IRFilter(alias="p", prop="salary", op=">", value=120000),
                IRFilter(alias="ct", prop="name", op="=", value="San Francisco"),
            ],
            with_items=["c", "count(p.id) AS headcount", "average(p.salary) AS avg_salary"],
            returns=[
                IRReturn(expr="c.name", alias="company_name"),
                IRReturn(expr="c.id", alias="company_id"),
                IRReturn(expr="count(p.id)", alias="headcount"),
                IRReturn(expr="average(p.salary)", alias="avg_salary"),
            ],
            order_by=[IROrder(expr="headcount", direction="DESC"), IROrder(expr="avg_salary", direction="DESC")],
            limit=10,
        )

        refiner._ensure_grouping(ir)

        self.assertEqual(
            ir.with_items,
            [
                "c",
                "count(p.id) AS headcount",
                "average(p.salary) AS avg_salary",
                "c.name AS company_name",
                "c.id AS company_id",
            ],
        )
        self.assertEqual([r.expr for r in ir.returns], ["company_name", "company_id", "headcount", "avg_salary"])
        self.assertEqual([o.expr for o in ir.order_by], ["headcount", "avg_salary"])

class RefinerDistinctFilterTests(unittest.TestCase):
    def test_existing_distinct_filter_is_recorded_for_coverage(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)

        dummy_runner = types.SimpleNamespace(validate=lambda _query: SyntaxResult(ok=True, rows=[]))
        dummy_logic = types.SimpleNamespace(validate=lambda *_args, **_kwargs: (True, None))
        intent_stub = types.SimpleNamespace(
            evaluate=lambda *_args, **_kwargs: IntentJudgeResult(valid=True, reasons=[], missing_requirements=[])
        )
        refiner = Refiner(
            graph,
            generator=types.SimpleNamespace(),
            logic_validator=dummy_logic,
            runner=dummy_runner,
            intent_judge=intent_stub,
        )

        ir = ISOQueryIR(
            nodes={
                "n3": IRNode(alias="n3", label="City"),
                "ci": IRNode(alias="ci", label="City"),
            },
            filters=[
                IRFilter(alias="n3", prop="id", op="<>", value={"ref_alias": "ci", "ref_property": "id"}),
            ],
            returns=[IRReturn(expr="n3.name")],
        )

        contract = RequirementContract(
            role_constraints={
                "n3": types.SimpleNamespace(label="City", distinct_from=["city_distinct_2"]),
                "city_distinct_2": types.SimpleNamespace(label="City", distinct_from=["n3"]),
            }
        )
        # Patch in real RoleConstraint objects to match contract type.
        from nl2gql.pipeline.requirements import RoleConstraint as RC

        contract.role_constraints = {
            "n3": RC(label="City", distinct_from=["city_distinct_2"]),
            "city_distinct_2": RC(label="City", distinct_from=["n3"]),
        }

        refiner._enforce_contract_structure(ir, contract)
        self.assertIn(("ci", "n3"), contract.role_distinct_filters)


class RefinerDistinctRelRoleTests(unittest.TestCase):
    def test_required_distinct_roles_use_distinct_aliases(self):
        schema_text = "\n".join(
            [
                "- Flight: id",
                "- Airport: id, code",
                "- (Flight)-[:ORIGIN]->(Airport)",
                "- (Flight)-[:DESTINATION]->(Airport)",
            ]
        )
        graph = SchemaGraph.from_text(schema_text)

        dummy_runner = types.SimpleNamespace(validate=lambda _query: SyntaxResult(ok=True, rows=[]))
        dummy_logic = types.SimpleNamespace(validate=lambda *_args, **_kwargs: (True, None))
        intent_stub = types.SimpleNamespace(
            evaluate=lambda *_args, **_kwargs: IntentJudgeResult(valid=True, reasons=[], missing_requirements=[])
        )
        refiner = Refiner(
            graph,
            generator=types.SimpleNamespace(),
            logic_validator=dummy_logic,
            runner=dummy_runner,
            intent_judge=intent_stub,
        )

        ir = ISOQueryIR(
            nodes={
                "f": IRNode(alias="f", label="Flight"),
                "a": IRNode(alias="a", label="Airport"),
            },
            edges=[
                IREdge(left_alias="f", rel="ORIGIN", right_alias="a"),
            ],
            returns=[IRReturn(expr="a.code")],
        )

        contract = RequirementContract(
            required_labels={"Flight", "Airport"},
            required_edges={
                ("Flight", "ORIGIN", "Airport"),
                ("Flight", "DESTINATION", "Airport"),
            },
            required_distinct_roles={("Flight", "ORIGIN", "DESTINATION", "Airport")},
        )

        refiner._enforce_contract_structure(ir, contract)
        origins = [e.right_alias for e in ir.edges if e.rel == "ORIGIN"]
        dests = [e.right_alias for e in ir.edges if e.rel == "DESTINATION"]
        self.assertTrue(origins and dests)
        self.assertNotEqual(origins[0], dests[0])


class CoverageNormalizationTests(unittest.TestCase):
    def test_missing_required_output_detected(self):
        ir = ISOQueryIR(
            nodes={
                "p": IRNode(alias="p", label="Person"),
                "c": IRNode(alias="c", label="Company"),
            },
            edges=[IREdge(left_alias="p", rel="WORKS_AT", right_alias="c")],
            returns=[IRReturn(expr="count(p.id)", alias="headcount")],
        )

        contract = RequirementContract(required_outputs=["Company.name", "count(Person.id)"])
        errors = coverage_violations(contract, ir, ir.render())
        self.assertIn("missing required output company.name", errors)

    def test_with_alias_satisfies_required_output(self):
        ir = ISOQueryIR(
            nodes={
                "p": IRNode(alias="p", label="Person"),
                "c": IRNode(alias="c", label="Company"),
            },
            edges=[IREdge(left_alias="p", rel="WORKS_AT", right_alias="c")],
            with_items=["c.name AS company_name", "count(p.id) AS employee_count"],
            returns=[IRReturn(expr="company_name"), IRReturn(expr="employee_count")],
        )

        contract = RequirementContract(required_outputs=["Company.name", "count(Person.id)"])
        errors = coverage_violations(contract, ir, ir.render())
        self.assertEqual(errors, [])

    def test_alias_order_matches_required_aggregate(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)
        ir = ISOQueryIR(
            nodes={
                "p": IRNode(alias="p", label="Person"),
                "c": IRNode(alias="c", label="Company"),
                "ct": IRNode(alias="ct", label="City"),
            },
            edges=[
                IREdge(left_alias="p", rel="WORKS_AT", right_alias="c"),
                IREdge(left_alias="c", rel="LOCATED_IN", right_alias="ct"),
            ],
            with_items=[
                "count(p.id) AS headcount",
                "average(p.salary) AS avg_salary",
            ],
            returns=[
                IRReturn(expr="c.name", alias="company_name"),
                IRReturn(expr="c.id", alias="company_id"),
                IRReturn(expr="headcount"),
                IRReturn(expr="avg_salary"),
            ],
            order_by=[
                IROrder(expr="headcount", direction="DESC"),
                IROrder(expr="avg_salary", direction="DESC"),
            ],
            limit=10,
        )

        contract = RequirementContract(
            required_order=["headcount DESC", "average(Person.salary) DESC"]
        )
        errors = coverage_violations(contract, ir, ir.render())
        self.assertNotIn("missing required order key average(person.salary)", errors)
        self.assertEqual(errors, [])

    def test_alias_order_matches_unknown_function(self):
        """Ensure arbitrary function aliases resolve for order coverage."""
        graph = SchemaGraph.from_text(SCHEMA_TEXT)
        ir = ISOQueryIR(
            nodes={
                "p": IRNode(alias="p", label="Person"),
                "c": IRNode(alias="c", label="Company"),
            },
            edges=[
                IREdge(left_alias="p", rel="WORKS_AT", right_alias="c"),
            ],
            with_items=[
                "WeIrDFuNc(p.salary) AS funky_salary",
            ],
            returns=[
                IRReturn(expr="c.name", alias="company_name"),
                IRReturn(expr="funky_salary"),
            ],
            order_by=[
                IROrder(expr="funky_salary", direction="DESC"),
            ],
            limit=10,
        )

        contract = RequirementContract(required_order=["WeIrDFuNc(Person.salary) DESC"])
        errors = coverage_violations(contract, ir, ir.render())
        self.assertEqual(errors, [])

    def test_missing_required_label_detected(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)
        ir = ISOQueryIR(
            nodes={
                "p": IRNode(alias="p", label="Person"),
                "c": IRNode(alias="c", label="Company"),
            },
            edges=[IREdge(left_alias="p", rel="WORKS_AT", right_alias="c")],
            returns=[IRReturn(expr="c.name")],
        )
        contract = RequirementContract(required_labels={"City"})
        errors = coverage_violations(contract, ir, ir.render())
        self.assertEqual(errors, ["missing required label City"])

    def test_limit_violation_reported(self):
        graph = SchemaGraph.from_text(SCHEMA_TEXT)
        ir = ISOQueryIR(
            nodes={"p": IRNode(alias="p", label="Person")},
            returns=[IRReturn(expr="p.name")],
            limit=50,
        )
        contract = RequirementContract(limit=10)
        errors = coverage_violations(contract, ir, ir.render())
        self.assertEqual(errors, ["limit should be <= 10"])
if __name__ == "__main__":  # pragma: no cover
    unittest.main()
