# GitHub Actions CI/CD Workflows

This directory contains the GitHub Actions workflows for GraphLite's continuous integration and deployment pipeline.

## Workflows Overview

### 1. CI Workflow ([ci.yml](ci.yml))
**Triggers:** Push to main/develop branches, Pull requests

Comprehensive continuous integration pipeline that runs on every push and PR:

- **Lint and Format Check**
  - Code formatting with `rustfmt`
  - Linting with `clippy`

- **Build and Test** (Ubuntu & macOS)
  - Debug and Release builds
  - Unit tests
  - Integration tests (185 unit + 250+ integration tests)
  - Binary artifacts upload

- **Security Audit**
  - Dependency vulnerability scanning with `cargo-audit`

- **Code Pattern Checks**
  - Custom code pattern validation

- **Documentation Tests**
  - Doc tests execution
  - Documentation build verification

- **Code Coverage**
  - Coverage reports with `cargo-tarpaulin`
  - Upload to Codecov

### 2. Release Workflow ([release.yml](release.yml))
**Triggers:** Version tags (v*.*.*), Manual dispatch

Automated release pipeline for building and publishing releases:

- **Create Release**
  - GitHub release creation
  - Release notes generation

- **Build Release Binaries**
  - Multi-platform builds:
    - Linux (x86_64)
    - macOS (x86_64, ARM64/M1)
  - Binary stripping and optimization
  - Checksum generation (SHA256)
  - Asset upload to GitHub releases

- **Python Wheels**
  - Multi-platform Python wheel builds
  - Python 3.8 - 3.12 support
  - PyPI publishing (on tag push)

### 3. Python Bindings Workflow ([python-bindings.yml](python-bindings.yml))
**Triggers:** Push/PR to python bindings code

Tests and validates Python bindings:

- **Test Python Bindings**
  - Multi-platform testing (Ubuntu, macOS)
  - Python 3.8 - 3.12 compatibility
  - FFI library building
  - pytest test suite
  - Code coverage

- **Code Quality**
  - Black formatting checks
  - mypy type checking

- **Build Wheels**
  - Wheel building for distribution

- **Quick Start Example**
  - Validates quick start example works

### 4. Dependency Updates Workflow ([dependencies.yml](dependencies.yml))
**Triggers:** Weekly schedule (Monday 9 AM UTC), Manual dispatch

Automated dependency management:

- **Update Rust Dependencies**
  - Weekly dependency updates
  - Automated testing
  - Pull request creation

- **Security Audit**
  - Weekly security scans
  - Automatic issue creation on vulnerabilities

### 5. Benchmark Workflow ([benchmark.yml](benchmark.yml))
**Triggers:** Push to main, Pull requests, Manual dispatch

Performance monitoring:

- **Run Benchmarks**
  - Cargo benchmark execution
  - Performance regression detection
  - Results tracking over time

- **Performance Tests**
  - CLI performance testing
  - Binary size monitoring

## Build Matrix

### Operating Systems
- **Ubuntu Latest** (Primary Linux target)
- **macOS Latest** (x86_64 and ARM64)

### Build Modes
- **Debug** - Fast compilation, with debug assertions
- **Release** - Optimized production builds

### Python Versions
- Python 3.8
- Python 3.9
- Python 3.10
- Python 3.11
- Python 3.12

## Caching Strategy

All workflows use GitHub Actions caching to speed up builds:

1. **Cargo Registry** - Downloaded crates
2. **Cargo Git Index** - Git dependencies
3. **Build Artifacts** - Compiled dependencies

Cache keys are based on:
- Operating system
- `Cargo.lock` hash
- Build mode (debug/release)

## Secrets Required

For full functionality, configure these secrets in your GitHub repository:

- `GITHUB_TOKEN` - Auto-provided by GitHub Actions
- `PYPI_API_TOKEN` - For PyPI package publishing (optional)
- `CODECOV_TOKEN` - For Codecov uploads (optional)

## Running Workflows Locally

### Build and Test
```bash
# Debug build with tests
./scripts/build_all.sh --test

# Release build with tests
./scripts/build_all.sh --release --test

# Run integration tests
./scripts/run_tests.sh --release
```

### Security Audit
```bash
cargo install cargo-audit
cargo audit
```

### Code Formatting
```bash
# Check formatting
cargo fmt --all -- --check

# Auto-format
cargo fmt --all
```

### Linting
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Workflow Badges

Add these badges to your README.md:

```markdown
![CI](https://github.com/GraphLite-AI/GraphLite/workflows/CI/badge.svg)
![Release](https://github.com/GraphLite-AI/GraphLite/workflows/Release/badge.svg)
[![codecov](https://codecov.io/gh/GraphLite-AI/GraphLite/branch/main/graph/badge.svg)](https://codecov.io/gh/GraphLite-AI/GraphLite)
```

## Troubleshooting

### Tests Failing in CI but Passing Locally
- Tests now run in parallel by default (instance-based session isolation)
- Check if the build mode matches (debug vs release)
- Verify all test dependencies are properly isolated

### Cache Issues
- Clear caches by changing the cache key version
- Use workflow dispatch to manually trigger cache refresh

### Python Binding Failures
- Verify FFI library builds successfully
- Check Python version compatibility
- Ensure ctypes is available (standard library)

## Maintenance

### Weekly Tasks
- Review dependency update PRs (automated)
- Check security audit results

### Release Process
1. Update version in `Cargo.toml`
2. Create and push version tag: `git tag v0.1.0 && git push --tags`
3. Release workflow automatically builds and publishes
4. Verify release artifacts on GitHub Releases page

## Contributing

When adding new workflows:
1. Test locally first when possible
2. Use caching to minimize build times
3. Add appropriate triggers
4. Document the workflow in this README
5. Consider resource usage (GitHub Actions minutes)
