# Using GraphLite as a Crate

**Embed GraphLite directly in your Rust applications - no build required!**

GraphLite is available as Rust crates on [crates.io](https://crates.io/search?q=graphlite), making it easy to add graph database capabilities to your application without cloning and building the entire repository.

## Available Crates

GraphLite provides three crates on crates.io:

1. **[graphlite](https://crates.io/crates/graphlite)** - Core library for embedding in applications
2. **[graphlite-rust-sdk](https://crates.io/crates/graphlite-rust-sdk)** - High-level ergonomic SDK (recommended)
3. **[gql-cli](https://crates.io/crates/gql-cli)** - Command-line interface tool

## Table of Contents

1. [Quick Start](#quick-start)
2. [Installation](#installation)
3. [Usage Options](#usage-options)
4. [Complete Example](#complete-example)
5. [Installing the CLI from crates.io](#installing-the-cli-from-cratesio)
6. [Next Steps](#next-steps)

---

## Quick Start

**Get GraphLite running in your Rust app in 2 steps:**

### Step 1: Add GraphLite to Your Project

**Option A: Using the SDK (Recommended)**
```bash
# Using cargo add (recommended)
cargo add graphlite-rust-sdk

# Or manually add to Cargo.toml
# graphlite-rust-sdk = "0.0.1"
```

**Option B: Using the Core Library**
```bash
# Using cargo add
cargo add graphlite

# Or manually add to Cargo.toml
# graphlite = "0.0.1"
```

### Step 2: Use in Your Code

```rust
use graphlite::QueryCoordinator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "./my_app_db";

    // Initialize database
    let coordinator = QueryCoordinator::from_path(db_path)?;

    // Set admin password
    coordinator.set_user_password("admin", "my_secure_password")?;

    // Create session
    let session_id = coordinator.create_simple_session("admin")?;

    // Create schema and graph
    coordinator.process_query("CREATE SCHEMA IF NOT EXISTS /myschema", &session_id)?;
    coordinator.process_query("CREATE GRAPH IF NOT EXISTS /myschema/social", &session_id)?;
    coordinator.process_query("SESSION SET GRAPH /myschema/social", &session_id)?;

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

    // Process results
    for row in &result.rows {
        println!("Name: {:?}, Age: {:?}",
            row.values.get("p.name"),
            row.values.get("p.age")
        );
    }

    // Close session
    coordinator.close_session(&session_id)?;

    Ok(())
}
```

That's it! No need to clone the repository or build from source.

---

## Installation

Choose the installation method based on your use case:

### For Application Development (SDK - Recommended)

Add the high-level SDK to your `Cargo.toml`:

**Using Cargo Command:**
```bash
cargo add graphlite-rust-sdk
```

**Manual Addition:**
```toml
[dependencies]
graphlite-rust-sdk = "0.0.1"
```

### For Advanced/Low-Level Usage (Core Library)

Add the core library to your `Cargo.toml`:

**Using Cargo Command:**
```bash
cargo add graphlite
```

**Manual Addition:**
```toml
[dependencies]
graphlite = "0.0.1"
```

### For CLI Tool

Install the CLI globally:

```bash
cargo install gql-cli
```

Then build your project:
```bash
cargo build
```

---

## Usage Options

GraphLite provides two APIs for different use cases:

### Option 1: SDK (Recommended for Most Applications)

The **GraphLite SDK** provides a high-level, ergonomic API similar to SQLite:

```rust
use graphlite_sdk::GraphLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open database (SQLite-style API)
    let db = GraphLite::open("./myapp_db")?;

    // Create session
    let session = db.session("user")?;

    // Execute commands
    session.execute("CREATE SCHEMA myschema")?;
    session.execute("USE SCHEMA myschema")?;
    session.execute("CREATE GRAPH social")?;
    session.execute("USE GRAPH social")?;

    // Use transactions
    let mut tx = session.transaction()?;
    tx.execute("INSERT (:Person {name: 'Alice'})")?;
    tx.commit()?;

    // Query data
    let result = session.query("MATCH (p:Person) RETURN p.name")?;

    Ok(())
}
```


**See also:** [SDK Examples](../sdk-rust/examples/) and [SDK README](../sdk-rust/README.md)

### Option 2: Core Library (Advanced)

The **core library** provides direct access to the `QueryCoordinator` for advanced use cases:

```rust
use graphlite::QueryCoordinator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database from path
    let coordinator = QueryCoordinator::from_path("./myapp_db")?;

    // Create session
    let session_id = coordinator.create_simple_session("user")?;

    // Execute queries
    coordinator.process_query("CREATE SCHEMA /myschema", &session_id)?;
    coordinator.process_query("CREATE GRAPH /myschema/social", &session_id)?;
    coordinator.process_query("SESSION SET GRAPH /myschema/social", &session_id)?;

    // Insert data
    coordinator.process_query(
        "INSERT (:Person {name: 'Alice'})",
        &session_id
    )?;

    // Query data
    let result = coordinator.process_query(
        "MATCH (p:Person) RETURN p.name",
        &session_id
    )?;

    // Process results
    for row in &result.rows {
        println!("Name: {:?}", row.values.get("p.name"));
    }

    Ok(())
}
```


**See also:** [Examples](../examples/) - Includes SDK (high-level) and bindings (low-level) examples

---

## Complete Example

Here's a complete working example demonstrating common operations:

```rust
use graphlite::QueryCoordinator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "./my_app_db";

    // 1. Initialize database (creates all files and components)
    let coordinator = QueryCoordinator::from_path(db_path)?;

    // 2. Set admin user password
    // The 'admin' user is created automatically during initialization
    coordinator.set_user_password("admin", "my_secure_password")?;

    // 3. Create a session
    let session_id = coordinator.create_simple_session("admin")?;

    // 4. Create schema and graph
    coordinator.process_query("CREATE SCHEMA IF NOT EXISTS /myschema", &session_id)?;
    coordinator.process_query("CREATE GRAPH IF NOT EXISTS /myschema/social", &session_id)?;
    coordinator.process_query("SESSION SET GRAPH /myschema/social", &session_id)?;

    // 5. Insert data
    coordinator.process_query(
        "INSERT (:Person {name: 'Alice', age: 30}),
                (:Person {name: 'Bob', age: 25}),
                (:Person {name: 'Carol', age: 28})",
        &session_id
    )?;

    // 6. Create relationships
    coordinator.process_query(
        "MATCH (alice:Person {name: 'Alice'}), (bob:Person {name: 'Bob'})
         INSERT (alice)-[:KNOWS {since: '2020-01-15'}]->(bob)",
        &session_id
    )?;

    // 7. Query data
    let result = coordinator.process_query(
        "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age",
        &session_id
    )?;

    // 8. Process results
    println!("\nPeople in database:");
    for row in &result.rows {
        println!("  Name: {:?}, Age: {:?}",
            row.values.get("p.name"),
            row.values.get("p.age")
        );
    }

    // 9. Find relationships
    let result = coordinator.process_query(
        "MATCH (a:Person)-[k:KNOWS]->(b:Person)
         RETURN a.name AS from, b.name AS to, k.since",
        &session_id
    )?;

    println!("\nRelationships:");
    for row in &result.rows {
        println!("  {:?} knows {:?} (since {:?})",
            row.values.get("from"),
            row.values.get("to"),
            row.values.get("k.since")
        );
    }

    // 10. Close session when done
    coordinator.close_session(&session_id)?;

    Ok(())
}
```

**Expected output:**
```
People in database:
  Name: String("Bob"), Age: Integer(25)
  Name: String("Carol"), Age: Integer(28)
  Name: String("Alice"), Age: Integer(30)

Relationships:
  String("Alice") knows String("Bob") (since String("2020-01-15"))
```

---

## Installing the CLI from crates.io

You can install the GraphLite CLI tool directly from crates.io without cloning the repository:

```bash
# Install graphlite CLI globally
cargo install gql-cli
```

This will install the `graphlite` binary to your Cargo bin directory (usually `~/.cargo/bin/`).

**Verify installation:**
```bash
graphlite --version
```

**Use the CLI:**
```bash
# Initialize a database
graphlite install --path ./my_db --admin-user admin --admin-password secret

# Start the interactive REPL
graphlite gql --path ./my_db -u admin -p secret
```

**Benefits of cargo install:**
- No need to clone the repository
- Automatic PATH setup (if Cargo bin is in PATH)
- Easy updates with `cargo install gql-cli --force`
- Works on any system with Rust installed

**See also:** [Quick Start.md](Quick%20Start.md) for detailed CLI usage guide.

---

## Next Steps

### Learn GQL Query Language

**[Getting Started With GQL.md](Getting%20Started%20With%20GQL.md)** - Complete GQL tutorial covering:
- Advanced pattern matching
- Aggregations and grouping
- String and date/time functions
- Sorting and pagination
- Complex graph traversals

### Explore Code Examples

**SDK Examples (Recommended):**
- [basic_usage.rs](../sdk-rust/examples/basic_usage.rs) - Complete SDK walkthrough
- [query_builder.rs](../sdk-rust/examples/query_builder.rs) - Query builder patterns

**More Examples:**
- [Rust Examples](../examples/rust/) - SDK and bindings examples
- [Python Examples](../examples/python/) - Python SDK and bindings
- [Java Examples](../examples/java/) - Java bindings


## Getting Help

- **Issues**: [GitHub Issues](https://github.com/GraphLite-AI/GraphLite/issues)
- **Contributing**: [CONTRIBUTING.md](../CONTRIBUTING.md)
