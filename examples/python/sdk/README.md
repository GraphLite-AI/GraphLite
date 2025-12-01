# GraphLite Python High-Level SDK Examples

This repository contains examples using the GraphLite High-Level Python SDK (from the `python-sdk` branch).

## Overview

The High-Level SDK provides an ergonomic, Rust SDK-like API for GraphLite with:
- Session-centric API (session objects instead of session IDs)
- Typed exception hierarchy
- Cleaner, more Pythonic interface
- (Planned) Transaction support with context managers
- (Planned) Query builder for fluent query construction
- (Planned) Typed result deserialization

## Architecture

```
Your Application
      ↓
GraphLite SDK (python-sdk/src/)
      ↓
GraphLite FFI Adapter (graphlite_ffi.py)
      ↓
GraphLite FFI Bindings (bindings/python/)
      ↓
libgraphlite_ffi.so (Rust)
```

## Setup

### Prerequisites

1. **Build the GraphLite FFI library**:
   ```bash
   cd ~/github/graphlite-ai/GraphLite
   cargo build --release -p graphlite-ffi
   ```

2. **Install the low-level bindings**:
   ```bash
   cd ~/github/graphlite-ai/GraphLite/bindings/python
   pip install --break-system-packages -e .
   ```

3. **Python SDK Dependency**

   The high-level Python SDK is currently in the `deepgraphai/GraphLite` repository on the `python-sdk` branch:

   ```bash
   # Clone and checkout python-sdk branch
   cd ~/github/deepgraphai
   git clone https://github.com/deepgraphai/GraphLite.git  # if not already cloned
   cd GraphLite
   git checkout python-sdk
   ```

   The examples will automatically find the SDK at `~/github/deepgraphai/GraphLite/python-sdk/`

   Alternatively, edit the `drug_discovery.py` file to update the path to your python-sdk location.

## Examples

### Drug Discovery Example

A comprehensive pharmaceutical research example demonstrating:
- Modeling proteins (disease targets), compounds (drugs), and assays (experiments)
- Creating relationships: TESTED_IN, MEASURES_ACTIVITY_ON, INHIBITS
- Real-world data: IC50 measurements, clinical trial phases
- Analytical queries: IC50 filtering, pathway traversal, aggregation

**Run:**
```bash
python3 drug_discovery.py
```

## API Differences from Low-Level Bindings

### Low-Level FFI Bindings
```python
from graphlite import GraphLite

db = GraphLite("./mydb")
session_id = db.create_session("admin")
result = db.query(session_id, "MATCH (n) RETURN n")
```

### High-Level SDK (This Example)
```python
from src.connection import GraphLite

db = GraphLite.open("./mydb")
session = db.session("admin")
result = session.query("MATCH (n) RETURN n")
```

**Key Differences**:
1. Use `.open()` static method instead of constructor
2. Session object with methods instead of session ID strings
3. Cleaner, session-centric API
4. Typed exceptions (ConnectionError, SessionError, QueryError, etc.)

## Requirements

- Python 3.8+
- GraphLite FFI library (built from GraphLite repository)
- GraphLite FFI Python bindings

## Current Status

The High-Level SDK is under development:
- Database connection (GraphLite class)
- Session management (Session class)
- Query execution
- Typed error hierarchy
- Transaction support (planned)
- Query builder (planned)
- Typed result deserialization (planned)

Currently, the SDK uses the FFI bindings' QueryResult class for results. Planned features will add transaction context managers, query builders, and dataclass deserialization.

## Domain Model (Drug Discovery)

```
Compound → TESTED_IN → Assay → MEASURES_ACTIVITY_ON → Target (Protein)
Compound → INHIBITS → Target (with IC50 measurements)
```

**Use Cases**: Target-based drug discovery, compound optimization, clinical trial tracking, pharmaceutical knowledge graphs.
