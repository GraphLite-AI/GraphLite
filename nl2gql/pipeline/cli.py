from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Optional

from .config import DEFAULT_DB_PATH, DEFAULT_OPENAI_MODEL_FIX, DEFAULT_OPENAI_MODEL_GEN
from .openai_client import reset_usage_log, usage_totals
from .pipeline import NL2GQLPipeline
from .refiner import PipelineFailure
from .sample_suite import run_sample_suite
from .ui import Spinner, style


def _fmt_block(text: str, indent: int = 6) -> str:
    pad = " " * indent
    return "\n".join(pad + line for line in text.splitlines())


def print_timeline(nl_query: str, validation_log: List[Dict[str, any]], max_attempts: int) -> None:
    print("\n" + "=" * 80)
    print("PIPELINE EXECUTION SUMMARY")
    print("=" * 80)
    print(f"Query: {nl_query}")
    print(f"Max Attempts: {max_attempts}")

    grouped: Dict[int, List[Dict[str, any]]] = defaultdict(list)
    for entry in validation_log:
        grouped[entry.get("attempt", 0)].append(entry)

    print("\nTimeline (per attempt):")
    for attempt in sorted(grouped):
        print("-" * 80)
        print(f"Attempt {attempt}")
        for entry in grouped[attempt]:
            phase = entry.get("phase")
            if phase == "intent":
                print("  • Intent frame")
                print(_fmt_block(json.dumps(entry.get("frame"), indent=2)))
            elif phase == "link":
                print("  • Schema links")
                print(_fmt_block(json.dumps(entry.get("links"), indent=2)))
            elif phase == "generate":
                print("  • Candidates")
                print(_fmt_block(json.dumps(entry.get("candidates"), indent=2)))
            else:
                print("  • Candidate evaluation")
                details = {k: v for k, v in entry.items() if k not in {"attempt"}}
                print(_fmt_block(json.dumps(details, indent=2)))
    print("=" * 80)


def read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read().strip()


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate ISO GQL queries from natural language (schema-agnostic).")
    parser.add_argument("--nl", help="Natural language request")
    parser.add_argument("--schema-file", help="Path to schema context text")
    parser.add_argument("--schema", help="Schema context as a string (overrides --schema-file)")
    parser.add_argument("--max-attempts", type=int, default=4, help="Max refinement loops")
    parser.add_argument("--gen-model", help="OpenAI model for generation (default: gpt-4o-mini)")
    parser.add_argument("--fix-model", help="OpenAI model for fixes/logic validation (default: gpt-4o-mini)")
    parser.add_argument("--verbose", action="store_true", help="Print attempt timeline")
    parser.add_argument("--sample-suite", action="store_true", help="Run all queries in the sample suite manifest")
    parser.add_argument("--suite-file", default="nl2gql/sample_suites.json", help="Path to sample suite manifest (JSON)")
    parser.add_argument("--suite-workers", type=int, help="Worker threads for sample suite (default: min(4, total queries))")
    parser.add_argument("--db-path", help="GraphLite DB path for syntax validation (defaults to temp or NL2GQL_DB_PATH)")
    parser.add_argument("--spinner", dest="spinner", action="store_true", help="Show live spinner updates when running single queries")
    parser.add_argument("--no-spinner", dest="spinner", action="store_false")
    parser.set_defaults(spinner=None)

    args = parser.parse_args(argv)

    schema_context: Optional[str] = None
    if args.schema is not None:
        potential_path = Path(args.schema)
        if potential_path.exists():
            schema_context = read_text(str(potential_path))
        else:
            schema_context = args.schema
    elif args.schema_file:
        schema_context = read_text(args.schema_file)

    if args.sample_suite:
        try:
            results = run_sample_suite(
                args.suite_file,
                max_iterations=args.max_attempts,
                verbose=args.verbose,
                db_path=args.db_path or DEFAULT_DB_PATH,
                workers=args.suite_workers,
            )
        except Exception as exc:
            print(f"error: failed to run sample suite - {exc}", file=sys.stderr)
            return 1

        total = len(results)
        passes = sum(1 for r in results if r.get("success"))
        color_enabled = sys.stdout.isatty()

        print("\nSAMPLE SUITE SUMMARY")
        print(f"Results: {passes}/{total} passed")
        for res in results:
            label = f"{res['suite']} #{res['query_idx']}"
            status = "[ok]" if res.get("success") else "[fail]"
            color = "green" if res.get("success") else "red"
            print(f"{style(status, color, color_enabled)} {label}: {res['nl']}")

        print("\nDETAILS")
        for res in results:
            print("-" * 80)
            label = f"{res['suite']} [query {res['query_idx']}]"
            outcome = "OK" if res.get("success") else "FAIL"
            print(f"{label} → {outcome}")
            print(f"NL : {res['nl']}")
            if res.get("success"):
                print("ISO GQL:")
                print(_fmt_block(res["query"], indent=4))
                if args.verbose:
                    usage = res.get("usage", {})
                    print(
                        f"Usage → prompt: {usage.get('prompt_tokens', 0)}, "
                        f"completion: {usage.get('completion_tokens', 0)}, "
                        f"total: {usage.get('total_tokens', 0)}"
                    )
            else:
                print(f"Error: {res.get('error', 'unspecified error')}")
                failures = res.get("failures") or []
                if failures:
                    print("Failure reasons:")
                    for fail in sorted(set(failures)):
                        print(f"  - {fail}")
        return 0 if passes == total else 2

    if not schema_context:
        print("error: schema context is required via --schema or --schema-file", file=sys.stderr)
        return 1
    if not args.nl:
        print("error: --nl is required when not running the sample suite", file=sys.stderr)
        return 1

    spinner = Spinner(enabled=args.spinner if args.spinner is not None else sys.stdout.isatty())
    reset_usage_log()
    spinner.start("Starting pipeline...")
    try:
        pipeline = NL2GQLPipeline(
            schema_context,
            gen_model=args.gen_model or DEFAULT_OPENAI_MODEL_GEN,
            fix_model=args.fix_model or DEFAULT_OPENAI_MODEL_FIX,
            db_path=args.db_path or DEFAULT_DB_PATH,
            max_refinements=args.max_attempts,
        )
        query, timeline = pipeline.run(args.nl, spinner=spinner)
        spinner.stop("✓ Query generated.", color="green")
        if args.verbose:
            print_timeline(args.nl, timeline, args.max_attempts)
            usage = usage_totals()
            print(
                f"\nToken usage → prompt: {usage['prompt_tokens']}, "
                f"completion: {usage['completion_tokens']}, total: {usage['total_tokens']}"
            )
        print(query)
        return 0
    except PipelineFailure as exc:
        spinner.stop("✗ Pipeline failed.", color="red")
        if args.verbose:
            print_timeline(args.nl, exc.timeline, args.max_attempts)
            if exc.failures:
                print("Failures:")
                for f in exc.failures:
                    print(f"  - {f}")
            usage = usage_totals()
            print(
                f"\nToken usage → prompt: {usage['prompt_tokens']}, "
                f"completion: {usage['completion_tokens']}, total: {usage['total_tokens']}"
            )
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1
    except Exception as exc:
        spinner.stop("✗ Pipeline failed.", color="red")
        if args.verbose:
            usage = usage_totals()
            print(
                f"\nToken usage → prompt: {usage['prompt_tokens']}, "
                f"completion: {usage['completion_tokens']}, total: {usage['total_tokens']}"
            )
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())


