# GraphLite Python Bindings

Python bindings for GraphLite embedded graph database using FFI (ctypes).

## Installation

### Prerequisites

1. **Build the FFI library** first:
   ```bash
   cd /path/to/GraphLite
   cargo build --release -p graphlite-ffi
   ```

2. **Install Python package**:
   ```bash
   cd bindings/python
   pip install -e .
   ```

## Quick Start

```python
from graphlite import GraphLite

# Open database
db = GraphLite("./mydb")

# Create session
session = db.create_session("admin")

# Execute queries
db.execute(session, "CREATE (p:Person {name: 'Alice', age: 30})")

# Query data
result = db.query(session, "MATCH (p:Person) RETURN p.name, p.age")
for row in result.rows:
    print(row)

# Close
db.close()
```

## Usage

### Opening a Database

```python
from graphlite import GraphLite

# Open database (creates if doesn't exist)
db = GraphLite("./mydb")

# Or use as context manager (auto-closes)
with GraphLite("./mydb") as db:
    # ... use database
    pass  # Automatically closed
```

### Creating Sessions

```python
# Create session for user
session_id = db.create_session("username")

# Use session for queries
result = db.query(session_id, "MATCH (n) RETURN n")

# Close session when done
db.close_session(session_id)
```

### Executing Statements

```python
# DDL statements
db.execute(session, "CREATE SCHEMA myschema")
db.execute(session, "USE SCHEMA myschema")
db.execute(session, "CREATE GRAPH social")
db.execute(session, "USE GRAPH social")

# DML statements
db.execute(session, "CREATE (p:Person {name: 'Alice', age: 30})")
db.execute(session, "CREATE (p:Person {name: 'Bob', age: 25})")
```

### Querying Data

```python
# Simple query
result = db.query(session, "MATCH (p:Person) RETURN p.name, p.age")

# Access result properties
print(f"Found {result.row_count} rows")
print(f"Columns: {result.variables}")

# Iterate rows
for row in result.rows:
    print(f"Name: {row['p.name']}, Age: {row['p.age']}")

# Get first row
first_row = result.first()
if first_row:
    print(first_row)

# Get column values
names = result.column('p.name')
print(names)  # ['Alice', 'Bob']
```

### Complex Queries

```python
# WHERE clause
result = db.query(
    session,
    "MATCH (p:Person) WHERE p.age > 25 RETURN p.name, p.age"
)

# ORDER BY
result = db.query(
    session,
    "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age DESC"
)

# Aggregation
result = db.query(
    session,
    "MATCH (p:Person) RETURN count(p) as total, avg(p.age) as avg_age"
)
stats = result.first()
print(f"Total: {stats['total']}, Average: {stats['avg_age']}")
```

### Error Handling

```python
from graphlite import GraphLite, GraphLiteError, ErrorCode

try:
    db = GraphLite("./mydb")
    session = db.create_session("admin")
    result = db.query(session, "MATCH (n) RETURN n")

except GraphLiteError as e:
    print(f"Error: {e}")
    print(f"Error code: {e.code}")

    if e.code == ErrorCode.QUERY_ERROR:
        print("Query syntax error")
    elif e.code == ErrorCode.DATABASE_OPEN_ERROR:
        print("Cannot open database")
```

### Context Manager Pattern

```python
# Recommended: automatic cleanup
with GraphLite("./mydb") as db:
    session = db.create_session("admin")

    db.execute(session, "CREATE (p:Person {name: 'Alice'})")
    result = db.query(session, "MATCH (p:Person) RETURN p")

    for row in result.rows:
        print(row)

# Database automatically closed
```

## API Reference

### GraphLite

Main database class.

#### Methods

- `__init__(path: str)` - Open database at path
- `create_session(username: str) -> str` - Create session, returns session ID
- `query(session_id: str, query: str) -> QueryResult` - Execute query, returns results
- `execute(session_id: str, statement: str) -> None` - Execute statement without results
- `close_session(session_id: str) -> None` - Close a session
- `close() -> None` - Close database
- `version() -> str` - Get GraphLite version (static method)

### QueryResult

Query result wrapper.

#### Properties

- `variables: List[str]` - Column names from RETURN clause
- `rows: List[Dict[str, Any]]` - List of result rows
- `row_count: int` - Number of rows

#### Methods

- `first() -> Optional[Dict[str, Any]]` - Get first row or None
- `column(name: str) -> List[Any]` - Get all values from a column
- `to_dict() -> Dict[str, Any]` - Get raw dictionary

### GraphLiteError

Exception raised for GraphLite errors.

#### Properties

- `code: ErrorCode` - Error code enum
- `message: str` - Error message

### ErrorCode

Error code enumeration.

#### Values

- `SUCCESS = 0` - Operation succeeded
- `NULL_POINTER = 1` - Null pointer error
- `INVALID_UTF8 = 2` - Invalid UTF-8 string
- `DATABASE_OPEN_ERROR = 3` - Failed to open database
- `SESSION_ERROR = 4` - Session operation failed
- `QUERY_ERROR = 5` - Query execution failed
- `PANIC_ERROR = 6` - Internal panic
- `JSON_ERROR = 7` - JSON parsing error

## Examples

Run the basic example:

```bash
# Make sure FFI library is built
cargo build --release -p graphlite-ffi

# Run example
python examples/basic_usage.py
```

## Performance Considerations

- **FFI Overhead**: ~10-20% overhead compared to Rust SDK
- **JSON Serialization**: Results are serialized to JSON across FFI boundary
- **Session Reuse**: Create sessions once and reuse for better performance
- **Batch Operations**: Execute multiple statements in a transaction when possible

## Troubleshooting

### "Could not find GraphLite library"

Build the FFI library first:
```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

The Python bindings look for the library in:
- `target/release/libgraphlite_ffi.{so,dylib,dll}`
- `target/debug/libgraphlite_ffi.{so,dylib,dll}`
- `/usr/local/lib/`
- `/usr/lib/`

### "Session error"

Make sure to create a session before querying:
```python
session = db.create_session("username")
result = db.query(session, "MATCH (n) RETURN n")
```

### Import errors

Make sure the package is installed:
```bash
cd bindings/python
pip install -e .
```

## Development

### Running Tests

```bash
pip install -e ".[dev]"
pytest
```

### Code Formatting

```bash
black graphlite/
```

### Type Checking

```bash
mypy graphlite/
```

## Comparison with Rust SDK

| Feature | Python (via FFI) | Rust SDK |
|---------|------------------|----------|
| **Performance** | ~80-90% of native | ~100% native |
| **Ease of Use** | Very easy | Easy |
| **Type Safety** | Runtime (dicts) | Compile-time |
| **Installation** | pip install | Cargo dependency |
| **Overhead** | FFI + JSON serialization | None |

## License

Apache-2.0 - See [LICENSE](../../LICENSE) for details.
