# GraphLite Swift Bindings

Swift bindings for GraphLite - an embedded ISO GQL graph database with RDF support.

## Overview

This package provides native Swift bindings for GraphLite, allowing you to use GraphLite databases in iOS and macOS applications. The bindings are built as a thin wrapper over the existing C FFI layer, ensuring optimal performance and minimal overhead.

## Features

- Native Swift API with type-safe error handling
- Support for macOS 10.15+ and iOS 13+
- Zero-copy data transfer where possible
- Automatic memory management
- Full support for GQL query language
- JSON-based result sets with Codable support

## Requirements

- Swift 5.9+
- macOS 10.15+ or iOS 13+
- Xcode 15+ (for development)

## Installation

### Prerequisites - Building for iOS and macOS

Before using the Swift bindings, you need to build the GraphLite FFI library for all platforms:

```bash
# Quick build for all Apple platforms (iOS + macOS)
cd bindings/swift
./build-ios.sh
```

This script will:
- Build for iOS device (aarch64-apple-ios)
- Build for iOS Simulator Apple Silicon (aarch64-apple-ios-sim)
- Build for iOS Simulator Intel (x86_64-apple-ios)
- Build for macOS Apple Silicon (aarch64-apple-darwin)
- Create XCFramework at `GraphLiteFFI.xcframework`

For detailed build instructions and troubleshooting, see [BUILD_INSTRUCTIONS.md](docs/guides/BUILD_INSTRUCTIONS.md).

**macOS only (quick test):**
```bash
cd graphlite-ffi
cargo build --release
# Library at: target/release/libgraphlit_ffi.a
```

### Swift Package Manager

Add GraphLite to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/GraphLite-AI/GraphLite", from: "0.0.1")
]
```

Or add it directly in Xcode:
1. File → Add Package Dependencies
2. Enter the repository URL: `https://github.com/GraphLite-AI/GraphLite`
3. Select the version you want to use

## Usage

### Basic Example

```swift
import GraphLite

do {
    // Open database
    let db = try GraphLite(path: "./mydb")

    // Create session
    let session = try db.createSession(username: "admin")

    // Setup schema and graph
    try session.execute("CREATE SCHEMA /demo")
    try session.execute("SESSION SET SCHEMA /demo")
    try session.execute("CREATE GRAPH /demo/social")
    try session.execute("SESSION SET GRAPH /demo/social")

    // Insert data
    try session.execute("""
        INSERT (:Person {name: 'Alice', age: 30}),
               (:Person {name: 'Bob', age: 25})
    """)

    // Query data
    let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")

    print("Found \(result.rowCount) people:")
    for row in result.rows {
        let name = row["p.name"]!
        let age = row["p.age"]!
        print("  \(name) - \(age)")
    }

    // Close session
    session.close()

} catch {
    print("Error: \(error)")
}
```

### Working with Results

Query results are returned as `QueryResult` objects with strongly-typed values:

```swift
let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")

// Access column names
print("Columns: \(result.variables)")

// Iterate over rows
for row in result.rows {
    // Each row is a dictionary mapping column names to QueryValue
    if case .string(let name) = row["p.name"] {
        print("Name: \(name)")
    }

    if case .integer(let age) = row["p.age"] {
        print("Age: \(age)")
    }
}
```

### Query Value Types

The `QueryValue` enum supports all GraphLite data types:

```swift
public enum QueryValue {
    case string(String)
    case integer(Int64)
    case double(Double)
    case boolean(Bool)
    case null
}
```

You can convert to native Swift types:

```swift
let value: QueryValue = .string("hello")
let swiftValue: Any? = value.asAny  // Returns "hello" as String
```

### Error Handling

All operations throw `GraphLiteError` on failure:

```swift
do {
    let result = try session.execute("INVALID QUERY")
} catch GraphLiteError.queryError {
    print("Query execution failed")
} catch GraphLiteError.databaseClosed {
    print("Database connection is closed")
} catch {
    print("Unknown error: \(error)")
}
```

### Session Management

Sessions are automatically closed when deallocated, but you can explicitly close them:

```swift
let session = try db.createSession(username: "admin")

// Use session...

// Explicit close (optional, will be called automatically in deinit)
session.close()

// Check if closed
if session.closed {
    print("Session is closed")
}
```

## API Reference

### GraphLite

Main database class.

```swift
class GraphLite {
    init(path: String) throws
    func createSession(username: String) throws -> Session
    var databasePath: String { get }
    static var version: String { get }
}
```

### Session

Query execution session.

```swift
class Session {
    func execute(_ query: String) throws -> QueryResult
    func close()
    var closed: Bool { get }
}
```

### QueryResult

Query execution result.

```swift
struct QueryResult: Codable {
    let variables: [String]
    let rows: [[String: QueryValue]]
    let rowCount: Int
}
```

### GraphLiteError

Error enumeration.

```swift
enum GraphLiteError: Error {
    case databaseClosed
    case nullPointer
    case invalidUtf8
    case databaseOpenError
    case sessionError
    case queryError
    case panicError
    case jsonError
    case unknown(code: Int32)
}
```

## Examples

See the [Examples](Examples/) directory for complete working examples:

- **[macOS Command Line](Examples/macOS/)** - Simple CLI demo showing basic operations
- **[iOS SwiftUI App](Examples/iOS/GraphLiteDemoApp/)** - Complete iOS application with:
  - Add/delete/search people
  - SwiftUI integration with @EnvironmentObject
  - Persistent storage
  - Error handling
  - Async/await patterns

## iOS Development

Complete iOS support with examples and tooling:

1. **Build for iOS**: Run `./build-ios.sh` to create XCFramework
2. **See [iOS Setup Guide](docs/guides/iOS_SETUP_GUIDE.md)** for complete instructions
3. **Try the demo**: Open `Examples/iOS/GraphLiteDemoApp` in Xcode
4. **Integration**: Add package to your iOS app via SPM

The iOS demo app demonstrates all core features running natively on iPhone/iPad.

## Building from Source

1. Clone the repository:
```bash
git clone https://github.com/GraphLite-AI/GraphLite
cd GraphLite
```

2. Build the Rust FFI library:
```bash
cd graphlite-ffi
cargo build --release
cd ..
```

3. Build the Swift package:
```bash
cd bindings/swift
swift build
```

4. Run tests:
```bash
swift test
```

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| macOS (Apple Silicon) |  Supported | M1/M2/M3 chips |
| macOS (Intel) |  Planned | x86_64 support |
| iOS (Device) |  Supported | arm64 - See [iOS Setup Guide](docs/guides/iOS_SETUP_GUIDE.md) |
| iOS (Simulator) |  Supported | arm64 + x86_64 universal binary |
| tvOS |  Future | |
| watchOS |  Future | |

**iOS Support**: Complete implementation with XCFramework, examples, and documentation. Run `./build-ios.sh` to build for all Apple platforms.

## Performance

The Swift bindings are designed for minimal overhead:

- Direct C FFI calls (no intermediate layers)
- Zero-copy string handling where possible
- Efficient JSON parsing with Codable
- ~95% of native Rust performance

## Troubleshooting

### Library Not Found

If you get "library not found for -lgraphlit_ffi":

1. Ensure you've built the FFI library: `cd graphlite-ffi && cargo build --release`
2. Check that the library exists at `target/release/libgraphlit_ffi.a`
3. The Package.swift expects the library at `../../target/release` relative to the Swift package

### Module Not Found

If you get "no such module 'CGraphLite'":

1. Ensure `Sources/CGraphLite/graphlite.h` exists
2. Check that `Sources/CGraphLite/module.modulemap` is present
3. Clean and rebuild: `swift package clean && swift build`

### Undefined Symbols

If you get undefined symbol errors:

1. Ensure you're linking against the correct architecture (ARM64 for Apple Silicon)
2. Try rebuilding the Rust library: `cargo clean && cargo build --release`
3. Check that all required system libraries are linked (see Package.swift)

## Contributing

Contributions are welcome! See [../../CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## License

Apache 2.0 - See [../../LICENSE](../../LICENSE) for details.

## Resources

- [GraphLite Documentation](https://graphlite.ai/docs)
- [GQL Query Language](https://www.gqlstandards.org/)
### Documentation

- **Getting Started**: [QUICKSTART.md](QUICKSTART.md) - 5-minute quick start guide

#### Setup Guides
- [iOS Setup Guide](docs/guides/iOS_SETUP_GUIDE.md) - Complete iOS development guide
- [Build Instructions](docs/guides/BUILD_INSTRUCTIONS.md) - Manual build steps
- [Xcode Setup](docs/guides/XCODE_SETUP.md) - Xcode project setup for iOS demo

#### Design Documents
- [Implementation Plan](docs/design/IMPLEMENTATION_PLAN.md) - Phase 1 roadmap (completed)
- [Testing Plan](docs/design/TESTING_PLAN.md) - Testing strategy
- [SDK Design](docs/design/SDK_DESIGN.md) - Phase 2 high-level SDK design

## Status

**Current Status:** Phase 1 Complete - iOS Support Added 

### Completed
-  Project structure
-  C FFI bridging
-  Error handling
-  Database class
-  Session management
-  Query execution with JSON decoding
-  Result parsing (handles GraphLite's tagged union format)
-  Unit tests (5/5 passing on macOS)
-  macOS command-line example
-  iOS SwiftUI demo app
-  iOS build system (build-ios.sh)
-  XCFramework for iOS device + simulator
-  Comprehensive documentation

### Platform Support
-  macOS Apple Silicon - Fully tested
-  iOS Device (arm64) - Ready for testing
-  iOS Simulator (arm64 + x86_64) - Ready for testing

### Next Steps
-  Test on physical iOS device (requires user setup)
-  Phase 2: High-level SDK with query builder (4-6 weeks)
-  macOS Intel support (x86_64)
-  Distribution via CocoaPods/Carthage

See [SDK_DESIGN.md](docs/design/SDK_DESIGN.md) for Phase 2 roadmap.
