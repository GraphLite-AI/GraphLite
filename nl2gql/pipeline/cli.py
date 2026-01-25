from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional

from .config import DEFAULT_DB_PATH, DEFAULT_OPENAI_MODEL_FIX, DEFAULT_OPENAI_MODEL_GEN
from .openai_client import reset_usage_log, usage_totals
from .pipeline import NL2GQLPipeline
from .refiner import PipelineFailure
from .run_logger import format_timeline
from .sample_suite import run_sample_suite
from .ui import Spinner, style


def _fmt_block(text: str, indent: int = 6) -> str:
    pad = " " * indent
    return "\n".join(pad + line for line in text.splitlines())


def _print_log_hint(log_dir: Optional[str], color_enabled: bool) -> None:
    """Surface where structured run logs were written for debugging."""
    if not log_dir:
        return
    try:
        resolved = Path(log_dir).expanduser().resolve()
    except Exception:
        resolved = Path(log_dir)
    print(f"{style('[logs]', 'cyan', color_enabled)} detailed run logs: {resolved}")


def print_timeline(nl_query: str, validation_log: List[Dict[str, any]], max_attempts: int) -> None:
    print(format_timeline(nl_query, validation_log, max_attempts))


def read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read().strip()


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate ISO GQL queries from natural language (schema-agnostic).")
    parser.add_argument("--nl", help="Natural language request")
    parser.add_argument("--schema-file", help="Path to schema context text")
    parser.add_argument("--schema", help="Schema context as a string (overrides --schema-file)")
    parser.add_argument("--max-attempts", type=int, default=2, help="Max refinement loops")
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
    parser.add_argument(
        "--raw",
        action="store_true",
        help="Output only the generated query (no status messages), for programmatic use",
    )
    parser.add_argument(
        "--trace-json",
        help="Directory to store structured run logs (defaults to ./nl2gql-logs)",
    )

    args = parser.parse_args(argv)
    color_enabled = sys.stdout.isatty()

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
                trace_path=args.trace_json,
            )
        except Exception as exc:
            print(f"error: failed to run sample suite - {exc}", file=sys.stderr)
            return 1

        total = len(results)
        passes = sum(1 for r in results if r.get("success"))

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
            if "elapsed_ms" in res:
                print(f"Elapsed: {res['elapsed_ms']} ms  | workers: {res.get('worker_count')}")
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

    gen_model = args.gen_model or DEFAULT_OPENAI_MODEL_GEN
    fix_model = args.fix_model or DEFAULT_OPENAI_MODEL_FIX

    # In raw mode, disable spinner and verbose output
    raw_mode = args.raw
    spinner = Spinner(enabled=False if raw_mode else (args.spinner if args.spinner is not None else sys.stdout.isatty()))
    reset_usage_log()
    spinner.start(nl_query=args.nl)
    start = time.perf_counter()
    try:
        pipeline = NL2GQLPipeline(
            schema_context,
            gen_model=gen_model,
            fix_model=fix_model,
            db_path=args.db_path or DEFAULT_DB_PATH,
            max_refinements=args.max_attempts,
        )
        query, timeline = pipeline.run(args.nl, spinner=spinner, trace_path=args.trace_json)
        usage = usage_totals()
        if pipeline.last_run_logger:
            pipeline.last_run_logger.log_usage(usage)
        if not raw_mode:
            spinner.stop("✓ Query generated.", color="green")
        if args.verbose and not raw_mode:
            print_timeline(args.nl, timeline, args.max_attempts)
            print(
                f"\nToken usage → prompt: {usage['prompt_tokens']}, "
                f"completion: {usage['completion_tokens']}, total: {usage['total_tokens']}"
            )
            elapsed_ms = int((time.perf_counter() - start) * 1000)
            print(f"Elapsed: {elapsed_ms} ms")
            log_dir = pipeline.last_run_logger.run_dir if pipeline.last_run_logger else args.trace_json
            _print_log_hint(str(log_dir) if log_dir else None, color_enabled)
            print()
            print()
        print(query)
        return 0
    except PipelineFailure as exc:
        if not raw_mode:
            spinner.stop("✗ Pipeline failed.", color="red")
        usage = usage_totals()
        if pipeline.last_run_logger:
            pipeline.last_run_logger.log_usage(usage)
        if args.verbose and not raw_mode:
            print_timeline(args.nl, exc.timeline, args.max_attempts)
            if exc.failures:
                print("Failures:")
                for f in exc.failures:
                    print(f"  - {f}")
            print(
                f"\nToken usage → prompt: {usage['prompt_tokens']}, "
                f"completion: {usage['completion_tokens']}, total: {usage['total_tokens']}"
            )
            elapsed_ms = int((time.perf_counter() - start) * 1000)
            print(f"Elapsed: {elapsed_ms} ms")
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        if args.verbose and not raw_mode:
            log_dir = pipeline.last_run_logger.run_dir if pipeline.last_run_logger else args.trace_json
            _print_log_hint(str(log_dir) if log_dir else None, color_enabled)
        return 1
    except Exception as exc:
        if not raw_mode:
            spinner.stop("✗ Pipeline failed.", color="red")
        usage = usage_totals()
        if pipeline.last_run_logger:
            pipeline.last_run_logger.log_usage(usage)
        if args.verbose and not raw_mode:
            print(
                f"\nToken usage → prompt: {usage['prompt_tokens']}, "
                f"completion: {usage['completion_tokens']}, total: {usage['total_tokens']}"
            )
            elapsed_ms = int((time.perf_counter() - start) * 1000)
            print(f"Elapsed: {elapsed_ms} ms")
            log_dir = pipeline.last_run_logger.run_dir if pipeline.last_run_logger else args.trace_json
            _print_log_hint(str(log_dir) if log_dir else None, color_enabled)
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())

