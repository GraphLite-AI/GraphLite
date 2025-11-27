# GraphLite Python SDK

High-level, ergonomic Python SDK for GraphLite - the fast, embedded graph database.

## Overview

The GraphLite Python SDK provides a developer-friendly API for working with GraphLite databases in Python applications. It wraps the low-level FFI bindings with a clean, intuitive interface following patterns from popular embedded databases like SQLite.

## Features

- **Simple API** - Clean, Pythonic interface following SQLite conventions
- **Session Management** - User context and permissions support
- **Transactions** - ACID guarantees with automatic rollback (context managers)
- **Query Builder** - Fluent API for constructing GQL queries
- **Typed Results** - Deserialize query results into Python dataclasses
- **Zero External Dependencies** - Fully embedded, no server required
- **Type Hints** - Full type annotation support for better IDE integration

## Installation

```bash
# Install from source (for now)
cd sdk-python
pip install -e .
```

## Quick Start

Basic usage:

```python
from graphlite_sdk import GraphLite

# Open database
db = GraphLite.open("./mydb")

# Create session
session = db.session("admin")

# Execute query
result = session.query("MATCH (p:Person) RETURN p.name")

for row in result.rows:
    print(row)
```

## Core Concepts

### Opening a Database

GraphLite is an embedded database - no server required. Just open a directory:

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./mydb")
```

This creates or opens a database at the specified path.

### Sessions

Unlike SQLite, GraphLite uses sessions for user context and permissions:

```python
session = db.session("username")
```

Sessions provide:
- User authentication and authorization
- Transaction isolation
- Audit logging

### Executing Queries

Simple query execution:

```python
result = session.query("MATCH (n:Person) RETURN n")
```

Or for statements that don't return results:

```python
session.execute("INSERT (p:Person {name: 'Alice'})")
```

### Transactions

Transactions use Python context managers with automatic rollback:

```python
# Transaction with explicit commit
with session.transaction() as tx:
    tx.execute("INSERT (p:Person {name: 'Alice'})")
    tx.execute("INSERT (p:Person {name: 'Bob'})")
    tx.commit()  # Persist changes

# Transaction with automatic rollback
with session.transaction() as tx:
    tx.execute("INSERT (p:Person {name: 'Charlie'})")
    # tx exits without commit - changes are automatically rolled back
```

### Query Builder

Build queries fluently:

```python
result = (session.query_builder()
    .match_pattern("(p:Person)")
    .where_clause("p.age > 25")
    .return_clause("p.name, p.age")
    .order_by("p.age DESC")
    .limit(10)
    .execute())
```

### Typed Results

Deserialize results into Python dataclasses:

```python
from dataclasses import dataclass
from graphlite_sdk import TypedResult

@dataclass
class Person:
    name: str
    age: int

result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")
typed = TypedResult(result)
people = typed.deserialize_rows(Person)

for person in people:
    print(f"{person.name} is {person.age} years old")
```

## Examples

### Basic CRUD Operations

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./mydb")
session = db.session("admin")

# Create schema and graph
session.execute("CREATE SCHEMA IF NOT EXISTS /example")
session.execute("SESSION SET SCHEMA /example")
session.execute("CREATE GRAPH IF NOT EXISTS social")
session.execute("SESSION SET GRAPH social")

# Create nodes
session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
session.execute("INSERT (p:Person {name: 'Bob', age: 25})")

# Create relationships
session.execute("""
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    INSERT (a)-[:KNOWS]->(b)
""")

# Query
result = session.query("""
    MATCH (p:Person)-[:KNOWS]->(f:Person)
    RETURN p.name as person, f.name as friend
""")

for row in result.rows:
    print(f"{row['person']} knows {row['friend']}")
```

### Transaction Example

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./mydb")
session = db.session("admin")

with session.transaction() as tx:
    # Delete old relationship
    tx.execute("MATCH (a)-[r:FOLLOWS]->(b) WHERE a.name = 'Alice' DELETE r")

    # Create new relationship
    tx.execute("""
        MATCH (a {name: 'Alice'}), (c {name: 'Charlie'})
        INSERT (a)-[:FOLLOWS]->(c)
    """)

    tx.commit()
```

### Query Builder Example

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./mydb")
session = db.session("admin")

result = (session.query_builder()
    .match_pattern("(u:User)")
    .where_clause("u.status = 'active'")
    .where_clause("u.lastLogin > '2024-01-01'")
    .return_clause("u.name, u.email")
    .order_by("u.lastLogin DESC")
    .limit(20)
    .execute())

for row in result.rows:
    print(f"{row['name']}: {row['email']}")
```

### Typed Deserialization Example

```python
from dataclasses import dataclass
from graphlite_sdk import GraphLite, TypedResult

@dataclass
class User:
    name: str
    email: str
    age: int

db = GraphLite.open("./mydb")
session = db.session("admin")

result = session.query(
    "MATCH (u:User) RETURN u.name as name, u.email as email, u.age as age"
)

typed = TypedResult(result)
users = typed.deserialize_rows(User)

for user in users:
    print(f"{user.name} ({user.email}): {user.age}")
```

### Scalar and First Methods

```python
from dataclasses import dataclass
from graphlite_sdk import TypedResult

# Get a scalar value (single value from first row, first column)
result = session.query("MATCH (p:Person) RETURN count(p)")
typed = TypedResult(result)
count = typed.scalar()
print(f"Total persons: {count}")

# Get first row as typed object
@dataclass
class Count:
    count: int

result = session.query("MATCH (p:Person) RETURN count(p) as count")
typed = TypedResult(result)
count_obj = typed.first(Count)
print(f"Total persons: {count_obj.count}")
```

## API Comparison with SQLite

GraphLite SDK follows similar patterns to Python's sqlite3 but adapted for graph databases:

| Operation | sqlite3 (SQLite) | sdk-rust (GraphLite) |
|-----------|------------------|---------------------------|
| Open DB | `sqlite3.connect()` | `GraphLite.open()` |
| Execute | `conn.execute()` | `session.execute()` |
| Query | `conn.execute()` | `session.query()` |
| Transaction | `with conn:` | `with session.transaction():` |
| Commit | `conn.commit()` | `tx.commit()` |
| Rollback | `conn.rollback()` | `tx.rollback()` or auto |

**Key Differences:**
- GraphLite uses **sessions** for user context (SQLite doesn't have sessions)
- GraphLite uses **GQL** (Graph Query Language) instead of SQL
- GraphLite is optimized for **graph data** (nodes, edges, paths)

## Architecture

```text
Your Application
       
       

  GraphLite SDK (this package)           
  - GraphLite (main API)                   ← You are here
  - Session (session management)         
  - Transaction (ACID support)           
  - QueryBuilder (fluent queries)        
  - TypedResult (deserialization)        

       
       

  GraphLite FFI Bindings                 
  (Low-level ctypes wrapper)             

       
       

  GraphLite Core (Rust)                  
  - QueryCoordinator                     
  - Storage Engine                       
  - Catalog Manager                      

```

## Language Bindings

The GraphLite Python SDK is specifically for **Python applications**. For other languages:

- **Rust** - Use `../sdk-rust/` (native Rust SDK, zero-overhead)
- **Swift** - Use `bindings/swift/` (via FFI)
- **Java** - Use `bindings/java/` (via FFI)
- **JavaScript/Node.js** - Use `bindings/javascript/` (via FFI/WASM)
- **Go** - Use `bindings/go/` (via FFI)
- **WASM** - Use `bindings/wasm/` (for browser/web)

See the main [MULTI_LANGUAGE_BINDINGS_DESIGN.md](../MULTI_LANGUAGE_BINDINGS_DESIGN.md) for details.

## Performance

GraphLite Python SDK provides good performance via FFI:
- Direct ctypes FFI calls (minimal overhead)
- JSON serialization only for query results
- Python-native data structures

Benchmark comparison:
- **Rust SDK**: ~100% of native performance
- **Python SDK** (via FFI): ~80-90% of native
- **JavaScript bindings** (via WASM): ~70-80% of native

## Error Handling

The SDK uses a comprehensive error hierarchy:

```python
from graphlite_sdk import (
    GraphLiteError,      # Base error class
    ConnectionError,     # Database connection errors
    SessionError,        # Session management errors
    QueryError,          # Query execution errors
    TransactionError,    # Transaction errors
    SerializationError,  # Deserialization errors
)

try:
    db = GraphLite.open("./mydb")
    session = db.session("admin")
    result = session.query("MATCH (n) RETURN n")
except ConnectionError as e:
    print(f"Failed to connect: {e}")
except QueryError as e:
    print(f"Query failed: {e}")
except GraphLiteError as e:
    print(f"GraphLite error: {e}")
```

## Examples

Run the examples:

```bash
# Basic usage example
python3 examples/basic_usage.py

# More examples coming soon
```

## Development

To work on the SDK:

```bash
# Install in development mode
cd sdk-python
pip install -e .

# Run tests (coming soon)
pytest

# Type checking
mypy src/
```

## API Reference

### GraphLite

```python
class GraphLite:
    @classmethod
    def open(cls, path: str) -> GraphLite

    def session(self, username: str) -> Session

    def close(self) -> None
```

### Session

```python
class Session:
    def query(self, query: str) -> QueryResult

    def execute(self, statement: str) -> None

    def transaction(self) -> Transaction

    def query_builder(self) -> QueryBuilder
```

### Transaction

```python
class Transaction:
    def execute(self, statement: str) -> None

    def query(self, query: str) -> QueryResult

    def commit(self) -> None

    def rollback(self) -> None
```

### QueryBuilder

```python
class QueryBuilder:
    def match_pattern(self, pattern: str) -> QueryBuilder

    def where_clause(self, condition: str) -> QueryBuilder

    def with_clause(self, clause: str) -> QueryBuilder

    def return_clause(self, clause: str) -> QueryBuilder

    def order_by(self, clause: str) -> QueryBuilder

    def skip(self, n: int) -> QueryBuilder

    def limit(self, n: int) -> QueryBuilder

    def build(self) -> str

    def execute(self) -> QueryResult
```

### TypedResult

```python
class TypedResult:
    def row_count(self) -> int

    def column_names(self) -> List[str]

    def get_row(self, index: int) -> Optional[Dict[str, Any]]

    def deserialize_rows(self, target_type: Type[T]) -> List[T]

    def deserialize_row(self, row: Dict[str, Any], target_type: Type[T]) -> T

    def first(self, target_type: Type[T]) -> T

    def scalar(self) -> Any

    def is_empty(self) -> bool

    def rows(self) -> List[Dict[str, Any]]
```

## Contributing

Contributions welcome! Areas where help is needed:

- **ORM Features** - Decorators for mapping classes to graph nodes
- **Query Validation** - Runtime query validation
- **Async Support** - asyncio integration
- **Connection Pooling** - Multi-threaded access patterns
- **Graph Algorithms** - Built-in graph algorithms (shortest path, centrality, etc.)
- **Documentation** - More examples and tutorials

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.
