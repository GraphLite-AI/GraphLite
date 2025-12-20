#!/bin/bash
# Script to run all unit tests and provide a summary
# Usage: ./scripts/run_unit_tests.sh

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  GraphLite Unit Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Store start time
START_TIME=$(date +%s)

# Create a temporary file to store test output
TEMP_FILE=$(mktemp)

# Run unit tests and capture output
echo -e "${YELLOW}Running unit tests...${NC}"
echo ""

if cargo test --lib -p graphlite 2>&1 | tee "$TEMP_FILE"; then
    TEST_SUCCESS=true
else
    TEST_SUCCESS=false
fi

# Store end time
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Extract test results
TOTAL_PASSED=$(grep -o "[0-9]* passed" "$TEMP_FILE" | tail -1 | awk '{print $1}')
TOTAL_FAILED=$(grep -o "[0-9]* failed" "$TEMP_FILE" | tail -1 | awk '{print $1}')
TOTAL_IGNORED=$(grep -o "[0-9]* ignored" "$TEMP_FILE" | tail -1 | awk '{print $1}')

# Count tests by module
LOGICAL_BUILDER_TESTS=$(grep "plan::builders::logical_builder::tests::" "$TEMP_FILE" | wc -l | tr -d ' ')
PHYSICAL_BUILDER_TESTS=$(grep "plan::builders::physical_builder::tests::" "$TEMP_FILE" | wc -l | tr -d ' ')
LOGICAL_OPTIMIZER_TESTS=$(grep "plan::optimizers::logical_optimizer::tests::" "$TEMP_FILE" | wc -l | tr -d ' ')
PHYSICAL_OPTIMIZER_TESTS=$(grep "plan::optimizers::physical_optimizer::tests::" "$TEMP_FILE" | wc -l | tr -d ' ')

# Calculate totals
NEW_TESTS=$((LOGICAL_BUILDER_TESTS + PHYSICAL_BUILDER_TESTS + LOGICAL_OPTIMIZER_TESTS + PHYSICAL_OPTIMIZER_TESTS))
EXISTING_TESTS=$((TOTAL_PASSED - NEW_TESTS))

# Display summary
echo -e "Total Duration: ${DURATION}s"
echo ""
echo -e "${GREEN}Passed:${NC}  $TOTAL_PASSED tests"
if [ "$TOTAL_FAILED" != "0" ]; then
    echo -e "${RED}Failed:${NC}  $TOTAL_FAILED tests"
fi
if [ "$TOTAL_IGNORED" != "0" ]; then
    echo -e "${YELLOW}Ignored:${NC} $TOTAL_IGNORED tests"
fi
echo ""

echo -e "${BLUE}Breakdown by Module:${NC}"
echo ""
echo -e "  ${GREEN}New Phase 3 Tests (Optimizer Refactoring):${NC}"
echo -e "    - logical_builder.rs:    $LOGICAL_BUILDER_TESTS tests"
echo -e "    - physical_builder.rs:   $PHYSICAL_BUILDER_TESTS tests"
echo -e "    - logical_optimizer.rs:  $LOGICAL_OPTIMIZER_TESTS tests"
echo -e "    - physical_optimizer.rs: $PHYSICAL_OPTIMIZER_TESTS tests"
echo -e "    ${BLUE}Subtotal: $NEW_TESTS tests${NC}"
echo ""
echo -e "  ${GREEN}Existing Unit Tests:${NC} $EXISTING_TESTS tests"
echo ""

# Display overall result
echo -e "${BLUE}========================================${NC}"
if [ "$TEST_SUCCESS" = true ]; then
    echo -e "${GREEN}✓ All unit tests passed!${NC}"
else
    echo -e "${RED}✗ Some tests failed${NC}"
fi
echo -e "${BLUE}========================================${NC}"
echo ""

# Cleanup
rm -f "$TEMP_FILE"

# Exit with appropriate code
if [ "$TEST_SUCCESS" = true ]; then
    exit 0
else
    exit 1
fi
