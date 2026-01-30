from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Dict, List


def summarize(base_dir: str) -> Dict[str, object]:
    base = Path(base_dir)
    summaries = list(base.glob("**/summary.json"))
    status_counts = Counter()
    top_errors = Counter()
    per_query: Dict[str, List[str]] = defaultdict(list)

    for summary_file in summaries:
        try:
            data = json.loads(summary_file.read_text())
        except Exception:
            continue
        status = data.get("status", "unknown")
        status_counts[status] += 1
        slug = summary_file.parent.name
        per_query[slug].append(status)
        for fail in data.get("failures", []) or []:
            top_errors[fail[:120]] += 1

    aggregate = {
        "total_runs": len(summaries),
        "status_counts": dict(status_counts),
        "top_errors": top_errors.most_common(10),
        "per_query": {k: Counter(v) for k, v in per_query.items()},
    }
    return aggregate


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Summarize nl2gql-logs results.")
    parser.add_argument("--logs", default="nl2gql-logs", help="Root directory containing run logs")
    args = parser.parse_args(argv)

    report = summarize(args.logs)
    total = report["total_runs"]
    print(f"Total runs: {total}")
    for status, count in report["status_counts"].items():
        print(f"  {status}: {count}")
    if report["top_errors"]:
        print("\nTop errors:")
        for msg, count in report["top_errors"]:
            print(f"  ({count}) {msg}")
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
