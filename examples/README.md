# GraphLite Examples

This directory contains examples for using GraphLite across different programming languages and API levels.

## Directory Structure

```
examples/
├── rust/
│   ├── bindings/      # Core library examples (low-level API)
│   │   ├── simple_usage.rs
│   │   └── drug_discovery/
│   └── sdk/           # SDK examples (high-level ergonomic API)
│       ├── basic_usage.rs
│       └── drug_discovery/
├── python/
│   ├── bindings/      # FFI bindings examples (low-level)
│   │   ├── basic_usage.py
│   │   └── drug_discovery.py
│   └── sdk/           # High-level SDK examples
│       └── drug_discovery.py
└── java/
    └── bindings/      # Java bindings examples
        └── BasicUsage.java
```

## Quick Start by Language

### Rust Examples

#### Core Library (Bindings)
Low-level API with direct access to GraphLite core:
```bash
# Simple usage
cargo run --example simple_usage -p graphlite

# Drug discovery (comprehensive)
cargo run --example drug_discovery -p graphlite
```

#### Rust SDK
High-level ergonomic API:
```bash
# Basic usage
cargo run --example basic_usage -p graphlite-rust-sdk

# Drug discovery
cargo run --example drug_discovery -p graphlite-rust-sdk
```

### Python Examples

#### FFI Bindings
Low-level Python bindings via ctypes:
```bash
cd examples/python/bindings

# Basic usage
python3 basic_usage.py

# Drug discovery
python3 drug_discovery.py
```

#### High-Level SDK
Session-centric Pythonic API:
```bash
cd examples/python/sdk

# Drug discovery
python3 drug_discovery.py
```

### Java Examples

```bash
cd examples/java/bindings
# Compile and run (instructions in BasicUsage.java)
```

## Example Descriptions

### Simple/Basic Usage
Demonstrates fundamental operations:
- Opening a database
- Creating sessions
- Executing queries
- Basic CRUD operations

### Drug Discovery
Comprehensive pharmaceutical research example showing:
- **Domain**: Proteins (disease targets), Compounds (drugs), Assays (experiments)
- **Relationships**: TESTED_IN, MEASURES_ACTIVITY_ON, INHIBITS
- **Data**: IC50 measurements, clinical trial phases
- **Queries**: Filtering, traversal, aggregation, sorting

## API Level Comparison

| Aspect | Bindings (Low-Level) | SDK (High-Level) |
|--------|---------------------|------------------|
| **Abstraction** | Direct core access | Ergonomic wrapper |
| **Verbosity** | More code required | Concise |
| **Control** | Fine-grained | Simplified |
| **Use Case** | Advanced users | Recommended start |

## Language-Specific Documentation

- [Rust Bindings Examples](./rust/bindings/README.md)
- [Rust SDK Examples](./rust/sdk/README.md)
- [Python Bindings](./python/bindings/README.md)
- [Python SDK](./python/sdk/README.md)

## Prerequisites

### Rust
```bash
cargo build --release
```

### Python
```bash
# Build FFI library
cargo build --release -p graphlite-ffi

# Install Python bindings
cd bindings/python
pip install -e .
```

### Java
```bash
# Build JNI library
cargo build --release -p graphlite-jni
```

## Contributing

When adding new examples:
1. Place in appropriate language/api-level directory
2. Follow naming convention: `example_name.{rs,py,java}`
3. Include inline documentation
4. Add entry to this README
5. Test across all supported platforms

## Related Documentation

- [GraphLite Core Documentation](../README.md)
- [Rust SDK Documentation](../graphlite-sdk/README.md)
- [Python Bindings Documentation](../bindings/python/README.md)
- [Java Bindings Documentation](../bindings/java/README.md)
