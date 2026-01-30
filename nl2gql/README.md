# NL2GQL Inference Pipeline

Schema-grounded NL → ISO GQL generation with staged planning: intent framing, schema linking, constrained AST, syntax/logic validation, and bounded repair loops.

Defaults use `gpt-4o-mini` for generation/repair to keep runs inexpensive, but you can override models via env vars or CLI flags.

## Setup

- Install deps in a venv: `pip install openai tenacity python-dotenv`.
- Install GraphLite Python bindings (syntax validator): `cargo build -p graphlite-ffi --release && pip install -e bindings/python` (from repo root).
- Copy `nl2gql/config.example.env` → `nl2gql/config.env` (or set env vars). Required: `OPENAI_API_KEY`. Optional defaults: `OPENAI_MODEL_GEN=gpt-4o-mini`, `OPENAI_MODEL_FIX=gpt-4o-mini`, `NL2GQL_DB_PATH=./.nl2gql_cache`, `NL2GQL_USER=admin`, `NL2GQL_SCHEMA=nl2gql`, `NL2GQL_GRAPH=scratch`.

## Quickstart (single query)

```bash
python3 -m nl2gql.pipeline.cli \
  --nl "List people older than 30" \
  --schema-file nl2gql/sample_schema.txt \
  --trace-json /tmp/nl2gql_trace_ok \
  --verbose --no-spinner
```

- Prints the final ISO GQL plus a per-attempt timeline when `--verbose` is set.
- `--trace-json` writes per-attempt JSON files (prompts, links, contracts, candidates, validation results) to the given directory; the CLI echoes the path after completion.
- More sample NL prompts: `nl2gql/sample_queries.txt`.

## Debug with JSON traces (recommended for agents)

- For a failing request, keep traces for inspection:
```bash
python3 -m nl2gql.pipeline.cli \
  --nl "List spacecraft missions launched before 1990" \
  --schema-file nl2gql/sample_schema.txt \
  --trace-json /tmp/nl2gql_trace_fail \
  --verbose --no-spinner --max-attempts 2
```
- Each `attempt_*.json` includes: normalized NL + schema summary, intent frame, schema links, contract, logic hints, generator prompt/raw output, and every candidate with parse/schema/coverage/syntax/logic results.
- Use these files to quickly spot grounding gaps, malformed plans, or syntax/logic failures without rerunning the pipeline.

## Component harness (stage smoke tests)

- Default cases: `python -m nl2gql.pipeline.component_harness`
- Custom cases/reporting:
```bash
python -m nl2gql.pipeline.component_harness \
  --cases nl2gql/tests/component/data/component_cases.json \
  --include-output --json /tmp/component_report.json --csv /tmp/component_report.csv
```
- Flags: `--schema-file/--schema`, `--skip-syntax` (run without GraphLite bindings), `--check-logic` (LLM logic validator), `--include-output` (keep per-stage details).

## Sample suite (end-to-end)

```bash
python3 -m nl2gql.pipeline.cli \
  --sample-suite \
  --suite-file nl2gql/sample_suites.json \
  --max-attempts 3
```

Prints pass/fail per query plus optional usage totals when `--verbose` is set.

## Flags at a glance

- `--nl` (NL prompt), `--schema-file` or `--schema` (inline text)
- `--max-attempts` (refinement loops, capped at 3)
- `--gen-model` / `--fix-model` (OpenAI models)
- `--db-path` (GraphLite DB for syntax checks)
- `--verbose` (per-attempt timeline + usage)
- `--trace-json <dir>` (override log directory; defaults to `./nl2gql-logs`)
- `--spinner/--no-spinner` (live status)
- `--sample-suite`, `--suite-file`, `--suite-workers`

## Tests

- Run all: `pytest`
- Covered: schema parsing, preprocessing, IR render/round-trip, grounding/link normalization, refiner happy paths.
- Component harness doubles as stage-level smoke + reporting for agents (see above).

## Troubleshooting

- Missing GraphLite bindings → rebuild/install (`cargo build -p graphlite-ffi --release && pip install -e bindings/python`).
- Missing API key → set `OPENAI_API_KEY` (optionally via `config.env`).
- Every run writes a full timeline + traces to `./nl2gql-logs` (capped at 20 runs). Use `--trace-json <dir>` to override.
- Rerun with `--verbose` to mirror the timeline that is already written to disk.
