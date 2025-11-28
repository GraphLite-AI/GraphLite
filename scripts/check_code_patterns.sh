#!/bin/bash

# Check Rule Compliance Across Entire Codebase
# This script checks all Rust files for violations, not just staged files

echo "🔍 Checking rule compliance across entire codebase..."
echo ""

# Get all Rust files (excluding target, docs, and hooks)
# Support both workspace structure (graphlite/src, graphlite-cli/src) and simple structure (src, tests)
all_rust_files=$(find . -name "*.rs" 2>/dev/null | grep -E "(src/|tests/)" | grep -v "target/" | grep -v "docs/" | grep -v "hooks/" || true)

if [ -z "$all_rust_files" ]; then
    echo "❌ No Rust files found in src/ or tests/ directories"
    exit 1
fi

file_count=$(echo "$all_rust_files" | wc -l | tr -d ' ')
echo "📋 Checking $file_count Rust files..."
echo ""

violations=0

# Rule #1: ExecutionContext Management
echo "🔍 Rule #1: ExecutionContext singleton pattern..."
rule1_violations=$(grep -rn "ExecutionContext::new()" $all_rust_files 2>/dev/null || true)
if [ -n "$rule1_violations" ]; then
    echo "❌ RULE #1 VIOLATIONS: Found ExecutionContext::new()"
    echo "$rule1_violations" | head -10
    violation_count=$(echo "$rule1_violations" | wc -l | tr -d ' ')
    echo "   Found $violation_count occurrence(s)"
    echo "   💡 Use existing ExecutionContext instead of creating new instances"
    echo "   📖 See Rule #1"
    echo ""
    violations=$((violations + 1))
fi

# Rule #2: StorageManager Singleton Pattern
echo "🔍 Rule #2: StorageManager singleton pattern..."
rule2_violations=$(grep -rn "StorageManager::new()" $all_rust_files 2>/dev/null || true)
if [ -n "$rule2_violations" ]; then
    echo "❌ RULE #2 VIOLATIONS: Found StorageManager::new()"
    echo "$rule2_violations" | head -10
    violation_count=$(echo "$rule2_violations" | wc -l | tr -d ' ')
    echo "   Found $violation_count occurrence(s)"
    echo "   💡 Use existing StorageManager from session context"
    echo "   📖 See Rule #2"
    echo ""
    violations=$((violations + 1))
fi

# Rule #3: Read vs Write Lock Usage Pattern
echo "🔍 Rule #3: Read vs Write lock usage..."
rule3_violations=$(grep -rn "\.write().*\.\(list_\|get_\|describe_\|query_\|authenticate_\)" $all_rust_files 2>/dev/null || true)
if [ -n "$rule3_violations" ]; then
    echo "❌ RULE #3 VIOLATIONS: Using write lock for read operations"
    echo "$rule3_violations" | head -10
    violation_count=$(echo "$rule3_violations" | wc -l | tr -d ' ')
    echo "   Found $violation_count occurrence(s)"
    echo "   💡 Use .read() for queries, .write() only for modifications"
    echo "   📖 See Rule #3"
    echo ""
    violations=$((violations + 1))
fi

# Rule #4: CatalogManager Singleton Pattern
echo "🔍 Rule #4: CatalogManager singleton pattern..."
rule4_violations=$(grep -rn "Arc::new(RwLock::new(CatalogManager::new" $all_rust_files 2>/dev/null || true)
if [ -n "$rule4_violations" ]; then
    echo "❌ RULE #4 VIOLATIONS: Creating new CatalogManager instances"
    echo "$rule4_violations" | head -10
    violation_count=$(echo "$rule4_violations" | wc -l | tr -d ' ')
    echo "   Found $violation_count occurrence(s)"
    echo "   💡 Use existing CatalogManager from SessionManager"
    echo "   📖 See Rule #4"
    echo ""
    violations=$((violations + 1))
fi

# Rule #5: Async Runtime Management
echo "🔍 Rule #5: Async runtime management..."
rule5_violations=$(grep -rn "tokio::runtime::Runtime::new()" $all_rust_files 2>/dev/null || true)
if [ -n "$rule5_violations" ]; then
    echo "❌ RULE #5 VIOLATIONS: Creating new Tokio runtime in operation code"
    echo "$rule5_violations" | head -10
    violation_count=$(echo "$rule5_violations" | wc -l | tr -d ' ')
    echo "   Found $violation_count occurrence(s)"
    echo "   💡 Use existing runtime or spawn tasks instead"
    echo "   📖 See Rule #5"
    echo ""
    violations=$((violations + 1))
fi

# Rule #6: Helper Method Implementation Pattern (simplified check)
echo "🔍 Rule #6: Helper method recursion..."
# This is a complex pattern - just flag potential issues
rule6_potential=$(grep -rn "fn get_.*self\.get_" $all_rust_files 2>/dev/null | grep -v "get_session\|// " || true)
if [ -n "$rule6_potential" ]; then
    echo "⚠️  RULE #6 POTENTIAL ISSUES: Possible recursive helper methods"
    echo "$rule6_potential" | head -5
    echo "   💡 Manual review needed - helper methods should access fields directly"
    echo "   📖 See Rule #6"
    echo ""
fi

# Rule #7: Async Runtime Context Detection Pattern
echo "🔍 Rule #7: Async runtime context detection..."
# Check for block_on without try_current
block_on_files=$(grep -l "\.block_on(" $all_rust_files 2>/dev/null || true)
if [ -n "$block_on_files" ]; then
    for file in $block_on_files; do
        # Check if this file has block_on but not try_current
        if ! grep -q "tokio::runtime::Handle::try_current()" "$file" 2>/dev/null; then
            # Exclude main.rs and build scripts
            if [[ ! "$file" =~ main\.rs$ ]] && [[ ! "$file" =~ build\.rs$ ]]; then
                echo "⚠️  RULE #7 WARNING: $file"
                echo "   Uses block_on() without try_current() check"
            fi
        fi
    done
    echo "   💡 Use tokio::runtime::Handle::try_current() before block_on()"
    echo "   📖 See Rule #7"
    echo ""
fi

# Rule #9: Test Case Integrity Pattern
echo "🔍 Rule #9: Test case integrity..."
test_files=$(find . -path "*/tests/*.rs" 2>/dev/null | grep -v "target/" || true)
if [ -n "$test_files" ]; then
    # Check for commented test functions
    commented_tests=$(grep -rn "//.*#\[test\]" $test_files 2>/dev/null || true)
    if [ -n "$commented_tests" ]; then
        echo "⚠️  RULE #9 WARNING: Commented test functions found"
        echo "$commented_tests" | head -5
        echo "   💡 Use #[ignore] with reason instead of commenting"
        echo "   📖 See Rule #9"
        echo ""
    fi
fi

# Rule #10: Session Manager Test Isolation Pattern
echo "🔍 Rule #10: Session Manager test isolation..."
if [ -n "$test_files" ]; then
    # Check for SessionManager::new in tests
    rule10_violations=$(grep -rn "SessionManager::new" $test_files 2>/dev/null | grep -v "get_session_manager" || true)
    if [ -n "$rule10_violations" ]; then
        echo "⚠️  RULE #10 POTENTIAL VIOLATIONS: SessionManager in tests"
        echo "$rule10_violations" | head -10
        violation_count=$(echo "$rule10_violations" | wc -l | tr -d ' ')
        echo "   Found $violation_count occurrence(s)"
        echo "   💡 Use get_session_manager() instead of creating new instances"
        echo "   📖 See Rule #10"
        echo ""
    fi
fi

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
if [ $violations -eq 0 ]; then
    echo "✅ No critical rule violations found!"
    echo ""
    echo "⚠️  Some warnings may have been raised - review them above"
else
    echo "❌ Found $violations critical rule violation(s)"
    echo ""
    echo "🔧 To fix:"
    echo "   1. Review the violations listed above"
    echo "   3. Fix the issues before committing"
fi
echo ""
echo ""
