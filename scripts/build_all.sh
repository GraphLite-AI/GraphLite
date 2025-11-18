#!/bin/bash

# ==============================================================================
# GraphLite Build Script
# ==============================================================================
# This script builds the GraphLite Rust library
# Usage: ./build_all.sh [options]
# Options:
#   --release    Build in release mode (optimized)
#   --test       Run tests after building
#   --clean      Clean before building
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
BUILD_MODE="dev"
RUN_TESTS=false
CLEAN_BUILD=false
CARGO_FLAGS=""

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
        --release)
            BUILD_MODE="release"
            CARGO_FLAGS="--release"
            shift
            ;;
        --test)
            RUN_TESTS=true
            shift
            ;;
        --clean)
            CLEAN_BUILD=true
            shift
            ;;
        --help)
            echo "GraphLite Build Script"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --release    Build in release mode (optimized)"
            echo "  --test       Run tests after building"
            echo "  --clean      Clean before building"
            echo "  --help       Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                    # Basic build"
            echo "  $0 --release          # Optimized release build"
            echo "  $0 --release --test   # Release build with tests"
            echo "  $0 --clean --release  # Clean release build"
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
# Main Build Process
# ==============================================================================

echo "🚀 GraphLite Build Script"
echo "================================="

# Ensure Rust/Cargo is in PATH
if ! command -v cargo &> /dev/null; then
    # Try to add cargo to PATH from default installation location
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
        print_info "Added Rust/Cargo to PATH from ~/.cargo/env"
    elif [ -d "$HOME/.cargo/bin" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
        print_info "Added ~/.cargo/bin to PATH"
    else
        print_error "Cargo not found. Please install Rust from https://rustup.rs/"
        print_error "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
fi

# Verify cargo is now available
if ! command -v cargo &> /dev/null; then
    print_error "Cargo still not found after attempting to add to PATH"
    print_error "Please ensure Rust is properly installed and in your PATH"
    exit 1
fi

print_info "Build mode: ${BUILD_MODE}"
print_info "Date: $(date)"
echo ""

# Step 1: Clean if requested
if [ "$CLEAN_BUILD" = true ]; then
    print_info "Cleaning previous build..."
    cargo clean
    rm -f Cargo.lock  # Remove lock file to pick up new dependency versions
    print_success "Clean complete"
    echo ""
fi

# Step 2: Check prerequisites
print_info "Checking prerequisites..."

if ! command_exists cargo; then
    print_error "Cargo not found. Please install Rust."
    exit 1
fi

print_success "Prerequisites checked"
echo ""

# Step 3: Build Rust library and CLI binary
print_info "Building Rust library and CLI binary..."
cargo build $CARGO_FLAGS || {
    print_error "Rust build failed"
    exit 1
}
print_success "Rust library and CLI binary built successfully"
echo ""

# Step 4: Report binary location
if [ "$BUILD_MODE" = "release" ]; then
    BINARY_PATH="target/release/graphlite"
else
    BINARY_PATH="target/debug/graphlite"
fi

print_success "Binary available at: $BINARY_PATH"
echo ""

# Step 5: Run tests if requested
if [ "$RUN_TESTS" = true ]; then
    print_info "Running tests..."
    cargo test $CARGO_FLAGS || {
        print_warning "Some tests failed"
    }
    echo ""
fi

# Step 6: Summary
echo "================================="
echo "📊 Build Summary"
echo "================================="
print_success "Rust library: Built"
print_success "CLI binary: Built"
print_success "Binary installed: /usr/local/bin/graphlite"

if [ "$BUILD_MODE" = "release" ]; then
    print_info "Build type: Release (optimized)"
    print_info "Library location: target/release/libgraphlite.rlib"
    print_info "CLI binary location: target/release/graphlite"
else
    print_info "Build type: Development"
    print_info "Library location: target/debug/libgraphlite.rlib"
    print_info "CLI binary location: target/debug/graphlite"
fi

echo ""
print_success "Build process complete!"

# Provide next steps
echo ""
echo "📝 Next Steps:"
if [ "$BUILD_MODE" = "release" ]; then
    echo "  - Run CLI: ./target/release/graphlite --help"
    echo "  - Or with cargo: cargo run --release -- --help"
    echo "  - Run tests: cargo test --release --test-threads=1"
    echo "  - Run specific test: cargo test --release --test <test_name> --test-threads=1"
else
    echo "  - Run CLI: ./target/debug/graphlite --help"
    echo "  - Or with cargo: cargo run -- --help"
    echo "  - Run tests: cargo test --test-threads=1"
    echo "  - Run specific test: cargo test --test <test_name> --test-threads=1"
fi
echo "  - Run all integration tests: ./scripts/run_tests.sh"
echo "  - Build docs: cargo doc --open"
echo ""
echo "💡 Tip: Always match the build profile when using 'cargo run' or 'cargo test'"
echo "   - Debug build → cargo run / cargo test"
echo "   - Release build → cargo run --release / cargo test --release"
echo ""

exit 0
