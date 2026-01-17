# Changelog

All notable changes to GraphLite will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

### Performance Improvements

#### Concurrent Session Performance Optimizations
- **Lock Partitioning** - 16-partition hash-based session storage for reduced contention
  - Eliminates single RwLock bottleneck in SessionManager
  - Up to 16x throughput improvement for concurrent session operations
  - Maintains backward-compatible API with internal optimization
- **Catalog Cache** - Per-session caching for schema and graph metadata
  - Version-based cache invalidation (schema_version, graph_version)
  - Integrated with `gql.list_schemas()` and `gql.list_graphs()` system procedures
  - Automatic cache invalidation on DDL operations (CREATE/DROP SCHEMA/GRAPH)
  - Benchmark: 57K schemas/sec, 243K graphs/sec for cached queries
- **Session Modes** - Support for Instance and Global session management
  - Instance mode: Isolated sessions per QueryCoordinator (embedded use)
  - Global mode: Shared session pool across coordinators (server use)
  - Configurable via `SessionMode` enum

### Testing
- **Updated Test Count** - 189 unit tests, 537 total tests
- **New Benchmarks** - Added session throughput and catalog cache benchmarks
- **Test Isolation** - All 537 tests pass with `--test-threads=16`

## [0.0.1] - 2025-11-16

### Added

#### Core Features
- **ISO GQL Query Engine** - Full implementation of ISO GQL standard based on OpenGQL grammar
- **Pattern Matching** - MATCH clauses for graph traversal with relationship patterns
- **Aggregations** - Support for COUNT, SUM, AVG, MIN, MAX with GROUP BY and HAVING
- **ACID Transactions** - ACID transaction support with rollback
- **Embedded Storage** - Sled-based embedded database with no server required
- **Query Optimization** - Cost-based query optimizer with execution plan generation
- **Multi-Schema Support** - CREATE/DROP/ALTER SCHEMA for logical database separation
- **Graph Management** - CREATE/DROP/ALTER GRAPH for managing graph collections

#### Query Language Support
- **DDL Statements** - CREATE/DROP/ALTER for schemas, and graphs
- **DML Statements** - INSERT nodes and edges with property maps
- **Query Statements** - MATCH with complex graph patterns
- **WHERE Clauses** - Property filtering and condition expressions
- **ORDER BY** - Result sorting with ASC/DESC
- **LIMIT** - Result set limiting
- **String Functions** - UPPER, LOWER, SUBSTRING, TRIM, LENGTH, CONCAT
- **Date/Time Functions** - NOW, DATE, TIME, DATETIME parsing and formatting
- **Math Functions** - ABS, CEIL, FLOOR, ROUND, SQRT, POW
- **Path Expressions** - Property access on nodes and relationships

#### API & SDK
- **Rust SDK** - High-level ergonomic API (`sdk-rust` crate)
- **Connection Management** - `GraphLite::open()` for database connections
- **Session Management** - `session()` for user context and permissions
- **Transaction API** - `transaction()` with auto-rollback on drop
- **Query Builder** - Fluent API for building type-safe queries
- **Typed Results** - Deserialize query results into Rust structs
- **Query Coordinator** - Single public API for embedded use (`QueryCoordinator::from_path()`)

#### Language Bindings
- **Python Bindings** - Full Python API using ctypes for FFI
- **Java Bindings** - Complete Java API using JNA for native calls
- **C FFI Layer** - `graphlite-ffi` crate with C-compatible interface
- **Cross-Platform** - Support for macOS, and Linux

#### CLI & Tools
- **Interactive REPL** - GQL console with command history and multi-line support

#### Testing Infrastructure
- **185 Unit Tests** - Embedded tests in source files covering all modules
- **250+ Integration Tests** - 40 test files for end-to-end scenarios
- **TestFixture Pattern** - Reusable test setup with randomized schema/graph names
- **Parallel Test Execution** - Instance-based isolation enables concurrent testing (~10x speedup)
- **Test Runner Script** - `run_integration_tests.sh` with debug and release modes
- **Build Script** - `build_all.sh` for automated compilation

#### Documentation
- **README.md** - Comprehensive project overview (14 KB)
- **Quick Start Guide** - 5-minute tutorial for new users (11 KB)
- **GQL Language Guide** - Complete query language reference (26 KB)
- **Testing Guide** - Comprehensive testing documentation (16 KB)
- **Configuration Guide** - Advanced configuration and deployment (14 KB)
- **Contribution Guide** - Developer guidelines with AI transparency (13 KB)
- **ROADMAP** - Development roadmap with v0.1.0 accomplishments
- **API Examples** - SDK and core library usage examples

#### Build & Deployment
- **Cargo Workspace** - Multi-crate project with 4 main crates
- **Release Builds** - Optimized binaries at 11 MB
- **Build Scripts** - Automated build with `--release` and `--clean` options
- **Cross-compilation** - Support for multiple platforms
- **Static Linking** - Single binary distribution

### Technical Details
- **Rust Version** - Requires Rust 1.70+
- **Storage Backend** - Sled embedded database
- **License** - Apache License 2.0
- **Lines of Code** - ~50,000+ Rust
- **Dependencies** - Minimal external dependencies
- **Platform Support** - macOS, Linux

---

## Version History

### Version Numbering

GraphLite follows [Semantic Versioning](https://semver.org/):

- **Major version** (X.0.0) - Breaking API changes
- **Minor version** (0.X.0) - New features, backward compatible
- **Patch version** (0.0.X) - Bug fixes, backward compatible

### Release Cadence

- **Major releases** - As needed for breaking changes
- **Minor releases** - Every 2-3 months with new features
- **Patch releases** - As needed for critical bug fixes

---

## Upgrade Guide (Placeholder)

### From Development Builds to v0.1.0

If you were using development builds before v0.0.1:

1. **Update Cargo.toml**
   ```toml
   [dependencies]
   sdk-rust = "0.0.1"
   ```

2. **Rebuild your project**
   ```bash
   cargo clean
   cargo build --release
   ```

3. **Update your code** (if using internal APIs)
   - Replace any internal API usage with public SDK API
   - See [Migration Examples](docs/Getting%20Started%20With%20GQL.md)

4. **No database migration needed** - v0.1.0 databases are forward compatible

---

## Links

- **Source Code**: https://github.com/GraphLite-AI/GraphLite
- **Issue Tracker**: https://github.com/GraphLite-AI/GraphLite/issues
- **Documentation**: [docs/](docs/)
- **Examples**: [sdk-rust/examples/](sdk-rust/examples/)

---

### Community
We welcome contributions! See [CONTRIBUTING.md](docs/Contribution%20Guide.md) for guidelines.

---

## Acknowledgments

GraphLite is built on excellent open source projects:

- **[OpenGQL](https://github.com/opengql/grammar)** - ISO GQL grammar specification
- **[Sled](https://github.com/spacejam/sled)** - Embedded database engine
- **Rust Community** - For the amazing ecosystem

See [NOTICE](NOTICE) for complete attributions.

---

## Archive

Older versions and their changelogs are available in the [releases](https://github.com/GraphLite-AI/GraphLite/releases) page.

---

**Last Updated**: November 16, 2025
**Current Version**: v0.1.0
**Next Release**: v0.2.0 (Planned: Q1 2026)
