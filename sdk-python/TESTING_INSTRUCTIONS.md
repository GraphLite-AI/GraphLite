# Testing Instructions for GraphLite Python SDK

This document provides step-by-step instructions to test the GraphLite Python SDK from scratch, following the README documentation.

## Prerequisites

1. **Python 3.8+** installed (tested with Python 3.13)
2. **GraphLite repository** cloned locally
3. **Low-level bindings** installed (the `graphlite` package from `bindings/python/`)

## Step 1: Install the SDK

```bash
# Navigate to the SDK directory
cd sdk-python

# Install the SDK in editable mode
pip install -e .
# OR if using Python 3.13 specifically:
/opt/local/bin/python3.13 -m pip install -e .
```

## Step 2: Verify Installation

```bash
# Test that the SDK can be imported
python3 -c "from graphlite_sdk import GraphLite; print('✓ SDK installed successfully')"
```

## Step 3: Run the Comprehensive Test Suite

A comprehensive test script has been created that tests all README examples:

```bash
# From the GraphLite root directory or sdk-python directory
python3 sdk-python/test_readme_examples.py
```

This will run 13 tests covering:
- ✓ Quick Start Example
- ✓ Opening a Database
- ✓ Sessions
- ✓ Executing Queries
- ✓ Transactions
- ✓ Query Builder
- ✓ Typed Results
- ✓ Basic CRUD Operations
- ✓ Transaction Example
- ✓ Query Builder Example
- ✓ Typed Deserialization Example
- ✓ Scalar and First Methods
- ✓ Error Handling

## Step 4: Manual Testing (Optional)

You can also test individual features manually:

### Quick Start Test

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./test_db")
session = db.session("admin")

# Setup schema and graph (required)
session.execute("CREATE SCHEMA IF NOT EXISTS /example")
session.execute("SESSION SET SCHEMA /example")
session.execute("CREATE GRAPH IF NOT EXISTS social")
session.execute("SESSION SET GRAPH social")

# Insert data
session.execute("INSERT (p:Person {name: 'Alice', age: 30})")

# Query
result = session.query("MATCH (p:Person) RETURN p.name as name")
for row in result.rows:
    print(row)
```

### Test Transactions

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./test_db")
session = db.session("admin")

# Setup (same as above)
session.execute("CREATE SCHEMA IF NOT EXISTS /example")
session.execute("SESSION SET SCHEMA /example")
session.execute("CREATE GRAPH IF NOT EXISTS social")
session.execute("SESSION SET GRAPH social")

# Transaction with commit
with session.transaction() as tx:
    tx.execute("INSERT (p:Person {name: 'Alice'})")
    tx.commit()  # Changes persist

# Transaction with rollback
with session.transaction() as tx:
    tx.execute("INSERT (p:Person {name: 'Bob'})")
    # No commit - automatically rolls back
```

### Test Query Builder

```python
from graphlite_sdk import GraphLite

db = GraphLite.open("./test_db")
session = db.session("admin")

# Setup (same as above)
# ... schema/graph setup ...

# Insert test data
session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
session.execute("INSERT (p:Person {name: 'Bob', age: 25})")

# Use query builder
result = (session.query_builder()
    .match_pattern("(p:Person)")
    .where_clause("p.age > 25")
    .return_clause("p.name, p.age")
    .order_by("p.age DESC")
    .limit(10)
    .execute())

for row in result.rows:
    print(row)
```

### Test Typed Results

```python
from dataclasses import dataclass
from graphlite_sdk import GraphLite, TypedResult

@dataclass
class Person:
    name: str
    age: int

db = GraphLite.open("./test_db")
session = db.session("admin")

# Setup and insert data...
result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")
typed = TypedResult(result)
people = typed.deserialize_rows(Person)

for person in people:
    print(f"{person.name} is {person.age} years old")
```

## Step 5: Test Error Handling

```python
from graphlite_sdk import (
    GraphLite,
    GraphLiteError,
    ConnectionError,
    SessionError,
    QueryError,
    TransactionError,
    SerializationError,
)

try:
    db = GraphLite.open("./test_db")
    session = db.session("admin")
    result = session.query("INVALID QUERY")
except QueryError as e:
    print(f"Query error caught: {e}")
except GraphLiteError as e:
    print(f"GraphLite error: {e}")
```

## Common Issues and Solutions

### Issue: `ImportError: cannot import name 'GraphLite' from 'graphlite'`

**Solution:** Make sure the low-level bindings are installed:
```bash
cd bindings/python
pip install -e .
```

### Issue: `ModuleNotFoundError: No module named 'graphlite_sdk'`

**Solution:** Make sure the SDK is installed:
```bash
cd sdk-python
pip install -e .
```

### Issue: Query fails with "Query failed"

**Solution:** Make sure you've set up the schema and graph:
```python
session.execute("CREATE SCHEMA IF NOT EXISTS /example")
session.execute("SESSION SET SCHEMA /example")
session.execute("CREATE GRAPH IF NOT EXISTS social")
session.execute("SESSION SET GRAPH social")
```

### Issue: `CREATE GRAPH IF NOT EXISTS` fails on second run

**Solution:** This is expected if the graph already exists. The error can be safely ignored, or wrap in try/except:
```python
try:
    session.execute("CREATE GRAPH IF NOT EXISTS social")
except QueryError:
    pass  # Graph already exists
```

## Expected Test Results

When running `test_readme_examples.py`, you should see:

```
============================================================
TEST 1: Quick Start Example
============================================================
✓ Quick Start test passed

============================================================
TEST 2: Opening a Database
============================================================
✓ Database opened at /tmp/...

... (all 13 tests should pass) ...

============================================================
ALL TESTS COMPLETED
============================================================
```

All tests should pass with ✓ checkmarks. If any test fails, check the error message and refer to the "Common Issues" section above.

## Notes

- The test script uses temporary directories, so no cleanup is needed
- Each test is independent and creates its own database
- The tests follow the exact examples from the README
- Some tests may show warnings about existing graphs - this is expected and handled gracefully

