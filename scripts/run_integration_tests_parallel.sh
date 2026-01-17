#!/bin/bash

# GraphLite Parallel Test Runner (using GNU Parallel)
# Runs test FILES in parallel (each file still uses --test-threads=1 internally)
# Requires: GNU parallel (brew install parallel)

set -euo pipefail

# Configuration
PARALLEL_JOBS=4  # Number of test files to run simultaneously
BUILD_MODE="debug"
RUN_ANALYZE=false
SHOW_PROGRESS=true

# Parse arguments
for arg in "$@"; do
    case $arg in
        --release)
            BUILD_MODE="release"
            ;;
        --debug)
            BUILD_MODE="debug"
            ;;
        --jobs=*)
            PARALLEL_JOBS="${arg#*=}"
            ;;
        -j*)
            PARALLEL_JOBS="${arg#-j}"
            ;;
        --analyze)
            RUN_ANALYZE=true
            ;;
        --no-progress)
            SHOW_PROGRESS=false
            ;;
        --help|-h)
            cat << 'HELP'
GraphLite Parallel Test Runner (GNU Parallel)

Usage: ./scripts/run_tests_parallel.sh [OPTIONS]

Options:
  --debug          Run tests in debug mode (default)
  --release        Run tests in release mode (faster)
  --jobs=N, -jN    Number of parallel test files (default: 4)
  --analyze        Show detailed failure output
  --no-progress    Disable progress bar
  --help           Show this help message

Examples:
  ./scripts/run_tests_parallel.sh                    # 4 parallel jobs (debug)
  ./scripts/run_tests_parallel.sh --jobs=8           # 8 parallel jobs
  ./scripts/run_tests_parallel.sh -j8 --release      # Release, 8 parallel
  ./scripts/run_tests_parallel.sh --analyze          # With failure details

Performance:
  - Sequential (--test-threads=1): ~15-20 minutes
  - Parallel -j4: ~5-7 minutes
  - Parallel -j8: ~3-4 minutes (with 8+ cores)

Note: Each test file runs with --test-threads=1 internally for GraphLite.
HELP
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Check if GNU parallel is installed
if ! command -v parallel &> /dev/null; then
    echo "❌ ERROR: GNU parallel is not installed"
    echo ""
    echo "Install with:"
    echo "  brew install parallel    # macOS"
    echo "  apt install parallel     # Ubuntu/Debian"
    echo ""
    exit 1
fi

# Set cargo flags
CARGO_FLAGS=""
if [ "$BUILD_MODE" = "release" ]; then
    CARGO_FLAGS="--release"
    echo "=== GraphLite Parallel Test Runner (RELEASE BUILD) ==="
else
    echo "=== GraphLite Parallel Test Runner (DEBUG BUILD) ==="
fi

echo "Parallel jobs: $PARALLEL_JOBS test files simultaneously"
echo "Date: $(date)"
echo ""

# Test list
integration_tests=(
    "aggregation_tests"
    "cache_tests"
    "call_where_clause_test"
    "cli_fixture_tests"
    "ddl_independent_tests"
    "ddl_shared_tests"
    "debug_fraud_fixture"
    "delimited_identifiers_tests"
    "dml_tests"
    "dql_tests"
    "duplicate_edge_warning_test"
    "duplicate_insert_test"
    "fixture_tests"
    "function_expression_insert_test"
    "function_tests"
    "identity_based_set_ops_test"
    "insert_node_identifier_regression_test"
    "intersect_debug_test"
    "list_graphs_bug_test"
    "list_graphs_bug_test_simple"
    "match_set_transactional_test"
    "match_with_tests"
    "pattern_tests"
    "readme_examples_test"
    "role_management_tests"
    "rollback_batch_test"
    "rollback_simple_test"
    "security_role_user_tests"
    "set_function_expression_test"
    "set_operations_tests"
    "simple_insert_test"
    "simple_let_test"
    "simple_role_test"
    "simple_union_test"
    "storage_verification_test"
    "stored_procedure_no_prefix_test"
    "transactional_set_test"
    "unknown_procedure_test"
    "utility_functions_test"
    "with_clause_property_access_bug"
)

# Create temp directory for results
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

# Export variables for parallel
export CARGO_FLAGS
export TEMP_DIR

# Function to run a single test
run_single_test() {
    local test=$1
    local output_file="$TEMP_DIR/${test}.out"
    local status_file="$TEMP_DIR/${test}.status"
    local timing_file="$TEMP_DIR/${test}.time"

    # Start timing
    local start=$(date +%s)

    # Run test
    if cargo test $CARGO_FLAGS --test "$test" -- --test-threads=1 &> "$output_file"; then
        echo "PASSED" > "$status_file"
    else
        echo "FAILED" > "$status_file"
    fi

    # Record timing
    local end=$(date +%s)
    echo $((end - start)) > "$timing_file"

    # Return test name for progress tracking
    echo "$test"
}

export -f run_single_test

# Start timer
start_time=$(date +%s)

echo "Building and running ${#integration_tests[@]} test files..."
echo ""

# Run tests in parallel with GNU parallel
if [ "$SHOW_PROGRESS" = true ]; then
    # With progress bar
    printf '%s\n' "${integration_tests[@]}" | \
        parallel --bar --jobs "$PARALLEL_JOBS" --line-buffer \
        run_single_test {}
else
    # Without progress bar (faster)
    printf '%s\n' "${integration_tests[@]}" | \
        parallel --jobs "$PARALLEL_JOBS" --line-buffer \
        run_single_test {} > /dev/null
fi

# Calculate total duration
end_time=$(date +%s)
total_duration=$((end_time - start_time))
minutes=$((total_duration / 60))
seconds=$((total_duration % 60))

echo ""
echo "=== RESULTS ==="
echo ""
echo "Test File | Status | Time | Details"
echo "----------|--------|------|--------"

# Process results
passed_count=0
failed_count=0
failed_tests=()
total_test_count=0
total_test_time=0

for test in "${integration_tests[@]}"; do
    output_file="$TEMP_DIR/${test}.out"
    status_file="$TEMP_DIR/${test}.status"
    timing_file="$TEMP_DIR/${test}.time"

    if [ -f "$status_file" ]; then
        status=$(cat "$status_file")
        test_time=$(cat "$timing_file" 2>/dev/null || echo "0")
        total_test_time=$((total_test_time + test_time))

        if [ "$status" = "PASSED" ]; then
            # Extract test count
            passed=$(grep "test result: ok" "$output_file" | sed -E 's/.*([0-9]+) passed.*/\1/' 2>/dev/null || echo "?")
            ignored=$(grep "test result: ok" "$output_file" | sed -E 's/.*([0-9]+) ignored.*/\1/' 2>/dev/null || echo "0")

            if [ "$passed" != "?" ]; then
                total_test_count=$((total_test_count + passed))
            fi

            if [ "$ignored" = "0" ] || [ -z "$ignored" ]; then
                printf "%-30s | ✅ PASSED | %3ss | %s tests\n" "$test" "$test_time" "$passed"
            else
                printf "%-30s | ✅ PASSED | %3ss | %s tests, %s ignored\n" "$test" "$test_time" "$passed" "$ignored"
            fi
            ((passed_count++))
        else
            # Check if it's a failure or error
            if grep -q "test result: FAILED" "$output_file"; then
                passed=$(grep "test result: FAILED" "$output_file" | sed -E 's/.*([0-9]+) passed.*/\1/' 2>/dev/null || echo "0")
                failed=$(grep "test result: FAILED" "$output_file" | sed -E 's/.*([0-9]+) failed.*/\1/' 2>/dev/null || echo "?")

                if [ "$passed" != "0" ]; then
                    total_test_count=$((total_test_count + passed))
                fi

                printf "%-30s | ❌ FAILED | %3ss | %s failed, %s passed\n" "$test" "$test_time" "$failed" "$passed"
            else
                printf "%-30s | ⚠️  ERROR  | %3ss | Could not run\n" "$test" "$test_time"
            fi
            failed_tests+=("$test")
            ((failed_count++))
        fi
    else
        printf "%-30s | ⚠️  ERROR  |   ?s | No result\n" "$test"
        failed_tests+=("$test")
        ((failed_count++))
    fi
done

# Summary
echo ""
echo "=== SUMMARY ==="
echo "Build mode:         $BUILD_MODE"
echo "Parallel jobs:      $PARALLEL_JOBS"
echo "Total test files:   ${#integration_tests[@]}"
echo "Total tests run:    $total_test_count"
echo "✅ Passed files:     $passed_count"
echo "❌ Failed files:     $failed_count"
echo ""
echo "⏱️  Wall time:        ${minutes}m ${seconds}s"

# Calculate efficiency
if [ $total_test_time -gt 0 ]; then
    efficiency=$((total_duration * 100 / total_test_time))
    echo "⚡ Parallel efficiency: ${efficiency}%"
    echo "   (Lower is better - 100% = no parallelism, 25% = 4x speedup)"
fi

# Success rate
if [ ${#integration_tests[@]} -gt 0 ]; then
    success_rate=$(( (passed_count * 100) / ${#integration_tests[@]} ))
    echo ""
    echo "Success rate: $success_rate%"
fi

# Show failed tests
if [ ${#failed_tests[@]} -gt 0 ]; then
    echo ""
    echo "=== FAILED TESTS ($failed_count) ==="
    for failed_test in "${failed_tests[@]}"; do
        echo "  • $failed_test"
    done

    if [ "$RUN_ANALYZE" = true ]; then
        echo ""
        echo "=== FAILURE DETAILS ==="
        for failed_test in "${failed_tests[@]}"; do
            echo ""
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "📋 $failed_test"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

            # Show last 30 lines of output
            tail -30 "$TEMP_DIR/${failed_test}.out"
            echo ""
        done
    else
        echo ""
        echo "💡 For detailed failure output, run with --analyze:"
        echo "   ./scripts/run_tests_parallel.sh --analyze"
    fi

    echo ""
    echo "To run a specific failed test:"
    if [ "$BUILD_MODE" = "release" ]; then
        echo "  cargo test --release --test <test_name> -- --test-threads=1"
    else
        echo "  cargo test --test <test_name> -- --test-threads=1"
    fi

    echo ""
    exit 1
else
    echo ""
    echo "🎉 All integration tests passed!"
    echo ""
    exit 0
fi
