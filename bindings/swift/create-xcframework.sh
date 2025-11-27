#!/bin/bash
#
# Create XCFramework from already-built Rust libraries
# Use this if you've already built the Rust FFI libraries
#
# Prerequisites:
#   - Libraries must exist in target/*/release/libgraphlite_ffi.a
#   - Run from bindings/swift directory

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "=========================================="
echo "Creating GraphLite XCFramework"
echo "=========================================="
echo ""

# Verify libraries exist
echo "${YELLOW}Checking for required libraries...${NC}"

LIBS=(
    "../../target/aarch64-apple-ios/release/libgraphlite_ffi.a"
    "../../target/aarch64-apple-ios-sim/release/libgraphlite_ffi.a"
    "../../target/x86_64-apple-ios/release/libgraphlite_ffi.a"
    "../../target/aarch64-apple-darwin/release/libgraphlite_ffi.a"
)

for lib in "${LIBS[@]}"; do
    if [ ! -f "$lib" ]; then
        echo "❌ Missing: $lib"
        echo ""
        echo "You need to build the Rust FFI libraries first:"
        echo "  cd ../../graphlite-ffi"
        echo "  cargo build --release --target aarch64-apple-ios"
        echo "  cargo build --release --target aarch64-apple-ios-sim"
        echo "  cargo build --release --target x86_64-apple-ios"
        echo "  cargo build --release --target aarch64-apple-darwin"
        echo ""
        echo "Or run the full build script:"
        echo "  ./build-ios.sh"
        exit 1
    fi
    echo "  ✓ Found: $lib"
done

echo "${GREEN}✓ All libraries found${NC}"
echo ""

# Create XCFramework structure
echo "${YELLOW}Creating XCFramework structure...${NC}"
rm -rf GraphLiteFFI.xcframework build
mkdir -p build/ios-arm64
mkdir -p build/ios-arm64-simulator
mkdir -p build/macos-arm64

# Copy iOS device library
cp ../../target/aarch64-apple-ios/release/libgraphlite_ffi.a build/ios-arm64/
cp ../../graphlite-ffi/graphlite.h build/ios-arm64/

# Create universal simulator library (arm64 + x86_64)
echo "${YELLOW}Creating universal simulator binary...${NC}"
lipo -create \
    ../../target/aarch64-apple-ios-sim/release/libgraphlite_ffi.a \
    ../../target/x86_64-apple-ios/release/libgraphlite_ffi.a \
    -output build/ios-arm64-simulator/libgraphlite_ffi.a
cp ../../graphlite-ffi/graphlite.h build/ios-arm64-simulator/
echo "${GREEN}✓ Universal simulator binary created${NC}"

# Copy macOS library
cp ../../target/aarch64-apple-darwin/release/libgraphlite_ffi.a build/macos-arm64/
cp ../../graphlite-ffi/graphlite.h build/macos-arm64/

# Create XCFramework
echo ""
echo "${YELLOW}Building XCFramework...${NC}"
xcodebuild -create-xcframework \
    -library build/ios-arm64/libgraphlite_ffi.a \
    -headers build/ios-arm64 \
    -library build/ios-arm64-simulator/libgraphlite_ffi.a \
    -headers build/ios-arm64-simulator \
    -library build/macos-arm64/libgraphlite_ffi.a \
    -headers build/macos-arm64 \
    -output GraphLiteFFI.xcframework

echo "${GREEN}✓ XCFramework created${NC}"

# Clean up
echo ""
echo "${YELLOW}Cleaning up temporary files...${NC}"
rm -rf build
echo "${GREEN}✓ Cleanup complete${NC}"

# Summary
echo ""
echo "=========================================="
echo "${GREEN}✓ XCFramework Ready!${NC}"
echo "=========================================="
echo ""
echo "Location: $(pwd)/GraphLiteFFI.xcframework"
echo ""
echo "Platforms:"
echo "  ✓ iOS Device (arm64)"
echo "  ✓ iOS Simulator (arm64 + x86_64)"
echo "  ✓ macOS (arm64)"
echo ""
echo "Library sizes:"
ls -lh GraphLiteFFI.xcframework/*/libgraphlite_ffi.a 2>/dev/null | awk '{print "  " $9 ": " $5}' || echo "  (size check failed)"
echo ""
echo "Next steps:"
echo "  1. Open Xcode project"
echo "  2. Add this XCFramework: General → Frameworks → +"
echo "  3. Or follow: Examples/iOS/XCODE_SETUP.md"
echo ""
