# Quick Start: Testing CI/CD Workflows

This guide helps you test the GitHub Actions workflows **before pushing to GitHub**.

## Option 1: Quick Test (Recommended - 2 minutes)

Run the automated test script:

```bash
# Quick check (formatting + linting only)
./scripts/test_ci_locally.sh --quick

# Full check (includes build + tests)
./scripts/test_ci_locally.sh --full
```

This tests all the same checks that CI will run!

## Option 2: Using `act` (Advanced - Local GitHub Actions)

Install `act` to run GitHub Actions locally:

### Install `act`

**Ubuntu/WSL:**
```bash
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash
```

**macOS:**
```bash
brew install act
```

### Run Workflows Locally

```bash
# List all workflows and jobs
act -l

# Test the lint job (fast)
act -j lint -W .github/workflows/ci.yml

# Dry run - see what would happen
act -n -W .github/workflows/ci.yml

# Run full CI workflow (takes time)
act push -W .github/workflows/ci.yml
```

**Note:** The `.actrc` file is already configured with optimal settings for GraphLite.

## Option 3: Push to Test Branch (Safest)

Test on GitHub without affecting main branch:

```bash
# Create test branch
git checkout -b test/verify-ci

# Commit workflows
git add .github/ scripts/
git commit -m "test: verify CI workflows"

# Push to test branch
git push origin test/verify-ci

# Go to GitHub Actions tab and watch it run
# URL: https://github.com/GraphLite-AI/GraphLite/actions
```

Monitor the results, then:

```bash
# If successful, merge back
git checkout chore/implement-ci-cd
git merge test/verify-ci

# Clean up test branch
git branch -D test/verify-ci
git push origin --delete test/verify-ci
```

## What Gets Tested

✅ **Formatting** - `cargo fmt --all -- --check`
✅ **Linting** - `cargo clippy --all-targets --all-features`
✅ **Build** - `./scripts/build_all.sh --release`
✅ **Tests** - `./scripts/run_tests.sh --release`
✅ **Docs** - `cargo doc --no-deps --all-features`
✅ **Security** - `cargo audit` (if installed)

## Troubleshooting

### Tests fail locally but should pass?

**Fix formatting:**
```bash
cargo fmt --all
```

**Fix clippy warnings:**
Review the warnings and fix them, or allow specific ones if needed.

### Want to test on specific OS?

Use the test branch method (Option 3) and check both Ubuntu and macOS results on GitHub.

### `act` fails with Docker errors?

Ensure Docker is running:
```bash
sudo systemctl start docker  # Linux
# or start Docker Desktop on macOS
```

## Recommended Workflow

1. **Make changes** to workflows or code
2. **Quick test locally:**
   ```bash
   ./scripts/test_ci_locally.sh --quick
   ```
3. **If passing, full test:**
   ```bash
   ./scripts/test_ci_locally.sh --full
   ```
4. **If all local tests pass, push to test branch:**
   ```bash
   git push origin test/verify-ci
   ```
5. **Monitor GitHub Actions, then merge if successful**

## Need Help?

- See detailed docs: [.github/workflows/TEST_WORKFLOWS.md](.github/workflows/TEST_WORKFLOWS.md)
- Check workflow configs: [.github/workflows/](.github/workflows/)
- Run with help: `./scripts/test_ci_locally.sh --help`
