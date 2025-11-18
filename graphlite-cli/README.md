# GraphLite CLI

Command-line interface for GraphLite graph database.

## Overview

The GraphLite CLI provides an interactive console and command-line tools for managing GraphLite databases. It offers a SQLite-like experience for graph databases - simple, embedded, and zero-configuration.

## Installation

The CLI is built as part of the GraphLite workspace:

```bash
# Build the CLI
cargo build --release --bin graphlite
```

After building, the binary will be available at `target/release/graphlite`.

## Commands

### 1. Install Database

Initialize a new GraphLite database with an admin user:

```bash
graphlite install --path ./mydb --admin-user admin --admin-password secret
```

**Options:**
- `--path <PATH>` - Database directory path (default: `./db`)
- `--admin-user <USER>` - Admin username
- `--admin-password <PASS>` - Admin password
- `--yes` - Skip confirmation prompts

**What it does:**
- Creates database directory and files
- Sets up admin user with credentials
- Creates default admin and user roles
- Initializes the default schema

### 2. Interactive Console (GQL REPL)

Start an interactive GQL console:

```bash
graphlite gql --path ./mydb -u admin -p secret
```

**Options:**
- `--path <PATH>` - Database directory path (default: `./db`)
- `-u, --user <USER>` - Username for authentication
- `-p, --password <PASS>` - Password for authentication

**Example Session:**
```
$ graphlite gql --path ./mydb -u admin -p secret
GraphLite v0.1.0 - Interactive GQL Console
Type 'exit' or 'quit' to exit, 'help' for help

gql> CREATE SCHEMA /social;
Schema created successfully

gql> CREATE GRAPH /social/friends;
Graph created successfully

gql> SESSION SET GRAPH /social/friends;
Graph context set

gql> INSERT (:Person {name: 'Alice', age: 30});
1 node inserted

gql> MATCH (p:Person) RETURN p.name, p.age;
┌────────┬───────┐
│ name   │ age   │
├────────┼───────┤
│ Alice  │ 30    │
└────────┴───────┘
1 row

gql> exit
Goodbye!
```

### 3. Execute Single Query

Run a single GQL query and exit:

```bash
graphlite query --path ./mydb -u admin -p secret "MATCH (n) RETURN n LIMIT 10"
```

**Options:**
- `--path <PATH>` - Database directory path (default: `./db`)
- `-u, --user <USER>` - Username for authentication
- `-p, --password <PASS>` - Password for authentication
- `--format <FORMAT>` - Output format: `table`, `json`, or `csv` (default: `table`)

**Output Formats:**

**Table (default):**
```bash
graphlite query --path ./mydb -u admin -p secret --format table "MATCH (p:Person) RETURN p.name"
```
```
┌────────┐
│ name   │
├────────┤
│ Alice  │
│ Bob    │
└────────┘
```

**JSON:**
```bash
graphlite query --path ./mydb -u admin -p secret --format json "MATCH (p:Person) RETURN p.name"
```
```json
{
  "columns": ["name"],
  "rows": [
    {"name": "Alice"},
    {"name": "Bob"}
  ],
  "row_count": 2
}
```

**CSV:**
```bash
graphlite query --path ./mydb -u admin -p secret --format csv "MATCH (p:Person) RETURN p.name, p.age"
```
```
name,age
Alice,30
Bob,25
```

### 4. Session Management

Create and manage database sessions:

```bash
graphlite session --path ./mydb -u admin -p secret
```

**Options:**
- `--path <PATH>` - Database directory path
- `-u, --user <USER>` - Username
- `-p, --password <PASS>` - Password

### 5. Version Information

Display version information:

```bash
graphlite --version
# or
graphlite version
```

### 6. Help

Show help information:

```bash
graphlite --help
graphlite <command> --help
```

## Quick Start

```bash
# 1. Install database
graphlite install --path ./demo --admin-user admin --admin-password secret --yes

# 2. Launch interactive console
graphlite gql --path ./demo -u admin -p secret

# 3. Create and use a graph (in the console)
gql> CREATE SCHEMA /demo;
gql> CREATE GRAPH /demo/social;
gql> SESSION SET GRAPH /demo/social;

# 4. Insert data
gql> INSERT (:Person {name: 'Alice', age: 30});
gql> INSERT (:Person {name: 'Bob', age: 25});
gql> INSERT (:Person {name: 'Alice'})-[:KNOWS]->(:Person {name: 'Bob'});

# 5. Query data
gql> MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age;

# 6. Exit
gql> exit
```

## Environment Variables

- `GRAPHLITE_DB_PATH` - Default database path (overridden by `--path`)
- `GRAPHLITE_USER` - Default username (overridden by `-u`)

## Configuration Files

Currently, GraphLite CLI does not use configuration files. All settings are passed via command-line arguments or environment variables.

## Database Location

The default database path is `./db` in the current directory. You can specify a different path with the `--path` option.

**Database structure:**
```
mydb/
├── catalog/          # Catalog metadata
├── graphs/           # Graph data
├── wal/              # Write-ahead log
└── [sled files]      # Embedded storage files
```

## Error Handling

The CLI provides clear error messages for common issues:

- **Database not found:** Run `graphlite install` first
- **Authentication failed:** Check username and password
- **Query syntax error:** The error message shows the issue location
- **Permission denied:** User doesn't have access to the resource

## Scripting with GraphLite CLI

### Execute queries from file:

```bash
cat queries.gql | while read query; do
  graphlite query --path ./mydb -u admin -p secret "$query"
done
```

### Batch insert from CSV (with preprocessing):

```bash
# Convert CSV to GQL INSERT statements
awk -F',' 'NR>1 {print "INSERT (:Person {name: '\''" $1 "'\'', age: " $2 "});"}' data.csv | \
  while read query; do
    graphlite query --path ./mydb -u admin -p secret "$query"
  done
```

### JSON output for further processing:

```bash
graphlite query --path ./mydb -u admin -p secret --format json \
  "MATCH (p:Person) RETURN p.name, p.age" | jq '.rows[] | .name'
```

## Development

The CLI is part of the GraphLite workspace. To build from source:

```bash
# Clone repository
git clone https://github.com/yourusername/graphlite.git
cd graphlite

# Build CLI only
cargo build --bin graphlite

# Run without installing
cargo run --bin graphlite -- --help

# Run with arguments
cargo run --bin graphlite -- install --path ./testdb --admin-user admin --admin-password secret
```

## Architecture

The GraphLite CLI is a thin wrapper around the GraphLite core library (`graphlite` crate). It:

- Uses the public `QueryCoordinator` API only
- Handles user input/output formatting
- Manages authentication and sessions
- Provides interactive REPL experience

For embedding GraphLite in applications, use the core library directly instead of the CLI.

## See Also

- [Main README](../README.md) - GraphLite overview
- [GQL Guide](../Getting%20Started%20With%20GQL.md) - Complete GQL syntax reference
- [examples/simple_usage.rs](../examples-core/simple_usage.rs) - Embedding GraphLite in Rust

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.
