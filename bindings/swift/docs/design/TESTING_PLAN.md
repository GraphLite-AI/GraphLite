# GraphLite Swift Bindings - Testing Plan

**Target Platforms:** macOS (M3), iOS Simulator, iOS Physical Devices
**Estimated Time:** 12-17 hours over 5 days
**Status:** Planning Phase

---

## Table of Contents

1. [Environment Setup](#environment-setup)
2. [Build Strategy](#build-strategy)
3. [Testing Strategy](#testing-strategy)
4. [Test Scenarios](#test-scenarios)
5. [Debugging and Monitoring](#debugging-and-monitoring)
6. [Automated Testing](#automated-testing)
7. [Success Criteria](#success-criteria)
8. [Timeline](#timeline)

---

## Environment Setup

### Prerequisites Check

Verify you have the necessary tools:

```bash
# Check Xcode installation
xcodebuild -version
# Expected: Xcode 15.x or later

# Check available simulators
xcrun simctl list devices available

# Check Swift version
swift --version
# Expected: Swift 5.9+

# Check Rust toolchain
rustc --version
cargo --version

# Check if you have the macOS target installed
rustup target list | grep apple-darwin
```

### Install Required Rust Targets

```bash
# For M3 Mac (native)
rustup target add aarch64-apple-darwin

# For iOS physical devices (iPhone/iPad with Apple Silicon)
rustup target add aarch64-apple-ios

# For iOS simulator on M3 Mac
rustup target add aarch64-apple-ios-sim

# For legacy x86_64 simulator (optional, for older Macs)
rustup target add x86_64-apple-ios
```

### Optional Helper Tools

```bash
# For easier cross-compilation
cargo install cargo-lipo

# xcodebuild is provided by Xcode (no separate install needed)
```

---

## Build Strategy

### Build GraphLite C FFI for All Platforms

**Script:** `scripts/build-apple-platforms.sh`

```bash
#!/bin/bash
set -e

echo "Building GraphLite FFI for Apple platforms..."

# Clean previous builds
cargo clean

# Build for macOS (M3)
echo "Building for macOS (Apple Silicon)..."
cargo build --package graphlite-ffi --target aarch64-apple-darwin --release

# Build for iOS Device (iPhone/iPad)
echo "Building for iOS Device..."
cargo build --package graphlite-ffi --target aarch64-apple-ios --release

# Build for iOS Simulator (M3 Mac)
echo "Building for iOS Simulator (Apple Silicon)..."
cargo build --package graphlite-ffi --target aarch64-apple-ios-sim --release

# Optional: Build for older Intel simulators
# cargo build --package graphlite-ffi --target x86_64-apple-ios --release

echo "Build complete!"
echo "Libraries located at:"
echo "  macOS: target/aarch64-apple-darwin/release/libgraphlite_ffi.a"
echo "  iOS Device: target/aarch64-apple-ios/release/libgraphlite_ffi.a"
echo "  iOS Simulator: target/aarch64-apple-ios-sim/release/libgraphlite_ffi.a"

# Copy header file
cp graphlite-ffi/graphlite.h graphlite-swift/Sources/CGraphLite/
echo "Header copied to graphlite-swift/Sources/CGraphLite/graphlite.h"
```

**Execute:**
```bash
chmod +x scripts/build-apple-platforms.sh
./scripts/build-apple-platforms.sh
```

### Create XCFramework (Universal Binary)

**Script:** `scripts/create-xcframework.sh`

```bash
#!/bin/bash
set -e

echo "Creating XCFramework..."

# Create temporary framework directories
mkdir -p build/ios-device
mkdir -p build/ios-simulator
mkdir -p build/macos

# Copy libraries
cp target/aarch64-apple-ios/release/libgraphlite_ffi.a build/ios-device/
cp target/aarch64-apple-ios-sim/release/libgraphlite_ffi.a build/ios-simulator/
cp target/aarch64-apple-darwin/release/libgraphlite_ffi.a build/macos/

# Copy headers to each directory
cp graphlite-ffi/graphlite.h build/ios-device/
cp graphlite-ffi/graphlite.h build/ios-simulator/
cp graphlite-ffi/graphlite.h build/macos/

# Create XCFramework
xcodebuild -create-xcframework \
    -library build/ios-device/libgraphlite_ffi.a -headers build/ios-device \
    -library build/ios-simulator/libgraphlite_ffi.a -headers build/ios-simulator \
    -library build/macos/libgraphlite_ffi.a -headers build/macos \
    -output graphlite-swift/GraphLiteFFI.xcframework

echo "XCFramework created at graphlite-swift/GraphLiteFFI.xcframework"

# Verify
xcodebuild -checkFirstLaunchStatus
ls -lh graphlite-swift/GraphLiteFFI.xcframework
```

**Execute:**
```bash
chmod +x scripts/create-xcframework.sh
./scripts/create-xcframework.sh
```

---

## Testing Strategy

### macOS Testing (M3 Mac)

#### Option A: Command-Line Swift (Fastest)

**File:** `graphlite-swift/Examples/macOS/demo.swift`

```swift
import Foundation
import GraphLite

// Simple command-line test
do {
    print("GraphLite macOS Demo")
    print("====================\n")

    // Create database
    print("1. Opening database...")
    let db = try GraphLite(path: "./test_db")

    // Create session
    print("2. Creating session...")
    let session = try db.createSession(username: "admin")

    // Execute query
    print("3. Executing query...")
    try session.execute("CREATE SCHEMA /test")
    try session.execute("SESSION SET SCHEMA /test")
    try session.execute("CREATE GRAPH /test/demo")
    try session.execute("SESSION SET GRAPH /test/demo")

    // Insert data
    print("4. Inserting data...")
    try session.execute("""
        INSERT (:Person {name: 'Alice', age: 30}),
               (:Person {name: 'Bob', age: 25})
    """)

    // Query data
    print("5. Querying data...")
    let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")

    print("\nResults:")
    print("Columns: \(result.variables.joined(separator: ", "))")
    for (index, row) in result.rows.enumerated() {
        print("Row \(index): \(row)")
    }

    print("\n Test completed successfully!")

} catch {
    print(" Error: \(error)")
}
```

**Run:**
```bash
cd graphlite-swift
swift run Examples/macOS/demo.swift
```

#### Option B: Swift Package Tests

**File:** `Tests/GraphLiteTests/GraphLiteTests.swift`

```swift
import XCTest
@testable import GraphLite

final class GraphLiteTests: XCTestCase {
    var db: GraphLite?
    var session: Session?

    override func setUp() async throws {
        db = try GraphLite(path: "./test_db_\(UUID().uuidString)")
        session = try db?.createSession(username: "test_user")
    }

    override func tearDown() async throws {
        session?.close()
        db = nil
    }

    func testDatabaseCreation() throws {
        XCTAssertNotNil(db)
    }

    func testSessionCreation() throws {
        XCTAssertNotNil(session)
    }

    func testBasicQuery() throws {
        let session = try XCTUnwrap(self.session)

        // Create schema and graph
        try session.execute("CREATE SCHEMA /test")
        try session.execute("SESSION SET SCHEMA /test")
        try session.execute("CREATE GRAPH /test/demo")
        try session.execute("SESSION SET GRAPH /test/demo")

        // Insert data
        try session.execute("INSERT (:Person {name: 'Test'})")

        // Query data
        let result = try session.execute("MATCH (p:Person) RETURN p.name")

        XCTAssertEqual(result.rowCount, 1)
        XCTAssertEqual(result.variables, ["p.name"])
    }

    func testRDFSupport() throws {
        let session = try XCTUnwrap(self.session)

        // TODO: Add RDF testing when available
        // try session.loadRDF(...)
    }
}
```

**Run:**
```bash
cd graphlite-swift
swift test
```

### iOS Simulator Testing (M3 Mac)

#### Create iOS App in Xcode

**Steps:**

1. Open Xcode
2. File → New → Project
3. Choose "iOS" → "App"
4. Name: "GraphLiteDemo"
5. Language: Swift
6. Interface: SwiftUI

**Add GraphLite Package:**
- In Xcode, select project
- Select "Package Dependencies" tab
- Click "+" → "Add Local..."
- Select `graphlite-swift` folder
- Add "GraphLite" library to app target

**Simple Test View:**

**File:** `GraphLiteDemo/ContentView.swift`

```swift
import SwiftUI
import GraphLite

struct ContentView: View {
    @State private var status = "Not started"
    @State private var results: [String] = []

    var body: some View {
        VStack(spacing: 20) {
            Text("GraphLite iOS Test")
                .font(.title)

            Text(status)
                .foregroundColor(.secondary)

            Button("Run Test") {
                runTest()
            }
            .buttonStyle(.borderedProminent)

            List(results, id: \.self) { result in
                Text(result)
                    .font(.caption)
            }
        }
        .padding()
    }

    func runTest() {
        results = []
        status = "Running..."

        Task {
            do {
                // Get documents directory for iOS
                let docDir = FileManager.default.urls(
                    for: .documentDirectory,
                    in: .userDomainMask
                ).first!
                let dbPath = docDir.appendingPathComponent("test.db").path

                results.append("Opening database at: \(dbPath)")

                let db = try GraphLite(path: dbPath)
                results.append(" Database opened")

                let session = try db.createSession(username: "admin")
                results.append(" Session created")

                try session.execute("CREATE SCHEMA /test")
                try session.execute("SESSION SET SCHEMA /test")
                try session.execute("CREATE GRAPH /test/demo")
                try session.execute("SESSION SET GRAPH /test/demo")
                results.append(" Schema and graph created")

                try session.execute("INSERT (:Person {name: 'Alice', age: 30})")
                results.append(" Data inserted")

                let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")
                results.append(" Query executed: \(result.rowCount) rows")

                for row in result.rows {
                    results.append("  Row: \(row)")
                }

                status = " Test completed!"

            } catch {
                status = " Error"
                results.append("Error: \(error)")
            }
        }
    }
}
```

**Run in Simulator:**
- Select "iPhone 15 Pro" (or any simulator)
- Click Run (⌘R)
- Click "Run Test" button in the app

### iOS Physical Device Testing

**Requirements:**
- Physical iPhone or iPad with iOS 13+
- USB-C cable to connect to M3 Mac
- Apple Developer account (free tier is fine)

**Steps:**

1. **Enable Developer Mode on iPhone:**
   - Settings → Privacy & Security → Developer Mode → Enable
   - Restart device

2. **Connect iPhone to Mac:**
   - Connect via USB-C
   - Trust computer on iPhone when prompted

3. **In Xcode:**
   - Select your iPhone from device list (top toolbar)
   - Xcode → Settings → Accounts → Add Apple ID
   - Select project → Signing & Capabilities → Select your Team

4. **Run on Device:**
   - Click Run (⌘R)
   - App installs and runs on your iPhone

**Why Physical Device Testing is Important:**
- Tests actual ARM64 binary (not simulator)
- Tests real filesystem performance
- Tests memory constraints
- Tests battery impact
- Tests with actual network conditions

---

## Test Scenarios

### Basic Functionality Tests

**Test Matrix:**

| Test Case | macOS | iOS Simulator | iOS Device |
|-----------|-------|---------------|------------|
| Database creation |  |  |  |
| Session management |  |  |  |
| Schema creation |  |  |  |
| Graph creation |  |  |  |
| Node insertion |  |  |  |
| Relationship insertion |  |  |  |
| Basic queries |  |  |  |
| Pattern matching |  |  |  |
| Aggregations |  |  |  |

### Performance Tests

```swift
func testPerformance() throws {
    let session = try XCTUnwrap(self.session)

    // Setup
    try session.execute("CREATE SCHEMA /perf")
    try session.execute("SESSION SET SCHEMA /perf")
    try session.execute("CREATE GRAPH /perf/test")
    try session.execute("SESSION SET GRAPH /perf/test")

    // Measure insertion
    measure {
        for i in 0..<1000 {
            try! session.execute("INSERT (:Node {id: \(i)})")
        }
    }

    // Measure query
    measure {
        _ = try! session.execute("MATCH (n:Node) RETURN n LIMIT 100")
    }
}
```

### RDF Tests (Future)

```swift
func testRDFIngestion() throws {
    let session = try XCTUnwrap(self.session)

    let turtle = """
    @prefix ex: <http://example.org/> .
    ex:Alice a ex:Person ;
             ex:name "Alice" ;
             ex:age 30 .
    """

    try session.loadRDF(data: turtle, format: .turtle, graph: "knowledge")

    let result = try session.execute("""
        MATCH (s)-[:rdf:type]->(o)
        RETURN s, o
    """)

    XCTAssertGreaterThan(result.rowCount, 0)
}
```

### Stress Tests

```swift
func testLargeDataset() throws {
    // Test with 100K nodes
    // Test with 1M relationships
    // Test with complex queries
}

func testConcurrentSessions() throws {
    // Test multiple sessions
    // Test concurrent queries
}

func testMemoryPressure() throws {
    // Test under low memory conditions (iOS)
}
```

---

## Debugging and Monitoring

### Xcode Instruments

**Profile your iOS app:**

```bash
# Launch Instruments
open -a Instruments

# Select profiling template:
# - Time Profiler (CPU usage)
# - Allocations (memory usage)
# - Leaks (memory leaks)
# - File Activity (disk I/O)
```

**What to Monitor:**
- Memory usage (should stay under 50MB for 1M triples)
- CPU usage during queries
- Disk I/O patterns
- No memory leaks

### Console Logging

**Add debug output:**

```swift
// In GraphLite.swift
public func execute(_ query: String) throws -> QueryResult {
    print(" Executing query: \(query)")
    let start = Date()

    // ... execution ...

    let duration = Date().timeIntervalSince(start)
    print("⏱  Query completed in \(duration)s")

    return result
}
```

### Crash Reports

**For iOS device testing:**
- Window → Devices and Simulators
- Select your device
- View Device Logs
- Look for GraphLite crashes

---

## Automated Testing

### GitHub Actions (CI/CD)

**File:** `.github/workflows/swift-tests.yml`

```yaml
name: Swift Tests

on: [push, pull_request]

jobs:
  test-macos:
    runs-on: macos-14  # M1/M2/M3 runner
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: aarch64-apple-darwin

      - name: Build C FFI
        run: cargo build --package graphlite-ffi --release

      - name: Run Swift tests
        run: |
          cd graphlite-swift
          swift test

  test-ios-simulator:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v3

      - name: Build for iOS Simulator
        run: ./scripts/build-apple-platforms.sh

      - name: Test on iOS Simulator
        run: |
          xcrun simctl boot "iPhone 15 Pro" || true
          xcodebuild test \
            -scheme GraphLiteDemo \
            -destination 'platform=iOS Simulator,name=iPhone 15 Pro'
```

### Local Testing Script

**File:** `scripts/test-all-platforms.sh`

```bash
#!/bin/bash
set -e

echo "Running GraphLite Swift tests on all platforms..."

# Test macOS
echo "1. Testing macOS..."
cd graphlite-swift
swift test
cd ..

# Test iOS Simulator
echo "2. Testing iOS Simulator..."
xcodebuild test \
    -scheme GraphLite \
    -destination 'platform=iOS Simulator,name=iPhone 15 Pro'

echo " All tests passed!"
```

---

## Success Criteria

### macOS (M3)

-  Build completes without errors
-  All unit tests pass
-  Query latency < 10ms for 100K triples
-  Memory usage < 50MB
-  No memory leaks
-  Binary size < 5MB

### iOS Simulator

-  Same as macOS
-  Database persists between app launches
-  Works with app sandboxing

### iOS Device

-  All simulator tests pass
-  Performance comparable to simulator
-  Works offline
-  Survives app backgrounding
-  Low battery impact

### Known Limitations

**iOS Specific:**
- File size limits (app sandbox)
- Background execution limits
- Memory pressure on older devices
- App Store review requirements (if distributing)

---

## Timeline

### Day 1: Setup (1-2 hours)

- [ ] Install Rust targets
- [ ] Build C FFI for all platforms
- [ ] Create XCFramework
- [ ] Setup Swift package structure

### Day 2: macOS Testing (2-3 hours)

- [ ] Write basic Swift wrapper
- [ ] Create unit tests
- [ ] Run command-line demo
- [ ] Fix any issues

### Day 3: iOS Simulator Testing (2-3 hours)

- [ ] Create iOS demo app in Xcode
- [ ] Test basic functionality
- [ ] Profile with Instruments
- [ ] Document issues

### Day 4: iOS Device Testing (2-3 hours)

- [ ] Run on physical iPhone
- [ ] Test performance
- [ ] Test offline functionality
- [ ] Test app lifecycle

### Day 5: Polish & Document (2-3 hours)

- [ ] Fix remaining issues
- [ ] Write documentation
- [ ] Create examples
- [ ] Prepare for release

**Total:** ~12-17 hours over 5 days

---

## Next Steps After Testing

### If Tests Pass

1. Document platform-specific considerations
2. Create example apps (Notes app, Knowledge graph viewer)
3. Publish Swift Package to GitHub
4. Write Swift API documentation
5. Create video demos for iOS/macOS

### If Tests Fail

**Common Issues and Solutions:**

| Issue | Solution |
|-------|----------|
| Linker errors | Check XCFramework architecture |
| Missing symbols | Verify C FFI exports |
| Crashes on startup | Check library loading |
| Slow queries | Profile with Instruments |
| Memory leaks | Use Xcode Memory Graph |
| Sandbox violations | Check file paths (iOS) |
| Code signing issues | Check developer certificates |

---

## Appendix

### Useful Commands

```bash
# List all simulators
xcrun simctl list

# Boot simulator
xcrun simctl boot "iPhone 15 Pro"

# Install app on simulator
xcrun simctl install booted path/to/app.app

# View simulator logs
xcrun simctl spawn booted log stream --predicate 'processImagePath contains "GraphLite"'

# Clean build artifacts
swift package clean
xcodebuild clean

# Reset simulator
xcrun simctl erase all
```

### Troubleshooting Resources

- [Apple Developer Documentation](https://developer.apple.com/documentation/)
- [Swift Forums](https://forums.swift.org/)
- [GraphLite GitHub Issues](https://github.com/GraphLite-AI/GraphLite/issues)
- Xcode → Help → Developer Documentation

---

**Document Version:** 1.0
**Last Updated:** 2025-11-25
**Maintained By:** GraphLite Contributors
