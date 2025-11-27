# Building GraphLite for iOS

This guide explains how to build GraphLite FFI for all Apple platforms and create an XCFramework.

## Prerequisites

- macOS with Xcode installed
- Rust toolchain (rustup, cargo)
- Xcode Command Line Tools

## Quick Start

Run the automated build script:

```bash
cd bindings/swift
./build-ios.sh
```

This will:
1. Add all required Rust targets
2. Build for iOS device, iOS Simulator (both architectures), and macOS
3. Create a universal XCFramework
4. Output to `GraphLiteFFI.xcframework`

## Manual Build Steps

If you prefer to build manually or the script fails:

### Step 1: Add Rust Targets

```bash
rustup target add aarch64-apple-ios           # iOS device
rustup target add aarch64-apple-ios-sim       # iOS Simulator (Apple Silicon)
rustup target add x86_64-apple-ios            # iOS Simulator (Intel)
rustup target add aarch64-apple-darwin        # macOS (Apple Silicon)
```

### Step 2: Build for Each Platform

```bash
cd graphlite-ffi

# iOS Device (arm64)
cargo build --release --target aarch64-apple-ios

# iOS Simulator - Apple Silicon (arm64)
cargo build --release --target aarch64-apple-ios-sim

# iOS Simulator - Intel (x86_64)
cargo build --release --target x86_64-apple-ios

# macOS - Apple Silicon (arm64)
cargo build --release --target aarch64-apple-darwin
```

**Build outputs:**
- `target/aarch64-apple-ios/release/libgraphlit_ffi.a`
- `target/aarch64-apple-ios-sim/release/libgraphlit_ffi.a`
- `target/x86_64-apple-ios/release/libgraphlit_ffi.a`
- `target/aarch64-apple-darwin/release/libgraphlit_ffi.a`

### Step 3: Create Universal Simulator Binary

The iOS Simulator needs to support both Apple Silicon (arm64) and Intel (x86_64) Macs:

```bash
cd bindings/swift
mkdir -p build

lipo -create \
    ../../target/aarch64-apple-ios-sim/release/libgraphlit_ffi.a \
    ../../target/x86_64-apple-ios/release/libgraphlit_ffi.a \
    -output build/libgraphlit_ffi-simulator.a
```

### Step 4: Create XCFramework

Create the final XCFramework that bundles all platforms:

```bash
cd bindings/swift

xcodebuild -create-xcframework \
    -library ../../target/aarch64-apple-ios/release/libgraphlit_ffi.a \
    -headers ../../graphlite-ffi \
    -library build/libgraphlit_ffi-simulator.a \
    -headers ../../graphlite-ffi \
    -library ../../target/aarch64-apple-darwin/release/libgraphlit_ffi.a \
    -headers ../../graphlite-ffi \
    -output GraphLiteFFI.xcframework
```

### Step 5: Verify XCFramework

```bash
ls -lh GraphLiteFFI.xcframework/*/libgraphlit_ffi.a
xcodebuild -checkFirstLaunchStatus
```

You should see three library files:
- `ios-arm64/libgraphlit_ffi.a` (iOS device)
- `ios-arm64_x86_64-simulator/libgraphlit_ffi.a` (iOS Simulator)
- `macos-arm64/libgraphlit_ffi.a` (macOS)

## Troubleshooting

### Error: "library not found for -lgraphlit_ffi"

This means the XCFramework wasn't created or isn't in the expected location. Check:

```bash
cd bindings/swift
ls -la GraphLiteFFI.xcframework
```

### Error: "target may not be installed"

Install the missing target:

```bash
rustup target add <target-name>
```

### Error: "xcodebuild: command not found"

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

### Error: Building for Intel Mac

If you're on an Intel Mac (not Apple Silicon), you'll need different targets:

```bash
rustup target add x86_64-apple-darwin  # macOS Intel
cargo build --release --target x86_64-apple-darwin
```

## Platform Support Matrix

| Platform | Target Triple | Architecture | Status |
|----------|---------------|--------------|--------|
| iOS Device | `aarch64-apple-ios` | ARM64 |  Supported |
| iOS Simulator (M1/M2/M3) | `aarch64-apple-ios-sim` | ARM64 |  Supported |
| iOS Simulator (Intel) | `x86_64-apple-ios` | x86_64 |  Supported |
| macOS (Apple Silicon) | `aarch64-apple-darwin` | ARM64 |  Supported |
| macOS (Intel) | `x86_64-apple-darwin` | x86_64 |  Not built by default |

## Testing

After building:

```bash
# Test on macOS
cd bindings/swift
swift test

# Test on iOS Simulator
# (requires creating an Xcode project or using xcodebuild)
```

## File Sizes

Typical library sizes after optimization:

- iOS Device: ~35-40 MB
- iOS Simulator: ~70-80 MB (universal binary)
- macOS: ~35-40 MB

Total XCFramework size: ~150-160 MB (uncompressed)

## Distribution

The XCFramework can be:
1. **Embedded in Xcode projects** - Drag into project navigator
2. **Distributed via Swift Package Manager** - Reference in Package.swift
3. **Distributed via CocoaPods** - Create .podspec file
4. **Distributed via Carthage** - Use binary framework

## Next Steps

After building the XCFramework:

1. Update `Package.swift` to reference the XCFramework
2. Create examples (macOS CLI, iOS app)
3. Test on physical iOS device
4. Document iOS-specific usage

## References

- [Apple XCFramework Documentation](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle)
- [Rust iOS Development](https://mozilla.github.io/firefox-browser-architecture/experiments/2017-09-06-rust-on-ios.html)
- [Swift-Rust Integration](https://betterprogramming.pub/from-rust-to-swift-df9bde59b7cd)
