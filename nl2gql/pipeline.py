"""Natural-language to ISO GQL inference pipeline for GraphLite.

Requirements (install into your venv):
    pip install openai tenacity python-dotenv
    pip install -e bindings/python   # from repo root

Environment (config.env in this folder is read automatically if present):
    OPENAI_API_KEY=sk-...
    OPENAI_MODEL_GEN=gpt-4o-mini     # optional override
    OPENAI_MODEL_FIX=gpt-4o-mini     # optional override
    NL2GQL_DB_PATH=./.nl2gql_cache   # optional scratch DB for validation
    NL2GQL_USER=admin                # session user for validation
    NL2GQL_SCHEMA=nl2gql
    NL2GQL_GRAPH=scratch

Usage (CLI):
    python nl2gql/pipeline.py --nl "find people older than 30" \
      --schema-file ./schema.txt --verbose
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from tenacity import retry, stop_after_attempt, wait_fixed

try:  # Local config
    from dotenv import load_dotenv
except ImportError:  # pragma: no cover - optional helper
    load_dotenv = None  # type: ignore


def _load_graphlite_sdk():
    """Import GraphLite bindings, preferring the bindings/python package.

    The repo root contains a Rust crate folder named `graphlite/` that can
    shadow the Python package when running scripts from the repo root. To avoid
    that, we temporarily prepend bindings/python to sys.path before import.
    """

    try:
        from graphlite import GraphLite, GraphLiteError  # type: ignore

        # If the module is a namespace (no attributes), fallback below.
        if getattr(GraphLite, "__name__", None):
            return GraphLite, GraphLiteError
    except Exception:
        pass

    bindings_path = Path(__file__).resolve().parents[1] / "bindings" / "python"
    if bindings_path.exists():
        sys.path.insert(0, str(bindings_path))
        # Drop any namespace that was cached from the repo root
        sys.modules.pop("graphlite", None)
        from graphlite import GraphLite, GraphLiteError  # type: ignore

        return GraphLite, GraphLiteError

    raise SystemExit(
        "GraphLite Python bindings are missing. Build the FFI and install with: "
        "cargo build -p graphlite-ffi --release && pip install -e bindings/python"
    )


GraphLite, GraphLiteError = _load_graphlite_sdk()


try:  # OpenAI
    from openai import OpenAI
except Exception as exc:  # pragma: no cover
    raise SystemExit(
        "OpenAI client missing. Install with: pip install openai"
    ) from exc


# ---------------------------------------------------------------------------
# Environment helpers
# ---------------------------------------------------------------------------

_ENV_PATH = Path(__file__).with_name("config.env")
if load_dotenv:
    if _ENV_PATH.exists():
        load_dotenv(_ENV_PATH)
    else:
        load_dotenv()

DEFAULT_OPENAI_MODEL_GEN = os.getenv("OPENAI_MODEL_GEN", "gpt-4o-mini")
DEFAULT_OPENAI_MODEL_FIX = os.getenv("OPENAI_MODEL_FIX", "gpt-4o-mini")

DEFAULT_DB_PATH = os.getenv("NL2GQL_DB_PATH")
DEFAULT_DB_USER = os.getenv("NL2GQL_USER", "admin")
DEFAULT_DB_SCHEMA = os.getenv("NL2GQL_SCHEMA", "nl2gql")
DEFAULT_DB_GRAPH = os.getenv("NL2GQL_GRAPH", "scratch")


# ---------------------------------------------------------------------------
# OpenAI helpers
# ---------------------------------------------------------------------------

_client_singleton: Optional[OpenAI] = None


def _client() -> OpenAI:
    global _client_singleton
    if _client_singleton is None:
        _client_singleton = OpenAI()
    return _client_singleton


@retry(stop=stop_after_attempt(3), wait=wait_fixed(0.2))
def chat_complete(
    model: str,
    system: str,
    user: str,
    *,
    temperature: float = 0.3,
    top_p: float = 0.9,
    max_tokens: int = 400,
) -> Tuple[str, Optional[Dict[str, Any]]]:
    """Simple chat completion with retries and usage extraction."""

    resp = _client().chat.completions.create(
        model=model,
        messages=[{"role": "system", "content": system}, {"role": "user", "content": user}],
        temperature=temperature,
        top_p=top_p,
        max_tokens=max_tokens,
    )

    text = (resp.choices[0].message.content or "").strip()
    usage_data = getattr(resp, "usage", None)
    if usage_data:
        usage = {
            "prompt_tokens": getattr(usage_data, "prompt_tokens", 0),
            "completion_tokens": getattr(usage_data, "completion_tokens", 0),
            "total_tokens": getattr(usage_data, "total_tokens", 0),
        }
        return text, usage
    return text, None


# ---------------------------------------------------------------------------
# GraphLite syntax validation
# ---------------------------------------------------------------------------


class GraphLiteValidator:
    """Lightweight syntax validator backed by the GraphLite Python SDK."""

    def __init__(
        self,
        *,
        db_path: Optional[str] = None,
        user: str = DEFAULT_DB_USER,
        schema: str = DEFAULT_DB_SCHEMA,
        graph: str = DEFAULT_DB_GRAPH,
    ) -> None:
        self._owns_db = db_path is None
        self._db_path = Path(db_path) if db_path else Path(tempfile.mkdtemp(prefix="graphlite_nl2gql_"))
        self._user = user
        self._schema = schema
        self._graph = graph
        self._db: Optional[GraphLite] = None
        self._session: Optional[str] = None
        self._ready = False

    def __enter__(self) -> "GraphLiteValidator":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def _ensure_ready(self) -> None:
        if self._ready:
            return

        self._db = GraphLite(str(self._db_path))
        self._session = self._db.create_session(self._user)

        # Minimal schema + graph so user queries can run even if they only read.
        bootstrap = [
            f"CREATE SCHEMA IF NOT EXISTS {self._schema}",
            f"SESSION SET SCHEMA {self._schema}",
            f"CREATE GRAPH IF NOT EXISTS {self._graph}",
            f"SESSION SET GRAPH {self._graph}",
        ]
        for stmt in bootstrap:
            self._db.execute(self._session, stmt)

        self._ready = True

    def close(self) -> None:
        if self._db:
            try:
                if self._session:
                    try:
                        self._db.close_session(self._session)
                    except GraphLiteError:
                        pass
                self._db.close()
            finally:
                self._db = None
                self._session = None

        if self._owns_db and self._db_path.exists():
            try:
                # Remove scratch directory quietly
                for path in sorted(self._db_path.rglob("*"), reverse=True):
                    if path.is_file():
                        path.unlink(missing_ok=True)
                    else:
                        path.rmdir()
                self._db_path.rmdir()
            except Exception:
                pass

    def validate(self, query: str) -> Tuple[bool, Optional[str]]:
        if not query.strip():
            return False, "empty query"

        try:
            self._ensure_ready()
            assert self._db is not None and self._session is not None
            self._db.query(self._session, query.strip())
            return True, None
        except GraphLiteError as exc:  # surfaced syntax/runtime issues
            return False, exc.message
        except Exception as exc:  # pragma: no cover - unexpected
            return False, str(exc)


# ---------------------------------------------------------------------------
# Prompt templates
# ---------------------------------------------------------------------------

SYSTEM_GENERATE = (
    "You write ISO GQL (GraphLite dialect) queries from natural language. "
    "Use only MATCH, WHERE, RETURN, WITH, ORDER BY, LIMIT, DISTINCT, and aggregates (COUNT, SUM, AVG, MIN, MAX). "
    "Use single quotes for strings. IN clauses use square brackets. Do not invent labels or properties outside schema_context. "
    "Return the query only with no commentary. Always include a RETURN clause (and LIMIT if applicable); do not stop early."
)

USER_GENERATE_TEMPLATE = (
    "SCHEMA:\n{schema_context}\n\n"
    "REQUEST: {nl}\n\n"
    "QUERY:"
)

SYSTEM_FIX = (
    "Fix the ISO GQL query so it parses and keeps the same intent. "
    "Use only elements from schema_context. Output the corrected query only."
)

USER_FIX_TEMPLATE = (
    "SCHEMA:\n{schema_context}\n\n"
    "REQUEST: {nl}\n\n"
    "BROKEN QUERY: {query}\n\n"
    "ERROR: {error}\n\n"
    "FIXED QUERY:"
)

SYSTEM_VALIDATE_LOGIC = (
    "You judge if an ISO GQL query logically satisfies the natural language request using the provided schema. "
    "Respond only with 'VALID' or 'INVALID: <reason>'."
)

USER_VALIDATE_LOGIC_TEMPLATE = (
    "SCHEMA:\n{schema_context}\n\n"
    "REQUEST: {nl}\n\n"
    "GENERATED QUERY: {query}\n\n"
    "Does this query logically satisfy the request?"
)


# ---------------------------------------------------------------------------
# Generation + validation pipeline
# ---------------------------------------------------------------------------


def generate_isogql_initial(nl: str, schema_context: str) -> Tuple[str, Optional[Dict[str, Any]]]:
    return generate_isogql_initial_with_model(nl, schema_context, DEFAULT_OPENAI_MODEL_GEN)


def generate_isogql_initial_with_model(
    nl: str, schema_context: str, model: str
) -> Tuple[str, Optional[Dict[str, Any]]]:
    user = USER_GENERATE_TEMPLATE.format(schema_context=schema_context, nl=nl)
    text, usage = chat_complete(model, SYSTEM_GENERATE, user, temperature=0.3, top_p=0.9)
    return text.strip(), usage


def generate_isogql_with_feedback(
    nl: str, schema_context: str, feedback: List[str], model: str
) -> Tuple[str, Optional[Dict[str, Any]]]:
    if not feedback:
        return generate_isogql_initial_with_model(nl, schema_context, model)

    feedback_text = "\n".join(f"- {item}" for item in feedback[-4:])
    enhanced_system = (
        SYSTEM_GENERATE
        + "\n\nCRITICAL FEEDBACK FROM PREVIOUS FAILED ATTEMPTS:\n"
        + feedback_text
        + "\n\nAddress all feedback above and avoid repeating the listed mistakes."
    )

    enhanced_user = (
        f"SCHEMA:\n{schema_context}\n\n"
        f"REQUEST: {nl}\n\n"
        "QUERY:"
    )

    text, usage = chat_complete(model, enhanced_system, enhanced_user, temperature=0.3, top_p=0.9)
    return text.strip(), usage


def fix_isogql_syntax_with_model(
    nl: str, schema_context: str, query: str, error: str, model: str
) -> Tuple[str, Optional[Dict[str, Any]]]:
    user = USER_FIX_TEMPLATE.format(schema_context=schema_context, nl=nl, query=query, error=error)
    fixed, usage = chat_complete(model, SYSTEM_FIX, user, temperature=0.2, top_p=0.9)
    if "\n" in fixed:
        fixed = fixed.splitlines()[0].strip()
    return fixed, usage


def validate_logical_correctness(
    nl: str, schema_context: str, query: str, model: str = DEFAULT_OPENAI_MODEL_FIX
) -> Tuple[bool, Optional[str], Optional[Dict[str, Any]]]:
    user = USER_VALIDATE_LOGIC_TEMPLATE.format(schema_context=schema_context, nl=nl, query=query)
    result, usage = chat_complete(model, SYSTEM_VALIDATE_LOGIC, user, temperature=0.1, top_p=0.9)

    verdict = result.strip().upper()
    if verdict.startswith("VALID"):
        return True, None, usage
    if verdict.startswith("INVALID:"):
        return False, verdict[len("INVALID:") :].strip(), usage
    return False, f"Unexpected validation response: {verdict}", usage


def generate_isogql(
    nl: str,
    schema_context: str,
    *,
    max_attempts: int = 3,
    gen_model: Optional[str] = None,
    fix_model: Optional[str] = None,
    validator: Optional[GraphLiteValidator] = None,
) -> Tuple[Optional[str], List[Dict[str, Any]], List[Dict[str, Any]]]:
    gen = gen_model or DEFAULT_OPENAI_MODEL_GEN
    fix = fix_model or DEFAULT_OPENAI_MODEL_FIX

    owns_validator = False
    if validator is None:
        validator = GraphLiteValidator(db_path=DEFAULT_DB_PATH)
        owns_validator = True

    try:
        return generate_isogql_with_models(nl, schema_context, max_attempts, gen, fix, validator)
    finally:
        if owns_validator and validator:
            validator.close()


def generate_isogql_with_models(
    nl: str,
    schema_context: str,
    max_attempts: int,
    gen_model: str,
    fix_model: str,
    validator: GraphLiteValidator,
) -> Tuple[Optional[str], List[Dict[str, Any]], List[Dict[str, Any]]]:
    if not nl.strip():
        return None, [], []

    usage_data: List[Dict[str, Any]] = []
    validation_log: List[Dict[str, Any]] = []
    feedback: List[str] = []

    for attempt in range(max_attempts):
        attempt_num = attempt + 1

        query, usage = generate_isogql_with_feedback(nl, schema_context, feedback, gen_model)
        if usage:
            usage.update({"attempt": attempt_num, "call_type": "generation", "model": gen_model})
            usage_data.append(usage)

        validation_log.append(
            {
                "attempt": attempt_num,
                "action": "generated",
                "query": query,
                "feedback": feedback.copy(),
            }
        )

        # Short-circuit obvious early stops so we don't waste validation on partial outputs.
        if len(query) < 32 or "RETURN" not in query.upper():
            feedback.append(
                "Incomplete query: generation stopped early (missing RETURN or too short). Emit a full query with RETURN (and LIMIT if applicable)."
            )
            validation_log.append(
                {
                    "attempt": attempt_num,
                    "action": "incomplete_generation",
                    "query": query,
                    "error": "missing RETURN / too short",
                }
            )
            continue

        syntax_valid, syntax_error = validator.validate(query)
        validation_log.append(
            {
                "attempt": attempt_num,
                "action": "validated_syntax",
                "query": query,
                "valid": syntax_valid,
                "error": syntax_error,
            }
        )

        logic_valid, logic_error, logic_usage = validate_logical_correctness(nl, schema_context, query, fix_model)
        if logic_usage:
            logic_usage.update({"attempt": attempt_num, "call_type": "validate_logic", "model": fix_model})
            usage_data.append(logic_usage)

        validation_log.append(
            {
                "attempt": attempt_num,
                "action": "validated_logic",
                "query": query,
                "valid": logic_valid,
                "error": logic_error,
            }
        )

        if syntax_valid and logic_valid:
            return query, usage_data, validation_log

        if not syntax_valid:
            fixed_query, fix_usage = fix_isogql_syntax_with_model(
                nl, schema_context, query, syntax_error or "", fix_model
            )
            if fix_usage:
                fix_usage.update({"attempt": attempt_num, "call_type": "fix_syntax", "model": fix_model})
                usage_data.append(fix_usage)

            validation_log.append(
                {
                    "attempt": attempt_num,
                    "action": "fixed_syntax",
                    "query": fixed_query,
                }
            )

            syntax_valid, syntax_error = validator.validate(fixed_query)
            validation_log.append(
                {
                    "attempt": attempt_num,
                    "action": "validated_syntax",
                    "query": fixed_query,
                    "valid": syntax_valid,
                    "error": syntax_error,
                }
            )

            if syntax_valid:
                logic_valid, logic_error, logic_usage = validate_logical_correctness(
                    nl, schema_context, fixed_query, fix_model
                )
                if logic_usage:
                    logic_usage.update(
                        {"attempt": attempt_num, "call_type": "validate_logic_after_fix", "model": fix_model}
                    )
                    usage_data.append(logic_usage)

                validation_log.append(
                    {
                        "attempt": attempt_num,
                        "action": "validated_logic",
                        "query": fixed_query,
                        "valid": logic_valid,
                        "error": logic_error,
                    }
                )

                if logic_valid:
                    return fixed_query, usage_data, validation_log

                query = fixed_query

        if not syntax_valid and syntax_error:
            feedback.append(f"Syntax error: {syntax_error}")
        if not logic_valid and logic_error:
            if "sum" in logic_error.lower() and "where" in logic_error.lower():
                feedback.append("CRITICAL: Aggregates in WHERE require a WITH clause and alias before filtering.")
            else:
                feedback.append(f"Logic issue: {logic_error}")
            feedback.append(f"AVOID this pattern: {query}")

    return None, usage_data, validation_log


# ---------------------------------------------------------------------------
# Reporting + CLI
# ---------------------------------------------------------------------------


def print_verbose_info(
    nl_query: str,
    usage_data: List[Dict[str, Any]],
    validation_log: List[Dict[str, Any]],
    max_attempts: int,
    gen_model: str,
    fix_model: str,
) -> None:
    def _fmt_feedback(fb: Optional[List[Dict[str, str]]]) -> str:
        if not fb:
            return "none"
        formatted = []
        for item in fb:
            if isinstance(item, dict):
                formatted.append(f"{item.get('type', 'note')}: {item.get('reason', '')}".strip())
            else:
                formatted.append(str(item))
        return "; ".join(formatted)

    print("\n" + "=" * 80)
    print("PIPELINE EXECUTION SUMMARY")
    print("=" * 80)
    print(f"Query: {nl_query}")
    print(f"Models: {gen_model} (gen) | {fix_model} (fix)")
    print(f"Max Attempts: {max_attempts}")

    # Group timeline entries by attempt for cleaner display.
    grouped: Dict[int, List[Dict[str, Any]]] = {}
    for entry in validation_log:
        grouped.setdefault(entry["attempt"], []).append(entry)

    print("\nTimeline (per attempt):")
    for attempt in sorted(grouped.keys()):
        print("-" * 80)
        print(f"Attempt {attempt}")
        for entry in grouped[attempt]:
            action = entry["action"]
            valid = entry.get("valid")

            if action == "generated":
                fb_text = _fmt_feedback(entry.get("feedback"))
                print(f"  • Generated (used feedback: {fb_text})")
                print("    Query:")
                print("      " + "\n      ".join(entry.get("query", "").splitlines() or ["<empty>"]))
            elif action == "incomplete_generation":
                print("  • Incomplete generation (missing RETURN or too short)")
                print("    Query:")
                print("      " + "\n      ".join(entry.get("query", "").splitlines() or ["<empty>"]))
            elif action == "validated_syntax":
                status = "✓ SYNTAX VALID" if valid else "✗ SYNTAX INVALID"
                print(f"  • {status}")
                if entry.get("error"):
                    print("    Error:")
                    print("      " + "\n      ".join(str(entry["error"]).splitlines()))
            elif action == "fixed_syntax":
                print("  • Applied syntax fix")
                print("    Query:")
                print("      " + "\n      ".join(entry.get("query", "").splitlines() or ["<empty>"]))
            elif action == "validated_logic":
                status = "✓ LOGIC VALID" if valid else "✗ LOGIC INVALID"
                print(f"  • {status}")
                if entry.get("error"):
                    print("    Error:")
                    print("      " + "\n      ".join(str(entry["error"]).splitlines()))
            elif action == "fixed_logic":
                print("  • Applied logic fix")
                print("    Query:")
                print("      " + "\n      ".join(entry.get("query", "").splitlines() or ["<empty>"]))
            elif action == "auto_fixed_syntax_hint":
                print("  • Auto-applied property correction")
                print("    Query:")
                print("      " + "\n      ".join(entry.get("query", "").splitlines() or ["<empty>"]))

    total_tokens = sum(item.get("total_tokens", 0) for item in usage_data)
    print("\nAPI Usage:")
    print(f"  Calls: {len(usage_data)}")
    print(f"  Tokens: {total_tokens}")
    print("=" * 80)


def read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read().strip()


def run_pipeline(
    nl: str,
    schema_context: str,
    *,
    max_attempts: int = 3,
    gen_model: Optional[str] = None,
    fix_model: Optional[str] = None,
    db_path: Optional[str] = DEFAULT_DB_PATH,
    verbose: bool = False,
) -> str:
    with GraphLiteValidator(db_path=db_path) as validator:
        result, usage_data, validation_log = generate_isogql(
            nl,
            schema_context,
            max_attempts=max_attempts,
            gen_model=gen_model,
            fix_model=fix_model,
            validator=validator,
        )

    if verbose:
        print_verbose_info(nl, usage_data, validation_log, max_attempts, gen_model or DEFAULT_OPENAI_MODEL_GEN, fix_model or DEFAULT_OPENAI_MODEL_FIX)

    if result is None:
        raise RuntimeError("Failed to generate a valid ISO GQL query")

    return result


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate ISO GQL queries from natural language")
    parser.add_argument("--nl", required=True, help="Natural language request")
    parser.add_argument("--schema-file", help="Path to schema context text")
    parser.add_argument("--schema", help="Schema context as a string (overrides --schema-file)")
    parser.add_argument("--max-attempts", type=int, default=3, help="Max generation/fix attempts")
    parser.add_argument("--gen-model", help="OpenAI model for generation")
    parser.add_argument("--fix-model", help="OpenAI model for fixes/logic validation")
    parser.add_argument("--db-path", help="GraphLite DB path for syntax validation (defaults to temp or NL2GQL_DB_PATH)")
    parser.add_argument("--verbose", action="store_true", help="Print attempt timeline and token usage")

    args = parser.parse_args(argv)

    if args.schema is not None:
        schema_context = args.schema
    elif args.schema_file:
        schema_context = read_text(args.schema_file)
    else:
        print("error: schema context is required via --schema or --schema-file", file=sys.stderr)
        return 1

    try:
        query = run_pipeline(
            args.nl,
            schema_context,
            max_attempts=args.max_attempts,
            gen_model=args.gen_model,
            fix_model=args.fix_model,
            db_path=args.db_path or DEFAULT_DB_PATH,
            verbose=args.verbose,
        )
    except Exception as exc:
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1

    print(query)
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
