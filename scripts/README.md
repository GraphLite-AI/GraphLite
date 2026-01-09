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

### Development Scripts

#### `install_hooks.sh`
Installs Git hooks for the project.

```bash
./scripts/install_hooks.sh
```

#### `check_code_patterns.sh`
Validates code against established patterns and anti-patterns.

```bash
./scripts/check_code_patterns.sh
```

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
