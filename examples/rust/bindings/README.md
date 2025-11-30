# GraphLite Core Library Examples (Advanced)

⚠️ **Note**: These examples use the **low-level core library** (`graphlite` crate) directly. For most applications, we recommend using the **high-level SDK** instead.

👉 **For application developers**: See [graphlite-sdk/examples/](../graphlite-sdk/examples/) for the recommended SDK examples.

This directory contains **advanced examples** showing direct usage of the GraphLite core library (`QueryCoordinator`). Use these if you need:
- Direct access to internal components
- Custom integration beyond the SDK
- Deep understanding of GraphLite internals

## Examples Overview

### 1. simple_usage.rs

**Complete example showing the recommended way to embed GraphLite in your application.**

This example demonstrates:
- Initializing a GraphLite database from a path
- Creating sessions for query execution
- Validating and analyzing queries before execution
- Creating schemas and graphs
- Inserting and querying data
- Properly displaying results

**Run the example:**
```bash
cargo run --example simple_usage
```

**Expected output:**
```
=== GraphLite Simple Usage Example ===

1. Initializing database...
   ✓ Database initialized

2. Creating session...
   ✓ Session created: <session-id>

3. Validating and analyzing queries...
   → Validating query...
     ✓ Query is valid
     ✓ Query syntax check passed
   → Analyzing query...
     ✓ Query type: Select
     ✓ Read-only: true
   → Testing invalid query...
     ✓ Correctly detected invalid query
   → Explaining query execution plan...
     ✓ Query Plan: ...

4. Executing queries...
   → Creating schema...
     ✓ Schema created
   → Setting schema...
     ✓ Schema set
   → Creating graph...
     ✓ Graph created
   → Setting graph...
     ✓ Graph set
   → Inserting nodes...
     ✓ Nodes inserted
   → Querying data...
     ✓ Query executed

5. Results:
   Columns: ["p.name", "p.age"]
   Row count: 2
   Row 1: {"p.name": String("Bob"), "p.age": Integer(25)}
   Row 2: {"p.name": String("Alice"), "p.age": Integer(30)}

6. Closing session...
   ✓ Session closed

=== Example Complete ===
```

## Building Examples

All examples can be built using cargo:

```bash
# Build all examples
cargo build --examples

# Build a specific example
cargo build --example simple_usage

# Run an example
cargo run --example simple_usage
```

## Using GraphLite in Your Application

The examples follow the recommended pattern for embedding GraphLite:

### Step 1: Add GraphLite as a Dependency

**Advanced users** - Add the core library to your `Cargo.toml`:
```toml
[dependencies]
graphlite = "0.1.0"
```

**💡 Recommended** - Use the high-level SDK instead:
```toml
[dependencies]
graphlite-sdk = "0.1.0"
```
See [graphlite-sdk/examples/](../graphlite-sdk/examples/) for SDK usage.

### Step 2: Initialize the Database

```rust
use graphlite::QueryCoordinator;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize database - handles all internal component setup
    let coordinator = QueryCoordinator::from_path("./myapp_db")?;

    // Create a session for this user
    let session_id = coordinator.create_simple_session("username")?;

    // Ready to execute queries!
    Ok(())
}
```

### Step 3: Execute Queries

```rust
// Create schema and graph
coordinator.process_query("CREATE SCHEMA /myschema", &session_id)?;
coordinator.process_query("CREATE GRAPH /myschema/mygraph", &session_id)?;
coordinator.process_query("SESSION SET GRAPH /myschema/mygraph", &session_id)?;

// Insert data
coordinator.process_query(
    "INSERT (:Person {name: 'Alice', age: 30})",
    &session_id
)?;

// Query data
let result = coordinator.process_query(
    "MATCH (p:Person) RETURN p.name, p.age",
    &session_id
)?;

// Access results
for row in &result.rows {
    println!("Name: {:?}", row.values.get("p.name"));
    println!("Age: {:?}", row.values.get("p.age"));
}
```

## Public API Reference

### QueryCoordinator

The main entry point for GraphLite. All operations go through this coordinator.

**Initialization:**
- `QueryCoordinator::from_path(path)` - Initialize from a database path

**Session Management:**
- `create_simple_session(username)` - Create a session for query execution
- `close_session(session_id)` - Close a session when done

**Query Operations:**
- `process_query(query, session_id)` - Execute a GQL query
- `validate_query(query)` - Validate query syntax
- `is_valid_query(query)` - Check if query is valid
- `analyze_query(query)` - Get query analysis information
- `explain_query(query)` - Get query execution plan

**Return Types:**
- `QueryResult` - Contains `rows`, `variables`, `execution_time_ms`, `rows_affected`
- `Row` - Contains `values` (HashMap of variable names to values)

## Query Language

GraphLite implements ISO GQL (Graph Query Language). See the [GQL Guide](../GQL-GUIDE.md) for complete language reference.

**Basic patterns:**
```gql
-- Create schema and graph
CREATE SCHEMA /myschema;
CREATE GRAPH /myschema/social;
SESSION SET GRAPH /myschema/social;

-- Insert nodes
INSERT (:Person {name: 'Alice', age: 30});

-- Insert relationships
INSERT (:Person {name: 'Alice'})-[:KNOWS]->(:Person {name: 'Bob'});

-- Query with pattern matching
MATCH (p:Person)
WHERE p.age > 25
RETURN p.name, p.age
ORDER BY p.age DESC;

-- Graph traversal
MATCH (a:Person)-[:KNOWS]->(b:Person)
WHERE a.name = 'Alice'
RETURN b.name;
```

## Database Files

GraphLite creates an embedded database directory:

```
myapp_db/
├── catalog/          # Catalog metadata
├── graphs/           # Graph data
├── wal/              # Write-ahead log
└── [sled files]      # Embedded storage files
```

The database directory is created automatically when you initialize with `from_path()`.

## Error Handling

All GraphLite operations return `Result` types. Handle errors appropriately:

```rust
match coordinator.process_query(query, &session_id) {
    Ok(result) => {
        println!("Query succeeded: {} rows", result.rows.len());
    }
    Err(e) => {
        eprintln!("Query failed: {}", e);
    }
}
```

## Clean Up

After running examples, you can clean up the database files:

```bash
# Remove example database
rm -rf ./example_db/
```

## Advanced Usage

For more advanced use cases, see:
- [Main README](../README.md) - GraphLite overview
- [GQL Guide](../GQL-GUIDE.md) - Complete query language reference
- [CLI Documentation](../graphlite-cli/README.md) - Interactive console usage

## Contributing Examples

We welcome contributions of additional examples! Examples we'd love to see:
- Transaction handling
- Concurrent access patterns
- Index usage and optimization
- Integration with web frameworks (Actix, Axum)
- Desktop/mobile application integration
- Performance benchmarking

See [Contributing](../README.md#contributing) for guidelines.

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.
