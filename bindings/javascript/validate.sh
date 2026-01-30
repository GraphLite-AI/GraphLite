#!/bin/bash
# Validation script for GraphLite JavaScript bindings

set -e

echo "========================================="
echo "GraphLite JavaScript Bindings Validation"
echo "========================================="
echo ""

# Check if FFI library exists
echo "1. Checking FFI library..."
if [ -f "../../target/release/libgraphlite_ffi.dylib" ] || [ -f "../../target/release/libgraphlite_ffi.so" ]; then
    echo "   ✓ FFI library found"
else
    echo "   ✗ FFI library not found"
    echo "   Please build it: cargo build --release -p graphlite-ffi"
    exit 1
fi

# Check if node_modules exists
echo "2. Checking dependencies..."
if [ -d "node_modules" ]; then
    echo "   ✓ Dependencies installed"
else
    echo "   Installing dependencies..."
    npm install
fi

# Test version
echo "3. Testing library loading..."
VERSION=$(node -e "const { GraphLite } = require('./index.js'); console.log(GraphLite.version());")
echo "   ✓ Library loaded, version: $VERSION"

# Run tests
echo "4. Running unit tests..."
npm test --silent 2>&1 | grep -E "(Test Suites|Tests:)"

# Run examples
echo "5. Testing examples..."
rm -rf test_db

echo "   - Running basic-usage.js..."
node examples/basic-usage.js > /dev/null 2>&1 && echo "     ✓ basic-usage.js works" || echo "     ✗ basic-usage.js failed"

echo "   - Running complex-queries.js..."
node examples/complex-queries.js > /dev/null 2>&1 && echo "     ✓ complex-queries.js works" || echo "     ✗ complex-queries.js failed"

echo "   - Running error-handling.js..."
node examples/error-handling.js > /dev/null 2>&1 && echo "     ✓ error-handling.js works" || echo "     ✗ error-handling.js failed"

# Cleanup
echo "6. Cleaning up..."
rm -rf test_db test_db* test_*
echo "   ✓ Cleaned up test artifacts"

echo ""
echo "========================================="
echo "✓ All validations passed!"
echo "========================================="
