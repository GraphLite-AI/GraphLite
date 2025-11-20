"""
GraphLite SDK - High-level Python API for GraphLite

This package provides a high-level, developer-friendly SDK on top of GraphLite's core API.
It offers ergonomic patterns, type safety, session management, query builders, and
transaction support - everything needed to build robust graph-based applications in Python.

Quick Start
-----------

```python
from graphlite_sdk import GraphLite

# Open database
db = GraphLite.open("./mydb")

# Create session
session = db.session("admin")

# Execute query
result = session.query("MATCH (p:Person) RETURN p.name")

# Use transactions
with session.transaction() as tx:
    tx.execute("CREATE (p:Person {name: 'Alice'})")
    tx.commit()
```

Architecture
-----------

```
Your Application
       │
       ▼
┌─────────────────────────────────────────┐
│  GraphLite SDK (this package)           │
│  - GraphLite (main API)                 │
│  - Session (session management)         │
│  - Transaction (ACID support)           │
│  - QueryBuilder (fluent queries)        │
│  - TypedResult (deserialization)        │
└─────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────┐
│  GraphLite FFI Bindings                 │
│  (Low-level ctypes wrapper)             │
└─────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────┐
│  GraphLite Core (Rust)                  │
│  - QueryCoordinator                     │
│  - Storage Engine                       │
│  - Catalog Manager                      │
└─────────────────────────────────────────┘
```
"""

from .error import GraphLiteError
from .connection import GraphLite, Session
from .transaction import Transaction
# from .query import QueryBuilder
# from .result import TypedResult

__version__ = "0.1.0"

__all__ = [
    # Main API
    "GraphLite",
    "Session",

    # Transaction support
    "Transaction",

    # Query building
    # "QueryBuilder",

    # Result handling
    # "TypedResult",

    # Errors
    "GraphLiteError",

    # Version
    "__version__",
]
