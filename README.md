# GraphLite

**Graph Database as Simple as SQLite for Embedded Processes**

GraphLite is an embedded, light-weight, fast and portable graph database that brings the power of the new **ISO GQL (Graph Query Language)** standard, to the simplicity of SQLite.<p> 
Users can get setup using just a single binary and data without the need for server or daemon-based setups. Perfect for applications that need graph database capabilities without the complexity of client-server architectures.

## Features

- ✅ **ISO GQL Standard** - Full implementation of ISO GQL query language based on grammar optimized from [OpenGQL](https://github.com/opengql/grammar/tree/main) project
- ✅ **Pattern Matching** - Powerful MATCH clauses for graph traversal
- ✅ **ACID Transactions** - Full transaction support with isolation levels
- ✅ **Embedded Storage** - Sled-based embedded database (no server needed)
- ✅ **Type System** - Strong typing with validation and inference
- ✅ **Query Optimization** - Cost-based query optimization
- ✅ **Pure Rust** - Memory-safe implementation in Rust

## Prerequisites

Before building GraphLite, you need to install Rust and a C compiler/linker.

<details>
<summary><b>macOS</b></summary>

```bash
# Install Xcode Command Line Tools (C compiler, linker)
xcode-select --install

# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart terminal or run:
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```
</details>

<details>
<summary><b>Linux (Ubuntu/Debian)</b></summary>

```bash
# Install build essentials
sudo apt-get update
sudo apt-get install build-essential

# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart terminal or run:
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```
</details>

<details>
<summary><b>Linux (Fedora/RHEL)</b></summary>

```bash
# Install development tools
sudo dnf groupinstall "Development Tools"

# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart terminal or run:
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```
</details>

## Getting Started

Get up and running with GraphLite in 3 simple steps:

### Step 1: Clone and Build

```bash
# Clone the repository
git clone https://github.com/GraphLite-AI/GraphLite.git
cd GraphLite

# Build the project
./scripts/build_all.sh --release
```

After building, the binary will be available at `target/release/graphlite`.

### Advanced Build Options

```bash
# Development build (faster compilation, slower runtime)
./scripts/build_all.sh

# Build and run tests
./scripts/build_all.sh --release --test

# Clean build (useful when dependencies change)
./scripts/build_all.sh --clean --release

# View all options
./scripts/build_all.sh --help
```

### Alternative: Manual Build with Cargo

If you prefer to build manually without the script:

**Recommended for**:
- **Development**: `./scripts/build_all.sh` (debug mode, faster compilation)
- **Production/Distribution**: `./scripts/build_all.sh --release` (optimized binaries)
- **Clean builds**: `./scripts/build_all.sh --clean --release` (when dependencies change)
- **CI/CD validation**: `./scripts/build_all.sh --release --test` (build + test in one command)

### Cleanup Script

GraphLite includes a comprehensive cleanup script to uninstall and remove all project artifacts:

```bash
# Show help (also shown when no options provided)
./scripts/cleanup.sh --help

# Clean build artifacts only
./scripts/cleanup.sh --build

# Clean Python/Java bindings
./scripts/cleanup.sh --bindings

# Complete cleanup (bindings, build artifacts, data, config)
./scripts/cleanup.sh --all
```

**What gets cleaned**:
- `--build`: Rust build artifacts, compiled binaries, Cargo.lock
- `--bindings`: Python packages, Java artifacts, compiled libraries
- `--all`: Everything above plus database files, configuration, logs

**Safety**: The script requires an explicit option to prevent accidental cleanup.

### Step 2: Initialize Database

```bash
# Create a new database with admin credentials
./target/release/graphlite install --path ./my_db --admin-user admin --admin-password secret
```

This command:
- Creates database files at `./my_db`
- Sets up the admin user with the specified password
- Creates default admin and user roles
- Initializes the default schema

### Step 3: Start Using GQL

```bash
# Launch the interactive GQL console
./target/release/graphlite gql --path ./my_db -u admin -p secret
```

That's it! You're now ready to create graphs and run queries.

**📚 Next Steps:**
- **[Quick Start.md](docs/Quick%20Start.md)** - 5-minute tutorial with first queries
- **[Getting Started With GQL.md](docs/Getting%20Started%20With%20GQL.md)** - Complete query language reference

<details>
<summary><b>CLI Reference</b></summary>

**Show help:**
```bash
# All commands and options
./target/release/graphlite --help

# Help for specific commands
./target/release/graphlite gql --help
./target/release/graphlite install --help
```

**Global options** (available for all commands):
- `-u, --user <USER>` - Username for authentication
- `-p, --password <PASSWORD>` - Password for authentication
- `-l, --log-level <LEVEL>` - Set log level (error, warn, info, debug, trace, off)
- `-v, --verbose` - Verbose mode (equivalent to --log-level debug)
- `-h, --help` - Show help information
- `-V, --version` - Show version information

**Show version:**
```bash
./target/release/graphlite --version
```
</details>

## Testing

GraphLite includes comprehensive test coverage with **185 unit tests** and **250+ integration tests**.

**Note**: Tests are configured to run single-threaded by default (via `.cargo/config.toml`) to avoid database state conflicts.

### Quick Testing
```bash
# Fast feedback during development (uses optimized release build)
cargo test --release
```

### Comprehensive Testing
```bash
# Run all integration tests with organized output and summary
./scripts/run_tests.sh --release

# Include detailed failure analysis for debugging
./scripts/run_tests.sh --release --analyze
```

### Specific Tests
```bash
# Run a specific integration test
cargo test --release --test <test_name>

# Example: Run aggregation tests
cargo test --release --test aggregation_tests
```

**📚 Comprehensive testing documentation (In Progress)**, which will cover:
- Test configuration and architecture
- Test categories and organization
- Writing tests with TestFixture
- Debugging test failures
- CI/CD configuration
- Test runner script options

## Configuration

GraphLite provides flexible configuration for logging, performance tuning, and production deployment.

### Quick Configuration Examples

```bash
# Enable debug logging
./target/release/graphlite -v gql --path ./my_db -u admin -p secret
```

**📚 Comprehensive configuration documentation (In Progress)**, which will cover:
- Logging configuration (CLI flags, RUST_LOG, module-specific)
- Performance tuning (caching, indexing, batch operations)
- Production deployment (systemd, backups, monitoring)
- Storage backend configuration
- Security configuration (authentication, authorization)
- Environment variables

## Using GraphLite Like SQLite

GraphLite follows the same embedded database pattern as SQLite, making it familiar and easy to use:

### Similarities to SQLite

| Aspect | SQLite | GraphLite |
|--------|--------|-----------|
| **Architecture** | Embedded, file-based | Embedded, file-based |
| **Server** | No daemon required | No daemon required |
| **Setup** | Zero configuration | Zero configuration |
| **Deployment** | Single binary | Single binary (11 MB) |
| **Storage** | Single file | Directory with Sled files |

### Embedding in Your Application

Both databases can be embedded directly in your application without external dependencies:

**SQLite (Rust):**
```rust
use rusqlite::{Connection, Result};

fn main() -> Result<()> {
    // Open/create database file
    let conn = Connection::open("myapp.db")?;

    // Create table and insert data
    conn.execute("CREATE TABLE users (id INTEGER, name TEXT)", [])?;
    conn.execute("INSERT INTO users VALUES (1, 'Alice')", [])?;

    // Query data
    let mut stmt = conn.prepare("SELECT name FROM users")?;
    let names: Vec<String> = stmt.query_map([], |row| row.get(0))?.collect();

    Ok(())
}
```

**GraphLite (Rust) - Recommended SDK:**
```rust
use graphlite_sdk::GraphLite;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open database (SQLite-style API)
    let db = GraphLite::open("./myapp_db")?;

    // Create session
    let session = db.session("user")?;

    // Create schema and graph
    session.execute("CREATE SCHEMA myschema")?;
    session.execute("USE SCHEMA myschema")?;
    session.execute("CREATE GRAPH social")?;
    session.execute("USE GRAPH social")?;

    // Insert data with transaction
    let mut tx = session.transaction()?;
    tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    tx.commit()?;

    // Query data
    let result = session.query("MATCH (p:Person) RETURN p.name")?;

    Ok(())
}
```

**GraphLite (Rust) - Advanced Core Library:**
```rust
use graphlite::QueryCoordinator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database from path
    let coordinator = QueryCoordinator::from_path("./myapp_db")?;

    // Create session
    let session_id = coordinator.create_simple_session("user")?;

    // Create schema and graph
    coordinator.process_query("CREATE SCHEMA /myschema", &session_id)?;
    coordinator.process_query("CREATE GRAPH /myschema/social", &session_id)?;
    coordinator.process_query("SESSION SET GRAPH /myschema/social", &session_id)?;

    // Insert data
    coordinator.process_query(
        "CREATE (p:Person {name: 'Alice'})",
        &session_id
    )?;

    // Query data
    let result = coordinator.process_query(
        "MATCH (p:Person) RETURN p.name",
        &session_id
    )?;

    // Display results
    for row in &result.rows {
        println!("Name: {:?}", row.values.get("p.name"));
    }

    Ok(())
}
```

### Examples and Documentation

📚 **For Rust Applications:**
- **[SDK Examples](graphlite-sdk/examples/)** - Recommended high-level API (start here!)
- **[Core Library Examples](examples-core/)** - Advanced low-level usage

**See also:**
- [Getting Started With GQL.md](docs/Getting%20Started%20With%20GQL.md) - Complete query language reference
- [graphlite-sdk/README.md](graphlite-sdk/README.md) - Full SDK documentation



---

## License and Resources

### License

GraphLite is licensed under the **Apache License 2.0**.

### Documentation

GraphLite provides comprehensive documentation for all skill levels:

**Getting Started:**
- **[Quick Start.md](docs/Quick%20Start.md)** - Get running in 5 minutes
- **[Getting Started With GQL.md](docs/Getting%20Started%20With%20GQL.md)** - Complete query language reference

**Development (to be updated) :**
- "Testing Guide.md" - Comprehensive testing documentation
- "Configuration Guide.md" - Advanced configuration and deployment
- "Contribution Guide.md" - How to contribute

**Code Examples:**
- **[SDK Examples](graphlite-sdk/examples/)** - High-level API examples (recommended)
- **[Core Library Examples](examples-core/)** - Advanced low-level usage

**Legal:**
- **[LICENSE](LICENSE)** - Apache License 2.0 full text
- **[NOTICE](NOTICE)** - Third-party attributions

### Questions?

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
- Join discussions in open issues and PRs
- Contribution guidelines (In Progress)

---

## Contributing

We welcome contributions! GraphLite is built with transparent AI-assisted development practices and we sincerely appreciate help from the community.

### Quick start for Rustaceans:
```bash
git clone https://github.com/GraphLite-AI/GraphLite.git
cd GraphLite
cargo build
cargo test
```


**👉 Contribution guide (In Progress)** will include complete details on:
- How to contribute
- Development setup
- Testing guidelines
- Code style and quality standards
- AI-assisted development philosophy


## Acknowledgements

GraphLite is built on top of excellent open source projects. We are grateful to the maintainers and contributors of these libraries:

### Special Recognition

**[OpenGQL](https://github.com/opengql/grammar/tree/main)** - GraphLite's ISO GQL implementation is based on the grammar and specifications from the OpenGQL project, which provides the open-source reference grammar for the ISO Graph Query Language (GQL) standard. We are deeply grateful to the OpenGQL community and the ISO GQL Working Group for their work in standardizing graph query languages.

### Special Thanks

- **Rust Community** - For creating an amazing ecosystem of high-quality libraries
- All open source contributors whose work makes projects like GraphLite possible
---