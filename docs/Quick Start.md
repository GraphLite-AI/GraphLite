# GraphLite Quick Start Guide

Get GraphLite running and execute your first graph queries in **5 minutes**!

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
3. [Initialize Your First Database](#initialize-your-first-database)
4. [Start the REPL](#start-the-repl)
5. [Run Your First Queries](#run-your-first-queries)
6. [Next Steps](#next-steps)

---

## Prerequisites

**Required:**
- **Rust 1.70 or later** - Install from [rustup.rs](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

**Optional:**
- Git - For cloning the repository

---

## Installation

### Option 1: Build from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/GraphLite-AI/GraphLite.git
cd GraphLite

# Build in release mode (optimized)
cargo build --release
```

**Build time**: ~2-5 minutes on first build

After building, the binary will be available at `target/release/graphlite`.

### Option 2: Using the Build Script

GraphLite includes a comprehensive build script that simplifies the build process:

```bash
# Basic build (debug mode, default)
./scripts/build_all.sh

# Optimized release build (2-10x faster binaries) - RECOMMENDED
./scripts/build_all.sh --release

# Clean build (removes previous artifacts)
./scripts/build_all.sh --clean --release

# Build and run tests to verify installation
./scripts/build_all.sh --release --test

# Show help
./scripts/build_all.sh --help
```

**Benefits of the build script**:
- Automatically detects and adds Rust/Cargo to PATH if needed
- Release builds are optimized for production use (significantly faster execution)
- Builds both the library and CLI binary in one command
- Optional clean build support (`--clean` flag)
- Optional test execution after build (`--test` flag)
- Colored output with build summary and next steps

**Build output locations**:
- **Debug mode**: `target/debug/libgraphlite.rlib` and `target/debug/graphlite`
- **Release mode**: `target/release/libgraphlite.rlib` and `target/release/graphlite`

---

## Initialize Your First Database

Create a new GraphLite database with an admin user:

```bash
./target/release/graphlite install --path ./my_db --admin-user admin --admin-password secret
```

**What this does:**
- Creates the database files in `./my_db` directory
- Sets up the admin user with the specified password
- Creates default admin and user roles
- Initializes the default schema

**Expected output:**
```
Database installed successfully at ./my_db
Admin user 'admin' created
Default roles created: admin, user
Default schema initialized
```

---

## Start the REPL

Launch the interactive GraphLite console:

```bash
./target/release/graphlite gql --path ./my_db -u admin -p secret
```

**You should see:**
```
GraphLite v0.1.0 - ISO GQL Interactive Console
Connected to: ./my_db
User: admin
Type 'help' for help, 'exit' to quit

gql>
```

---

## Run Your First Queries

Now let's create a simple social network graph and run some queries!

### Step 1: Create Schema and Graph

```gql
-- Create a schema to organize your graphs
CREATE SCHEMA /social;

-- Set the session to use this schema
SESSION SET SCHEMA /social;

-- Create a graph for our social network
CREATE GRAPH /social/network;

-- Set the session to use this graph
SESSION SET GRAPH /social/network;
```

**Expected output:** Success messages for each command

### Step 2: Insert Some Data

```gql
-- Create people (multiple nodes in one INSERT statement)
INSERT (:Person {name: 'Alice', age: 30, city: 'New York'}),
       (:Person {name: 'Bob', age: 25, city: 'San Francisco'}),
       (:Person {name: 'Carol', age: 28, city: 'Chicago'});

-- Create friendships
MATCH (alice:Person {name: 'Alice'}), (bob:Person {name: 'Bob'})
INSERT (alice)-[:KNOWS {since: '2020-01-15'}]->(bob);

MATCH (bob:Person {name: 'Bob'}), (carol:Person {name: 'Carol'})
INSERT (bob)-[:KNOWS {since: '2021-06-20'}]->(carol);
```

**Expected output:** Row counts showing successful insertions

### Step 3: Query the Data

```gql
-- Find all people
MATCH (p:Person)
RETURN p.name, p.age, p.city;
```

**Expected output:**
```
+-------+-----+---------------+
| name  | age | city          |
+-------+-----+---------------+
| Alice | 30  | New York      |
| Bob   | 25  | San Francisco |
| Carol | 28  | Chicago       |
+-------+-----+---------------+
3 rows
```

```gql
-- Find who Alice knows
MATCH (alice:Person {name: 'Alice'})-[:KNOWS]->(friend)
RETURN friend.name, friend.city;
```

**Expected output:**
```
+------+---------------+
| name | city          |
+------+---------------+
| Bob  | San Francisco |
+------+---------------+
1 row
```

```gql
-- Find friends of friends (2-hop path)
MATCH (p:Person {name: 'Alice'})-[:KNOWS]->(friend)-[:KNOWS]->(fof)
RETURN fof.name AS friend_of_friend;
```

**Expected output:**
```
+------------------+
| friend_of_friend |
+------------------+
| Carol            |
+------------------+
1 row
```

### Step 4: Try Aggregations

```gql
-- Count people by city
MATCH (p:Person)
RETURN p.city, COUNT(p) AS population
GROUP BY p.city
ORDER BY population DESC;
```

**Expected output:**
```
+---------------+------------+
| city          | population |
+---------------+------------+
| New York      | 1          |
| San Francisco | 1          |
| Chicago       | 1          |
+---------------+------------+
3 rows
```

---

## CLI Quick Reference

### Essential Commands

```bash
# Show help
./target/release/graphlite --help

# Show version
./target/release/graphlite version

# Initialize database
./target/release/graphlite install --path ./db --admin-user admin --admin-password pwd

# Start REPL
./target/release/graphlite gql --path ./db -u admin -p pwd

# Execute single query
./target/release/graphlite query "MATCH (n) RETURN n" --path ./db -u admin -p pwd

# Enable debug logging
./target/release/graphlite -v gql --path ./db -u admin -p pwd
./target/release/graphlite --log-level debug gql --path ./db -u admin -p pwd
```

### Global Options

Available for all commands:
- `-u, --user <USER>` - Username for authentication
- `-p, --password <PASSWORD>` - Password for authentication
- `-l, --log-level <LEVEL>` - Set log level (error, warn, info, debug, trace, off)
- `-v, --verbose` - Verbose mode (equivalent to --log-level debug)
- `-h, --help` - Show help information
- `-V, --version` - Show version information

---

## Troubleshooting

### "Cargo not found" Error

**Solution**: Make sure Rust is installed and in your PATH
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add to PATH (usually done automatically)
source $HOME/.cargo/env
```

### Build Fails with "linker error"

**Solution**: Install build essentials
```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# macOS (install Xcode Command Line Tools)
xcode-select --install
```

### Database Already Exists Error

**Solution**: Remove existing database or use different path
```bash
# Remove existing database
rm -rf ./my_db

# Or use different path
./target/release/graphlite install --path ./new_db --admin-user admin --admin-password secret
```

---

## Next Steps

### Learn More About GQL

📚 **[Getting Started With GQL.md](Getting%20Started%20With%20GQL.md)** - Comprehensive GQL query language tutorial covering:
- Pattern matching and graph traversal
- Aggregations (GROUP BY, HAVING)
- String and date/time functions
- ORDER BY and LIMIT
- Advanced query examples

### Integrate GraphLite in Your Application

**For Rust Applications:**

🎯 **[SDK Examples](/graphlite-sdk/examples/)** - Recommended high-level API
- `basic_usage.rs` - Complete SDK walkthrough
- Transaction management
- Query builder API
- Typed result deserialization

🔧 **[Core Library Examples](/examples-core/)** - Advanced low-level usage
- Direct QueryCoordinator API
- Fine-grained control
- Advanced features

**SDK Quick Example:**
```rust
use graphlite_sdk::GraphLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = GraphLite::open("./myapp_db")?;
    let session = db.session("user")?;

    session.execute("CREATE SCHEMA myschema")?;
    session.execute("USE SCHEMA myschema")?;
    session.execute("CREATE GRAPH social")?;
    session.execute("USE GRAPH social")?;

    let mut tx = session.transaction()?;
    tx.execute("INSERT (:Person {name: 'Alice'})")?;
    tx.commit()?;

    let result = session.query("MATCH (p:Person) RETURN p.name")?;

    Ok(())
}
```

### For Contributors

👥 **[Contribution Guide.md](/CONTRIBUTING.md)** - How to contribute:
- Development setup
- Testing guidelines
- Code style and quality standards
- Pull request process
---

## Example Queries Cheat Sheet

### Data Insertion

```gql
-- Insert single node
INSERT (:Label {property: 'value'});

-- Insert multiple nodes (comma-separated)
INSERT (:Person {name: 'Alice'}),
       (:Person {name: 'Bob'}),
       (:Person {name: 'Carol'});

-- Insert relationship
MATCH (a:Label1 {id: 1}), (b:Label2 {id: 2})
INSERT (a)-[:RELATIONSHIP {prop: 'val'}]->(b);
```

### Pattern Matching

```gql
-- Match all nodes
MATCH (n:Label) RETURN n;

-- Match with properties
MATCH (n:Label {property: 'value'}) RETURN n;

-- Match relationships
MATCH (a)-[r:REL]->(b) RETURN a, r, b;

-- Match with WHERE
MATCH (n:Label) WHERE n.age > 25 RETURN n;
```

### Aggregations

```gql
-- Count
MATCH (n:Label) RETURN COUNT(n);

-- Group by
MATCH (n:Label)
RETURN n.category, COUNT(n) AS count
GROUP BY n.category;

-- Having clause
MATCH (n:Label)
RETURN n.city, AVG(n.age) AS avg_age
GROUP BY n.city
HAVING AVG(n.age) > 30;
```

### Multi-hop Queries

```gql
-- 2-hop path (chained relationships)
MATCH (a:Person {name: 'Alice'})-[:KNOWS]->(b)-[:KNOWS]->(c)
RETURN c.name;
```

---

## Getting Help

- **Documentation**: See links in [Next Steps](#next-steps) section above
- **Issues**: Report bugs on [GitHub Issues](https://github.com/GraphLite-AI/GraphLite/issues)
- **Questions**: Check existing issues or open a new one
- **Contributing**: See [Contribution Guide.md](/CONTRIBUTING.md)

---

**Congratulations! You now have GraphLite up and running!**

Start exploring graph queries with [Getting Started With GQL.md](Getting%20Started%20With%20GQL.md) or integrate GraphLite into your application with our [SDK examples](/graphlite-sdk/examples/).