# GraphLite Python SDK Examples

This repository contains examples for using the GraphLite Python SDK.

## Setup

The GraphLite Python SDK uses the GraphLite FFI library. Make sure GraphLite is built first:

```bash
# Build GraphLite (from the GraphLite repository)
cd ~/github/deepgraphai/GraphLite
cargo build --release -p graphlite-ffi
```

The Python bindings will automatically find the library in the GraphLite build directory.

## Examples

### Drug Discovery Example (Recommended Start)

A comprehensive pharmaceutical research example demonstrating:
- Modeling proteins (disease targets), compounds (drugs), and assays (experiments)
- Creating relationships: TESTED_IN, MEASURES_ACTIVITY_ON, INHIBITS
- Real-world data: IC50 measurements, clinical trial phases
- Analytical queries: IC50 filtering, pathway traversal, aggregation

**Run:**
```bash
python3 drug_discovery.py
```

### Basic Examples

- `examples/basic_usage.py` - Basic graph operations
- `examples/advanced_usage.py` - Advanced features and queries

## Requirements

- Python 3.8+
- GraphLite FFI library (built from GraphLite repository)

## Domain Model (Drug Discovery)

```
Compound → TESTED_IN → Assay → MEASURES_ACTIVITY_ON → Target (Protein)
Compound → INHIBITS → Target (with IC50 measurements)
```

**Use Cases:** Target-based drug discovery, compound optimization, clinical trial tracking, pharmaceutical knowledge graphs.
