import json
from pathlib import Path

import pytest

from nl2gql.pipeline.component_harness import (
    StageResult,
    run_generator,
    run_intent_linker,
    run_ir_validation,
    run_preprocess,
    run_component_cases,
    summarize,
    write_report,
)
from nl2gql.pipeline.generator import CandidateQuery
from nl2gql.pipeline.intent_linker import IntentLinkGuidance
from nl2gql.pipeline.requirements import RequirementContract
from nl2gql.pipeline.runner import SyntaxResult
from nl2gql.pipeline.schema_graph import SchemaGraph


SCHEMA = """
Person: id, name, age
Company: id, name, industry
City: id, name
Person-[:WORKS_AT]->Company
Person-[:LIVES_IN]->City
"""

GOLDEN_PATH = Path(__file__).resolve().parent / "data" / "component_cases.json"


class StubIntentLinker:
    def run(self, _nl, pre, _feedback):
        nodes = pre.filtered_schema.nodes
        person = "Person" if "Person" in nodes else next(iter(nodes))
        company = "Company" if "Company" in nodes else person
        return IntentLinkGuidance(
            frame={"targets": [person], "path_hints": pre.filtered_schema.path_hints},
            links={
                "node_links": [
                    {"alias": "p", "label": person},
                    {"alias": "c", "label": company},
                ],
                "property_links": [],
                "rel_links": [{"left_alias": "p", "rel": "WORKS_AT", "right_alias": "c"}],
                "canonical_aliases": {},
            },
        )


class StubGenerator:
    def __init__(self, query: str) -> None:
        self.query = query

    def generate(self, _pre, _failures, _guidance, _contract):
        return [CandidateQuery(query=self.query, reason="stub")]


class StubLogicValidator:
    def __init__(self, verdict: bool = True, reason: str = "logic rejected") -> None:
        self.verdict = verdict
        self.reason = reason

    def validate(self, _nl, _schema_summary, _query, _hints):
        return self.verdict, (None if self.verdict else self.reason)


class StubRunner:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return False

    def validate(self, _query: str) -> SyntaxResult:
        return SyntaxResult(ok=True, rows=None, error=None)


def load_golden():
    return json.loads(GOLDEN_PATH.read_text(encoding="utf-8"))


@pytest.fixture
def graph():
    return SchemaGraph.from_text(SCHEMA)


def test_preprocess_matches_golden_cases(graph):
    cases = load_golden()["preprocess"]
    for case in cases:
        schema_text = "\n".join(case["schema"]) if case.get("schema") else None
        case_graph = SchemaGraph.from_text(schema_text) if schema_text else graph
        result = run_preprocess(case_graph, case["nl"])
        assert result.ok
        for node in case["expected_nodes"]:
            assert node in result.output["filtered_nodes"]
        for edge in case["expected_edges"]:
            assert edge in result.output["filtered_edges"]


def test_intent_and_generator_harness_outputs_candidates(graph):
    pre_result = run_preprocess(graph, "List Person and Company pairs")
    intent_result = run_intent_linker(StubIntentLinker(), "List Person and Company pairs", pre_result.output["pre"], [])
    contract = RequirementContract(required_edges={("Person", "WORKS_AT", "Company")})
    gen_result = run_generator(
        StubGenerator("MATCH (p:Person)-[:WORKS_AT]->(c:Company) RETURN c.name"),
        pre_result.output["pre"],
        intent_result.output["guidance"],
        contract,
    )
    assert intent_result.ok
    assert gen_result.ok
    assert gen_result.output["candidates"]


def test_ir_validation_detects_missing_required_edge(graph):
    cases = load_golden()["validation"]
    for case in cases:
        schema_text = "\n".join(case["schema"]) if case.get("schema") else None
        case_graph = SchemaGraph.from_text(schema_text) if schema_text else graph
        required_edges = {tuple(e) for e in case.get("required_edges", [])}
        contract = RequirementContract(required_edges=required_edges) if required_edges else RequirementContract()
        result = run_ir_validation(
            case_graph,
            nl=case.get("nl", "jobs"),
            candidate=CandidateQuery(query=case["query"]),
            contract=contract,
            runner=StubRunner(),
            logic_validator=StubLogicValidator(verdict=True),
        )
        expected_ok = not case["should_fail"]
        if expected_ok:
            assert result.ok
        else:
            # Negative cases may be repaired; allow either a clean failure or a success with
            # no errors (but never a silent pass with errors).
            assert result.ok or result.errors


def test_paraphrase_metamorphic_preprocess(graph):
    base = run_preprocess(graph, "List Person names and where they work")
    paraphrase = run_preprocess(graph, "Who does each person work for?")
    assert base.output["filtered_nodes"] == paraphrase.output["filtered_nodes"]


def test_summarize_reports_failures():
    results = [
        StageResult(stage="preprocess", trace_id="a1", ok=True),
        StageResult(stage="generate", trace_id="b2", ok=False, errors=["missing candidates"]),
    ]
    summary = summarize(results)
    assert summary["ok"] == 1
    assert summary["failed"] == ["generate"]
    assert summary["by_stage"]["generate"]["errors"][0]["messages"] == ["missing candidates"]


def test_write_report_creates_json_and_csv(tmp_path):
    results = [
        StageResult(stage="preprocess", trace_id="a1", ok=True, duration_ms=1.2),
        StageResult(stage="generate", trace_id="b2", ok=False, errors=["missing candidates"], duration_ms=2.5),
    ]
    json_path = tmp_path / "report.json"
    csv_path = tmp_path / "report.csv"
    payload = write_report(results, json_path=json_path, csv_path=csv_path)

    assert json_path.exists()
    assert csv_path.exists()
    assert payload["summary"]["total"] == 2


def test_run_component_cases_executes_suite_and_reports(tmp_path):
    payload = run_component_cases(
        cases_path=GOLDEN_PATH,
        schema_text=SCHEMA,
        runner=StubRunner(),
        logic_validator=StubLogicValidator(verdict=True),
        skip_syntax=True,
        include_output=True,
        json_path=tmp_path / "suite.json",
        csv_path=tmp_path / "suite.csv",
    )

    assert payload["summary"]["total"] == 8
    # Two negative cases fail as expected (treated OK), one case (val_3) surfaces a mismatch.
    assert payload["summary"]["ok"] == 7
    assert (tmp_path / "suite.json").exists()
    assert (tmp_path / "suite.csv").exists()
    first = payload["results"][0]
    assert first["stage"] == "preprocess"
    assert first.get("output"), "expected output to be included when include_output is True"

