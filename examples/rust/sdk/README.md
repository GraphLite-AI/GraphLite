# GraphLite SDK Examples (Recommended)

✅ **These examples show the recommended way to use GraphLite in your Rust applications.**

This directory contains examples using the **high-level GraphLite SDK** - an ergonomic, SQLite-inspired API for embedded graph databases.

## Why Use the SDK?

The SDK provides:
- 🎯 **Ergonomic API** - Clean, intuitive interface following rusqlite patterns
- 🔒 **Transaction Safety** - RAII pattern with automatic rollback
- 🏗️ **Query Builder** - Fluent API for constructing queries
- 📦 **Typed Results** - Deserialize into Rust structs
- ⚡ **Zero Overhead** - Direct Rust calls, no FFI

## Examples Overview

### 1. basic_usage.rs

**Complete example demonstrating all SDK features.**

This example shows:
- Opening a database (`GraphLite::open()`)
- Creating sessions for user context
- Executing DDL statements (CREATE SCHEMA, CREATE GRAPH)
- Using transactions with ACID guarantees
- Query builder API for fluent query construction
- Typed result deserialization with serde
- Transaction rollback behavior (RAII pattern)

**Run the example:**
```bash
cargo run --example basic_usage
```

**Expected output:**
```
=== GraphLite SDK Basic Usage Example ===

1. Opening database...
   ✓ Database opened at /tmp/graphlite_sdk_example

2. Creating session...
   ✓ Session created for user 'admin'

3. Creating schema and graph...
   ✓ Schema and graph created

4. Inserting data with transaction...
   ✓ Inserted 3 persons

5. Querying data...
   Found 3 persons:
   - Name: String("Alice"), Age: Number(30.0)
   - Name: String("Bob"), Age: Number(25.0)
   - Name: String("Charlie"), Age: Number(35.0)

6. Using query builder...
   Found 2 persons over 25:
   - Name: String("Charlie"), Age: Number(35.0)
   - Name: String("Alice"), Age: Number(30.0)

7. Using typed deserialization...
   Deserialized 3 persons:
   - Person { name: "Alice", age: 30.0 }
   - Person { name: "Bob", age: 25.0 }
   - Person { name: "Charlie", age: 35.0 }

8. Demonstrating transaction rollback...
   Created person 'David' in transaction
   Transaction rolled back (David not persisted)

   Person count after rollback: Number(3.0)

=== Example completed successfully ===
```

## Quick Start

### Step 1: Add SDK to Your Project

Add to your `Cargo.toml`:
```toml
[dependencies]
graphlite-sdk = "0.1"
serde = { version = "1.0", features = ["derive"] }  # For typed results
```

### Step 2: Basic Usage

```rust
use graphlite_sdk::{GraphLite, Error};

fn main() -> Result<(), Error> {
    // Open database (creates if doesn't exist)
    let db = GraphLite::open("./mydb")?;

    // Create session for user
    let session = db.session("admin")?;

    // Execute query
    let result = session.query("MATCH (n:Person) RETURN n")?;

    // Process results
    for row in result.rows {
        println!("{:?}", row);
    }

    Ok(())
}
```

### Step 3: Use Transactions

```rust
use graphlite_sdk::GraphLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = GraphLite::open("./mydb")?;
    let session = db.session("admin")?;

    // Transaction with explicit commit
    let mut tx = session.transaction()?;
    tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    tx.execute("CREATE (p:Person {name: 'Bob'})")?;
    tx.commit()?;  // Persist changes

    // Transaction with auto-rollback (RAII)
    {
        let mut tx = session.transaction()?;
        tx.execute("CREATE (p:Person {name: 'Charlie'})")?;
        // tx is dropped here - changes automatically rolled back
    }

    Ok(())
}
```

### Step 4: Query Builder

```rust
use graphlite_sdk::GraphLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = GraphLite::open("./mydb")?;
    let session = db.session("admin")?;

    // Build query fluently
    let result = session.query_builder()
        .match_pattern("(p:Person)")
        .where_clause("p.age > 25")
        .return_clause("p.name, p.age")
        .order_by("p.age DESC")
        .limit(10)
        .execute()?;

    println!("Found {} people", result.rows.len());
    Ok(())
}
```

### Step 5: Typed Deserialization

```rust
use graphlite_sdk::{GraphLite, TypedResult};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = GraphLite::open("./mydb")?;
    let session = db.session("admin")?;

    let result = session.query(
        "MATCH (p:Person) RETURN p.name as name, p.age as age"
    )?;

    // Deserialize into Rust structs
    let typed = TypedResult::from(result);
    let people: Vec<Person> = typed.deserialize_rows()?;

    for person in people {
        println!("{:?}", person);
    }

    Ok(())
}
```

## API Comparison: SDK vs Core Library

| Feature | Core Library (Advanced) | SDK (Recommended) |
|---------|------------------------|-------------------|
| **Initialization** | `QueryCoordinator::from_path()` | `GraphLite::open()` |
| **Session** | `create_simple_session()` | `db.session("user")` |
| **Query** | `process_query(query, session_id)` | `session.query(query)` |
| **Transaction** | Manual BEGIN/COMMIT | `session.transaction()?` with RAII |
| **Query Builder** | ❌ Not available | ✅ `session.query_builder()` |
| **Typed Results** | ❌ Manual deserialization | ✅ `TypedResult::deserialize_rows()` |
| **Error Handling** | `String` errors | Rich `Error` enum |
| **Ease of Use** | Low-level, verbose | High-level, ergonomic |

**When to use Core Library**: See [../examples/](../examples/) for advanced/internal usage

## Building and Running Examples

```bash
# Build all SDK examples
cargo build --examples -p graphlite-sdk

# Run an example
cargo run --example basic_usage

# Or from workspace root
cargo run -p graphlite-sdk --example basic_usage
```

## Common Patterns

### Pattern 1: CRUD Operations

```rust
let db = GraphLite::open("./mydb")?;
let session = db.session("admin")?;

// Setup
session.execute("CREATE SCHEMA app")?;
session.execute("USE SCHEMA app")?;
session.execute("CREATE GRAPH social")?;
session.execute("USE GRAPH social")?;

// Create
session.execute("CREATE (p:Person {name: 'Alice', age: 30})")?;

// Read
let result = session.query("MATCH (p:Person) RETURN p")?;

// Update
session.execute("MATCH (p:Person {name: 'Alice'}) SET p.age = 31")?;

// Delete
session.execute("MATCH (p:Person {name: 'Alice'}) DELETE p")?;
```

### Pattern 2: Batch Operations with Transactions

```rust
let db = GraphLite::open("./mydb")?;
let session = db.session("admin")?;

let mut tx = session.transaction()?;

for i in 0..1000 {
    tx.execute(&format!(
        "CREATE (p:Person {{id: {}, name: 'Person{}'}})",
        i, i
    ))?;
}

tx.commit()?;  // All or nothing
```

### Pattern 3: Conditional Updates

```rust
let db = GraphLite::open("./mydb")?;
let session = db.session("admin")?;

let result = session.query(
    "MATCH (p:Person {name: 'Alice'}) RETURN count(p) as count"
)?;

let typed = TypedResult::from(result);
if let Ok(count) = typed.scalar::<i64>() {
    if count > 0 {
        session.execute("MATCH (p:Person {name: 'Alice'}) SET p.verified = true")?;
    }
}
```

## Advanced Features

### Custom Error Handling

```rust
use graphlite_sdk::Error;

match session.query("MATCH (n) RETURN n") {
    Ok(result) => println!("Success: {} rows", result.rows.len()),
    Err(Error::Query(msg)) => eprintln!("Query error: {}", msg),
    Err(Error::Connection(msg)) => eprintln!("Connection error: {}", msg),
    Err(e) => eprintln!("Other error: {}", e),
}
```

### Transaction Drop Behavior

```rust
use graphlite_sdk::transaction::DropBehavior;

let mut tx = session.transaction()?;

// Change drop behavior (default is Rollback)
tx.set_drop_behavior(DropBehavior::Panic);  // Panic if not committed

tx.execute("CREATE (p:Person {name: 'Alice'})")?;
tx.commit()?;  // Must commit or will panic
```

## Documentation

- [SDK README](../README.md) - Full SDK documentation
// - [API Docs](https://docs.rs/graphlite-sdk) - Complete API reference
- [Bindings Examples](../bindings/) - Low-level bindings examples
- [GQL Guide](../../GQL-GUIDE.md) - Graph Query Language reference

## Performance Tips

1. **Use Transactions** - Batch operations for better performance
2. **Query Builder** - Leverage compile-time optimizations
3. **Typed Results** - Zero-cost deserialization with serde
4. **Connection Reuse** - Open database once, create multiple sessions

## Contributing

We welcome example contributions! Examples we'd love to see:
- Integration with web frameworks (Actix, Axum, Rocket)
- Connection pooling patterns
- Async usage (when supported)
- ORM-style patterns with derive macros
- Graph algorithms (shortest path, centrality)
- Full-text search integration
- Time-series data patterns

See [Contributing](../../README.md#contributing) for guidelines.

## License

Apache-2.0 - See [LICENSE](../../LICENSE) for details.
