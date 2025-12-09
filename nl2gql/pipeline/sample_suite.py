from __future__ import annotations

import json
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Dict, List, Optional

from .openai_client import reset_usage_log, usage_totals
from .pipeline import NL2GQLPipeline
from .refiner import PipelineFailure


def _extract_queries_from_file(path: str) -> List[str]:
    queries: List[str] = []
    import re

    pattern = re.compile(r'--nl\s+"([^"]+)"')
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            match = pattern.search(line)
            if match:
                queries.append(match.group(1))
    return queries


def _load_sample_suite(path: str) -> List[Dict[str, Any]]:
    manifest_path = Path(path)
    if not manifest_path.exists():
        raise FileNotFoundError(f"sample suite file not found: {path}")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover
        raise ValueError(f"sample suite file must be JSON: {exc}") from exc

    if not isinstance(manifest, list):
        raise ValueError("sample suite file must contain a list of suite definitions")

    suites: List[Dict[str, Any]] = []
    for idx, entry in enumerate(manifest):
        if not isinstance(entry, dict):
            raise ValueError(f"sample suite entry {idx + 1} is not an object")

        name = str(entry.get("name") or f"suite_{idx + 1}")
        raw_schema = entry.get("schema")
        if isinstance(raw_schema, list):
            schema_text = "\n".join(str(line).strip() for line in raw_schema if str(line).strip())
        elif isinstance(raw_schema, str):
            candidate_path = manifest_path.parent / raw_schema
            if candidate_path.exists():
                schema_text = candidate_path.read_text(encoding="utf-8").strip()
            else:
                schema_text = raw_schema.strip()
        else:
            raise ValueError(f"sample suite entry '{name}' is missing a schema block")

        queries = [str(q).strip() for q in entry.get("queries", []) if str(q).strip()]
        if not queries:
            raise ValueError(f"sample suite entry '{name}' is missing queries")

        suites.append({"name": name, "schema_text": schema_text, "queries": queries})

    if not suites:
        raise ValueError("sample suite file contained no usable entries")
    return suites


def run_sample_suite(
    suite_path: str,
    *,
    max_iterations: int = 3,
    verbose: bool = False,
    db_path: Optional[str] = None,
    workers: Optional[int] = None,
) -> List[Dict[str, Any]]:
    max_iterations = min(max_iterations, 3)
    suites = _load_sample_suite(suite_path)
    tasks: List[Dict[str, Any]] = []
    for suite_idx, suite in enumerate(suites, 1):
        for query_idx, nl in enumerate(suite["queries"], 1):
            tasks.append(
                {
                    "order": len(tasks),
                    "suite": suite["name"],
                    "suite_idx": suite_idx,
                    "query_idx": query_idx,
                    "schema_text": suite["schema_text"],
                    "nl": nl,
                }
            )

    if not tasks:
        return []

    max_workers = max(1, workers or min(4, len(tasks)))
    results: List[Dict[str, Any]] = []

    def _run_task(task: Dict[str, Any]) -> Dict[str, Any]:
        start = time.perf_counter()
        reset_usage_log()
        pipeline = NL2GQLPipeline(task["schema_text"], max_refinements=max_iterations, db_path=db_path)
        try:
            query, timeline = pipeline.run(task["nl"], spinner=None)
            usage = usage_totals()
            return {
                "order": task["order"],
                "suite": task["suite"],
                "suite_idx": task["suite_idx"],
                "query_idx": task["query_idx"],
                "nl": task["nl"],
                "query": query,
                "timeline": timeline,
                "usage": usage,
                "elapsed_ms": int((time.perf_counter() - start) * 1000),
                "worker_count": max_workers,
                "success": True,
            }
        except PipelineFailure as exc:
            usage = usage_totals()
            return {
                "order": task["order"],
                "suite": task["suite"],
                "suite_idx": task["suite_idx"],
                "query_idx": task["query_idx"],
                "nl": task["nl"],
                "error": str(exc),
                "timeline": exc.timeline,
                "failures": exc.failures,
                "usage": usage,
                "elapsed_ms": int((time.perf_counter() - start) * 1000),
                "worker_count": max_workers,
                "success": False,
            }
        except Exception as exc:
            usage = usage_totals()
            return {
                "order": task["order"],
                "suite": task["suite"],
                "suite_idx": task["suite_idx"],
                "query_idx": task["query_idx"],
                "nl": task["nl"],
                "error": str(exc),
                "usage": usage,
                "elapsed_ms": int((time.perf_counter() - start) * 1000),
                "worker_count": max_workers,
                "success": False,
            }

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = [executor.submit(_run_task, task) for task in tasks]
        for fut in as_completed(futures):
            results.append(fut.result())

    return sorted(results, key=lambda r: r["order"])


__all__ = ["run_sample_suite", "_extract_queries_from_file", "_load_sample_suite"]


