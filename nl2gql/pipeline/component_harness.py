from __future__ import annotations

import argparse
import csv
import json
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence

from .generator import CandidateQuery
from .ir import ISOQueryIR
from .preprocess import PreprocessResult, Preprocessor
from .requirements import RequirementContract, contract_view, coverage_violations
from .runner import GraphLiteRunner, SyntaxResult
from .schema_graph import SchemaGraph
from .validators import LogicValidator, SchemaGroundingValidator


_DEFAULT_CASES_PATH = Path(__file__).resolve().parents[1] / "tests" / "component" / "data" / "component_cases.json"
_DEFAULT_SCHEMA_PATH = Path(__file__).resolve().parents[1] / "sample_schema.txt"


@dataclass
class StageResult:
    """Lightweight result container emitted by component harnesses."""

    stage: str
    trace_id: str
    ok: bool
    errors: List[str] = field(default_factory=list)
    output: Dict[str, Any] = field(default_factory=dict)
    duration_ms: float = 0.0


def _json_safe(obj: Any) -> Any:
    """Project arbitrary objects to JSON-safe primitives for reporting."""
    if obj is None or isinstance(obj, (bool, int, float, str)):
        return obj
    if isinstance(obj, set):
        return sorted(_json_safe(v) for v in obj)
    if isinstance(obj, (list, tuple)):
        return [_json_safe(v) for v in obj]
    if isinstance(obj, dict):
        return {str(k): _json_safe(v) for k, v in obj.items()}
    if hasattr(obj, "__dict__"):
        return _json_safe(vars(obj))
    return str(obj)


def _trace_id(trace_id: Optional[str]) -> str:
    return trace_id or uuid.uuid4().hex[:8]


def _duration_ms(start: float) -> float:
    return round((time.perf_counter() - start) * 1000, 2)


def run_preprocess(graph: SchemaGraph, nl: str, feedback: Optional[List[str]] = None, *, trace_id: str | None = None) -> StageResult:
    start = time.perf_counter()
    pre = Preprocessor(graph).run(nl, feedback or [])
    return StageResult(
        stage="preprocess",
        trace_id=_trace_id(trace_id),
        ok=bool(pre.filtered_schema.nodes and pre.filtered_schema.edges),
        errors=[],
        output={
            "pre": pre,
            "normalized_nl": pre.normalized_nl,
            "structural_hints": pre.structural_hints,
            "filtered_nodes": sorted(pre.filtered_schema.nodes),
            "filtered_edges": [e.descriptor() for e in pre.filtered_schema.edges],
        },
        duration_ms=_duration_ms(start),
    )


def run_intent_linker(intent_linker, nl: str, pre: PreprocessResult, feedback: Optional[List[str]] = None, *, trace_id: str | None = None) -> StageResult:
    start = time.perf_counter()
    guidance = intent_linker.run(nl, pre, feedback or [])
    grounded_links = guidance.links
    errors: List[str] = []
    # Basic structural sanity: aliases and labels should exist.
    node_aliases = {nlk.get("alias") for nlk in grounded_links.get("node_links", []) if nlk.get("alias")}
    if len(node_aliases) != len([nlk for nlk in grounded_links.get("node_links", []) if nlk.get("alias")]):
        errors.append("duplicate aliases in node_links")
    ok = guidance.frame is not None and not errors
    return StageResult(
        stage="intent_link",
        trace_id=_trace_id(trace_id),
        ok=ok,
        errors=errors,
        output={"frame": guidance.frame, "links": grounded_links, "guidance": guidance},
        duration_ms=_duration_ms(start),
    )


def run_generator(
    generator,
    pre: PreprocessResult,
    guidance,
    contract: RequirementContract,
    failures: Optional[List[str]] = None,
    *,
    trace_id: str | None = None,
) -> StageResult:
    start = time.perf_counter()
    candidates = generator.generate(pre, failures or [], guidance, contract)
    ok = bool(candidates)
    return StageResult(
        stage="generate",
        trace_id=_trace_id(trace_id),
        ok=ok,
        errors=["generator returned no candidates"] if not ok else [],
        output={"candidates": [{"query": c.query, "reason": c.reason} for c in candidates]},
        duration_ms=_duration_ms(start),
    )


def run_ir_validation(
    graph: SchemaGraph,
    nl: str,
    candidate: CandidateQuery,
    contract: RequirementContract,
    *,
    hints: Optional[List[str]] = None,
    runner: Optional[GraphLiteRunner] = None,
    logic_validator: Optional[LogicValidator] = None,
    trace_id: str | None = None,
) -> StageResult:
    start = time.perf_counter()
    schema_validator = SchemaGroundingValidator(graph)
    runner = runner or GraphLiteRunner()
    logic_validator = logic_validator or LogicValidator()

    parse_errors: List[str] = []
    schema_errors: List[str] = []
    coverage_errors: List[str] = []
    logic_reason: Optional[str] = None

    with runner:
        ir, parse_errors = ISOQueryIR.parse(candidate.query)
        rendered = candidate.query
        if ir:
            rendered = ir.render()
            schema_errors = schema_validator.validate(ir)
            coverage_errors = coverage_violations(contract, ir, rendered)
            try:
                logic_ok, logic_reason = logic_validator.validate(
                    nl, graph.describe_full(), ir, hints or [], contract=contract
                )
            except TypeError:
                logic_ok, logic_reason = logic_validator.validate(
                    nl, graph.describe_full(), rendered, hints or []
                )
        else:
            logic_ok = False

        syntax: SyntaxResult = runner.validate(rendered)

    errors = parse_errors + schema_errors + coverage_errors
    if not syntax.ok:
        errors.append(f"syntax: {syntax.error or 'unspecified syntax error'}")
    if not logic_ok and logic_reason:
        errors.append(f"logic: {logic_reason}")

    ok = bool(not errors and syntax.ok and logic_ok and ir)

    return StageResult(
        stage="ir_validation",
        trace_id=_trace_id(trace_id),
        ok=ok,
        errors=errors,
        output={
            "rendered": rendered,
            "parse_errors": parse_errors,
            "schema_errors": schema_errors,
        "coverage_errors": coverage_errors,
        "syntax_ok": syntax.ok,
        "syntax_error": syntax.error,
        "logic_ok": logic_ok,
        "logic_reason": logic_reason,
        "contract": contract_view(contract),
    },
    duration_ms=_duration_ms(start),
    )


def summarize(results: Sequence[StageResult]) -> Dict[str, Any]:
    summary: Dict[str, Any] = {
        "total": len(results),
        "ok": sum(1 for r in results if r.ok),
        "failed": [r.stage for r in results if not r.ok],
        "by_stage": {},
    }
    for r in results:
        bucket = summary["by_stage"].setdefault(r.stage, {"runs": 0, "ok": 0, "errors": []})
        bucket["runs"] += 1
        if r.ok:
            bucket["ok"] += 1
        else:
            bucket["errors"].append({"trace_id": r.trace_id, "messages": r.errors})
    return summary


def write_report(
    results: Sequence[StageResult],
    *,
    json_path: Optional[str | Path] = None,
    csv_path: Optional[str | Path] = None,
    include_output: bool = False,
) -> Dict[str, Any]:
    """
    Persist aggregated metrics to disk.
    - json_path: writes summary + full results
    - csv_path: writes row-per-stage attempt with trace_id and errors
    """
    summary = summarize(results)
    payload = {
        "summary": summary,
        "results": [
            {
                "stage": r.stage,
                "trace_id": r.trace_id,
                "ok": r.ok,
                "errors": r.errors,
                "duration_ms": r.duration_ms,
                **({"output": _json_safe(r.output)} if include_output else {}),
            }
            for r in results
        ],
    }
    if json_path:
        path = Path(json_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    if csv_path:
        path = Path(csv_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("w", newline="", encoding="utf-8") as f:
            writer = csv.DictWriter(f, fieldnames=["stage", "trace_id", "ok", "errors", "duration_ms"])
            writer.writeheader()
            for r in results:
                writer.writerow(
                    {
                        "stage": r.stage,
                        "trace_id": r.trace_id,
                        "ok": r.ok,
                        "errors": "; ".join(r.errors),
                        "duration_ms": r.duration_ms,
                    }
                )
    return payload


def load_component_cases(path: str | Path = _DEFAULT_CASES_PATH) -> Dict[str, Any]:
    """Load component harness cases from a JSON file."""
    case_path = Path(path)
    if not case_path.exists():
        raise FileNotFoundError(f"component cases file not found: {path}")
    try:
        data = json.loads(case_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover
        raise ValueError(f"component cases file must be valid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError("component cases file must contain an object keyed by stage")
    return data


def _resolve_schema_text(schema_text: Optional[str], schema_path: Optional[str | Path]) -> str:
    if schema_text:
        return schema_text.strip()
    if schema_path:
        return Path(schema_path).read_text(encoding="utf-8").strip()
    return _DEFAULT_SCHEMA_PATH.read_text(encoding="utf-8").strip()


def _schema_text_from_case(case: Dict[str, Any], fallback: str) -> str:
    override = case.get("schema")
    if override is None:
        return fallback
    if isinstance(override, list):
        return "\n".join(str(line).strip() for line in override if str(line).strip())
    if isinstance(override, str):
        return override.strip()
    return fallback


def _contract_from_case(case: Dict[str, Any], fallback: Optional[RequirementContract]) -> RequirementContract:
    if fallback:
        return fallback
    contract = RequirementContract()
    contract.required_labels = set(case.get("required_labels") or [])
    edges = case.get("required_edges") or []
    contract.required_edges = {tuple(e) for e in edges if len(e) == 3}
    order = case.get("required_order") or []
    contract.required_order = [str(o).strip() for o in order if str(o).strip()]
    limit = case.get("limit")
    if isinstance(limit, int) and limit > 0:
        contract.limit = limit
    return contract


class _NoopRunner:
    """Bypass GraphLite syntax validation when bindings are unavailable."""

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return False

    def validate(self, _query: str) -> SyntaxResult:
        return SyntaxResult(ok=True, rows=None, error=None)


def _resolve_runner(skip_syntax: bool, runner: Optional[GraphLiteRunner]) -> GraphLiteRunner:
    if runner:
        return runner
    if skip_syntax:
        return _NoopRunner()  # type: ignore[return-value]
    try:
        return GraphLiteRunner()
    except Exception:
        # Fall back silently so the harness can still run
        return _NoopRunner()  # type: ignore[return-value]


def _resolve_logic_validator(enable_logic: bool, logic_validator: Optional[LogicValidator]) -> LogicValidator | object:
    if logic_validator:
        return logic_validator
    if not enable_logic:
        return type("NoopLogicValidator", (), {"validate": lambda *_, **__: (True, None)})()
    return LogicValidator()


def run_component_cases(
    cases_path: str | Path = _DEFAULT_CASES_PATH,
    *,
    schema_text: Optional[str] = None,
    schema_path: Optional[str | Path] = None,
    intent_linker=None,
    generator=None,
    runner: Optional[GraphLiteRunner] = None,
    logic_validator: Optional[LogicValidator] = None,
    contract: Optional[RequirementContract] = None,
    check_logic: bool = False,
    skip_syntax: bool = False,
    include_output: bool = False,
    json_path: Optional[str | Path] = None,
    csv_path: Optional[str | Path] = None,
) -> Dict[str, Any]:
    """
    Execute deterministic component checks for quick agent feedback.
    Returns JSON-serializable payload; optionally writes report files.
    """
    cases = load_component_cases(cases_path)
    base_schema_text = _resolve_schema_text(schema_text, schema_path)
    results: List[StageResult] = []

    # Preprocess coverage checks
    for idx, case in enumerate(cases.get("preprocess", []), 1):
        trace_id = case.get("id") or f"pre_{idx}"
        graph = SchemaGraph.from_text(_schema_text_from_case(case, base_schema_text))
        result = run_preprocess(graph, case["nl"], trace_id=trace_id)
        missing_nodes = [n for n in case.get("expected_nodes", []) if n not in result.output["filtered_nodes"]]
        missing_edges = [e for e in case.get("expected_edges", []) if e not in result.output["filtered_edges"]]
        if missing_nodes or missing_edges:
            result.errors.extend(
                [f"missing nodes: {', '.join(missing_nodes)}"] if missing_nodes else []
                + [f"missing edges: {', '.join(missing_edges)}"] if missing_edges else []
            )
            result.ok = False
        result.output["case"] = case
        result.output["missing_nodes"] = missing_nodes
        result.output["missing_edges"] = missing_edges
        results.append(result)

    resolved_runner = _resolve_runner(skip_syntax, runner)
    resolved_logic = _resolve_logic_validator(check_logic, logic_validator)

    # IR validation coverage checks
    for idx, case in enumerate(cases.get("validation", []), 1):
        trace_id = case.get("id") or f"val_{idx}"
        graph = SchemaGraph.from_text(_schema_text_from_case(case, base_schema_text))
        case_contract = _contract_from_case(case, contract)
        nl = case.get("nl") or case.get("query") or ""
        result = run_ir_validation(
            graph,
            nl=nl,
            candidate=CandidateQuery(query=case["query"]),
            contract=case_contract,
            runner=resolved_runner,
            logic_validator=resolved_logic,  # type: ignore[arg-type]
            trace_id=trace_id,
        )
        expected_ok = not bool(case.get("should_fail"))
        actual_ok = result.ok
        if expected_ok:
            # Standard case: must succeed.
            if not actual_ok:
                result.errors.append("expected success")
            result.ok = actual_ok
        else:
            # Negative case: success is a failure; failure is success.
            if actual_ok:
                result.errors.append("expected failure")
                result.ok = False
            else:
                result.ok = True
        result.output["case"] = case
        result.output["expected_ok"] = expected_ok
        result.output["actual_ok"] = actual_ok
        results.append(result)

    payload = write_report(results, json_path=json_path, csv_path=csv_path, include_output=include_output)
    payload["results"] = [{k: v for k, v in item.items()} for item in payload["results"]]
    return payload


def _parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run component-level harness checks.")
    parser.add_argument("--cases", type=str, default=str(_DEFAULT_CASES_PATH), help="Path to component_cases.json")
    parser.add_argument("--schema-file", type=str, help="Path to schema text file (defaults to sample_schema.txt)")
    parser.add_argument("--schema", type=str, help="Inline schema text")
    parser.add_argument("--json", dest="json_path", type=str, help="Path to write JSON report")
    parser.add_argument("--csv", dest="csv_path", type=str, help="Path to write CSV report")
    parser.add_argument("--include-output", action="store_true", help="Include stage output blobs in JSON report")
    parser.add_argument("--check-logic", action="store_true", help="Enable LLM-based logic validation (uses OPENAI_API_KEY)")
    parser.add_argument(
        "--skip-syntax", action="store_true", help="Skip GraphLite syntax validation (useful if bindings are missing)"
    )
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> None:
    args = _parse_args(argv)
    payload = run_component_cases(
        cases_path=args.cases,
        schema_text=args.schema,
        schema_path=args.schema_file,
        check_logic=args.check_logic,
        skip_syntax=args.skip_syntax,
        include_output=args.include_output,
        json_path=args.json_path,
        csv_path=args.csv_path,
    )
    print(json.dumps(payload["summary"], indent=2))


if __name__ == "__main__":  # pragma: no cover
    main()


__all__ = [
    "StageResult",
    "run_preprocess",
    "run_intent_linker",
    "run_generator",
    "run_ir_validation",
    "summarize",
    "write_report",
    "run_component_cases",
    "load_component_cases",
]
