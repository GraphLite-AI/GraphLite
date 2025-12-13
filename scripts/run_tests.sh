#!/bin/bash

# GraphLite Test Runner with Release/Debug Build Support
# Usage:
#   ./scripts/run_tests.sh              # Run debug build tests (default)
#   ./scripts/run_tests.sh --release    # Run release build tests (faster)
#   ./scripts/run_tests.sh --both       # Run both debug and release
#   ./scripts/run_tests.sh --analyze    # Include failure analysis
#   ./scripts/run_tests.sh --release --analyze  # Combine flags

# Parse command line arguments
BUILD_MODE="debug"
RUN_ANALYZE=false
RUN_BOTH=false

for arg in "$@"; do
    case $arg in
        --release)
            BUILD_MODE="release"
            ;;
        --debug)
            BUILD_MODE="debug"
            ;;
        --both)
            RUN_BOTH=true
            ;;
        --analyze)
            RUN_ANALYZE=true
            ;;
        --help|-h)
            echo "GraphLite Test Runner"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --debug      Run tests in debug mode (default)"
            echo "  --release    Run tests in release mode (faster, optimized)"
            echo "  --both       Run tests in both debug and release modes"
            echo "  --analyze    Include detailed failure analysis"
            echo "  --help       Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                        # Run debug tests"
            echo "  $0 --release              # Run optimized release tests"
            echo "  $0 --both                 # Run both debug and release"
            echo "  $0 --release --analyze    # Release tests with analysis"
            echo ""
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Function to run tests with specific build mode
run_test_suite() {
    local mode=$1
    local cargo_flags=""

    if [ "$mode" = "release" ]; then
        cargo_flags="--release"
        echo "=== GraphLite ISO GQL INTEGRATION TEST RUNNER (RELEASE BUILD) ==="
        echo "Build: Optimized release (faster execution)"
    else
        echo "=== GraphLite ISO GQL INTEGRATION TEST RUNNER (DEBUG BUILD) ==="
        echo "Build: Debug (with debug assertions)"
    fi

    echo "Date: $(date)"
    echo "Single-threaded: --test-threads=1 (required for GraphLite)"
    echo ""

    # ISO GQL compliant integration tests
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
        "fulltext_search_integration"
        "fulltext_search_executor_integration"
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

    # Initialize counters
    local passed_count=0
    local failed_count=0
    local error_count=0
    local failed_tests=()
    local total_test_count=0
    local start_time=$(date +%s)

    echo "Test File | Status | Details"
    echo "----------|--------|--------"

    # Run each test
    for test in "${integration_tests[@]}"; do
        output=$(cargo test $cargo_flags --test "$test" -- --test-threads=1 2>&1)

        if echo "$output" | grep -q "test result: ok"; then
            # Test passed
            passed=$(echo "$output" | grep "test result: ok" | sed -E 's/.*([0-9]+) passed.*/\1/')
            ignored=$(echo "$output" | grep "test result: ok" | sed -E 's/.*([0-9]+) ignored.*/\1/' 2>/dev/null || echo "0")
            total_test_count=$((total_test_count + passed))

            if [ "$ignored" = "0" ] || [ -z "$ignored" ]; then
                echo "$test | ✅ PASSED | $passed tests"
            else
                echo "$test | ✅ PASSED | $passed tests, $ignored ignored"
            fi
            ((passed_count++))
        elif echo "$output" | grep -q "test result: FAILED"; then
            # Test failed
            passed=$(echo "$output" | grep "test result: FAILED" | sed -E 's/.*([0-9]+) passed.*/\1/' 2>/dev/null || echo "0")
            failed=$(echo "$output" | grep "test result: FAILED" | sed -E 's/.*([0-9]+) failed.*/\1/')
            total_test_count=$((total_test_count + passed))
            echo "$test | ❌ FAILED | $failed failed, $passed passed"
            failed_tests+=("$test")
            ((failed_count++))
        else
            # Test had errors (compilation issues, etc.)
            echo "$test | ⚠️  ERROR | Could not run"
            failed_tests+=("$test")
            ((error_count++))
        fi
    done

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    local minutes=$((duration / 60))
    local seconds=$((duration % 60))

    echo ""
    echo "=== SUMMARY ($mode build) ==="
    echo "Total test files: ${#integration_tests[@]}"
    echo "Total individual tests: $total_test_count"
    echo "✅ Passed: $passed_count test files"
    echo "❌ Failed: $failed_count test files"
    echo "⚠️  Errors: $error_count test files"
    echo "⏱️  Duration: ${minutes}m ${seconds}s"
    echo ""

    # Calculate and display success rate
    if [ ${#integration_tests[@]} -gt 0 ]; then
        success_rate=$(( (passed_count * 100) / ${#integration_tests[@]} ))
        echo "Success rate: $success_rate%"
    else
        echo "Success rate: 0%"
    fi

    # Performance info for release builds
    if [ "$mode" = "release" ]; then
        echo ""
        echo "ℹ️  Release build benefits:"
        echo "   • Faster execution (optimized code)"
        echo "   • Better performance benchmarks"
        echo "   • Production-ready binary testing"
    fi

    # Show failed tests if any
    if [ ${#failed_tests[@]} -gt 0 ]; then
        echo ""
        echo "=== FAILED TESTS ($mode build) ==="
        for failed_test in "${failed_tests[@]}"; do
            echo "  • $failed_test"
        done
        echo ""
        echo "To run a specific failed test ($mode):"
        if [ "$mode" = "release" ]; then
            echo "  cargo test --release --test <test_name> -- --test-threads=1"
        else
            echo "  cargo test --test <test_name> -- --test-threads=1"
        fi
        echo ""

        # Add failure analysis if requested
        if [ "$RUN_ANALYZE" = true ]; then
            echo "=== FAILURE ANALYSIS ($mode build) ==="
            echo ""

            for failed_test in "${failed_tests[@]}"; do
                echo "--- $failed_test ---"

                # Get error output for analysis
                error_output=$(cargo test $cargo_flags --test "$failed_test" -- --test-threads=1 2>&1)

                # Extract specific error patterns
                if echo "$error_output" | grep -q "can call blocking only when running on the multi-threaded runtime"; then
                    echo "❌ ASYNC/RUNTIME ISSUE: Blocking calls in async context"
                fi

                if echo "$error_output" | grep -q "Model.*not found\|Model loading error\|Metadata not found"; then
                    echo "❌ MODEL LOADING ISSUE: Missing ML model files"
                fi

                if echo "$error_output" | grep -q "attempt to add with overflow"; then
                    echo "❌ OVERFLOW ERROR: Arithmetic overflow in text indexing"
                fi

                if echo "$error_output" | grep -q "IndexError\|Index.*failed\|index.*error"; then
                    echo "❌ INDEX OPERATION ISSUE: Text index problems"
                fi

                if echo "$error_output" | grep -q "GROUP BY\|Aggregation\|aggregate"; then
                    echo "❌ AGGREGATION ISSUE: GROUP BY or aggregation function problems"
                fi

                if echo "$error_output" | grep -q "UNION\|set operation"; then
                    echo "❌ SET OPERATION ISSUE: UNION or other set operation problems"
                fi

                if echo "$error_output" | grep -q "could not compile\|compilation error"; then
                    echo "❌ COMPILATION ERROR: Code compilation issues"
                    echo "$error_output" | grep -m1 "error:" | head -1 | sed 's/^/  /'
                fi

                if echo "$error_output" | grep -q "INSERT\|node creation\|CREATE"; then
                    echo "❌ DML OPERATION ISSUE: Data manipulation language problems"
                fi

                if echo "$error_output" | grep -q "cache\|persistence\|session"; then
                    echo "❌ CACHE/PERSISTENCE ISSUE: Data persistence or caching problems"
                fi

                if echo "$error_output" | grep -q "Pattern\|pattern matching"; then
                    echo "❌ PATTERN MATCHING ISSUE: Graph pattern matching problems"
                fi

                if echo "$error_output" | grep -q "assertion.*failed.*expected.*got"; then
                    echo "❌ ASSERTION FAILURE: Test expectations not met"
                    echo "$error_output" | grep -m1 "assertion.*failed" | head -1 | sed 's/^/  /'
                fi

                if echo "$error_output" | grep -q "procedure.*not found\|system procedure"; then
                    echo "❌ SYSTEM PROCEDURE ISSUE: Missing or non-functional system procedures"
                fi

                if echo "$error_output" | grep -q "Variable.*not found\|Expression.*failed"; then
                    echo "❌ VARIABLE/EXPRESSION ISSUE: Variable resolution or expression evaluation problems"
                fi

                if echo "$error_output" | grep -q "Property\|property access"; then
                    echo "❌ PROPERTY ACCESS ISSUE: Problems accessing node/edge properties"
                fi

                # Get first actual error line
                first_error=$(echo "$error_output" | grep -m1 -E "Error:|panicked at|assertion.*failed" | head -1)
                if [ ! -z "$first_error" ]; then
                    echo "First error: $first_error" | sed 's/^/  /'
                fi

                echo ""
            done
        else
            echo "For detailed failure analysis, run:"
            if [ "$mode" = "release" ]; then
                echo "  $0 --release --analyze"
            else
                echo "  $0 --analyze"
            fi
        fi
    fi

    echo ""

    # Return status based on results
    if [ $failed_count -eq 0 ] && [ $error_count -eq 0 ]; then
        echo "🎉 All integration tests passed in $mode mode!"
        return 0
    else
        echo "🔧 Some tests need attention in $mode mode. See failed tests above."
        return 1
    fi
}

# Main execution
if [ "$RUN_BOTH" = true ]; then
    echo "=== Running tests in BOTH debug and release modes ==="
    echo ""

    # Run debug tests
    run_test_suite "debug"
    debug_result=$?

    echo ""
    echo "========================================"
    echo ""

    # Run release tests
    run_test_suite "release"
    release_result=$?

    echo ""
    echo "=== OVERALL SUMMARY ==="
    if [ $debug_result -eq 0 ]; then
        echo "✅ Debug tests: PASSED"
    else
        echo "❌ Debug tests: FAILED"
    fi

    if [ $release_result -eq 0 ]; then
        echo "✅ Release tests: PASSED"
    else
        echo "❌ Release tests: FAILED"
    fi

    echo ""

    # Exit with failure if either mode failed
    if [ $debug_result -ne 0 ] || [ $release_result -ne 0 ]; then
        exit 1
    else
        echo "🎉 All tests passed in both debug and release modes!"
        exit 0
    fi
else
    # Run single mode
    run_test_suite "$BUILD_MODE"
    exit $?
fi
