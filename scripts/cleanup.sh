#!/bin/bash

# ==============================================================================
# GraphLite Cleanup Script
# ==============================================================================
# This script uninstalls and cleans up everything related to GraphLite
# Usage: ./cleanup.sh [options]
# Options:
#   --all        Complete cleanup including database files and config
#   --bindings   Only cleanup Python/Java bindings
#   --build      Only cleanup build artifacts
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
CLEAN_ALL=false
CLEAN_BINDINGS=false
CLEAN_BUILD=false

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
if [[ $# -eq 0 ]]; then
    # No arguments provided - show help instead of doing anything
    echo "GraphLite Cleanup Script"
    echo ""
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --all        Complete cleanup (bindings, build artifacts, data, config)"
    echo "  --bindings   Only cleanup Python/Java bindings"
    echo "  --build      Only cleanup build artifacts"
    echo "  --help       Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 --build        # Clean build artifacts only"
    echo "  $0 --bindings     # Uninstall Python/Java bindings"
    echo "  $0 --all          # Complete cleanup including data and config"
    echo ""
    echo "Note: You must specify at least one option. No default action to prevent accidental cleanup."
    exit 0
else
    while [[ $# -gt 0 ]]; do
        case $1 in
            --all)
                CLEAN_ALL=true
                shift
                ;;
            --bindings)
                CLEAN_BINDINGS=true
                shift
                ;;
            --build)
                CLEAN_BUILD=true
                shift
                ;;
            --help)
                echo "GraphLite Cleanup Script"
                echo ""
                echo "Usage: $0 [options]"
                echo ""
                echo "Options:"
                echo "  --all        Complete cleanup (bindings, build artifacts, data, config)"
                echo "  --bindings   Only cleanup Python/Java bindings"
                echo "  --build      Only cleanup build artifacts"
                echo "  --help       Show this help message"
                echo ""
                echo "Examples:"
                echo "  $0 --build        # Clean build artifacts only"
                echo "  $0 --bindings     # Uninstall Python/Java bindings"
                echo "  $0 --all          # Complete cleanup including data and config"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done
fi

# Validate that at least one cleanup option was selected
if [ "$CLEAN_ALL" = false ] && [ "$CLEAN_BINDINGS" = false ] && [ "$CLEAN_BUILD" = false ]; then
    print_error "No cleanup option specified. Use --help for usage information."
    exit 1
fi

# If --all is specified, enable everything
if [ "$CLEAN_ALL" = true ]; then
    CLEAN_BINDINGS=true
    CLEAN_BUILD=true
fi

# ==============================================================================
# Main Cleanup Process
# ==============================================================================

echo "🧹 GraphLite Cleanup Script"
echo "================================="
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# ==============================================================================
# Step 1: Cleanup Python Bindings
# ==============================================================================
if [ "$CLEAN_BINDINGS" = true ]; then
    print_info "Cleaning up Python bindings..."

    # Uninstall Python package if installed
    if command_exists pip; then
        if pip list 2>/dev/null | grep -q graphlite; then
            print_info "Uninstalling graphlite Python package..."
            pip uninstall -y graphlite || print_warning "Failed to uninstall Python package"
            print_success "Python package uninstalled"
        else
            print_info "Python package not installed, skipping"
        fi
    else
        print_warning "pip not found, skipping Python package uninstall"
    fi

    # Clean Python build artifacts
    if [ -d "bindings/python" ]; then
        print_info "Cleaning Python build artifacts..."
        cd bindings/python
        rm -rf build/ dist/ *.egg-info __pycache__ .pytest_cache
        find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
        find . -type f -name "*.pyc" -delete 2>/dev/null || true
        find . -type f -name "*.pyo" -delete 2>/dev/null || true
        cd "$PROJECT_ROOT"
        print_success "Python build artifacts cleaned"
    fi

    # Clean Java build artifacts
    if [ -d "bindings/java" ]; then
        print_info "Cleaning Java build artifacts..."
        cd bindings/java

        # Clean Maven artifacts
        if [ -f "pom.xml" ]; then
            if command_exists mvn; then
                mvn clean || print_warning "Maven clean failed"
            else
                rm -rf target/
            fi
        fi

        # Remove JAR files
        find . -type f -name "*.jar" -delete 2>/dev/null || true
        find . -type f -name "*.class" -delete 2>/dev/null || true

        cd "$PROJECT_ROOT"
        print_success "Java build artifacts cleaned"
    fi

    echo ""
fi

# ==============================================================================
# Step 2: Cleanup Rust Build Artifacts
# ==============================================================================
if [ "$CLEAN_BUILD" = true ]; then
    print_info "Cleaning Rust build artifacts..."

    # Ensure Rust/Cargo is in PATH
    if ! command -v cargo &> /dev/null; then
        # Try to add cargo to PATH from default installation location
        if [ -f "$HOME/.cargo/env" ]; then
            source "$HOME/.cargo/env"
        elif [ -d "$HOME/.cargo/bin" ]; then
            export PATH="$HOME/.cargo/bin:$PATH"
        fi
    fi

    # Run cargo clean if available
    if command_exists cargo; then
        print_info "Running cargo clean..."
        cargo clean
        print_success "Cargo clean complete"
    else
        print_warning "Cargo not found, manually removing target directory"
        rm -rf target/
    fi

    # Remove additional build artifacts
    print_info "Removing additional build artifacts..."
    rm -f Cargo.lock

    # Remove compiled binaries and libraries
    find . -type f \( -name "*.so" -o -name "*.dylib" -o -name "*.dll" -o -name "*.rlib" \) -delete 2>/dev/null || true

    print_success "Build artifacts cleaned"
    echo ""
fi

# ==============================================================================
# Step 3: Cleanup Data and Configuration (only with --all)
# ==============================================================================
if [ "$CLEAN_ALL" = true ]; then
    print_warning "Performing complete cleanup including data and configuration..."

    # Remove database files
    if [ -d "data" ]; then
        print_info "Removing database data directory..."
        rm -rf data/
        print_success "Database data removed"
    fi

    # Remove example database files
    if [ -d "example_db" ]; then
        print_info "Removing example database directory..."
        rm -rf example_db/
        print_success "Example database removed"
    fi

    # Remove mydb directory
    if [ -d "mydb" ]; then
        print_info "Removing mydb directory..."
        rm -rf mydb/
        print_success "mydb directory removed"
    fi

    # Remove .graphlite configuration directory
    if [ -d ".graphlite" ]; then
        print_info "Removing .graphlite configuration directory..."
        rm -rf .graphlite/
        print_success ".graphlite configuration removed"
    fi

    # Remove any *.db files in project root
    print_info "Removing database files from project root..."
    find . -maxdepth 1 -type f -name "*.db" -delete 2>/dev/null || true

    # Remove log files
    print_info "Removing log files..."
    find . -type f -name "*.log" -delete 2>/dev/null || true

    # Remove temporary files
    print_info "Removing temporary files..."
    find . -type f -name "*.tmp" -delete 2>/dev/null || true
    find . -type f -name "*~" -delete 2>/dev/null || true

    print_success "Data and configuration cleaned"
    echo ""
fi

# ==============================================================================
# Summary
# ==============================================================================
echo "================================="
echo "📊 Cleanup Summary"
echo "================================="

if [ "$CLEAN_BINDINGS" = true ]; then
    print_success "Python bindings: Cleaned"
    print_success "Java bindings: Cleaned"
fi

if [ "$CLEAN_BUILD" = true ]; then
    print_success "Rust build artifacts: Cleaned"
    print_success "Compiled binaries: Removed"
fi

if [ "$CLEAN_ALL" = true ]; then
    print_success "Database files: Removed"
    print_success "Configuration: Removed"
    print_success "Log files: Removed"
    print_success "Temporary files: Removed"
fi

echo ""
print_success "Cleanup complete!"

# Show what remains
echo ""
echo "📝 Remaining files:"
if [ "$CLEAN_ALL" = true ]; then
    echo "  - Source code files"
    echo "  - Documentation files"
    echo "  - Scripts"
    echo ""
    echo "To rebuild the project, run: ./scripts/build_all.sh"
else
    echo "  - Source code files"
    echo "  - Documentation files"
    if [ "$CLEAN_BUILD" = false ]; then
        echo "  - Build artifacts (use --build to clean)"
    fi
    if [ "$CLEAN_BINDINGS" = false ]; then
        echo "  - Language bindings (use --bindings to clean)"
    fi
    if [ "$CLEAN_ALL" = false ]; then
        echo "  - Database and config files (use --all to clean)"
    fi
fi

echo ""
exit 0
