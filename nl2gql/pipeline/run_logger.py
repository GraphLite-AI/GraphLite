from __future__ import annotations

import json
import os
import re
import shutil
import time
import uuid
from pathlib import Path
from typing import Any, Dict, List, Optional

DEFAULT_LOG_RETAIN = 20
SCHEMA_PREVIEW_LIMIT = 4000


def _utc_timestamp() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _status_icon(ok: bool) -> str:
    return "✓" if ok else "✗"


def _error_summary(bundle: Dict[str, Any]) -> str:
    """Summarize errors from a validation bundle."""
    parts = []
    parse = bundle.get("parse_errors", [])
    structural = bundle.get("structural_errors", [])
    schema = bundle.get("schema_errors", [])
    coverage = bundle.get("coverage_errors", [])
    syntax_ok = bundle.get("syntax_ok", True)
    logic_valid = bundle.get("logic_valid", True)

    if parse:
        parts.append(f"{len(parse)} parse")
    if structural:
        parts.append(f"{len(structural)} structural")
    if schema:
        parts.append(f"{len(schema)} schema")
    if coverage:
        parts.append(f"{len(coverage)} coverage")
    if not syntax_ok:
        parts.append("syntax")
    if not logic_valid:
        parts.append("logic")

    return ", ".join(parts) if parts else "none"


def _is_clean(bundle: Dict[str, Any]) -> bool:
    """Check if a bundle has no errors."""
    return (
        not bundle.get("parse_errors")
        and not bundle.get("structural_errors")
        and not bundle.get("schema_errors")
        and not bundle.get("coverage_errors")
        and bundle.get("syntax_ok", True)
        and bundle.get("logic_valid", True)
    )


def format_timeline(nl_query: str, validation_log: List[Dict[str, Any]], max_attempts: int) -> str:
    """Render the timeline into a human-readable string for log files."""
    lines: List[str] = []
    lines.append("=" * 80)
    lines.append("PIPELINE EXECUTION SUMMARY")
    lines.append("=" * 80)
    lines.append(f"Query: {nl_query}")
    lines.append(f"Max Attempts: {max_attempts}")
    lines.append("")

    attempts: Dict[int, List[Dict[str, Any]]] = {}
    for entry in validation_log:
        attempts.setdefault(entry.get("attempt", 0), []).append(entry)

    for attempt_num in sorted(attempts):
        lines.append("-" * 80)
        lines.append(f"ATTEMPT {attempt_num}")
        lines.append("-" * 80)

        entries = attempts[attempt_num]

        # Stage 1: Understanding
        intent_entry = next((e for e in entries if e.get("phase") == "intent"), None)
        link_entry = next((e for e in entries if e.get("phase") == "link"), None)
        if intent_entry or link_entry:
            lines.append("")
            lines.append("┌─ STAGE 1: UNDERSTANDING")
            if intent_entry:
                frame = intent_entry.get("frame", {})
                targets = frame.get("targets", [])
                filters = frame.get("filters", [])
                metrics = frame.get("metrics", [])
                order = frame.get("order_by", [])
                limit = frame.get("limit")
                lines.append(f"│  Targets: {', '.join(str(t) for t in targets[:5])}{'...' if len(targets) > 5 else ''}")
                if filters:
                    lines.append(f"│  Filters: {', '.join(str(f) for f in filters[:3])}{'...' if len(filters) > 3 else ''}")
                if metrics:
                    lines.append(f"│  Metrics: {', '.join(str(m) for m in metrics[:3])}")
                if order:
                    lines.append(f"│  Order: {', '.join(str(o) for o in order)}")
                if limit:
                    lines.append(f"│  Limit: {limit}")
            if link_entry:
                links = link_entry.get("links", {})
                node_links = links.get("node_links", [])
                rel_links = links.get("rel_links", [])
                node_strs = [f"{n.get('alias')}:{n.get('label')}" for n in node_links]
                lines.append(f"│  Nodes: {', '.join(node_strs)}")
                if rel_links:
                    lines.append(f"│  Edges: {len(rel_links)} relationship(s)")
            lines.append("└─ ✓ Understanding complete")

        # Stage 2: Contract
        contract_entry = next((e for e in entries if e.get("phase") == "contract"), None)
        if contract_entry:
            lines.append("")
            lines.append("┌─ STAGE 2: CONTRACT")
            reqs = contract_entry.get("requirements", {})
            labels = reqs.get("required_labels", [])
            edges = reqs.get("required_edges", [])
            props = reqs.get("required_properties", [])
            metrics = reqs.get("required_metrics", [])
            order = reqs.get("required_order", [])
            limit = reqs.get("limit")
            lines.append(f"│  Required labels: {', '.join(labels)}")
            if edges:
                edge_strs = [f"{e[0]}-[:{e[1]}]->{e[2]}" for e in edges[:3]]
                lines.append(f"│  Required edges: {', '.join(edge_strs)}{'...' if len(edges) > 3 else ''}")
            if props:
                lines.append(f"│  Required properties: {len(props)} property constraint(s)")
            if metrics:
                lines.append(f"│  Required metrics: {len(metrics)} metric(s)")
            if order:
                lines.append(f"│  Required order: {', '.join(order)}")
            if limit:
                lines.append(f"│  Limit: {limit}")
            lines.append("└─ ✓ Contract built")

        # Stage 3: Generation
        gen_entry = next((e for e in entries if e.get("phase") == "generate"), None)
        if gen_entry:
            lines.append("")
            lines.append("┌─ STAGE 3: GENERATION")
            candidates = gen_entry.get("candidates", [])
            lines.append(f"│  Generated {len(candidates)} candidate(s)")
            lines.append("└─ ✓ Generation complete")

        # Stage 4-6: Validation & Repair
        eval_entries = [e for e in entries if e.get("phase") is None and "pre_fix_bundle" in e]
        for eval_entry in eval_entries:
            pre_bundle = eval_entry.get("pre_fix_bundle", {})
            fixes = eval_entry.get("fixes", [])
            post_bundle = eval_entry.get("post_fix_bundle", {})

            lines.append("")
            lines.append("┌─ STAGE 4: INITIAL VALIDATION")
            pre_clean = _is_clean(pre_bundle)
            pre_errors = _error_summary(pre_bundle)
            lines.append(f"│  Status: {_status_icon(pre_clean)} {'PASS' if pre_clean else 'FAIL'}")
            if not pre_clean:
                lines.append(f"│  Errors: {pre_errors}")
                # Show specific errors
                for err in pre_bundle.get("schema_errors", [])[:2]:
                    lines.append(f"│    - Schema: {err}")
                for err in pre_bundle.get("coverage_errors", [])[:2]:
                    lines.append(f"│    - Coverage: {err}")
                if not pre_bundle.get("logic_valid", True) and pre_bundle.get("logic_reason"):
                    reason = pre_bundle.get("logic_reason", "")
                    if len(reason) > 80:
                        reason = reason[:77] + "..."
                    lines.append(f"│    - Logic: {reason}")
            lines.append(f"└─ {_status_icon(pre_clean)} Initial validation {'passed' if pre_clean else 'needs repair'}")

            # Stage 5: Repair (only if there were errors)
            if fixes or (not pre_clean and post_bundle):
                lines.append("")
                lines.append("┌─ STAGE 5: REPAIR")

                if fixes:
                    for fix in fixes:
                        note = fix.get("note", "unknown")
                        issues = fix.get("issues", [])
                        lines.append(f"│  Applied: {note}")
                        for issue in issues[:2]:
                            if len(issue) > 60:
                                issue = issue[:57] + "..."
                            lines.append(f"│    - Fixing: {issue}")

                fix_details = post_bundle.get("fix_details", "")
                if fix_details == "deterministic_schema_repair":
                    lines.append("│  Applied: deterministic_schema_repair (auto-flipped edges)")

                post_clean = _is_clean(post_bundle)
                post_errors = _error_summary(post_bundle)
                lines.append(f"│  After repair: {_status_icon(post_clean)} {'PASS' if post_clean else f'FAIL ({post_errors})'}")
                if not post_clean:
                    for err in post_bundle.get("schema_errors", [])[:2]:
                        lines.append(f"│    - Schema: {err}")
                lines.append(f"└─ {_status_icon(post_clean)} Repair {'successful' if post_clean else 'incomplete'}")

            # Stage 6: Final Validation (show final query)
            final_bundle = post_bundle if post_bundle else pre_bundle
            final_clean = _is_clean(final_bundle)
            lines.append("")
            lines.append("┌─ STAGE 6: FINAL RESULT")
            lines.append(f"│  Status: {_status_icon(final_clean)} {'SUCCESS' if final_clean else 'PARTIAL (best effort)'}")
            final_query = final_bundle.get("query", "")
            if final_query:
                # Show query in a readable format
                query_lines = final_query.strip().split("\n")
                lines.append("│  Query:")
                for ql in query_lines:
                    lines.append(f"│    {ql}")
            lines.append("└─")

        # Final selection entry
        final_entry = next((e for e in entries if e.get("phase") == "final"), None)
        if final_entry:
            status = final_entry.get("status", "unknown")
            lines.append("")
            lines.append(f">>> ATTEMPT {attempt_num} RESULT: {status.upper()}")

    lines.append("")
    lines.append("=" * 80)
    return "\n".join(lines)


class RunLogger:
    """
    Centralized logging utility for pipeline runs.
    - Writes per-run artifacts (timeline, traces, debug, summary).
    - Caps retained runs to avoid unbounded growth.
    """

    def __init__(self, base_dir: Optional[str] = None, retain: int = DEFAULT_LOG_RETAIN) -> None:
        env_dir = os.getenv("NL2GQL_LOG_DIR")
        self.base_dir = Path(base_dir or env_dir or (Path.cwd() / "nl2gql-logs"))
        self.retain = max(1, retain)
        self.run_dir: Optional[Path] = None
        self.trace_dir: Optional[Path] = None

    def start(self, nl: str, schema_context: str, params: Dict[str, Any]) -> Path:
        """Create a new run directory and capture initial metadata."""
        self.base_dir.mkdir(parents=True, exist_ok=True)
        stamp = time.strftime("%Y%m%d-%H%M%S")
        slug = re.sub(r"[^a-zA-Z0-9]+", "-", nl.strip())[:36].strip("-") or "run"
        run_id = f"{stamp}-{slug}-{uuid.uuid4().hex[:6]}"
        self.run_dir = self.base_dir / run_id
        self.run_dir.mkdir(parents=True, exist_ok=True)

        self.trace_dir = self.run_dir / "trace"
        self.trace_dir.mkdir(parents=True, exist_ok=True)

        metadata = {
            "nl": nl,
            "params": params,
            "schema_preview": schema_context[:SCHEMA_PREVIEW_LIMIT],
            "started_at": _utc_timestamp(),
        }
        self._write_json(self.run_dir / "metadata.json", metadata)
        return self.run_dir

    def log_timeline(self, nl: str, timeline: List[Dict[str, Any]], max_attempts: int) -> None:
        if not self.run_dir:
            return
        payload = {"nl": nl, "max_attempts": max_attempts, "timeline": timeline, "logged_at": _utc_timestamp()}
        self._write_json(self.run_dir / "timeline.json", payload)
        try:
            text = format_timeline(nl, timeline, max_attempts)
            (self.run_dir / "timeline.txt").write_text(text, encoding="utf-8")
        except Exception:
            pass

    def log_attempt_trace(self, attempt: int, payload: Dict[str, Any], empty: bool = False) -> None:
        if not self.trace_dir:
            return
        suffix = "empty" if empty else "detail"
        path = self.trace_dir / f"attempt_{attempt}_{suffix}.json"
        payload = dict(payload)
        payload["logged_at"] = _utc_timestamp()
        self._write_json(path, payload)

    def log_debug(self, payload: Dict[str, Any]) -> None:
        if not self.run_dir:
            return
        payload = dict(payload)
        payload["logged_at"] = _utc_timestamp()
        debug_path = self.run_dir / "debug.jsonl"
        try:
            with debug_path.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps(payload))
                fh.write("\n")
        except Exception:
            pass

    def log_usage(self, usage: Dict[str, Any]) -> None:
        if not self.run_dir:
            return
        payload = {"usage": usage, "logged_at": _utc_timestamp()}
        self._write_json(self.run_dir / "usage.json", payload)

    def finalize(self, status: str, extra: Optional[Dict[str, Any]] = None) -> None:
        if not self.run_dir:
            return
        summary = {"status": status, "finished_at": _utc_timestamp()}
        if extra:
            summary.update(extra)
        self._write_json(self.run_dir / "summary.json", summary)
        self._prune_old_runs()

    def _write_json(self, path: Path, payload: Dict[str, Any]) -> None:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        except Exception:
            # Logging must never block pipeline execution.
            pass

    def _prune_old_runs(self) -> None:
        try:
            if not self.base_dir.exists():
                return
            candidates = [p for p in self.base_dir.iterdir() if p.is_dir()]
            candidates.sort(key=lambda p: p.stat().st_mtime, reverse=True)
            for stale in candidates[self.retain :]:
                if self.run_dir and stale == self.run_dir:
                    continue
                shutil.rmtree(stale, ignore_errors=True)
        except Exception:
            # Never fail due to pruning issues.
            pass


__all__ = ["RunLogger", "format_timeline", "DEFAULT_LOG_RETAIN"]
