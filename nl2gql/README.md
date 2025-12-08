# NL2GQL Inference Pipeline

Multi-stage, schema-grounded natural language → ISO GQL generation modeled after RAT-SQL style planning: intent framing, schema linking, constrained AST, rendering, and syntax/logic validation (GraphLite + LLM jury).

## Setup

- Install Python deps in your venv: `pip install openai tenacity python-dotenv`.
- Install GraphLite Python bindings from repo root so syntax validation works: `cargo build -p graphlite-ffi --release && pip install -e bindings/python`.
- Copy `nl2gql/config.example.env` → `nl2gql/config.env` if you want a local file; only `OPENAI_API_KEY` is required. Everything else is optional.
- Optional env vars (defaults if unset): `OPENAI_MODEL_GEN=gpt-4o-mini`, `OPENAI_MODEL_FIX=gpt-4o-mini`, `NL2GQL_DB_PATH=./.nl2gql_cache`, `NL2GQL_USER=admin`, `NL2GQL_SCHEMA=nl2gql`, `NL2GQL_GRAPH=scratch`.

## Usage (modular pipeline)

- Single run:
```bash
python3 -m nl2gql.pipeline.cli \
  --nl "find people older than 30" \
  --schema-file nl2gql/sample_schema.txt \
  --verbose
```
- Sample suite:
```bash
python3 -m nl2gql.pipeline.cli \
  --sample-suite \
  --suite-file nl2gql/sample_suites.json \
  --max-attempts 3
```
- Flags: `--schema-file` or `--schema` (inline), `--max-attempts`, `--gen-model` / `--fix-model`, `--db-path`, `--spinner/--no-spinner`, `--verbose`.

## Pipeline outline

- Modules: `schema_graph`, `preprocess`, `intent_linker`, `generator`, `validators`, `runner`, `refiner`, `cli`.
- Steps: parse schema → draft intent → link to schema → generate constrained IR/ISO GQL → syntax (GraphLite) + logic check → repair loop (bounded).

## Tests

- Run all: `pytest`
- What they cover (unit-level, deterministic stubs):
  - Schema parsing / property & edge checks
  - Preprocessor filtering + hint surfacing
  - IR parse/render round-trip + alias normalization/repairs
  - Grounding/link normalization and refiner happy-path

## Troubleshooting

- If you see “GraphLite Python bindings are missing”, rebuild/install bindings from repo root.
- Ensure `config.env` is loaded or set `OPENAI_API_KEY` in your environment.
- When logic or syntax fails, rerun with `--verbose` to inspect per-attempt feedback and AST errors.
