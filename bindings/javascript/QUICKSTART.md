# GraphLite JavaScript Bindings - Quick Start

Get up and running with GraphLite JavaScript bindings in 5 minutes!

## Step 1: Install Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Node.js 14+ (if not already installed)
# Visit https://nodejs.org/ or use nvm:
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
```

## Step 2: Build GraphLite FFI

```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

## Step 3: Install JavaScript Package

```bash
cd bindings/javascript
npm install
```

## Step 4: Write Your First Script

Create `my-first-graph.js`:

```javascript
const { GraphLite } = require('graphlite');

async function main() {
  // Open database
  const db = new GraphLite('./my_graph_db');
  
  try {
    // Create session
    const session = db.createSession('me');
    
    // Setup
    await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS demo');
    await db.execute(session, 'SESSION SET SCHEMA demo');
    await db.execute(session, 'CREATE GRAPH IF NOT EXISTS mygraph');
    await db.execute(session, 'SESSION SET GRAPH mygraph');
    
    // Insert some data
    await db.execute(session, `
      INSERT (:Person {name: 'Alice', age: 30}),
             (:Person {name: 'Bob', age: 25})
    `);
    
    // Query the data
    const result = await db.query(session, `
      MATCH (p:Person)
      RETURN p.name, p.age
      ORDER BY p.age DESC
    `);
    
    // Display results
    console.log(`Found ${result.rowCount} people:`);
    for (const row of result.rows) {
      console.log(`- ${row['p.name']}, age ${row['p.age']}`);
    }
    
    // Cleanup
    await db.closeSession(session);
  } finally {
    db.close();
  }
}

main().catch(console.error);
```

## Step 5: Run It!

```bash
node my-first-graph.js
```

Expected output:
```
Found 2 people:
- Alice, age 30
- Bob, age 25
```

## Next Steps

### Explore Examples
```bash
node examples/basic-usage.js
node examples/complex-queries.js
node examples/error-handling.js
```

### Read Documentation
- [README.md](README.md) - Complete API reference
- [INSTALL.md](INSTALL.md) - Detailed installation guide
- [examples/README.md](examples/README.md) - Example code guide

### Try More Complex Queries

```javascript
// Aggregation
const stats = await db.query(session, `
  MATCH (p:Person)
  RETURN count(p) as total, avg(p.age) as avg_age
`);
console.log(stats.first());

// Filtering
const adults = await db.query(session, `
  MATCH (p:Person)
  WHERE p.age >= 18
  RETURN p.name, p.age
`);

// Using QueryResult methods
const names = adults.column('p.name');
const firstPerson = adults.first();
const filtered = adults.filter(row => row['p.age'] > 25);
```

### Error Handling

```javascript
const { GraphLiteError, ErrorCode } = require('graphlite');

try {
  const result = await db.query(session, 'MATCH (n) RETURN n');
} catch (error) {
  if (error instanceof GraphLiteError) {
    console.error(`Error ${error.code}: ${error.message}`);
  }
}
```

### TypeScript Usage

Create `app.ts`:

```typescript
import { GraphLite, QueryResult } from 'graphlite';

async function main(): Promise<void> {
  const db: GraphLite = new GraphLite('./mydb');
  
  try {
    const session: string = db.createSession('user');
    const result: QueryResult = await db.query(session, 'MATCH (n) RETURN n');
    
    console.log(`Rows: ${result.rowCount}`);
  } finally {
    db.close();
  }
}
```

## Common Patterns

### Setup Template
```javascript
const db = new GraphLite('./mydb');
const session = db.createSession('user');

await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS myschema');
await db.execute(session, 'SESSION SET SCHEMA myschema');
await db.execute(session, 'CREATE GRAPH IF NOT EXISTS mygraph');
await db.execute(session, 'SESSION SET GRAPH mygraph');
```

### Insert Pattern
```javascript
await db.execute(session, `
  INSERT (:Label {prop1: 'value1', prop2: 123}),
         (:Label {prop1: 'value2', prop2: 456})
`);
```

### Query Pattern
```javascript
const result = await db.query(session, `
  MATCH (n:Label)
  WHERE n.prop2 > 100
  RETURN n.prop1, n.prop2
  ORDER BY n.prop2 DESC
  LIMIT 10
`);

for (const row of result.rows) {
  console.log(row);
}
```

## Troubleshooting

### "Could not find GraphLite FFI library"

Build it first:
```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

Or set environment variable:
```bash
export GRAPHLITE_FFI_LIB=/path/to/GraphLite/target/release/libgraphlite_ffi.dylib
```

### "ffi-napi installation failed"

Install build tools:
- **macOS**: `xcode-select --install`
- **Linux**: `sudo apt-get install build-essential`
- **Windows**: Install Visual Studio Build Tools

Then:
```bash
npm install --build-from-source
```

## Help & Support

- 📖 Documentation: [README.md](README.md)
- 🐛 Issues: https://github.com/deepgraph/graphlite/issues
- 💬 Discussions: https://github.com/deepgraph/graphlite/discussions

## What's Next?

1. ✅ You've created your first graph!
2. 📚 Read the full [API documentation](README.md)
3. 🎯 Try the [examples](examples/)
4. 🧪 Run the [tests](test/)
5. 🚀 Build something awesome!

Happy graphing! 🎉
