# GraphLite SDK

High-level, ergonomic Rust SDK for GraphLite - the fast, embedded graph database.

## Overview

The GraphLite SDK provides a developer-friendly API for working with GraphLite databases in Rust applications. It follows patterns from popular embedded databases like SQLite (rusqlite) while providing graph-specific features.

## Features

- **Simple API** - Clean, intuitive interface following SQLite/rusqlite conventions
- **Session Management** - User context and permissions support
- **Transactions** - ACID guarantees with automatic rollback (RAII pattern)
- **Query Builder** - Fluent API for constructing GQL queries
- **Typed Results** - Deserialize query results into Rust structs
- **Zero External Dependencies** - Fully embedded, no server required
- **Connection Pooling** - Efficient concurrent access (future)
- **Async Support** - Full tokio integration (future)

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
sdk-rust = "0.1"
```

Basic usage:

```rust
use graphlite_sdk::{GraphLite, Error};

fn main() -> Result<(), Error> {
    // Open database
    let db = GraphLite::open("./mydb")?;

    // Create session
    let session = db.session("admin")?;

    // Execute query
    let result = session.query("MATCH (p:Person) RETURN p.name")?;

    for row in result.rows {
        println!("{:?}", row);
    }

    Ok(())
}
```

## Core Concepts

### Opening a Database

GraphLite is an embedded database - no server required. Just open a directory:

```rust
let db = GraphLite::open("./mydb")?;
```

This creates or opens a database at the specified path.

### Sessions

Unlike SQLite, GraphLite uses sessions for user context and permissions:

```rust
let session = db.session("username")?;
```

Sessions provide:
- User authentication and authorization
- Transaction isolation
- Audit logging

### Executing Queries

Simple query execution:

```rust
let result = session.query("MATCH (n:Person) RETURN n")?;
```

Or for statements that don't return results:

```rust
session.execute("INSERT (:Person {name: 'Alice'})")?;
```

### Transactions

Transactions follow the rusqlite pattern with automatic rollback:

```rust
// Transaction with explicit commit
let mut tx = session.transaction()?;
tx.execute("INSERT (:Person {name: 'Alice'}), (:Person {name: 'Bob'})")?;
tx.commit()?;  // Persist changes

// Transaction with automatic rollback
{
    let mut tx = session.transaction()?;
    tx.execute("INSERT (:Person {name: 'Charlie'})")?;
    // tx is dropped here - changes are automatically rolled back
}
```

### Query Builder

Build queries fluently:

```rust
let result = session.query_builder()
    .match_pattern("(p:Person)")
    .where_clause("p.age > 25")
    .return_clause("p.name, p.age")
    .order_by("p.age DESC")
    .limit(10)
    .execute()?;
```

### Typed Results

Deserialize results into Rust structs:

```rust
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
}

let result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")?;
let typed = TypedResult::from(result);
let people: Vec<Person> = typed.deserialize_rows()?;
```

## Examples

### Basic CRUD Operations

```rust
use graphlite_sdk::GraphLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = GraphLite::open("./mydb")?;
    let session = db.session("admin")?;

    // Create schema and graph
    session.execute("CREATE SCHEMA example")?;
    session.execute("USE SCHEMA example")?;
    session.execute("CREATE GRAPH social")?;
    session.execute("USE GRAPH social")?;

    // Create nodes (multiple in one INSERT statement)
    session.execute("INSERT (:Person {name: 'Alice', age: 30}), (:Person {name: 'Bob', age: 25})")?;

    // Create relationships
    session.execute(
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
         CREATE (a)-[:KNOWS]->(b)"
    )?;

    // Query
    let result = session.query(
        "MATCH (p:Person)-[:KNOWS]->(f:Person)
         RETURN p.name as person, f.name as friend"
    )?;

    for row in result.rows {
        println!("{:?}", row);
    }

    Ok(())
}
```

### Transaction Example

```rust
use graphlite_sdk::GraphLite;

fn transfer_relationship(db: &GraphLite) -> Result<(), Box<dyn std::error::Error>> {
    let session = db.session("admin")?;

    let mut tx = session.transaction()?;

    // Delete old relationship
    tx.execute("MATCH (a)-[r:FOLLOWS]->(b) WHERE a.name = 'Alice' DELETE r")?;

    // Create new relationship
    tx.execute(
        "MATCH (a {name: 'Alice'}), (c {name: 'Charlie'})
         CREATE (a)-[:FOLLOWS]->(c)"
    )?;

    tx.commit()?;
    Ok(())
}
```

### Query Builder Example

```rust
use graphlite_sdk::GraphLite;

fn find_active_users(db: &GraphLite) -> Result<(), Box<dyn std::error::Error>> {
    let session = db.session("admin")?;

    let result = session.query_builder()
        .match_pattern("(u:User)")
        .where_clause("u.status = 'active'")
        .where_clause("u.lastLogin > date('2024-01-01')")
        .with_clause("u, count(u.posts) as post_count")
        .where_clause("post_count > 10")
        .return_clause("u.name, u.email, post_count")
        .order_by("post_count DESC")
        .limit(20)
        .execute()?;

    for row in result.rows {
        println!("{:?}", row);
    }

    Ok(())
}
```

### Typed Deserialization Example

```rust
use graphlite_sdk::{GraphLite, TypedResult};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct User {
    name: String,
    email: String,
    age: u32,
}

fn get_users(db: &GraphLite) -> Result<Vec<User>, Box<dyn std::error::Error>> {
    let session = db.session("admin")?;

    let result = session.query(
        "MATCH (u:User) RETURN u.name as name, u.email as email, u.age as age"
    )?;

    let typed = TypedResult::from(result);
    let users = typed.deserialize_rows::<User>()?;

    Ok(users)
}
```

## API Comparison with SQLite

GraphLite SDK follows similar patterns to rusqlite but adapted for graph databases:

| Operation | rusqlite (SQLite) | sdk-rust (GraphLite) |
|-----------|-------------------|---------------------------|
| Open DB | `Connection::open()` | `GraphLite::open()` |
| Execute | `conn.execute()` | `session.execute()` |
| Query | `conn.query_row()` | `session.query()` |
| Transaction | `conn.transaction()?` | `session.transaction()?` |
| Commit | `tx.commit()?` | `tx.commit()?` |
| Rollback | `tx.rollback()?` or drop | `tx.rollback()?` or drop |

**Key Differences:**
- GraphLite uses **sessions** for user context (SQLite doesn't have sessions)
- GraphLite uses **GQL** (Graph Query Language) instead of SQL
- GraphLite is optimized for **graph data** (nodes, edges, paths)

## Architecture

```text
Your Application
       │
       ▼
┌─────────────────────────┐
│   GraphLite SDK         │
│   - GraphLite           │  ← You are here
│   - Session             │
│   - Transaction         │
│   - QueryBuilder        │
│   - TypedResult         │
└─────────────────────────┘
       │
       ▼
┌─────────────────────────┐
│   GraphLite Core        │
│   - QueryCoordinator    │
│   - Storage Engine      │
│   - Catalog Manager     │
└─────────────────────────┘
```

## Language Bindings

The GraphLite SDK is specifically for **Rust applications**. For other languages:

- **Python** - Use `bindings/python/` (via FFI)
- **Java** - Use `bindings/java/` (via JNI)
- **JavaScript/Node.js** - Use `bindings/javascript/` (via FFI/WASM)
- **Kotlin** - Use `bindings/kotlin/` (via JNI)

See the main [MULTI_LANGUAGE_BINDINGS_DESIGN.md](../MULTI_LANGUAGE_BINDINGS_DESIGN.md) for details.

## Performance

GraphLite SDK provides **zero-overhead** abstractions:
- Direct Rust function calls (no FFI overhead)
- No serialization for query results (unlike language bindings)
- Compile-time optimizations
- Same performance as using the core library directly

Benchmark comparison:
- **Rust SDK**: ~100% of native performance
- **Python bindings** (via FFI): ~80-90% of native
- **JavaScript bindings** (via WASM): ~70-80% of native

## Documentation

- [API Documentation](https://docs.rs/sdk-rust)
- [Examples](examples-core/)
- [GraphLite Core](../graphlite/)
- [Language Bindings](../bindings/)

## Examples

Run the examples:

```bash
# Basic usage example
cargo run --example basic_usage

# More examples coming soon
```

## Contributing

Contributions welcome! Areas where help is needed:

- **ORM Features** - Derive macros for mapping structs to graph nodes
- **Query Macros** - Compile-time query validation
- **Async Support** - Full tokio integration
- **Connection Pooling** - Multi-threaded access patterns
- **Graph Algorithms** - Built-in graph algorithms (shortest path, centrality, etc.)

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.
