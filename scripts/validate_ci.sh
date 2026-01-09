#!/bin/bash

# ==============================================================================
# GraphLite CI Local Testing Script
# ==============================================================================
# This script tests CI components locally before pushing to GitHub
# Usage: ./scripts/test_ci_locally.sh [options]
# Options:
#   --quick      Run only quick checks (formatting, linting)
#   --full       Run full suite including build and tests (default)
#   --help       Show this help message
# ==============================================================================

set -e

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Default mode
TEST_MODE="full"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            TEST_MODE="quick"
            shift
            ;;
        --full)
            TEST_MODE="full"
            shift
            ;;
        --help)
            echo "GraphLite CI Local Testing Script"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --quick      Run only quick checks (formatting, linting)"
            echo "  --full       Run full suite including build and tests (default)"
            echo "  --help       Show this help message"
            echo ""
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}=== Testing CI Components Locally ===${NC}"
echo ""
echo "Mode: $TEST_MODE"
echo "Date: $(date)"
echo ""

# Track results
FAILED_CHECKS=()

# Function to run a check
run_check() {
    local check_name="$1"
    local check_command="$2"

    echo -e "${BLUE}Running: $check_name${NC}"

    if eval "$check_command"; then
        echo -e "${GREEN}✅ $check_name passed${NC}"
        echo ""
        return 0
    else
        echo -e "${RED}❌ $check_name failed${NC}"
        echo ""
        FAILED_CHECKS+=("$check_name")
        return 1
    fi
}

# 1. Validate workflow syntax (if actionlint is available)
if command -v actionlint &> /dev/null; then
    run_check "Workflow syntax validation" "actionlint .github/workflows/*.yml" || true
else
    echo -e "${YELLOW}⚠️  actionlint not installed, skipping workflow syntax validation${NC}"
    echo "   Install with: brew install actionlint (macOS) or download from GitHub"
    echo ""
fi

# 2. Check code formatting
run_check "Code formatting check" "cargo fmt --all -- --check" || true

# 3. Run clippy
# Note: Allowing warnings to match CI workflow (see .github/workflows/ci.yml for TODO)
run_check "Clippy linting" "cargo clippy --all-targets --all-features -- -A clippy::not_unsafe_ptr_arg_deref -A clippy::approx_constant" || true

# Quick mode stops here
if [ "$TEST_MODE" = "quick" ]; then
    echo -e "${BLUE}=== Quick Check Summary ===${NC}"

    if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
        echo -e "${GREEN}✅ All quick checks passed!${NC}"
        echo ""
        echo "Next steps:"
        echo "  1. Run full tests with: $0 --full"
        echo "  2. Or push to test branch: git push origin test/ci-workflows"
        exit 0
    else
        echo -e "${RED}❌ Some checks failed:${NC}"
        for check in "${FAILED_CHECKS[@]}"; do
            echo "  - $check"
        done
        exit 1
    fi
fi

# Full mode continues with build and tests
echo -e "${BLUE}=== Running Full Test Suite ===${NC}"
echo ""

# 4. Build project
run_check "Release build" "./scripts/build_all.sh --release" || true

# 5. Run tests
run_check "Integration tests" "./scripts/run_tests.sh --release" || true

# 6. Build documentation
run_check "Documentation build" "cargo doc --no-deps --all-features" || true

# 7. Test doc tests
run_check "Documentation tests" "cargo test --doc" || true

# Optional: Security audit (if cargo-audit is installed)
if command -v cargo-audit &> /dev/null; then
    run_check "Security audit" "cargo audit" || true
else
    echo -e "${YELLOW}⚠️  cargo-audit not installed, skipping security audit${NC}"
    echo "   Install with: cargo install cargo-audit"
    echo ""
fi

# Optional: Python bindings test (if Python environment is set up)
if [ -d "bindings/python" ] && command -v python3 &> /dev/null; then
    echo -e "${BLUE}Testing Python bindings...${NC}"

    # Check if FFI library exists
    if [ ! -f "target/release/libgraphlite_ffi.so" ] && [ ! -f "target/release/libgraphlite_ffi.dylib" ]; then
        echo -e "${YELLOW}⚠️  FFI library not found, building...${NC}"
        (cd graphlite-ffi && cargo build --release) || echo -e "${RED}FFI build failed${NC}"
    fi

    echo -e "${YELLOW}Python binding tests skipped (run manually if needed)${NC}"
    echo "   To test: cd bindings/python && pip install -e '.[dev]' && pytest"
    echo ""
fi

# Final summary
echo -e "${BLUE}=== Full Test Summary ===${NC}"
echo ""

if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    echo -e "${GREEN}🎉 All CI component tests passed!${NC}"
    echo ""
    echo "Your code is ready for CI/CD. Next steps:"
    echo ""
    echo "1. Commit your changes:"
    echo "   git add .github/ scripts/"
    echo "   git commit -m 'feat: Add CI/CD pipeline with GitHub Actions'"
    echo ""
    echo "2. Push to test branch (recommended):"
    echo "   git checkout -b test/verify-ci"
    echo "   git push origin test/verify-ci"
    echo "   # Then check GitHub Actions tab"
    echo ""
    echo "3. Or push directly:"
    echo "   git push origin chore/implement-ci-cd"
    echo ""
    exit 0
else
    echo -e "${RED}❌ The following checks failed:${NC}"
    echo ""
    for check in "${FAILED_CHECKS[@]}"; do
        echo "  - $check"
    done
    echo ""
    echo "Please fix the failing checks before pushing to GitHub."
    exit 1
fi
