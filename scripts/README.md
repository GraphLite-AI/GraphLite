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

#### `run_tests.sh`
Runs all GraphLite tests (unit, integration, parser).

```bash
./scripts/run_tests.sh
```

#### `test_cli.sh`
Tests the GraphLite CLI functionality.

```bash
./scripts/test_cli.sh
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
