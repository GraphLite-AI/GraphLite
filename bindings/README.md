# GraphLite Language Bindings

Multi-language bindings for GraphLite embedded graph database.

## Overview

GraphLite provides bindings for multiple programming languages, all built on top of the C-compatible FFI layer ([graphlite-ffi](../graphlite-ffi/)). This allows you to use GraphLite from your preferred programming language while maintaining near-native performance.

## Available Bindings

| Language | Directory | Status | Performance | Installation |
|----------|-----------|--------|-------------|--------------|
| **Rust** | [../graphlite-sdk](../graphlite-sdk/) | ✅ Stable | ~100% native | `cargo add graphlite-sdk` |
| **Python** | [python/](python/) | ✅ Stable | ~80-90% native | `pip install graphlite` |
| **Java** | [java/](java/) | ✅ Stable | ~75-85% native | Maven/Gradle |
| **JavaScript** | javascript/ | ⏳ Planned | ~70-80% native (WASM) | `npm install graphlite` |
| **Kotlin** | kotlin/ | ⏳ Planned | ~75-85% native | Maven/Gradle |

## Quick Start by Language

### Rust (Recommended)

The Rust SDK provides zero-overhead access with a high-level API:

```rust
use graphlite_sdk::GraphLite;

let db = GraphLite::open("./mydb")?;
let session = db.session("admin")?;
let result = session.query("MATCH (n:Person) RETURN n")?;
```

**See**: [graphlite-sdk/README.md](../graphlite-sdk/README.md)

### Python

Python bindings use ctypes for FFI access:

```python
from graphlite import GraphLite

db = GraphLite("./mydb")
session = db.create_session("admin")
result = db.query(session, "MATCH (n:Person) RETURN n")
```

**See**: [python/README.md](python/README.md)

### Java

Java bindings use JNA (Java Native Access):

```java
import com.deepgraph.graphlite.GraphLite;

try (GraphLite db = GraphLite.open("./mydb")) {
    String session = db.createSession("admin");
    QueryResult result = db.query(session, "MATCH (n:Person) RETURN n");
}
```

**See**: [java/README.md](java/README.md)

## Architecture

All language bindings follow the same architecture:

```
┌─────────────────────────────────────┐
│   Your Application                  │
│   (Python, Java, JavaScript, etc.)  │
└─────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   Language Binding                  │
│   (Python: ctypes, Java: JNA)       │
└─────────────────────────────────────┘
              │ FFI calls
              ▼
┌─────────────────────────────────────┐
│   graphlite-ffi                     │
│   (C-compatible API)                │
│   - JSON serialization              │
│   - Panic safety                    │
│   - Memory management               │
└─────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   GraphLite Core                    │
│   (Rust implementation)             │
└─────────────────────────────────────┘
```

### Why FFI?

**FFI (Foreign Function Interface)** allows languages to call C-compatible functions:
- **Universal** - Almost all languages can call C functions
- **Mature** - Well-established, battle-tested approach
- **Performance** - Minimal overhead compared to other IPC methods
- **No Server** - Embedded database, no network calls

## Performance Comparison

| Language | FFI Library | Overhead | Throughput |
|----------|-------------|----------|------------|
| Rust SDK | N/A (direct) | 0% | 100% |
| Python | ctypes | 10-20% | 80-90% |
| Java | JNA | 15-25% | 75-85% |
| JavaScript | WASM | 20-30% | 70-80% |

**Note**: Performance varies based on query complexity and data size.

## Prerequisites

All bindings require the **FFI library** to be built first:

```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

This creates:
- **macOS**: `target/release/libgraphlite_ffi.dylib`
- **Linux**: `target/release/libgraphlite_ffi.so`
- **Windows**: `target/release/graphlite_ffi.dll`

## Installation by Language

### Python

```bash
cd bindings/python
pip install -e .
```

**See**: [python/README.md](python/README.md)

### Java

```bash
cd bindings/java
mvn clean install
```

**See**: [java/README.md](java/README.md)

## API Consistency

All language bindings provide the same conceptual API:

| Operation | Rust | Python | Java |
|-----------|------|--------|------|
| **Open DB** | `GraphLite::open(path)` | `GraphLite(path)` | `GraphLite.open(path)` |
| **Session** | `db.session(user)` | `db.create_session(user)` | `db.createSession(user)` |
| **Query** | `session.query(gql)` | `db.query(session, gql)` | `db.query(session, gql)` |
| **Execute** | `session.execute(gql)` | `db.execute(session, gql)` | `db.execute(session, gql)` |
| **Close** | `db.close()` (auto) | `db.close()` | `db.close()` (try-with-resources) |

## Examples

Each binding includes comprehensive examples:

- **Rust**: [graphlite-sdk/examples/](../graphlite-sdk/examples/)
- **Python**: [python/examples/](python/examples/)
- **Java**: [java/examples/](java/examples/)

## Building Bindings from Source

### Python

```bash
cd bindings/python
pip install -e ".[dev]"
pytest  # Run tests
```

### Java

```bash
cd bindings/java
mvn clean install
mvn test  # Run tests
```

## Distribution

### Python Package (PyPI)

```bash
cd bindings/python
python setup.py sdist bdist_wheel
twine upload dist/*
```

### Java Package (Maven Central)

```bash
cd bindings/java
mvn clean deploy
```

## Choosing a Language Binding

### Choose Rust SDK if:
- ✅ Maximum performance required
- ✅ Type safety at compile time
- ✅ Building a Rust application
- ✅ Need zero-overhead abstractions

### Choose Python if:
- ✅ Rapid development priority
- ✅ Data science / ML integration
- ✅ Scripting and automation
- ✅ Familiar with Python ecosystem

### Choose Java if:
- ✅ Enterprise Java application
- ✅ Spring Boot integration
- ✅ Android development
- ✅ JVM ecosystem (Scala, Kotlin, Groovy)

### Choose JavaScript if:
- ✅ Web application (browser or Node.js)
- ✅ TypeScript support needed
- ✅ React/Vue/Angular integration
- ✅ Server-side JavaScript

## Contributing

We welcome contributions to language bindings!

### Adding a New Language

1. Implement wrapper around FFI functions
2. Handle error codes appropriately
3. Manage memory (free strings, close handles)
4. Add JSON deserialization for results
5. Create examples and tests
6. Document API

See existing bindings (Python, Java) as reference implementations.

### Improving Existing Bindings

- Add more examples
- Improve error messages
- Add type hints (Python) or generics (Java)
- Performance optimizations
- Better documentation

## Testing

Each binding should include:
- Unit tests for API surface
- Integration tests with actual database
- Error handling tests
- Memory leak tests
- Performance benchmarks

## Documentation

Each binding includes:
- README.md with installation and usage
- API reference documentation
- Code examples
- Troubleshooting guide

## Support

For issues with specific bindings:
- **Python**: [python/README.md#troubleshooting](python/README.md)
- **Java**: [java/README.md#troubleshooting](java/README.md)
- **General**: [GitHub Issues](https://github.com/deepgraph/graphlite/issues)

## License

All bindings are licensed under Apache-2.0 - See [LICENSE](../LICENSE) for details.
