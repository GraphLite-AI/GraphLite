# GraphLite Swift Bindings - Quick Start

5-minute guide to get started with GraphLite on macOS and iOS.

## For macOS Development

### 1. Build (one-time setup)

```bash
cd graphlite-ffi
cargo build --release
```

### 2. Test

```bash
cd bindings/swift
swift test
```

Expected:  All 5 tests pass

### 3. Try the Example

```bash
cd Examples/macOS
swift CommandLineDemo.swift
```

### 4. Use in Your Code

```swift
import GraphLite

// Open database
let db = try GraphLite(path: "./mydb")
let session = try db.createSession(username: "admin")

// Setup
try session.execute("CREATE SCHEMA /app")
try session.execute("SESSION SET SCHEMA /app")
try session.execute("CREATE GRAPH /app/data")
try session.execute("SESSION SET GRAPH /app/data")

// Insert
try session.execute("INSERT (:Person {name: 'Alice', age: 30})")

// Query
let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")
for row in result.rows {
    print(row["p.name"]!, row["p.age"]!)
}

// Clean up
session.close()
```

---

## For iOS Development

### 1. Build for iOS (one-time setup)

```bash
cd bindings/swift
./build-ios.sh
```

This takes ~5-10 minutes and creates `GraphLiteFFI.xcframework`.

### 2. Open iOS Demo in Xcode

```bash
cd Examples/iOS/GraphLiteDemoApp
open .
```

Or create new Xcode project:
1. File → New → Project → iOS App
2. Add files: `GraphLiteDemoApp.swift`, `DatabaseManager.swift`, `ContentView.swift`
3. Add Package: File → Add Package Dependencies → Add Local → Select `bindings/swift`

### 3. Run on Simulator

1. Select iOS Simulator (iPhone 15)
2. Click  or press Cmd+R
3. Try the app:
   - Tap + to add people
   - Swipe left to delete
   - Search by city
   - Load sample data

### 4. Use in Your iOS App

```swift
import GraphLite
import SwiftUI

@MainActor
class DataManager: ObservableObject {
    @Published var items: [Item] = []
    private var db: GraphLite?
    private var session: Session?

    init() {
        // Use iOS Documents directory
        let docs = FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
        let dbPath = docs.appendingPathComponent("myapp.db").path

        do {
            db = try GraphLite(path: dbPath)
            session = try db?.createSession(username: "ios_user")

            try session?.execute("CREATE SCHEMA IF NOT EXISTS /app")
            try session?.execute("SESSION SET SCHEMA /app")
            try session?.execute("CREATE GRAPH IF NOT EXISTS /app/data")
            try session?.execute("SESSION SET GRAPH /app/data")
        } catch {
            print("Error: \(error)")
        }
    }

    func loadItems() async {
        let result = try? session?.execute("MATCH (i:Item) RETURN i")
        // Parse results...
    }
}
```

---

## Common Operations

### Create Schema & Graph

```swift
try session.execute("CREATE SCHEMA /myapp")
try session.execute("SESSION SET SCHEMA /myapp")
try session.execute("CREATE GRAPH /myapp/data")
try session.execute("SESSION SET GRAPH /myapp/data")
```

### Insert Nodes

```swift
// Single node
try session.execute("INSERT (:Person {name: 'Alice', age: 30})")

// Multiple nodes
try session.execute("""
    INSERT (:Person {name: 'Alice', age: 30}),
           (:Person {name: 'Bob', age: 25})
""")
```

### Query with Filter

```swift
let result = try session.execute("""
    MATCH (p:Person WHERE p.age > 25)
    RETURN p.name, p.age
    ORDER BY p.age DESC
""")

for row in result.rows {
    if case .string(let name) = row["p.name"],
       case .integer(let age) = row["p.age"] {
        print("\(name): \(age)")
    }
}
```

### Aggregations

```swift
let stats = try session.execute("""
    MATCH (p:Person)
    RETURN COUNT(p) AS total,
           AVG(p.age) AS avg_age,
           MIN(p.age) AS min_age,
           MAX(p.age) AS max_age
""")
```

### Delete

```swift
// Delete specific node
try session.execute("MATCH (p:Person WHERE p.name = 'Alice') DELETE p")

// Delete all nodes of a label
try session.execute("MATCH (p:Person) DELETE p")
```

---

## File Locations

| Platform | Database Path |
|----------|---------------|
| macOS | `./mydb` (relative to current directory) |
| iOS Simulator | `/Users/<you>/Library/Developer/CoreSimulator/.../Documents/` |
| iOS Device | App's Documents directory (sandboxed) |

**Best practice for iOS:**
```swift
let documentsPath = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("myapp.db")
    .path
```

---

## Error Handling

```swift
do {
    let result = try session.execute("MATCH (n) RETURN n")
} catch GraphLiteError.queryError {
    print("Query syntax error")
} catch GraphLiteError.databaseClosed {
    print("Database is closed")
} catch {
    print("Error: \(error.localizedDescription)")
}
```

---

## Troubleshooting

### "library not found for -lgraphlit_ffi"

Build the FFI library:
```bash
cd graphlite-ffi && cargo build --release
```

### "No such module 'GraphLite'"

Build the Swift package:
```bash
cd bindings/swift && swift build
```

### iOS: "Building for iOS, but the linked and embedded framework..."

Run the iOS build script:
```bash
cd bindings/swift && ./build-ios.sh
```

---

## Next Steps

### Learn More
- [README.md](README.md) - Full documentation
- [iOS_SETUP_GUIDE.md](docs/guides/iOS_SETUP_GUIDE.md) - Complete iOS guide
- [Examples](Examples/) - Working code examples

### Phase 2 SDK (Coming Soon)
High-level API with:
- Type-safe models
- Query builder
- SwiftUI property wrappers
- Relationship traversal

See [SDK_DESIGN.md](docs/design/SDK_DESIGN.md) for details.

---

## Getting Help

- GitHub Issues: https://github.com/GraphLite-AI/GraphLite/issues
- Documentation: https://graphlite.ai/docs
- GQL Spec: https://www.gqlstandards.org/

---

**Happy coding with GraphLite!** 
