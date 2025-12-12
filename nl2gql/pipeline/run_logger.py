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


def format_timeline(nl_query: str, validation_log: List[Dict[str, Any]], max_attempts: int) -> str:
    """Render the timeline into a human-readable string for log files."""
    lines: List[str] = []
    lines.append("=" * 80)
    lines.append("PIPELINE EXECUTION SUMMARY")
    lines.append("=" * 80)
    lines.append(f"Query: {nl_query}")
    lines.append(f"Max Attempts: {max_attempts}")

    attempts: Dict[int, List[Dict[str, Any]]] = {}
    for entry in validation_log:
        attempts.setdefault(entry.get("attempt", 0), []).append(entry)

    lines.append("")
    lines.append("Timeline (per attempt):")
    for attempt in sorted(attempts):
        lines.append("-" * 80)
        lines.append(f"Attempt {attempt}")
        for entry in attempts[attempt]:
            phase = entry.get("phase")
            if phase == "intent":
                lines.append("  • Intent frame")
                lines.append(json.dumps(entry.get("frame"), indent=2))
            elif phase == "link":
                lines.append("  • Schema links")
                lines.append(json.dumps(entry.get("links"), indent=2))
            elif phase == "contract":
                lines.append("  • Contract requirements")
                lines.append(json.dumps(entry.get("requirements"), indent=2))
            elif phase == "hints":
                lines.append("  • Logic hints")
                lines.append(json.dumps(entry.get("logic_hints"), indent=2))
            elif phase == "generate":
                lines.append("  • Candidates")
                lines.append(json.dumps(entry.get("candidates"), indent=2))
            elif phase == "final":
                lines.append("  • Final selection")
                summary = {"status": entry.get("status"), "query": entry.get("query")}
                lines.append(json.dumps(summary, indent=2))
            else:
                lines.append("  • Candidate evaluation")
                details = {k: v for k, v in entry.items() if k not in {"attempt"}}
                lines.append(json.dumps(details, indent=2))
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

