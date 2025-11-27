#!/bin/bash
#
# Build GraphLite FFI for all Apple platforms (iOS + macOS)
# This script builds static libraries for:
#   - iOS device (aarch64-apple-ios)
#   - iOS Simulator on Apple Silicon (aarch64-apple-ios-sim)
#   - iOS Simulator on Intel (x86_64-apple-ios)
#   - macOS Apple Silicon (aarch64-apple-darwin)
#
# Output: XCFramework suitable for distribution

set -e

echo "=========================================="
echo "Building GraphLite FFI for Apple Platforms"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Change to graphlite-ffi directory
cd "$(dirname "$0")/../../graphlite-ffi"

echo ""
echo "Current directory: $(pwd)"
echo ""

# Add required targets
echo "${YELLOW}Step 1: Adding Rust targets...${NC}"
rustup target add aarch64-apple-ios
rustup target add aarch64-apple-ios-sim
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-darwin
echo "${GREEN}✓ Targets added${NC}"

# Build for each platform
echo ""
echo "${YELLOW}Step 2: Building for iOS device (aarch64-apple-ios)...${NC}"
cargo build --release --target aarch64-apple-ios
echo "${GREEN}✓ iOS device build complete${NC}"

echo ""
echo "${YELLOW}Step 3: Building for iOS Simulator Apple Silicon (aarch64-apple-ios-sim)...${NC}"
cargo build --release --target aarch64-apple-ios-sim
echo "${GREEN}✓ iOS Simulator (Apple Silicon) build complete${NC}"

echo ""
echo "${YELLOW}Step 4: Building for iOS Simulator Intel (x86_64-apple-ios)...${NC}"
cargo build --release --target x86_64-apple-ios
echo "${GREEN}✓ iOS Simulator (Intel) build complete${NC}"

echo ""
echo "${YELLOW}Step 5: Building for macOS Apple Silicon (aarch64-apple-darwin)...${NC}"
cargo build --release --target aarch64-apple-darwin
echo "${GREEN}✓ macOS build complete${NC}"

# Create directories for XCFramework
echo ""
echo "${YELLOW}Step 6: Preparing XCFramework structure...${NC}"
cd ../bindings/swift
rm -rf GraphLiteFFI.xcframework
mkdir -p build/ios-arm64
mkdir -p build/ios-arm64-simulator
mkdir -p build/macos-arm64

# Copy libraries to build directories
cp ../../target/aarch64-apple-ios/release/libgraphlite_ffi.a build/ios-arm64/
cp ../../graphlite-ffi/graphlite.h build/ios-arm64/

# For simulator, we need to create a fat binary combining arm64 and x86_64
echo "${YELLOW}Step 7: Creating universal simulator library...${NC}"
lipo -create \
    ../../target/aarch64-apple-ios-sim/release/libgraphlite_ffi.a \
    ../../target/x86_64-apple-ios/release/libgraphlite_ffi.a \
    -output build/ios-arm64-simulator/libgraphlite_ffi.a
cp ../../graphlite-ffi/graphlite.h build/ios-arm64-simulator/
echo "${GREEN}✓ Universal simulator library created${NC}"

cp ../../target/aarch64-apple-darwin/release/libgraphlite_ffi.a build/macos-arm64/
cp ../../graphlite-ffi/graphlite.h build/macos-arm64/

# Create XCFramework
echo ""
echo "${YELLOW}Step 8: Creating XCFramework...${NC}"
xcodebuild -create-xcframework \
    -library build/ios-arm64/libgraphlite_ffi.a \
    -headers build/ios-arm64 \
    -library build/ios-arm64-simulator/libgraphlite_ffi.a \
    -headers build/ios-arm64-simulator \
    -library build/macos-arm64/libgraphlite_ffi.a \
    -headers build/macos-arm64 \
    -output GraphLiteFFI.xcframework

echo "${GREEN}✓ XCFramework created${NC}"

# Clean up build directory
echo ""
echo "${YELLOW}Step 9: Cleaning up temporary files...${NC}"
rm -rf build
echo "${GREEN}✓ Cleanup complete${NC}"

# Verify
echo ""
echo "=========================================="
echo "${GREEN}✓ Build Complete!${NC}"
echo "=========================================="
echo ""
echo "XCFramework created at:"
echo "  $(pwd)/GraphLiteFFI.xcframework"
echo ""
echo "Supported platforms:"
echo "  ✓ iOS Device (arm64)"
echo "  ✓ iOS Simulator (arm64 + x86_64)"
echo "  ✓ macOS (arm64)"
echo ""
echo "Library sizes:"
ls -lh GraphLiteFFI.xcframework/*/libgraphlite_ffi.a | awk '{print "  " $9 ": " $5}'
echo ""
echo "Next steps:"
echo "  1. Update Package.swift to use the XCFramework"
echo "  2. Run: swift build"
echo "  3. Test on iOS Simulator"
echo ""
