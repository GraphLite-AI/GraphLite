# iOS Setup Guide

Complete guide for building and running GraphLite on iOS devices and simulators.

## Overview

This guide walks you through:
1. Building GraphLite FFI for all Apple platforms
2. Creating XCFramework for iOS distribution
3. Running examples on iOS Simulator
4. Testing on physical iOS devices
5. Integrating into your own iOS app

## Prerequisites

- **macOS** with Xcode 15+
- **Rust toolchain** (rustup, cargo)
- **iOS SDK** (comes with Xcode)
- **Apple Developer account** (for physical device testing)

## Step 1: Build for iOS Platforms

### Quick Build (Recommended)

Run the automated build script:

```bash
cd bindings/swift
./build-ios.sh
```

This script will:
1. Add all required Rust targets
2. Build for:
   - iOS device (aarch64-apple-ios)
   - iOS Simulator Apple Silicon (aarch64-apple-ios-sim)
   - iOS Simulator Intel (x86_64-apple-ios)
   - macOS Apple Silicon (aarch64-apple-darwin)
3. Create universal simulator binary
4. Generate `GraphLiteFFI.xcframework`

**Expected output:**
```
========================================
 Build Complete!
========================================

XCFramework created at:
  .../GraphLiteFFI.xcframework

Supported platforms:
   iOS Device (arm64)
   iOS Simulator (arm64 + x86_64)
   macOS (arm64)
```

### Manual Build

See [BUILD_INSTRUCTIONS.md](BUILD_INSTRUCTIONS.md) for detailed manual steps.

## Step 2: Verify Build

Check that XCFramework was created:

```bash
ls -lh GraphLiteFFI.xcframework/*/libgraphlit_ffi.a
```

You should see three libraries:
- `ios-arm64/libgraphlit_ffi.a` (~35-40 MB)
- `ios-arm64_x86_64-simulator/libgraphlit_ffi.a` (~70-80 MB)
- `macos-arm64/libgraphlit_ffi.a` (~35-40 MB)

## Step 3: Update Package.swift (Optional)

If you want to use XCFramework with SPM:

```bash
# Backup current Package.swift
cp Package.swift Package-direct.swift

# Use XCFramework version
cp Package-xcframework.swift Package.swift
```

**Or keep the current Package.swift** - it links directly to build artifacts (works for macOS development).

## Step 4: Run iOS Simulator Example

### Option A: Using Xcode (Easiest)

1. **Create new Xcode project**:
   ```
   File → New → Project
   iOS → App
   Product Name: GraphLiteDemoApp
   Interface: SwiftUI
   ```

2. **Add Swift files**:
   - Drag `Examples/iOS/GraphLiteDemoApp/*.swift` into project

3. **Add GraphLite package**:
   ```
   File → Add Package Dependencies
   Add Local → Select bindings/swift folder
   ```

4. **Select Simulator**:
   - iPhone 15 (or any iOS 13+ simulator)

5. **Run**:
   - Click  or press Cmd+R

### Option B: Command Line

```bash
# Navigate to iOS app directory
cd Examples/iOS/GraphLiteDemoApp

# Create Xcode project programmatically
swift package init --type executable

# Add source files to Package.swift
# (requires manual editing)

# Build
swift build

# Note: Cannot run iOS apps from command line
# Must use Xcode or xcodebuild
```

## Step 5: Run on Physical Device

### Prerequisites

- iPhone or iPad with iOS 13+
- USB cable or WiFi sync enabled
- Apple Developer account (free or paid)

### Steps

1. **Connect device** to Mac via USB

2. **Open project in Xcode**

3. **Select device** in target dropdown (top bar)

4. **Configure signing**:
   ```
   Project Settings → Signing & Capabilities
   Team: Select your Apple ID
   Bundle Identifier: Make it unique (e.g., com.yourname.graphlite)
   ```

5. **Trust developer** (first time only):
   - On device: Settings → General → VPN & Device Management
   - Tap your developer certificate
   - Tap "Trust"

6. **Run**:
   - Click  in Xcode
   - App installs and launches on device

## Step 6: Test the Demo App

The iOS demo app demonstrates:

### Features to Test

1. **Add Person**:
   - Tap + button
   - Enter: Name="Alice", Age=30, City="New York"
   - Tap "Add"
   - Verify person appears in list

2. **Search by City**:
   - Enter "New York" in search field
   - Tap "Search"
   - Verify only NYC people show

3. **Delete Person**:
   - Swipe left on person row
   - Tap "Delete"
   - Verify person removed

4. **Load Sample Data**:
   - Tap ⋯ menu
   - Tap "Load Sample Data"
   - Verify 5 people added

5. **Clear Database**:
   - Tap ⋯ menu
   - Tap "Clear All"
   - Confirm empty state

### Expected Behavior

- **Fast queries**: < 10ms for small datasets
- **Persistent data**: Survives app restart
- **Smooth UI**: No lag or freezing
- **Error handling**: Graceful failure messages

## Step 7: Integrate into Your App

### Swift Package Manager

1. Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/GraphLite-AI/GraphLite", from: "0.0.1")
]
```

2. Import in your code:

```swift
import GraphLite

// Initialize database
let db = try GraphLite(path: documentsPath)
let session = try db.createSession(username: "user")

// Use it
try session.execute("CREATE SCHEMA /myapp")
```

### Xcode Project

1. **Add Package Dependency**:
   ```
   File → Add Package Dependencies
   https://github.com/GraphLite-AI/GraphLite
   ```

2. **Or add XCFramework manually**:
   - Drag `GraphLiteFFI.xcframework` into project
   - Frameworks, Libraries, and Embedded Content → Add
   - Select "Embed & Sign"

## Troubleshooting

### Build Issues

**Error: "target may not be installed"**

Add missing Rust target:
```bash
rustup target add aarch64-apple-ios
```

**Error: "xcodebuild: command not found"**

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

**Error: "lipo: can't open input file"**

Ensure all platforms built successfully:
```bash
ls target/aarch64-apple-ios/release/libgraphlit_ffi.a
ls target/aarch64-apple-ios-sim/release/libgraphlit_ffi.a
ls target/x86_64-apple-ios/release/libgraphlit_ffi.a
```

### Runtime Issues

**Error: "library not loaded"**

XCFramework not embedded properly:
- Xcode: Check "Embed & Sign" in Frameworks section
- SPM: Ensure `GraphLiteFFI.xcframework` exists in package

**Error: "Failed to open database"**

Path issue - use iOS Documents directory:
```swift
let documentsPath = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
    .path
```

**Error: "Code signing required for iOS"**

Select Team in Xcode:
- Project Settings → Signing & Capabilities → Team

### Simulator Issues

**Simulator crashes on launch**

Wrong architecture:
- M1/M2/M3 Mac: Use `aarch64-apple-ios-sim`
- Intel Mac: Use `x86_64-apple-ios`
- Universal binary: Combine both with `lipo`

**"Building for iOS Simulator, but linking in object file..."**

Clean build:
```bash
rm -rf .build
swift build
```

## Performance on iOS

Typical metrics on iPhone 15:

| Operation | Time |
|-----------|------|
| Database open | ~5ms |
| Session create | ~2ms |
| Simple INSERT | ~3ms |
| Simple MATCH | ~5ms |
| Complex query | ~10-20ms |

Database sizes:
- Empty database: ~100KB
- 1000 nodes: ~1-2MB
- 10000 nodes: ~10-20MB

## Best Practices for iOS

### 1. Use Documents Directory

```swift
let documentsURL = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
let dbPath = documentsURL.appendingPathComponent("myapp.db").path
```

### 2. Handle Background Mode

```swift
// Close session when app enters background
NotificationCenter.default.addObserver(
    forName: UIApplication.didEnterBackgroundNotification,
    object: nil,
    queue: .main
) { _ in
    session?.close()
}
```

### 3. Use Async/Await

```swift
@MainActor
class DataManager: ObservableObject {
    func loadData() async {
        // Run on background thread
        let result = await Task.detached {
            try? self.session?.execute("MATCH (n) RETURN n")
        }.value

        // Update UI on main thread
        self.items = parseResult(result)
    }
}
```

### 4. Handle Errors Gracefully

```swift
do {
    try session.execute(query)
} catch GraphLiteError.queryError {
    // Show user-friendly message
    errorMessage = "Invalid query. Please try again."
} catch {
    errorMessage = "Database error: \(error.localizedDescription)"
}
```

## File Locations on iOS

- **Simulator**: `/Users/<you>/Library/Developer/CoreSimulator/Devices/<UUID>/data/Containers/Data/Application/<UUID>/Documents/`

- **Device**: Access via Xcode:
  - Window → Devices and Simulators
  - Select device → Installed Apps → Your App
  - Download Container

## Next Steps

1.  Build XCFramework for iOS
2.  Test on iOS Simulator
3.  Test on physical device
4. Create your own iOS app with GraphLite
5. Explore Phase 2 SDK features (coming soon)

## Additional Resources

- [Swift Bindings README](README.md)
- [Build Instructions](BUILD_INSTRUCTIONS.md)
- [iOS Demo App README](Examples/iOS/GraphLiteDemoApp/README.md)
- [Apple Developer Documentation](https://developer.apple.com/documentation/)
- [Swift Package Manager](https://swift.org/package-manager/)

## Support

For issues or questions:
- GitHub Issues: https://github.com/GraphLite-AI/GraphLite/issues
- Documentation: https://graphlite.ai/docs
