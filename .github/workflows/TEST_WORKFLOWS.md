# Testing GitHub Actions Workflows Locally

This guide explains how to test your GitHub Actions workflows without pushing commits to the repository.

## Method 1: Using `act` (Recommended)

`act` runs your GitHub Actions workflows locally in Docker containers.

### Installation

**Ubuntu/Linux:**
```bash
# Using curl
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash

# Or using package manager (if available)
# Ubuntu/Debian
sudo apt-get install act
```

**macOS:**
```bash
brew install act
```

**Verify installation:**
```bash
act --version
```

### Basic Usage

**1. List available workflows:**
```bash
act -l
```

**2. Run all workflows (dry run):**
```bash
act -n
```

**3. Run specific workflow:**
```bash
# Run CI workflow
act push -W .github/workflows/ci.yml

# Run on pull_request event
act pull_request -W .github/workflows/ci.yml
```

**4. Run specific job:**
```bash
# List jobs in CI workflow
act -l -W .github/workflows/ci.yml

# Run only the lint job
act -j lint -W .github/workflows/ci.yml

# Run only build-and-test job
act -j build-and-test -W .github/workflows/ci.yml
```

**5. Test with different platforms:**
```bash
# Use medium-sized runner (more resources)
act -P ubuntu-latest=catthehacker/ubuntu:act-latest

# Use specific platform
act -P ubuntu-latest=catthehacker/ubuntu:rust-latest
```

### Configuration

Create `.actrc` in the repository root for default settings:

```bash
# .actrc
-P ubuntu-latest=catthehacker/ubuntu:act-latest
--container-architecture linux/amd64
```

### Common `act` Commands

```bash
# Dry run - see what would happen
act -n

# Run with verbose output
act -v

# Run specific event
act push
act pull_request
act workflow_dispatch

# Skip specific jobs
act --job lint --job build-and-test

# Use secrets from file
act --secret-file .secrets

# Bind workspace (helpful for debugging)
act --bind
```

## Method 2: Manual Component Testing

Test the individual components that workflows use:

### Test Build Scripts

```bash
# Test debug build
./scripts/build_all.sh

# Test release build
./scripts/build_all.sh --release

# Test with tests
./scripts/build_all.sh --release --test
```

### Test Formatting and Linting

```bash
# Check formatting (what CI does)
cargo fmt --all -- --check

# Auto-format if needed
cargo fmt --all

# Run clippy (what CI does)
cargo clippy --all-targets --all-features -- -D warnings
```

### Test Security Audit

```bash
# Install cargo-audit
cargo install cargo-audit

# Run audit (what CI does)
cargo audit
```

### Test Integration Tests

```bash
# Run all integration tests (debug)
./scripts/run_tests.sh --debug

# Run all integration tests (release)
./scripts/run_tests.sh --release

# Run with analysis
./scripts/run_tests.sh --release --analyze

# Run specific test (parallel execution enabled)
cargo test --release --test aggregation_tests
```

### Test Documentation

```bash
# Test doc tests
cargo test --doc

# Build documentation
cargo doc --no-deps --all-features

# Open docs in browser
cargo doc --no-deps --all-features --open
```

### Test Python Bindings

```bash
# Build FFI library
cd graphlite-ffi
cargo build --release
cd ..

# Install and test Python package
cd bindings/python
pip install -e ".[dev]"
pytest tests/ -v
black --check .
mypy graphlite --ignore-missing-imports
cd ../..
```

## Method 3: Use a Test Branch

Create a temporary test branch to push and test workflows without affecting main:

```bash
# Create test branch
git checkout -b test/ci-workflows

# Make your changes
git add .github/workflows/
git commit -m "test: CI workflows"

# Push to test branch
git push origin test/ci-workflows

# Watch the workflows run on GitHub
# Go to: https://github.com/GraphLite-AI/GraphLite/actions

# Clean up after testing
git checkout main
git branch -D test/ci-workflows
git push origin --delete test/ci-workflows
```

## Method 4: Validate Workflow Syntax

Use GitHub's workflow validator:

```bash
# Install actionlint
# Ubuntu/Linux
wget https://github.com/rhysd/actionlint/releases/latest/download/actionlint_linux_amd64.tar.gz
tar xf actionlint_linux_amd64.tar.gz
sudo mv actionlint /usr/local/bin/

# macOS
brew install actionlint

# Validate all workflows
actionlint .github/workflows/*.yml

# Validate specific workflow
actionlint .github/workflows/ci.yml
```

## Recommended Testing Strategy

### Before First Push

1. **Validate syntax:**
   ```bash
   actionlint .github/workflows/*.yml
   ```

2. **Test individual components:**
   ```bash
   # Formatting
   cargo fmt --all -- --check

   # Linting
   cargo clippy --all-targets --all-features -- -D warnings

   # Build
   ./scripts/build_all.sh --release

   # Tests
   ./scripts/run_tests.sh --release
   ```

3. **Test with `act` (if installed):**
   ```bash
   # Dry run
   act -n -W .github/workflows/ci.yml

   # Test lint job
   act -j lint -W .github/workflows/ci.yml

   # Test build job (takes longer)
   act -j build-and-test -W .github/workflows/ci.yml
   ```

4. **Push to test branch:**
   ```bash
   git checkout -b test/verify-ci
   git add .github/
   git commit -m "test: verify CI workflows"
   git push origin test/verify-ci
   ```

5. **Monitor on GitHub:**
   - Go to Actions tab
   - Watch workflows execute
   - Check for any failures
   - Review logs

6. **Merge if successful:**
   ```bash
   git checkout chore/implement-ci-cd
   git merge test/verify-ci
   git branch -D test/verify-ci
   git push origin --delete test/verify-ci
   ```

## Quick Test Script

Create a quick test script for common validations:

```bash
#!/bin/bash
# scripts/test_ci_locally.sh

set -e

echo "=== Testing CI Components Locally ==="
echo ""

echo "1. Validating workflow syntax..."
if command -v actionlint &> /dev/null; then
    actionlint .github/workflows/*.yml
    echo "Workflow syntax valid"
else
    echo "actionlint not installed, skipping syntax check"
fi
echo ""

echo "2. Checking code formatting..."
cargo fmt --all -- --check
echo "Code formatting passed"
echo ""

echo "3. Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings
echo "Clippy passed"
echo ""

echo "4. Building project (release)..."
./scripts/build_all.sh --release
echo "Build successful"
echo ""

echo "5. Running tests..."
./scripts/run_tests.sh --release
echo "Tests passed"
echo ""

echo "=== All CI component tests passed! ==="
echo "Ready to push to GitHub."
```

Make it executable and run:
```bash
chmod +x scripts/test_ci_locally.sh
./scripts/test_ci_locally.sh
```

## Troubleshooting

### `act` Issues

**Docker not running:**
```bash
sudo systemctl start docker
```

**Permission denied:**
```bash
sudo usermod -aG docker $USER
# Log out and log back in
```

**Container architecture mismatch:**
```bash
act --container-architecture linux/amd64
```

### Common Workflow Errors

**Cache key issues:**
- Caches are optional; workflows will work without them (just slower)

**Secret not found:**
- Use `act --secret-file .secrets` to provide secrets locally
- For CI, secrets are only needed for PyPI/Codecov (optional)

**Checkout fails:**
- `act` automatically uses your local repository
- No need to actually checkout when testing locally

## Summary

**Best for quick validation:**
```bash
# Install actionlint
brew install actionlint  # or download binary

# Validate workflows
actionlint .github/workflows/*.yml

# Test individual components
./scripts/build_all.sh --release --test
```

**Best for complete testing:**
```bash
# Install act
brew install act  # or curl method

# Test specific workflow
act -j lint -W .github/workflows/ci.yml

# Or push to test branch
git push origin test/ci-workflows
```

Choose the method that best fits your workflow!
