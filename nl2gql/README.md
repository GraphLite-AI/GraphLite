# NL2GQL Inference Pipeline

Multi-stage, schema-grounded natural language → ISO GQL generation modeled after RAT-SQL style planning: intent framing, schema linking, constrained AST, rendering, and syntax/logic validation (GraphLite + LLM jury).

## Setup

- Install Python deps in your venv: `pip install openai tenacity python-dotenv`.
- Install GraphLite Python bindings from repo root so syntax validation works: `cargo build -p graphlite-ffi --release && pip install -e bindings/python`.
- Copy `nl2gql/config.example.env` → `nl2gql/config.env` if you want a local file; only `OPENAI_API_KEY` is required. Everything else is optional.
- Optional env vars (defaults if unset): `OPENAI_MODEL_GEN=gpt-4o-mini`, `OPENAI_MODEL_FIX=gpt-4o-mini`, `NL2GQL_DB_PATH=./.nl2gql_cache`, `NL2GQL_USER=admin`, `NL2GQL_SCHEMA=nl2gql`, `NL2GQL_GRAPH=scratch`.

## Usage

Basic CLI run (schema required):
```bash
python nl2gql/pipeline.py \
  --nl "find people older than 30" \
  --schema-file nl2gql/sample_schema.txt \
  --verbose
```

Flags:
- `--schema-file path` or `--schema "inline schema text"` supply schema context.
- `--max-attempts N` controls generation/repair retries (default 3).
- `--gen-model` / `--fix-model` override OpenAI models.
- `--db-path` points to a GraphLite DB folder for syntax checks (temp is used otherwise).
- `--verbose` prints attempt timeline + token totals; `--spinner/--no-spinner` toggles live spinner output.

## Pipeline outline

- Parse schema into a graph of nodes/properties/edges.
- Draft an intent frame (targets, filters, metrics, ordering, limits).
- Link NL mentions to schema elements, ground aliases/edges to the real schema.
- Plan a constrained AST (single MATCH) and render ISO GQL.
- Validate syntax via GraphLite; validate logical coverage via LLM committee; retry with feedback on failures.

## Troubleshooting

- If you see “GraphLite Python bindings are missing”, rebuild/install bindings from repo root.
- Ensure `config.env` is loaded or set `OPENAI_API_KEY` in your environment.
- When logic or syntax fails, rerun with `--verbose` to inspect per-attempt feedback and AST errors.
