# NL2GQL Pipeline Components & Contracts

This captures the surface area, I/O contracts, and invariants for each stage so components can be exercised in isolation (without running the full sample suite).

## Stages
- **Preprocess (`preprocess.Preprocessor.run`)**
  - **In**: natural language (NL), recent failure feedback, `SchemaGraph`.
  - **Out**: `PreprocessResult` with `normalized_nl`, `phrases`, `FilteredSchema` (subset of nodes/edges + strategy hits), and `structural_hints` (path + heuristic cues).
  - **Invariants**: normalization is whitespace-stable; filtered schema must be non-empty; hints align to nodes/edges present.
- **Intent + Linking (`intent_linker` + `SchemaGraph`)**
  - **In**: NL, `PreprocessResult`, feedback list.
  - **Out**: `IntentLinkGuidance(frame, links)` where `frame` holds targets/filters/metrics/order/limit/path_hints and `links` contains grounded `node_links`, `property_links`, `rel_links`, `canonical_aliases`.
  - **Invariants**: links reference valid schema labels/props/rels after grounding; aliases are unique; path_hints are derivable from schema edges.
- **Contract Builder (`requirements.build_contract`)**
  - **In**: NL, preprocess output, link guidance, `SchemaGraph`.
  - **Out**: `RequirementContract` (required labels/edges/properties/metrics/order/limit).
  - **Invariants**: all required_* exist in schema; limit is positive when set.
- **Generation (`generator.QueryGenerator.generate`)**
  - **In**: preprocess output, prior failures, link guidance, contract.
  - **Out**: list of `CandidateQuery` (text + reason + usage).
  - **Invariants**: at least one candidate; candidates respect contract hints when provided; outputs are JSON-safe strings.
- **IR Parsing (`ir.ISOQueryIR.parse`)**
  - **In**: ISO GQL text.
  - **Out**: `(ISOQueryIR | None, parse_errors)`.
  - **Invariants**: MATCH block required; nodes/edges de-duplicate; renders round-trip without changing semantics.
- **Schema & Coverage Validation (`validators.SchemaGroundingValidator`, `requirements.coverage_violations`)**
  - **In**: `ISOQueryIR`, `SchemaGraph`, `RequirementContract`.
  - **Out**: schema errors (unknown labels/props/edges), coverage errors (missing required labels/edges/props/metrics/order/limit).
  - **Invariants**: errors are deterministic; no schema invention allowed.
- **Syntax Validation (`runner.GraphLiteRunner.validate`)**
  - **In**: rendered ISO GQL string.
  - **Out**: `SyntaxResult(ok, error, rows)`.
  - **Invariants**: fails fast on empty strings; uses in-memory DB when no path provided.
- **Logic Validation (`validators.LogicValidator.validate`)**
  - **In**: NL, schema summary, rendered query, hints.
  - **Out**: `(is_valid: bool, reason: Optional[str])`.
  - **Invariants**: votes across temperatures; reasons explain missing constraints.
- **Refinement Loop (`refiner.Refiner.run`)**
  - **In**: NL, injected preprocessor + intent linker, spinner (optional).
  - **Out**: final rendered query, timeline of attempts.
  - **Invariants**: bounded attempts; always records timeline; halts on first all-clear (parse + schema + coverage + syntax + logic).

## Failure Attribution Signals
- Trace each stage with a `trace_id` and persist: NL, schema subset, intent frame, grounded links, contract view, candidates, IR diagnostics, coverage errors, syntax/logic verdicts.
- First failing stage in the chain is the likely “weak link”; later stages depend on earlier outputs.

## How to Use This Doc
- When adding a component harness or tests, assert the invariants above.
- When debugging a failure, compare actual outputs with the contract for that stage to isolate which invariant broke.


