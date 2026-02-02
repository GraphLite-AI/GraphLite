#!/bin/bash

# Install Git Hooks for GraphLite
# This script sets up pre-commit hooks that enforce rules

set -e  # Exit on error

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

echo "🔧 Installing GraphLite Git Hooks..."
echo ""

# Check if .git directory exists
if [ ! -d "$PROJECT_ROOT/.git" ]; then
    echo "❌ Error: Not a git repository"
    echo "   Run 'git init' first"
    exit 1
fi

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

    echo "   Continue anyway? (y/n)"
    read -r response
    if [[ ! "$response" =~ ^[Yy]$ ]]; then
        echo "❌ Installation cancelled"
        exit 1
    fi

# Backup existing hook if present
if [ -f "$HOOKS_DIR/pre-commit" ]; then
    backup_file="$HOOKS_DIR/pre-commit.backup.$(date +%s)"
    echo "📦 Backing up existing pre-commit hook to: $(basename $backup_file)"
    cp "$HOOKS_DIR/pre-commit" "$backup_file"
fi

# Install pre-commit hook
echo "📝 Creating pre-commit hook..."

cat > "$HOOKS_DIR/pre-commit" << 'HOOK_EOF'
#!/bin/bash

# Rule Enforcement Pre-commit Hook for GraphLite
# This hook validates code changes against the defined rules
# It prevents commits that violate critical patterns and anti-patterns

echo "🔍 Validating rule compliance..."

# Get list of staged Rust files (excluding documentation, test files, and hook files)
staged_rust_files=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(rs)$' | grep -v "hooks/" | grep -v "docs/" | grep -v "pre-commit" || true)

# Function to check for violations in staged content (only additions, not deletions)
check_staged_content() {
    local pattern="$1"
    local files="$2"

    if [ -n "$files" ]; then
        # Only check added lines (starting with +), not deleted lines (starting with -)
        echo "$files" | xargs git diff --cached | grep -E "^\+.*$pattern" > /dev/null 2>&1
    else
        return 1
    fi
}

# Function to check violations in specific files (only additions, not deletions)
check_staged_files() {
    local pattern="$1"
    local files="$2"

    if [ -n "$files" ]; then
        # Only check added lines (starting with +), not deleted lines (starting with -)
        echo "$files" | xargs git diff --cached | grep -E "^\+.*$pattern" >/dev/null 2>&1
    else
        return 1
    fi
}

violations=0

if [ -n "$staged_rust_files" ]; then
    echo "📋 Checking staged Rust files: $(echo $staged_rust_files | wc -w) files"
else
    echo "📋 No Rust files staged"
    echo "✅ All rules passed! Commit allowed."
    exit 0
fi

# Rust file checks (Rules #1-7, #9-10)

# Rule #1: ExecutionContext Management
echo "  🔍 Rule #1: ExecutionContext singleton pattern..."
if check_staged_content "ExecutionContext::new\(\)" "$staged_rust_files"; then
    echo "❌ RULE #1 VIOLATION: Found ExecutionContext::new()"
    echo "   💡 Use existing ExecutionContext instead of creating new instances"
    echo "   📖 See Rule #1: ExecutionContext Management"
    violations=$((violations + 1))
fi

# Rule #2: StorageManager Singleton Pattern
echo "  🔍 Rule #2: StorageManager singleton pattern..."
if check_staged_content "StorageManager::new\(\)" "$staged_rust_files"; then
    echo "❌ RULE #2 VIOLATION: Found StorageManager::new()"
    echo "   💡 Use existing StorageManager from session context"
    echo "   📖 See Rule #2: StorageManager Singleton Pattern"
    violations=$((violations + 1))
fi

# Rule #3: Read vs Write Lock Usage Pattern
echo "  🔍 Rule #3: Read vs Write lock usage..."
if check_staged_files "(catalog_manager|manager)\.write\(\).*\.(list_|get_|describe_|query_|authenticate_)" "$staged_rust_files"; then
    echo "❌ RULE #3 VIOLATION: Using write lock for read operation"
    echo "   💡 Use .read() for queries, .write() only for modifications"
    echo "   📖 See Rule #3: Read vs Write Lock Usage Pattern"
    violations=$((violations + 1))
fi

# Rule #4: CatalogManager Singleton Pattern
echo "  🔍 Rule #4: CatalogManager singleton pattern..."
# Exclude infrastructure files (coordinator, session providers)
non_infrastructure_files=$(echo "$staged_rust_files" | grep -v "coordinator" | grep -v "session_provider" | grep -v "session/instance_provider" | grep -v "session/global_provider" || true)
if [ -n "$non_infrastructure_files" ]; then
    if check_staged_content "Arc::new(RwLock::new(CatalogManager::new" "$non_infrastructure_files"; then
        echo "❌ RULE #4 VIOLATION: Creating new CatalogManager instance"
        echo "   💡 Use existing CatalogManager from QueryCoordinator/SessionProvider"
        echo "   📖 See Rule #4: CatalogManager Singleton Pattern"
        violations=$((violations + 1))
    fi
fi

# Rule #5: Async Runtime Management
echo "  🔍 Rule #5: Async runtime management..."
if check_staged_content "tokio::runtime::Runtime::new\(\)" "$staged_rust_files"; then
    echo "❌ RULE #5 VIOLATION: Creating new Tokio runtime in operation code"
    echo "   💡 Use existing runtime or spawn tasks instead"
    echo "   📖 See Rule #5: Async Runtime Management"
    violations=$((violations + 1))
fi

# Rule #6: Helper Method Implementation Pattern
echo "  🔍 Rule #6: Helper method recursion..."
if check_staged_files "fn get_[a-zA-Z_]+.*\{[^}]*self\.get_[a-zA-Z_]+" "$staged_rust_files"; then
    echo "❌ RULE #6 VIOLATION: Potential recursive helper method detected"
    echo "   💡 Ensure helper methods access fields directly, not recursively"
    echo "   💡 If this is a false positive, use --no-verify to bypass"
    echo "   📖 See Rule #6: Helper Method Implementation Pattern"
    violations=$((violations + 1))
fi

# Rule #7: Async Runtime Context Detection Pattern
echo "  🔍 Rule #7: Async runtime context detection..."
if check_staged_content "\.block_on\(" "$staged_rust_files"; then
    # Check if block_on is used without try_current() check
    if ! check_staged_content "tokio::runtime::Handle::try_current\(\)" "$staged_rust_files"; then
        echo "❌ RULE #7 VIOLATION: Found block_on() without async context detection"
        echo "   💡 Use tokio::runtime::Handle::try_current() to detect async context first"
        echo "   💡 Consider using block_in_place() or skipping operation in async context"
        echo "   💡 If this is main() or initialization code, use --no-verify to bypass"
        echo "   📖 See Rule #7: Async Runtime Context Detection Pattern"
        violations=$((violations + 1))
    fi
fi

# Rule #9: Test Case Integrity Pattern
echo "  🔍 Rule #9: Test case integrity..."
test_files=$(echo "$staged_rust_files" | grep -E "(test|spec)" || true)
if [ -n "$test_files" ]; then
    # Check for suspicious assertion changes
    if check_staged_files "assert_eq.*\-.*[0-9]+.*\+.*[0-9]+" "$test_files"; then
        echo "❌ RULE #9 VIOLATION: Modified test assertions detected"
        echo "   💡 Ensure you're fixing test syntax, not hiding functional bugs"
        echo "   💡 Fix GraphLite functionality if tests reveal real issues"
        echo "   📖 See Rule #9: Test Case Integrity Pattern"
        violations=$((violations + 1))
    fi

    # Check for commented test functions (often done to hide failures)
    if check_staged_content "//.*#\[test\]\|/\*.*#\[test\]" "$test_files"; then
        echo "⚠️  RULE #9 WARNING: Commented test functions detected"
        echo "   💡 If hiding test failures, fix underlying GraphLite issues instead"
        echo "   💡 If feature is unimplemented, use #[ignore] with reason"
        echo "   📖 See Rule #9: Test Case Integrity Pattern"
        # Note: This is a warning, not a blocking violation
    fi
fi

# Rule #10: Session Provider Test Pattern
echo "  🔍 Rule #10: Session provider test pattern..."
test_files=$(echo "$staged_rust_files" | grep -E "(test|spec)" | grep -v -E "\.md$" || true)
if [ -n "$test_files" ]; then
    # Exclude coordinator and session provider infrastructure files
    non_infrastructure_tests=$(echo "$test_files" | grep -v "coordinator" | grep -v "session_provider" || true)

    if [ -n "$non_infrastructure_tests" ]; then
        # Check for direct SessionManager::new() or SessionManager::instance() in test functions
        if check_staged_content "SessionManager::new\|SessionManager::instance" "$non_infrastructure_tests"; then
            echo "❌ RULE #10a VIOLATION: Direct SessionManager creation in tests"
            echo "   💡 Use QueryCoordinator instead of creating SessionManager directly"
            echo "   💡 Example: let coord = QueryCoordinator::from_path(path)?;"
            echo "   💡 The coordinator manages session providers (Instance or Global mode)"
            echo "   📚 See Rule #10: Session Provider Test Pattern"
            violations=$((violations + 1))
        fi

        # Check for SessionManager fields in test structs
        if check_staged_content "session_manager:.*SessionManager" "$test_files"; then
            echo "⚠️  RULE #10b WARNING: SessionManager field in test struct"
            echo "   💡 Store QueryCoordinator or session_id instead"
            echo "   💡 Avoid coupling tests to internal SessionManager implementation"
            echo "   📚 See Rule #10: Session Provider Test Pattern"
            # Note: This is a warning, not blocking
        fi
    fi
fi

# Summary
echo ""
if [ $violations -eq 0 ]; then
    echo "✅ All rules passed! Commit allowed."
    echo ""
else
    echo "❌ Found $violations rule violation(s). Commit blocked."
    echo ""
    echo "🔧 To fix:"
    echo "   1. Review the violations above"
    echo "   3. Fix the issues and try committing again"
    echo ""
    echo "🆘 Need help? Check:"
    echo ""
    echo "💡 To bypass (use sparingly): git commit --no-verify"
    echo ""
    exit 1
fi
HOOK_EOF

# Make the hook executable
chmod +x "$HOOKS_DIR/pre-commit"

echo "✅ Pre-commit hook installed successfully!"
echo ""
echo "📍 Location: $HOOKS_DIR/pre-commit"
echo ""
echo "🔍 Rules enforced:"
echo "   • Rule #1: ExecutionContext Management"
echo "   • Rule #2: StorageManager Singleton Pattern"
echo "   • Rule #3: Read vs Write Lock Usage"
echo "   • Rule #4: CatalogManager Singleton Pattern"
echo "   • Rule #5: Async Runtime Management"
echo "   • Rule #6: Helper Method Recursion"
echo "   • Rule #7: Async Runtime Context Detection"
echo "   • Rule #9: Test Case Integrity"
echo "   • Rule #10: Session Provider Test Pattern"
echo ""
echo ""
echo "💡 To bypass hook (use sparingly): git commit --no-verify"
echo ""
echo "✨ You're all set! The hooks will run automatically on every commit."
