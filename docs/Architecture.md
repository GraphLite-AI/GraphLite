# GraphLite Architecture

> **[Project Overview & Quick Start](../../README.md)** - Main README with features, installation, and getting started

Comprehensive technical architecture documentation for GraphLite.

## Table of Contents

1. [System Overview](#system-overview)
2. [High-Level Architecture](#high-level-architecture)
3. [Component Architecture](#component-architecture)
4. [Query Execution Pipeline](#query-execution-pipeline)
5. [Storage Architecture](#storage-architecture)
6. [Caching System](#caching-system)
7. [Transaction System](#transaction-system)
8. [Session Management](#session-management)
9. [Security Architecture](#security-architecture)
10. [Performance Optimizations](#performance-optimizations)

---

## System Overview

GraphLite is a pure Rust implementation of the ISO GQL (Graph Query Language) standard, designed as an embedded graph database following the SQLite philosophy.

### Design Principles

1. **Embedded Database** - No daemon, direct file access
2. **Zero Dependencies** - Pure Rust, no external libraries required
3. **ACID Compliance** - Full transaction support with WAL
4. **Standards-Based** - ISO GQL compliant
5. **High Performance** - Multi-level caching, parallel execution
6. **Type Safety** - Rust's type system for correctness

### Key Characteristics

| Aspect | Description |
|--------|-------------|
| **Language** | Pure Rust (1.70+) |
| **Storage Backend** | Sled (embedded key-value store) |
| **Query Language** | ISO GQL (Graph Query Language) |
| **Transaction Model** | ACID with MVCC |
| **Concurrency** | Multi-threaded with Rayon |
| **Caching** | 3-level hierarchy (L1/L2/L3) |
| **Deployment** | Embedded library or CLI |

---

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        GraphLite System                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐        │
│  │     CLI      │    │   REPL       │    │   Library    │        │
│  │  Interface   │    │  Console     │    │     API      │        │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘        │
│         │                   │                   │                │
│         └───────────────────┴───────────────────┘                │
│                             │                                    │
│                    ┌────────▼──-──────┐                          │
│                    │ Query Coordinator│                          │
│                    └───────┬─--───────┘                          │
│                            │                                     │
│         ┌──────────────────┼──────────────────┐                  │
│         │                  │                  │                  │
│    ┌────▼─────┐      ┌─────▼──────┐     ┌─────▼──────┐           │
│    │  Parser  │      │  Executor  │     │  Session   │           │
│    │   & AST  │      │            │     │  Manager   │           │
│    └────┬─────┘      └─────┬──────┘     └─────┬──────┘           │
│         │                  │                  │                  │
│    ┌────▼─────┐      ┌─────▼──────┐     ┌─────▼──────┐           │
│    │ Planner  │      │   Cache    │     │Transaction │           │
│    │ &Optimize│      │  Manager   │     │  Manager   │           │
│    └────┬─────┘      └──-───┬─────┘     └─────┬──────┘           │
│         │                   │                 │                  │
│         └───────────────────┴─────────────────┘                  │
│                             │                                    │
│                    ┌────────▼────────┐                           │
│                    │ Storage Manager │                           │
│                    └────────┬────────┘                           │
│                             │                                    │
│                    ┌────────▼────────┐                           │
│                    │  Sled Database  │                           │
│                    │   (Persistent)  │                           │
│                    └─────────────────┘                           │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

**Presentation Layer:**
- CLI: Command-line interface (`graphlite` binary)
- REPL: Interactive console
- Library API: Rust API for embedding

**Processing Layer:**
- Query Coordinator: Entry point for all queries
- Session Manager: User session lifecycle
- Transaction Manager: ACID transaction control
- Parser: GQL → AST conversion
- Planner: Logical and physical query plans
- Executor: Query execution engine
- Cache Manager: Multi-level caching

**Data Layer:**
- Storage Manager: Graph data abstraction
- Sled: Persistent key-value storage
- WAL: Write-ahead logging

---

## User Interface Modes

GraphLite provides two distinct modes for interacting with the database, each optimized for different use cases.

### REPL Console vs CLI Interface

#### Overview

| Aspect | REPL Console (`gql`) | CLI Interface (`query`) |
|--------|---------------------|------------------------|
| **Mode** | Interactive shell | One-shot execution |
| **Session** | Persistent across queries | One query per session |
| **Use Case** | Development, exploration | Scripts, automation |
| **Command** | `graphlite gql` | `graphlite query` |

#### REPL Console (Interactive Mode)

**Command:**
```bash
graphlite gql --path ./my_db -u admin -p admin123
```

**What It Is:**

REPL = **R**ead-**E**val-**P**rint-**L**oop - An interactive shell for database exploration and development.

**Features:**
-  Multi-line query support (terminated by `;`)
-  Command history (saved to `.graphlite/.gql_history.txt`)
-  Line editing with Emacs key bindings
-  Persistent session state across queries
-  Special commands (`help`, `exit`, `clear`)
-  Context-aware prompting

**Example Session:**
```
$ graphlite gql --path ./my_db -u admin -p admin123
GraphLite
Type 'help' for commands, 'exit' or 'quit' to exit
Multi-line queries supported - use ';' to terminate

Authenticated as: admin
Session ID: 5e02ad9d-0bdb-4873-88b2-64127fb62c2f

admin::gql> CREATE SCHEMA /myschema;
 Schema 'myschema' created

admin::gql> SESSION SET GRAPH /myschema/social;
 Session graph set to: /myschema/social

admin::gql> INSERT (:Person {name: 'Alice', age: 30});
 Created 1 node

admin::gql> MATCH (p:Person) RETURN p.name, p.age;
┌────────┬───────┐
│ p.name ┆ p.age │
╞════════╪═══════╡
│ Alice  ┆ 30    │
└────────┴───────┘

admin::gql> exit
Goodbye!
```

**Implementation Details:**
- **Module**: `src/cli/graphlite_cli.rs::handle_gql()`
- **Library**: `rustyline` for readline editing
- **History**: Persistent command history with search (Ctrl+R)
- **Prompts**:
  - `user::gql>` - Ready for new query
  - `user::...>` - Multi-line continuation

**Special Commands:**
- `help` - Display available commands
- `exit` / `quit` - Exit the REPL
- `clear` - Clear screen
- Ctrl+C - Cancel current query buffer
- Ctrl+D - Exit (EOF)

**When to Use:**
- Developing and testing queries interactively
- Exploring data and schema structure
- Running multiple related queries (setup, insert, query)
- Learning GQL syntax
- Iterating on complex queries

---

#### CLI Interface (One-Shot Mode)

**Command:**
```bash
graphlite query "MATCH (p:Person) RETURN p.name" --path ./my_db -u admin -p admin123
```

**What It Is:**

A command-line interface for executing single queries and exiting - designed for automation and scripting.

**Features:**
- Single query execution
- Multiple output formats (table, JSON, CSV)
- AST inspection (`--ast`)
- Execution plan viewing (`--explain`)
- Anonymous sessions (no authentication required)
- Scriptable and pipeable

**Example Usage:**

**Basic query:**
```bash
$ graphlite query "MATCH (p:Person) RETURN p.name" --path ./my_db
┌────────┐
│ p.name │
╞════════╡
│ Alice  │
└────────┘
```

**JSON output:**
```bash
$ graphlite query "MATCH (p:Person) RETURN p.name" --format json
{"rows": [{"p.name": "Alice"}]}
```

**Show AST:**
```bash
$ graphlite query "MATCH (p:Person) RETURN p.name" --ast
Abstract Syntax Tree:
Document {
    statement: Query(QueryStatement {
        match_clauses: [MatchClause { ... }],
        return_clause: Some(ReturnClause { ... })
    })
}
```

**Script integration:**
```bash
#!/bin/bash
# backup_data.sh
graphlite query "MATCH (n) RETURN n" --format json > backup.json
```

**Implementation Details:**
- **Module**: `src/cli/gqlcli.rs::handle_query()`
- **Output Formats**:
  - `table` - Pretty-printed table (default)
  - `json` - JSON format for parsing
  - `csv` - CSV format for spreadsheets

**Anonymous Access:**

Queries can run without authentication by omitting the `-u` and `-p` flags:

```bash
# Anonymous session (omit credentials)
graphlite query "MATCH (n) RETURN n" --path ./my_db

# Authenticated session (with credentials)
graphlite query "MATCH (n) RETURN n" --path ./my_db -u admin -p admin123
```

Anonymous sessions have full query access but are intended for local development and testing. For production use, always authenticate with proper credentials.

**When to Use:**
- Automating queries in shell scripts
- CI/CD pipelines and testing
- Data export and backup
- Quick one-off queries
- Generating reports
- Integration with other tools

---

#### Architecture Comparison

Both modes use the same underlying execution pipeline but differ in user interaction:

```
┌─────────────────────────────────────────────┐
│          CLI Entry Point (main.rs)          │
│                                             │
│  ┌──────────────┐      ┌─────────────────┐  │
│  │ Commands::Gql│      │Commands::Query  │  │
│  │   (REPL)     │      │  (One-shot)     │  │
│  └──────┬───────┘      └────────┬────────┘  │
└─────────┼──────────────────────┼───────--───┘
          │                      │
          ▼                      ▼
   ┌──────────────┐      ┌──────────────┐
   │ handle_gql() │      │handle_query()│
   │              │      │              │
   │ • Loop until │      │ • Execute    │
   │   exit       │      │   once       │
   │ • History    │      │ • Format     │
   │ • Multi-line │      │ • Exit       │
   └──────┬───────┘      └──────┬───────┘
          │                     │
          └──────────┬──────────┘
                     ▼
          ┌──────────────────────┐
          │  QueryCoordinator    │
          │  process_query()     │
          └──────────────────────┘
                     │
                     ▼
              [Same Pipeline]
          Parser → Planner → Executor
```

**Key Differences:**

| Feature | REPL Console | CLI Interface |
|---------|-------------|---------------|
| **Session Lifecycle** | Long-lived (until exit) | Short-lived (one query) |
| **State Persistence** | Variables, graph context persist | Fresh state each run |
| **History** | Saved to disk, searchable | None |
| **Output Formats** | Table only | Table, JSON, CSV |
| **Authentication** | Required, prompted | Optional (anonymous OK) |
| **Multi-line Queries** |  Yes |  No (single string) |
| **Debugging** |  Limited |  `--ast`, `--explain` |
| **Startup Cost** | One-time | Per query |
| **Best For** | Humans | Machines |

---

#### Implementation Files

**REPL Console:**
- Entry: `src/main.rs` → `Commands::Gql`
- Handler: `src/cli/gqlcli.rs::handle_gql()`
- Dependencies: `rustyline` for readline, history management
- Features: Multi-line buffering, special command handling

**CLI Interface:**
- Entry: `src/main.rs` → `Commands::Query`
- Handler: `src/cli/gqlcli.rs::handle_query()`
- Output: `src/cli/output.rs::ResultFormatter`
- Features: Format selection, AST inspection, execution plans

**Shared Components:**
- Query Coordinator: `src/coordinator/query_coordinator.rs`
- Parser: `src/ast/parser.rs`
- Executor: `src/exec/executor.rs`
- Session Manager: `src/session/manager.rs`

---

## Component Architecture

### Component Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                     Component Breakdown                            │
└────────────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│ Parser & AST Module (src/ast/)                                │
├───────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌──────────┐  ┌───────────┐  ┌──────────────┐   │
│  │ Lexer   │→ │  Parser  │→ │    AST    │→ │  Validator   │   │
│  │(Tokens) │  │  (Nom)   │  │ (Nodes)   │  │ (Semantic)   │   │
│  └─────────┘  └──────────┘  └───────────┘  └──────────────┘   │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│ Query Planning Module (src/plan/)                             │
├───────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐    │
│  │   Logical    │→ │   Physical    │→ │   Optimizer      │    │
│  │   Planner    │  │   Planner     │  │ (Cost-Based)     │    │
│  └──────────────┘  └───────────────┘  └──────────────────┘    │
│         ↓                   ↓                    ↓            │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐    │
│  │   Pattern    │  │   Cost Model  │  │   Statistics     │    │
│  │  Analysis    │  │               │  │                  │    │
│  └──────────────┘  └───────────────┘  └──────────────────┘    │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│ Execution Engine (src/exec/)                                  │
├───────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐    │
│  │   Executor   │→ │   Context     │→ │   Iterator       |    │
│  │   (Main)     │  │  (Variables)  │  │  (Streaming)     │    │
│  └──────────────┘  └───────────────┘  └──────────────────┘    │
│         ↓                   ↓                    ↓            │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐    │
│  │  DML/DDL     │  │   Functions   │  │   Aggregates     │    │
│  │  Executors   │  │   (50+)       │  │   (COUNT, SUM)   │    │
│  └──────────────┘  └───────────────┘  └──────────────────┘    │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│ Storage Layer (src/storage/)                                  │
├───────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐    │
│  │   Storage    │→ │   Graph       │→ │    Index         │    │
│  │   Manager    │  │   Cache       │  │   Manager        │    │
│  └──────────────┘  └───────────────┘  └──────────────────┘    │
│         ↓                   ↓                    ↓            │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐    │
│  │     Sled     │  │  Multi-Graph  │  │   Value Types    │    │
│  │   Backend    │  │   Support     │  │                  │    │
│  └──────────────┘  └───────────────┘  └──────────────────┘    │
└───────────────────────────────────────────────────────────────┘
```

### Module Responsibilities

#### Parser & AST (`src/ast/`)
- **Lexer**: Tokenizes GQL text
- **Parser**: Nom-based recursive descent parser
- **AST**: Abstract syntax tree nodes
- **Validator**: Semantic validation
- **Pretty Printer**: AST formatting for debugging

#### Query Planning (`src/plan/`)
- **Logical Planner**: High-level query representation
- **Physical Planner**: Execution-ready plan
- **Optimizer**: Cost-based optimization rules
- **Pattern Analysis**: Graph pattern optimization
- **Cost Model**: Execution cost estimation

#### Execution Engine (`src/exec/`)
- **Executor**: Main execution coordinator
- **Context**: Variable bindings and state
- **Iterator**: Streaming result processing
- **DDL Executors**: Schema/graph operations
- **DML Executors**: Data modification
- **Functions**: 50+ built-in functions

#### Storage Layer (`src/storage/`)
- **Storage Manager**: Unified storage interface
- **Graph Cache**: In-memory caching
- **Index Manager**: Property indexes
- **Sled Backend**: Persistent storage
- **Value System**: Type system implementation

---

## Query Execution Pipeline

### Execution Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                    Query Execution Pipeline                      │
└──────────────────────────────────────────────────────────────────┘

┌─────────────┐
│  GQL Query  │
│   (Text)    │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Lexical Analysis                                   │
│  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐             │
│  │ MATCH  │  │   (    │  │   n    │  │   )    │  ...        │
│  │ Keyword│  │ LParen │  │ Ident  │  │ RParen │             │
│  └────────┘  └────────┘  └────────┘  └────────┘             │
└──────────────────────────────┬──────────────────────────────┘
                               │ Token Stream
                               ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Syntax Analysis (Parsing)                          │
│  ┌───────────────────────────────────────┐                  │
│  │         Abstract Syntax Tree          │                  │
│  │                                       │                  │
│  │        ┌──────────────┐               │                  │
│  │        │ MatchClause  │               │                  │
│  │        └──────┬───────┘               │                  │
│  │               │                       │                  │
│  │      ┌────────┴────────┐              │                  │
│  │      │  Pattern: (n)   │              │                  │
│  │      └─────────────────┘              │                  │
│  └───────────────────────────────────────┘                  │
└──────────────────────────────┬──────────────────────────────┘
                               │ AST
                               ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 3: Semantic Validation                                │
│  - Type checking                                            │
│  - Variable binding validation                              │
│  - Schema compliance (if typed)                             │
│  - Function signature matching                              │
└──────────────────────────────┬──────────────────────────────┘
                               │ Validated AST
                               ▼
┌────────────────────────────────────────────────────────────┐
│ Phase 4: Logical Planning                                  │
│  ┌────────────────────────────────────────┐                │
│  │        Logical Query Plan              │                │
│  │                                        │                │
│  │   ┌──────────────────────┐             │                │
│  │   │  LogicalScan(n)      │             │                │
│  │   └──────────┬───────────┘             │                │
│  │              │                         │                │
│  │   ┌──────────▼───────────┐             │                │
│  │   │  LogicalFilter       │             │                │
│  │   └──────────┬───────────┘             │                │
│  │              │                         │                │
│  │   ┌──────────▼───────────┐             │                │
│  │   │  LogicalProject      │             │                │
│  │   └──────────────────────┘             │                │
│  └────────────────────────────────────────┘                │
└──────────────────────────────┬─────────────────────────────┘
                               │
                               ▼
┌────────────────────────────────────────────────────────────┐
│ Phase 5: Physical Planning & Optimization                  │
│  ┌────────────────────────────────────────┐                │
│  │      Physical Query Plan               │                │
│  │                                        │                │
│  │   ┌──────────────────────┐             │                │
│  │   │  IndexScan(n)        │  ← Optimizer chooses         │
│  │   │  Cost: 10            │     index scan over          │
│  │   └──────────┬───────────┘     full scan                │
│  │              │                                          │
│  │   ┌──────────▼───────────┐                              │
│  │   │  FilterExec          │                              │
│  │   │  Cost: 15            │                              │
│  │   └──────────┬───────────┘                              │
│  │              │                                          │
│  │   ┌──────────▼───────────┐                              │
│  │   │  ProjectExec         │                              │
│  │   │  Cost: 20            │                              │
│  │   └──────────────────────┘                              │
│  └────────────────────────────────────────┘                │
└──────────────────────────────┬─────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 6: Cache Lookup (if applicable)                       │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐   │
│  │ Plan Cache   │    │Result Cache  │    │Subquery Cache│   │
│  │   (L3)       │    │   (L1/L2)    │    │              │   │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘   │
│         │                   │                   │           │
│         └───────── Hit? ────┴──── Return Cached Result      │
│                     │                                       │
│                    Miss                                     │
└─────────────────────┼────────────────────--─────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 7: Execution                                          │
│  ┌────────────────────────────────────────┐                 │
│  │    Execution with Storage Access       │                 │
│  │                                        │                 │
│  │  1. Index Scan                         │                 │
│  │     └─→ Read from storage              │                 │
│  │                                        │                 │
│  │  2. Apply Filters                      │                 │
│  │     └─→ Evaluate predicates            │                 │
│  │                                        │                 │
│  │  3. Project Results                    │                 │
│  │     └─→ Select columns                 │                 │
│  │                                        │                 │
│  │  4. Stream Results                     │                 │
│  │     └─→ Iterator-based                 │                 │
│  └────────────────────────────────────────┘                 │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 8: Result Formation                                   │
│  ┌────────────────────────────────────────┐                 │
│  │         Query Result                   │                 │
│  │                                        │                 │
│  │  Columns: [name, age]                  │                 │
│  │  Rows: [                               │                 │
│  │    {name: "Alice", age: 30},           │                 │
│  │    {name: "Bob", age: 25}              │                 │
│  │  ]                                     │                 │
│  │  Metadata: {rows: 2, time: 5ms}        │                 │
│  └────────────────────────────────────────┘                 │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
                       ┌───────────────┐
                       │    Return     │
                       │   to Client   │
                       └───────────────┘
```

### Pipeline Stages Detail

#### Stage 1: Lexical Analysis
- **Input**: Raw GQL text
- **Output**: Token stream
- **Time**: ~1-2% of total
- **Components**: Lexer (`src/ast/lexer.rs`)

#### Stage 2: Syntax Analysis
- **Input**: Token stream
- **Output**: AST
- **Time**: ~5-10% of total
- **Components**: Parser (`src/ast/parser.rs`)
- **Technology**: Nom parser combinators

#### Stage 3: Semantic Validation
- **Input**: AST
- **Output**: Validated AST
- **Time**: ~3-5% of total
- **Checks**:
  - Type compatibility
  - Variable scoping
  - Function signatures
  - Schema compliance

#### Stage 4: Logical Planning
- **Input**: Validated AST
- **Output**: Logical plan
- **Time**: ~5-8% of total
- **Operations**: Scan, Filter, Project, Join, Aggregate

#### Stage 5: Physical Planning
- **Input**: Logical plan
- **Output**: Physical plan
- **Time**: ~10-15% of total
- **Optimizations**:
  - Index selection
  - Join order optimization
  - Predicate pushdown
  - Projection pruning
  - Cost-based decisions

#### Stage 6: Cache Lookup
- **Input**: Query hash
- **Output**: Cached result (if hit)
- **Time**: < 1% (hit), skipped (miss)
- **Levels**: L1 (hot), L2 (warm), L3 (cold)

#### Stage 7: Execution
- **Input**: Physical plan
- **Output**: Result iterator
- **Time**: ~60-80% of total
- **Components**: Executor, Storage Manager
- **Parallelization**: Rayon for eligible operations

#### Stage 8: Result Formation
- **Input**: Result iterator
- **Output**: Formatted results
- **Time**: ~5-10% of total
- **Formats**: Table, JSON, CSV

---

## Storage Architecture

### Storage Layer Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Storage Architecture                     │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                   Application Layer                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   Executor   │  │   Planner    │  │   Catalog    │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
│         │                 │                  │              │
│         └─────────────────┴──────────────────┘              │
│                           │                                 │
└───────────────────────────┼─────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────┐
│              Storage Manager (Abstraction Layer)           │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Unified API for Graph Operations                    │  │
│  │  - get_node(), put_node()                            │  │
│  │  - get_relationship(), put_relationship()            │  │
│  │  - get_neighbors(), traverse()                       │  │
│  │  - batch_operations()                                │  │
│  └──────────────────────────────────────────────────────┘  │
└───────────────────────────┬────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│  Graph       │    │   Index      │    │  Multi-Graph │
│  Cache       │    │   Manager    │    │   Support    │
│  (Memory)    │    │              │    │              │
└──────┬───────┘    └──────┬───────┘    └──────┬───────┘
       │                   │                   │
       └───────────────────┴───────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                     Physical Storage                        │
│  ┌───────────────────────────────────────────────-───────┐  │
│  │            Sled Embedded Database                     │  │
│  │                                                       │  │
│  │  ┌─────────────────────────────────────────────┐      │  │
│  │  │  Key-Value Store Organization               │      │  │
│  │  │                                             │      │  │
│  │  │  Tree: nodes                                │      │  │
│  │  │    Key: node_id → Value: Node{labels, props}│      │  │
│  │  │                                             │      │  │
│  │  │  Tree: relationships                        │      │  │
│  │  │    Key: rel_id → Value: Rel{type, props}    │      │  │
│  │  │                                             │      │  │
│  │  │  Tree: adjacency_src                        │      │  │
│  │  │    Key: (src_id, rel_id) → Value: dst_id    │      │  │
│  │  │                                             │      │  │
│  │  │  Tree: adjacency_dst                        │      │  │
│  │  │    Key: (dst_id, rel_id) → Value: src_id    │      │  │
│  │  │                                             │      │  │
│  │  │  Tree: indexes                              │      │  │
│  │  │    Key: (label, prop, value) → Value: ids   │      │  │
│  │  │                                             │      │  │
│  │  │  Tree: catalog                              │      │  │
│  │  │    Key: catalog_key → Value: metadata       │      │  │
│  │  └─────────────────────────────────────────────┘      │  │
│  └───────────────────────────────────────────────────-───┘  │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   Persistent Storage                        │
│  ┌───────────────────────────────────────────────────-───┐  │
│  │               File System                             │  │
│  │                                                       │  │
│  │  ./mydb/                                              │  │
│  │    ├── conf                    (Sled config)          │  │
│  │    ├── db-*                    (Data files)           │  │
│  │    ├── snap.*                  (Snapshots)            │  │
│  │    └── blobs/                  (Large objects)        │  │
│  └──────────────────────────────────────────────────=────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Data Organization

#### Node Storage
```
Key Format: "node:{node_id}"
Value: {
  id: String,
  labels: Vec<String>,
  properties: HashMap<String, Value>
}
```

#### Relationship Storage
```
Key Format: "rel:{rel_id}"
Value: {
  id: String,
  type: String,
  source_id: String,
  target_id: String,
  properties: HashMap<String, Value>
}
```

#### Adjacency Lists
```
Forward:  "adj_out:{source_id}:{rel_id}" → target_id
Backward: "adj_in:{target_id}:{rel_id}" → source_id
```

#### Indexes
```
Property Index: "idx:{label}:{property}:{value}" → Vec<node_id>
```

### Storage Guarantees

- **Durability**: WAL ensures no data loss
- **Consistency**: ACID transactions
- **Atomicity**: Batch operations

---

## Caching System (Needs More review)

### Cache Architecture Diagram

```
┌───────────────────────────────────────────────────────────────┐
│                    Multi-Level Cache System                   │
└───────────────────────────────────────────────────────────────┘

                     ┌──────────────┐
                     │    Query     │
                     └──────┬───────┘
                            │
                            ▼
        ┌────────────────────────────────-───────┐
        │         Cache Lookup Decision          │
        └───────┬───────────────────┬────────────┘
                │                   │
        Query Hash              Plan Hash
                │                   │
                ▼                   ▼
┌─────────────────────────┐ ┌─────────────────────────┐
│   L1 Result Cache       │ │    L3 Plan Cache        │
│   (Hot - In Memory)     │ │  (Cold - Disk Backed)   │
│                         │ │                         │
│  Size: 100 MB           │ │  Size: 500 MB           │
│  TTL: 5 minutes         │ │  TTL: 1 hour            │
│  Eviction: LRU          │ │  Eviction: LRU          │
│  Hit Rate: ~85%         │ │  Hit Rate: ~92%         │
└────┬──────────────────┬-┘ └────┬────────────────────┘
     │ Hit              │        │ Hit
     │                Miss       │
     ▼                  │        ▼
 ┌─────────┐            │   ┌─────────┐
 │ Return  │            │   │Execute  │
 │ Cached  │            │   │With Plan│
 └─────────┘            │   └─────────┘
                        │
                        ▼
           ┌─────────────────────────┐
           │   L2 Result Cache       │
           │   (Warm - In Memory)    │
           │                         │
           │  Size: 200 MB           │
           │  TTL: 15 minutes        │
           │  Eviction: LRU          │
           │  Hit Rate: ~75%         │
           └────┬───────────────────┬┘
                │ Hit          Miss │
                ▼                   │
            ┌─────────┐             │
            │ Return  │             │
            │ Cached  │             │
            └─────────┘             │
                                    ▼
                   ┌──────────────────────────────┐
                   │   Subquery Cache             │
                   │   (Specialized)              │
                   │                              │
                   │  Size: 50 MB                 │
                   │  TTL: 10 minutes             │
                   │  Scope: Per-query subqueries │
                   └────┬───────────────────────┬-┘
                        │ Hit             Miss  │
                        ▼                       │
                    ┌─────────┐                 │
                    │ Return  │                 │
                    │ Cached  │                 │
                    └─────────┘                 │
                                                ▼
                                   ┌───────────────────────┐
                                   │  Execute Full Query   │
                                   │  Against Storage      │
                                   └───────────────────────┘
                                                │
                                                ▼
                                   ┌───────────────────────┐
                                   │   Cache Results       │
                                   │   (Store in L1/L2/L3) │
                                   └───────────────────────┘
                                                │
                                                ▼
                                        ┌──────────────┐
                                        │Return Results│
                                        └──────────────┘
```

### Cache Types

#### L1 Cache (Result Cache - Hot)
- **Purpose**: Frequently accessed query results
- **Size**: 100 MB (configurable)
- **TTL**: 5 minutes
- **Eviction**: LRU (Least Recently Used)
- **Hit Rate**: ~85%
- **Use Case**: Repeated identical queries

#### L2 Cache (Result Cache - Warm)
- **Purpose**: Less frequently accessed results
- **Size**: 200 MB (configurable)
- **TTL**: 15 minutes
- **Eviction**: LRU
- **Hit Rate**: ~75%
- **Use Case**: Occasional repeated queries

#### L3 Cache (Plan Cache - Cold)
- **Purpose**: Compiled query plans
- **Size**: 500 MB (configurable)
- **TTL**: 1 hour
- **Eviction**: LRU
- **Hit Rate**: ~92%
- **Use Case**: Same query structure, different parameters

#### Subquery Cache (Specialized)
- **Purpose**: Reusable subquery results within complex queries
- **Size**: 50 MB (configurable)
- **TTL**: 10 minutes (query-scoped)
- **Eviction**: LRU
- **Use Case**: Complex queries with repeated subqueries

### Cache Invalidation

```
┌─────────────────────────────────────────────────────────────┐
│              Cache Invalidation Strategy                    │
└─────────────────────────────────────────────────────────────┘

Write Operation (INSERT/UPDATE/DELETE)
            │
            ▼
    ┌───────────────┐
    │  Detect Scope │
    │  - Tables     │
    │  - Labels     │
    │  - Properties │
    └───────┬───────┘
            │
            ▼
    ┌───────────────────────────────────┐
    │  Invalidate Affected Caches       │
    │                                   │
    │  IF affects nodes/relationships:  │
    │    → Clear result caches          │
    │    → Clear subquery caches        │
    │                                   │
    │  IF schema change:                │
    │    → Clear ALL caches             │
    │    → Rebuild plan cache           │
    │                                   │
    │  IF index change:                 │
    │    → Clear plan cache             │
    │    → Keep result cache (if valid) │
    └───────────────────────────────────┘
```

### Cache Statistics

Accessible via `CALL gql.cache_stats()`:
- Entries count
- Hit rate percentage
- Memory usage
- Eviction count
- Hits/misses count

---

## Transaction System  (Needs More review)

### Transaction Architecture

```
┌───────────────────────────────────────────────────────────────┐
│                  Transaction System                           │
└───────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                   Transaction Manager                       │
│  ┌─────────────────────────────────────────────-─────────┐  │
│  │  Active Transactions                                  │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐             │  │
│  │  │ TXN-001  │  │ TXN-002  │  │ TXN-003  │             │  │
│  │  │ Read-Only│  │Read-Write│  │Read-Write│             │  │
│  │  │ Level: RC│  │Level: S  │  │Level: RR │             │  │
│  │  └──────────┘  └──────────┘  └──────────┘             │  │
│  └───────────────────────────────────────────────-───────┘  │
└─────────────────────────┬───────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│    WAL       │  │    Lock      │  │   MVCC       │
│(Write-Ahead  │  │   Manager    │  │ (Multi-      │
│   Log)       │  │              │  │  Version)    │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       │                 │                 │
       └─────────────────┴─────────────────┘
                         │
                         ▼
                ┌────────────────┐
                │    Storage     │
                │    Manager     │
                └────────────────┘
```

### Transaction Flow

```
BEGIN TRANSACTION
       │
       ▼
┌──────────────────────────────────────┐
│  1. Create Transaction Context       │
│     - Assign transaction ID          │
│     - Set isolation level            │
│     - Initialize WAL entry           │
│     - Acquire locks (if needed)      │
└──────┬───────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────┐
│  2. Execute Operations               │
│     ┌────────────────────────────┐   │
│     │ INSERT/UPDATE/DELETE       │   │
│     │   ↓                        │   │
│     │ Write to WAL               │   │
│     │   ↓                        │   │
│     │ Create version             │   │
│     │   ↓                        │   │
│     │ Apply to transaction view  │   │
│     └────────────────────────────┘   │
└──────┬───────────────────────────────┘
       │
       ├──→ COMMIT
       │        │
       │        ▼
       │   ┌──────────────────────────┐
       │   │  3a. Commit Phase        │
       │   │     - Flush WAL          │
       │   │     - Apply changes      │
       │   │     - Release locks      │
       │   │     - Mark committed     │
       │   └──────┬───────────────────┘
       │          │
       │          ▼
       │   ┌──────────────────────────┐
       │   │  4a. Post-Commit         │
       │   │     - Invalidate caches  │
       │   │     - Update stats       │
       │   │     - Cleanup            │
       │   └──────────────────────────┘
       │
       └──→ ROLLBACK
                │
                ▼
           ┌──────────────────────────┐
           │  3b. Rollback Phase      │
           │     - Discard changes    │
           │     - Release locks      │
           │     - Cleanup WAL        │
           │     - Mark rolled back   │
           └──────┬───────────────────┘
                  │
                  ▼
           ┌──────────────────────────┐
           │  4b. Post-Rollback       │
           │     - Restore state      │
           │     - Cleanup            │
           └──────────────────────────┘
```

### Isolation Levels

| Level | Dirty Read | Non-Repeatable Read | Phantom Read | Implementation |
|-------|------------|---------------------|--------------|----------------|
| **READ UNCOMMITTED** | Possible | Possible | Possible | Minimal locking |
| **READ COMMITTED** | Prevented | Possible | Possible | Read locks released immediately |
| **REPEATABLE READ** | Prevented | Prevented | Possible | Read locks held until commit |
| **SERIALIZABLE** | Prevented | Prevented | Prevented | Full MVCC with validation |

### MVCC (Multi-Version Concurrency Control). (To be Tested and reviewed for Multi-user/Concurrent sessions. Not relevant in single session/single user embedded mode.)

```
┌────────────────────────────────────────────────────────────┐
│               Version Chain Example                        │
└────────────────────────────────────────────────────────────┘

Node ID: node_123
Property: age

┌──────────────────────────────────────────────────────────┐
│  Version Chain (newest → oldest)                         │
│                                                          │
│  V3: age=31  [TXN-103, active]    ← Latest write         │
│   ↓                                                      │
│  V2: age=30  [TXN-102, committed] ← Previous version     │
│   ↓                                                      │
│  V1: age=29  [TXN-101, committed] ← Original version     │
└──────────────────────────────────────────────────────────┘

Transaction Visibility:
- TXN-103: Sees V3 (own changes)
- TXN-104 (after TXN-102): Sees V2
- TXN-100 (before TXN-101): Sees nothing
```

---

## Session Management

GraphLite implements a unified session architecture supporting two deployment modes:
- **Instance Mode** (default): Each QueryCoordinator has isolated session management for embedded use
- **Global Mode**: Process-wide session pool for server/daemon deployments

### Session Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    Session Manager                       │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Active Sessions                       │  │
│  │                                                    │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────┐   │  │
│  │  │ Session-001  │  │ Session-002  │  │ Session │   │  │
│  │  │ User: alice  │  │ User: bob    │  │  -003   │   │  │
│  │  │ Schema: prod │  │ Schema: dev  │  │  ...    │   │  │
│  │  │ Graph: main  │  │ Graph: test  │  │         │   │  │
│  │  └──────┬───────┘  └──────┬───────┘  └────┬────┘   │  │
│  │         │                 │                 │      │  │
│  └─────────┼─────────────────┼─────────────────┼──────┘  │
└────────────┼─────────────────┼─────────────────┼─────────┘
             │                 │                 │
             ▼                 ▼                 ▼
    ┌────────────────┐ ┌────────────────┐ ┌────────────────┐
    │ Transaction    │ │ Transaction    │ │ Transaction    │
    │ Context        │ │ Context        │ │ Context        │
    └────────────────┘ └────────────────┘ └────────────────┘
             │                 │                 │
             ▼                 ▼                 ▼
    ┌────────────────┐ ┌────────────────┐ ┌────────────────┐
    │ Permission     │ │ Permission     │ │ Permission     │
    │ Cache          │ │ Cache          │ │ Cache          │
    └────────────────┘ └────────────────┘ └────────────────┘
             │                 │                 │
             └─────────────────┴─────────────────┘
                               │
                               ▼
                     ┌─────────────────┐
                     │  Catalog        │
                     │  Manager        │
                     └─────────────────┘
```

### Session Lifecycle

```
1. CONNECT → 2. AUTHENTICATE → 3. SET CONTEXT → 4. EXECUTE → 5. DISCONNECT

┌──────────────┐
│  1. Connect  │
│  - Create ID │
│  - Init state│
└──────┬───────┘
       │
       ▼
┌──────────────────┐
│  2. Authenticate │
│  - Verify user   │
│  - Load roles    │
│  - Check perms   │
└──────┬───────────┘
       │
       ▼
┌────────────────────┐
│  3. Set Context    │
│  - SESSION SET     │
│    SCHEMA          │
│  - SESSION SET     │
│    GRAPH           │
└──────┬─────────────┘
       │
       ▼
┌────────────────────┐
│  4. Execute Queries│
│  - Parse           │
│  - Plan            │
│  - Execute         │
│  - Return results  │
└──────┬─────────────┘
       │
       ▼
┌────────────────────┐
│  5. Disconnect     │
│  - Cleanup         │
│  - Release locks   │
│  - Close TXN       │
└────────────────────┘
```

---

## Security Architecture

### Security Layers

```
┌───────────────────────────────────────────────────────────┐
│                    Security Architecture                  │
└───────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Layer 1: Authentication                                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Username/Password Authentication                    │   │
│  │  - Bcrypt password hashing                           │   │
│  │  - Secure credential storage in catalog              │   │
│  │  - Session token generation                          │   │
│  └──────────────────────────────────────────────────────┘   │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: Authorization (RBAC)                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Role Hierarchy                                      │   │
│  │                                                      │   │
│  │        ┌───────────┐                                 │   │
│  │        │   admin   │  (All permissions)              │   │
│  │        └─────┬─────┘                                 │   │
│  │              │                                       │   │
│  │      ┌───────┴────────┐                              │   │
│  │      │                │                              │   │
│  │  ┌───▼────┐      ┌─-──▼───┐                          │   │
│  │  │  user  │      │analyst │                          │   │
│  │  └────────┘      └────────┘                          │   │
│  │  (Read/Write)    (Read-only)                         │   │
│  └──────────────────────────────────────────────────────┘   │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: Permission Enforcement                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Permission Checks                                   │   │
│  │  - Schema access                                     │   │
│  │  - Graph access                                      │   │
│  │  - Operation permissions (DDL/DML/DQL)               │   │
│  │  - Row-level security (future)                       │   │
│  └─────────────────────────────────────────--───────────┘   │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 4: Audit Logging (future)                            │
│  - Query logging                                            │
│  - Access attempts                                          │
│  - Security events                                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Performance Optimizations

### Optimization Strategies

```
┌────────────────────────────────────────────────────────────┐
│              Performance Optimization Stack                │
└────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  1. Query-Level Optimizations                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  - Predicate pushdown                                │   │
│  │  - Projection pruning                                │   │
│  │  - Join order optimization                           │   │
│  │  - Constant folding                                  │   │
│  │  - Dead code elimination                             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  2. Index Optimizations (Future)                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  - Automatic index selection                         │   │
│  │  - Covering indexes                                  │   │
│  │  - Index-only scans                                  │   │
│  │  - Composite indexes                                 │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  3. Execution Optimizations                                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  - Parallel execution (Rayon)                        │   │
│  │  - Lazy evaluation                                   │   │
│  │  - Iterator-based streaming                          │   │
│  │  - Batch operations                                  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  4. Storage Optimizations                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  - Compression (Sled built-in)                       │   │
│  │  - Adjacency list optimization                       │   │
│  │  - Batch reads/writes                                │   │
│  │  - Memory-mapped files                               │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  5. Cache Optimizations                                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  - Multi-level caching                               │   │
│  │  - Intelligent cache invalidation                    │   │
│  │  - Query result caching                              │   │
│  │  - Plan caching                                      │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Performance Characteristics

| Operation | Complexity | Typical Time | Notes |
|-----------|-----------|--------------|-------|
| Node lookup by ID | O(1) | < 1ms | Direct hash lookup |
| Node scan | O(n) | ~10ms per 1K nodes | Sequential scan |
| Index lookup | O(log n) | < 5ms | B-tree index |
| 1-hop traversal | O(degree) | < 2ms | Adjacency list |
| Variable-length path | O(n^k) | Varies | Path length dependent |
| Aggregation | O(n) | ~50ms per 10K rows | Parallel execution |
| Join | O(n * m) | Varies | Join algorithm dependent |

---

## Conclusion

GraphLite's architecture is designed for:

**Performance** - Multi-level caching, parallel execution, cost-based optimization

**Reliability** - ACID transactions, WAL, MVCC

**Scalability** - Efficient storage, index support, streaming results

**Simplicity** - Embedded deployment, zero configuration

**Standards** - ISO GQL compliance

The modular design allows for easy extension and maintenance while providing excellent performance for graph workloads.

---
