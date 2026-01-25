#!/bin/bash

# ==============================================================================
# GraphLite Clippy Linting Script
# ==============================================================================
# This script runs Clippy linter on the GraphLite project
# Usage: ./clippy_all.sh [options]
# Options:
#   --fix        Automatically apply Clippy suggestions where possible
#   --strict     Treat all warnings as errors (-D warnings)
#   --pedantic   Enable pedantic lints (extra strict)
#   --all        Check all targets (lib, bins, tests, benches, examples)
#   --help       Show this help message
# ==============================================================================

set -e  # Exit on error

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default options
FIX_MODE=false
STRICT_MODE=false
PEDANTIC_MODE=false
ALL_TARGETS=false
CLIPPY_FLAGS=""

# Function to print colored messages
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --fix)
            FIX_MODE=true
            shift
            ;;
        --strict)
            STRICT_MODE=true
            shift
            ;;
        --pedantic)
            PEDANTIC_MODE=true
            shift
            ;;
        --all)
            ALL_TARGETS=true
            shift
            ;;
        --help)
            echo "GraphLite Clippy Linting Script"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --fix        Automatically apply Clippy suggestions where possible"
            echo "  --strict     Treat all warnings as errors (-D warnings)"
            echo "  --pedantic   Enable pedantic lints (extra strict)"
            echo "  --all        Check all targets (lib, bins, tests, benches, examples)"
            echo "  --help       Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                    # Basic clippy check"
            echo "  $0 --strict           # Treat warnings as errors (CI mode)"
            echo "  $0 --fix              # Auto-fix suggestions"
            echo "  $0 --all --strict     # Check all targets with strict mode"
            echo "  $0 --pedantic         # Extra strict linting"
            echo ""
            echo "Modes:"
            echo "  Default:    Standard lints for main library code"
            echo "  --all:      Check library + binaries + tests + examples"
            echo "  --strict:   Fail on any warnings (recommended for CI)"
            echo "  --pedantic: Enable additional pedantic lints"
            echo "  --fix:      Automatically apply safe fixes"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# ==============================================================================
# Main Clippy Process
# ==============================================================================

echo "🔍 GraphLite Clippy Linting Script"
echo "================================="
echo ""

# Check if Rust/Cargo is installed
if ! command_exists cargo; then
    print_error "Cargo is not installed or not in PATH"
    print_info "Install Rust from https://rustup.rs/"
    exit 1
fi

# Check if clippy is installed
if ! command_exists cargo-clippy; then
    print_warning "Clippy is not installed"
    print_info "Installing clippy..."
    rustup component add clippy || {
        print_error "Failed to install clippy"
        exit 1
    }
    print_success "Clippy installed successfully"
fi

# Build clippy command
CLIPPY_CMD="cargo clippy"

# Add target flags
if [ "$ALL_TARGETS" = true ]; then
    CLIPPY_CMD="$CLIPPY_CMD --all-targets --all-features"
    print_info "Mode: Checking all targets (lib, bins, tests, benches, examples)"
else
    CLIPPY_CMD="$CLIPPY_CMD --lib --bins"
    print_info "Mode: Checking library and binaries"
fi

# Add lint level flags
LINT_FLAGS=""

if [ "$STRICT_MODE" = true ]; then
    LINT_FLAGS="$LINT_FLAGS -D warnings"
    print_info "Strict mode: Warnings will be treated as errors"
fi

if [ "$PEDANTIC_MODE" = true ]; then
    LINT_FLAGS="$LINT_FLAGS -W clippy::pedantic"
    print_info "Pedantic mode: Extra strict linting enabled"
fi

# Add fix mode
if [ "$FIX_MODE" = true ]; then
    CLIPPY_CMD="$CLIPPY_CMD --fix --allow-dirty --allow-staged"
    print_info "Fix mode: Automatically applying safe fixes"
fi

# Construct final command
if [ -n "$LINT_FLAGS" ]; then
    CLIPPY_CMD="$CLIPPY_CMD -- $LINT_FLAGS"
fi

echo ""
print_info "Running: $CLIPPY_CMD"
echo ""

# ==============================================================================
# Run Clippy
# ==============================================================================

START_TIME=$(date +%s)

if eval "$CLIPPY_CMD"; then
    print_success "Clippy check passed!"
else
    print_error "Clippy found issues"
    exit 1
fi

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# ==============================================================================
# Summary
# ==============================================================================

echo ""
echo "================================="
print_success "Clippy Linting Complete!"
echo "================================="
print_info "Duration: ${DURATION}s"

if [ "$FIX_MODE" = true ]; then
    print_info "Fixes have been applied"
    print_warning "Review changes with: git diff"
fi

if [ "$STRICT_MODE" = false ]; then
    print_warning "Run with --strict to treat warnings as errors (CI mode)"
fi

if [ "$ALL_TARGETS" = false ]; then
    print_info "Run with --all to check all targets including tests and examples"
fi

echo ""
echo "📝 Next Steps:"
echo "  - Review any warnings above"
if [ "$FIX_MODE" = false ]; then
    echo "  - Run with --fix to auto-apply safe suggestions"
fi
echo "  - Run with --strict before committing (CI requirement)"
echo "  - See clippy help: cargo clippy --help"
echo ""
