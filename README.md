# GraphLite

**A graph database as simple as SQLite for embedded processes**

GraphLite is a fast, light-weight and portable embedded graph database that brings the power of the new **ISO GQL (Graph Query Language)** standard to the simplicity of SQLite.<p> 
GraphLite uses a single binary and is an ideal solution for applications requiring graph database capabilities without the complexity of client-server architectures.

## Features

- **ISO GQL Standard** - Full implementation of ISO GQL query language based on grammar optimized from [OpenGQL](https://github.com/opengql/grammar/tree/main) project
- **Pattern Matching** - Powerful MATCH clauses for graph traversal
- **ACID Transactions** - Full transaction support with isolation levels
- **Embedded Storage** - Sled-based embedded database (no server needed)
- **Type System** - Strong typing with validation and inference
- **Query Optimization** - Cost-based query optimization
- **Pure Rust** - Memory-safe implementation in Rust

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

### Step 1: Installation

**Choose your installation method:**

#### Option A: Use as a Crate (Recommended for Rust Applications)

Add GraphLite to your Rust project - no cloning or building required:

```bash
# For application development (SDK - recommended)
cargo add graphlite-rust-sdk

# For advanced/low-level usage
cargo add graphlite
```

**See:** **[Using GraphLite as a Crate](docs/Using%20GraphLite%20as%20a%20Crate.md)** for complete integration guide.

#### Option B: Use Docker (Easiest for Quick Start)

Run GraphLite instantly with Docker - no installation required:

```bash
# Initialize database
docker run -it -v $(pwd)/mydb:/data ghcr.io/graphlite-ai/graphlite:latest \
  graphlite install --path /data/mydb --admin-user admin --admin-password secret

# Start interactive GQL shell
docker run -it -v $(pwd)/mydb:/data \
  -e GRAPHLITE_DB_PATH=/data/mydb \
  -e GRAPHLITE_USER=admin \
  -e GRAPHLITE_PASSWORD=secret \
  ghcr.io/graphlite-ai/graphlite:latest
```

**See:** **[Docker Guide](docs/Docker.md)** for complete Docker setup including multi-architecture builds and Docker Compose.

#### Option C: Install CLI from crates.io

Install the GraphLite CLI tool directly from crates.io:

```bash
cargo install gql-cli
```

After installation, the `graphlite` binary will be available in your PATH.

#### Option D: Clone and Build (For Development/Contributing)

```bash
# Clone the repository
git clone https://github.com/GraphLite-AI/GraphLite.git
cd GraphLite

# Build the project
./scripts/build_all.sh --release
```

After building, the binary will be available at `target/release/graphlite`.

<details>
<summary><b>Custom Build Options</b></summary>

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
</details>

<details>
<summary><b>Advanced: Manual Build with Cargo</b></summary>

If you prefer to build manually without the script:

1. Build in `release` mode for production-use:
    ```bash
    cargo build --release
    ```

2. Build in `debug` mode for development:

    ```bash
    cargo build
    ```
</details>

### Step 2: Initialize Database (For CLI Usage)

**Note:** If you're using GraphLite as a crate in your application, skip to **[Using GraphLite as a Crate](docs/Using%20GraphLite%20as%20a%20Crate.md)** instead.

```bash
# If you installed via 'cargo install gql-cli' (Option B)
graphlite install --path ./my_db --admin-user admin --admin-password secret

# If you built from source (Option C)
./target/release/graphlite install --path ./my_db --admin-user admin --admin-password secret
```

This command:
- Creates a new database at path: `./my_db`.
- Sets up the `admin` user with the specified password.
- Creates default admin and user roles.
- Initializes the default schema.

### Step 3: Start Using GQL (CLI)

```bash
# If you installed via 'cargo install gql-cli' (Option B)
graphlite gql --path ./my_db -u admin -p secret

# If you built from source (Option C)
./target/release/graphlite gql --path ./my_db -u admin -p secret
```

That's it! You're now ready to create graphs and run queries:
```bash
$ gql>
```

**Next Steps:**
- **[Using GraphLite as a Crate](docs/Using%20GraphLite%20as%20a%20Crate.md)** - Embed in your Rust application (recommended)
- **[Quick Start.md](docs/Quick%20Start.md)** - 5-minute tutorial with CLI and first queries
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

GraphLite includes comprehensive test coverage with **189 unit tests** and **537 total tests** (including integration and benchmark tests).

**Note**: Tests now run in parallel by default thanks to instance-based session isolation, providing ~10x faster test execution compared to the previous single-threaded approach.

### Quick Testing
```bash
# Fast feedback during development (uses optimized release build)
cargo test --release
```

### Comprehensive Testing

**Recommended: Parallel Test Runner** (~10x faster)
```bash
# Fast parallel execution (8 jobs, ~75 seconds for 169 tests)
./scripts/run_integration_tests_parallel.sh --release --jobs=8

# With failure analysis
./scripts/run_integration_tests_parallel.sh --release --jobs=8 --analyze
```

**Note**: Requires GNU Parallel (`brew install parallel` on macOS, `apt install parallel` on Ubuntu)

**Alternative: Sequential Test Runner** (slower but no dependencies)
```bash
# Run all integration tests sequentially (~10-15 minutes)
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

** Comprehensive testing documentation (In Progress)**, which will cover:
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

** Comprehensive configuration documentation (In Progress)**, which will cover:
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
    tx.execute("INSERT (:Person {name: 'Alice'})")?;
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
        "INSERT (:Person {name: 'Alice'})",
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

**For Rust Applications:**
- **[SDK Examples](sdk-rust/examples/)** - Recommended high-level API (start here!)
- **[Examples](examples/)** - SDK (high-level) and bindings (low-level) examples for Rust, Python, and Java

**See also:**
- [Getting Started With GQL.md](docs/Getting%20Started%20With%20GQL.md) - Complete query language reference
- [sdk-rust/README.md](sdk-rust/README.md) - Full SDK documentation

<details>
<summary><b>Uninstall options</b></summary>

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

</details>

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
- **[SDK Examples](sdk-rust/examples/)** - High-level API examples (recommended)
- **[Examples](examples/)** - SDK (high-level) and bindings (low-level) examples for Rust, Python, and Java

**Legal:**
- **[LICENSE](LICENSE)** - Apache License 2.0 full text
- **[NOTICE](NOTICE)** - Third-party attributions

### Questions?

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
- Join discussions in open issues and PRs
- **[Join our Discord](https://discord.gg/Kcs2QakHqs)** - Connect with the community
- **[Contribution Guidelines](CONTRIBUTING.md)** - How to contribute

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


** See [CONTRIBUTING.md](CONTRIBUTING.md) for complete details on:**
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

## Security

If you discover a security vulnerability in GraphLite, please report it to **gl@deepgraphai.com**. Do not create public GitHub issues for security vulnerabilities.