# macOS Command Line Example

A simple command-line demo showing GraphLite usage on macOS.

## What it demonstrates

- Database initialization
- Session management
- Schema and graph creation
- Data insertion
- Basic queries
- Filtering with WHERE clauses
- Aggregations (COUNT, AVG, MIN, MAX)
- Grouping and ordering results

## Running the example

### Option 1: Using Swift Package Manager

From the `bindings/swift` directory:

```bash
# Build the package first
swift build

# Then run the demo (requires modifying Package.swift to add executable target)
swift run CommandLineDemo
```

### Option 2: Direct Swift execution

```bash
cd Examples/macOS
swift CommandLineDemo.swift
```

This will import GraphLite from the parent package and run the demo.

### Option 3: Compile and run

```bash
cd Examples/macOS

# Compile
swiftc \
    -I ../../Sources \
    -I ../../Sources/CGraphLite/include \
    -L ../../.build/debug \
    -lgraphlit_ffi \
    CommandLineDemo.swift \
    ../../Sources/GraphLite/*.swift \
    -o demo

# Run
./demo
```

## Expected output

```
========================================
GraphLite macOS Command Line Demo
========================================

Creating database at: /tmp/graphlite_demo_<UUID>
 Database initialized
 GraphLite version: 0.1.0

 Session created

Setting up schema and graph...
 Schema '/demo' and graph '/demo/social' created

Inserting people...
 4 people inserted

Querying all people...
Found 4 people:
  1. Bob, age 25, from San Francisco
  2. Carol, age 28, from Los Angeles
  3. Alice, age 30, from New York
  4. Dave, age 32, from New York

Querying people from New York...
Found 2 people in New York:
  - Alice, age 30
  - Dave, age 32

Calculating statistics...
Statistics:
  Total people: 4
  Average age: 28.8
  Age range: 25 - 32

People by city...
City distribution:
  New York: 2 people
  Los Angeles: 1 person
  San Francisco: 1 person

 Session closed
 Database cleaned up

========================================
Demo completed successfully!
========================================
```

## Code walkthrough

1. **Database initialization**: Creates a temporary database
2. **Session creation**: Establishes a session for query execution
3. **Schema setup**: Creates `/demo` schema and `/demo/social` graph
4. **Data insertion**: Inserts 4 Person nodes with properties
5. **Queries**:
   - Fetch all people ordered by age
   - Filter by city (WHERE clause)
   - Calculate aggregations (COUNT, AVG, MIN, MAX)
   - Group by city and count

## Next steps

Try modifying the demo to:
- Add relationships between people (KNOWS, FRIENDS_WITH)
- Query relationship patterns
- Update existing nodes
- Delete nodes
- Use transactions for batch operations

## Troubleshooting

**Error: "cannot find 'GraphLite' in scope"**

Make sure you've built the Swift package:
```bash
cd bindings/swift
swift build
```

**Error: "library not found for -lgraphlit_ffi"**

Build the FFI library first:
```bash
cd graphlite-ffi
cargo build --release
```
