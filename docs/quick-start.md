# GraphLite Quick Start Guide

Get GraphLite running and execute your first graph queries in a few minutes.

## Prerequisites

- Rust (via `rustup`) — tested on Rust 1.70+
- Git (for cloning the repository)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Installation (build from source)

```bash
git clone https://github.com/GraphLite-AI/GraphLite.git
cd GraphLite
cargo build --release
```

The CLI binary will be at `target/release/graphlite` after a successful build.

## Initialize a database

```bash
./target/release/graphlite install --path ./my_db --admin-user admin --admin-password secret
```

## Start the REPL

```bash
./target/release/graphlite gql --path ./my_db -u admin -p secret
```

## Example: insert and query

```gql
CREATE SCHEMA /social;
SESSION SET SCHEMA /social;
CREATE GRAPH /social/network;
SESSION SET GRAPH /social/network;

INSERT (:Person {name: 'Alice'}), (:Person {name: 'Bob'});

MATCH (p:Person) RETURN p.name;
```

## CLI quick commands

```bash
./target/release/graphlite --help
./target/release/graphlite version
./target/release/graphlite query "MATCH (n) RETURN n" --path ./my_db -u admin -p secret
```

## Next steps

See `Getting Started With GQL.md` for a language tutorial and `graphlite-sdk/examples` for integration examples.
