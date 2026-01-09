# GraphLite Scripts

This directory contains utility scripts for building, testing, and managing the GraphLite project.

## Available Scripts

### Build Scripts

#### `build_all.sh`
Builds the GraphLite Rust library and CLI binary.

```bash
# Basic build (debug mode)
./scripts/build_all.sh

# Optimized release build
./scripts/build_all.sh --release

# Clean build
./scripts/build_all.sh --clean --release

# Build and run tests
./scripts/build_all.sh --release --test
```

**Options:**
- `--release` - Build in release mode (optimized)
- `--test` - Run tests after building
- `--clean` - Clean before building
- `--help` - Show help message

**Build output locations:**
- **Debug mode**: `target/debug/libgraphlite.rlib` and `target/debug/graphlite`
- **Release mode**: `target/release/libgraphlite.rlib` and `target/release/graphlite`

### Cleanup Scripts

#### `cleanup.sh`
Uninstalls and cleans up all GraphLite project artifacts.

```bash
# Show help (also shown when no options provided)
./scripts/cleanup.sh --help

# Clean build artifacts only
./scripts/cleanup.sh --build

# Clean Python/Java bindings
./scripts/cleanup.sh --bindings

# Complete cleanup (bindings, build, data, config)
./scripts/cleanup.sh --all
```

**Options:**
- `--build` - Clean build artifacts
- `--bindings` - Uninstall Python/Java bindings
- `--all` - Complete cleanup including data and configuration
- `--help` - Show help message

**Safety:** Requires explicit option - no default action to prevent accidental cleanup.

**What gets cleaned:**
- `--build`: Rust build artifacts (`target/`), compiled binaries (`.so`, `.dylib`, `.dll`), `Cargo.lock`
- `--bindings`: Python packages (uninstall via pip), Python build artifacts (`build/`, `dist/`, `*.egg-info`), Java artifacts (Maven `target/`, JAR files)
- `--all`: All of the above plus database files (`data/`, `example_db/`, `mydb/`), configuration (`.graphlite/`), log files, temporary files

### Testing Scripts

#### `run_unit_tests.sh`
Runs only unit tests (fast, ~2-3 seconds).

```bash
./scripts/run_unit_tests.sh
```

#### `run_tests.sh`
Runs integration tests sequentially (slower, ~10-15 minutes).

```bash
# Debug mode (default)
./scripts/run_tests.sh

# Release mode (faster execution)
./scripts/run_tests.sh --release

# With failure analysis
./scripts/run_tests.sh --release --analyze
```

#### `run_integration_tests_parallel.sh`
Runs integration tests in parallel using GNU Parallel (~1.5-4 minutes, **10x faster**).

**Prerequisite:** Requires GNU Parallel to be installed:
```bash
# macOS
brew install parallel

# Ubuntu/Debian
sudo apt install parallel
```

**Usage:**
```bash
# Default: 4 parallel jobs, debug mode
./scripts/run_integration_tests_parallel.sh

# Release mode with 8 parallel jobs (recommended)
./scripts/run_integration_tests_parallel.sh --release --jobs=8

# With failure analysis
./scripts/run_integration_tests_parallel.sh --release --analyze
```

**Performance:** With 8 jobs, completes 169 integration tests in ~75 seconds vs 10-15 minutes sequential.

#### `test_cli.sh`
End-to-end tests for the GraphLite CLI binary functionality.

```bash
./scripts/test_cli.sh
```

#### `validate_ci.sh`
Validates that code will pass GitHub Actions CI/CD pipeline before pushing.

```bash
# Quick check: formatting + linting only (~30 seconds)
./scripts/validate_ci.sh --quick

# Full check: includes build + tests (~5-10 minutes)
./scripts/validate_ci.sh --full
```

### Linting Scripts

#### `clippy_all.sh`
Runs Clippy linter on the GraphLite project with configurable strictness levels.

```bash
# Basic clippy check (library and binaries)
./scripts/clippy_all.sh

# Check all targets (lib, bins, tests, benches, examples)
./scripts/clippy_all.sh --all

# Strict mode: treat warnings as errors (CI requirement)
./scripts/clippy_all.sh --strict

# Auto-fix suggestions where possible
./scripts/clippy_all.sh --fix

# Pedantic mode: extra strict linting
./scripts/clippy_all.sh --pedantic

# Combined: check all targets with strict mode (CI simulation)
./scripts/clippy_all.sh --all --strict
```

**Options:**
- `--fix` - Automatically apply Clippy suggestions where possible
- `--strict` - Treat all warnings as errors (required for CI)
- `--pedantic` - Enable pedantic lints (extra strict)
- `--all` - Check all targets (lib, bins, tests, benches, examples)
- `--help` - Show help message

**Modes:**
- **Default**: Standard lints for main library code (lib + bins)
- **--all**: Comprehensive check of all targets
- **--strict**: Fail on any warnings (recommended before committing)
- **--pedantic**: Additional pedantic lints for code quality
- **--fix**: Automatically apply safe fixes

**CI Usage:** The GitHub Actions CI pipeline uses `./scripts/clippy_all.sh --all` to ensure consistent linting.

**Note:** Currently ~26 non-critical warnings remain in the codebase (mostly type complexity). Future work will fix these to enable `--strict` mode in CI.

### Development Scripts

#### `install_hooks.sh`
Installs Git hooks for the project.

```bash
./scripts/install_hooks.sh
```

#### `check_code_patterns.sh`
Enforces GraphLite-specific architectural patterns and coding rules.

```bash
./scripts/check_code_patterns.sh
```

**What it checks:**
- **11 critical architectural rules** specific to GraphLite
- Custom pattern violations (not covered by standard Rust linting)
- Ensures singleton patterns are followed (ExecutionContext, StorageManager, CatalogManager)
- Validates proper lock usage (read locks for reads, write locks for writes)
- Checks async runtime management patterns
- Enforces test integrity and API boundary rules
- Validates documentation standards (no emojis in markdown)

**When to run:**
- Before committing changes to `src/` or `tests/`
- When modifying core execution, storage, or catalog code
- As part of pre-commit workflow

**Example output:**
```
Checking critical rules...
✓ Rule 1: No new ExecutionContext instances
✓ Rule 2: No new StorageManager instances
✗ Rule 3: Read vs Write locks - Found 2 violations
  - src/exec/executor.rs:145: Use read() for read operations
```

**See also:** CONTRIBUTING.md for complete list of all 11 rules

---

## Script Comparison Guide

### `check_code_patterns.sh` vs `validate_ci.sh`

**Use `check_code_patterns.sh` for:**
- GraphLite-specific architectural rules
- Fast local validation (~5 seconds)
- Before committing code changes
- Catching pattern violations early

**Use `validate_ci.sh` for:**
- Simulating what CI will check
- Standard Rust tooling (fmt, clippy, build, test)
- Before pushing to GitHub
- Comprehensive pre-push validation

**Quick reference:**

| Check | check_code_patterns.sh | validate_ci.sh |
|-------|----------------------|----------------|
| GraphLite rules | ✅ | ❌ |
| Code formatting | ❌ | ✅ |
| Clippy linting | ❌ | ✅ |
| Build/tests | ❌ | ✅ (--full) |
| Speed | ~5 seconds | ~30s (quick) / ~10min (full) |
| When | Before commit | Before push |

**Recommended workflow:**
```bash
# Before committing:
cargo fmt --all
./scripts/clippy_all.sh --all
./scripts/check_code_patterns.sh

# Before pushing:
./scripts/validate_ci.sh --quick
```

---

## Common Workflows

### Fresh Build
```bash
# Clean everything and rebuild from scratch
./scripts/cleanup.sh --all
./scripts/build_all.sh --release
```

### Development Cycle
```bash
# Build in debug mode (faster compilation)
./scripts/build_all.sh

# Make changes...

# Clean and rebuild when needed
./scripts/cleanup.sh --build
./scripts/build_all.sh
```

### Testing Workflow
```bash
# Build and test
./scripts/build_all.sh --test

# Or run tests separately
./scripts/run_tests.sh
```

### Pre-Commit Workflow

**REQUIRED before every commit:**

```bash
# 1. Format code (auto-fix)
cargo fmt --all

# 2. Run clippy linter on all targets (REQUIRED - must pass)
./scripts/clippy_all.sh --all

# 3. Quick validation that CI will pass (RECOMMENDED)
./scripts/validate_ci.sh --quick
```

**Note:** All contributors must run `./scripts/clippy_all.sh --all` before committing. This ensures consistent code quality and prevents CI failures.

### Complete Uninstall
```bash
# Remove everything (bindings, build artifacts, data, config)
./scripts/cleanup.sh --all
```

## Script Requirements

- **Bash**: All scripts require Bash shell
- **Rust/Cargo**: Required for build scripts
- **Python/pip**: Required for Python binding cleanup
- **Java/Maven**: Required for Java binding cleanup

## Notes

- All scripts include colored output for better readability
- Scripts automatically detect and configure Rust/Cargo PATH when needed
- Use `--help` with any script to see detailed usage information
- Scripts are safe to run multiple times (idempotent)
