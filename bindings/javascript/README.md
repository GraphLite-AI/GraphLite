# GraphLite JavaScript/Node.js Bindings

JavaScript/Node.js bindings for GraphLite embedded graph database using FFI.

## Installation

### Prerequisites

1. **Build the GraphLite FFI library first**:
   ```bash
   cd /path/to/GraphLite
   cargo build --release -p graphlite-ffi
   ```

2. **Install the npm package**:
   ```bash
   cd bindings/javascript
   npm install
   ```

## Quick Start

```javascript
const { GraphLite } = require('graphlite');

async function main() {
  // Open database
  const db = new GraphLite('./mydb');
  
  try {
    // Create session
    const session = db.createSession('admin');
    
    // Set up schema and graph
    await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS example');
    await db.execute(session, 'SESSION SET SCHEMA example');
    await db.execute(session, 'CREATE GRAPH IF NOT EXISTS social');
    await db.execute(session, 'SESSION SET GRAPH social');
    
    // Insert data
    await db.execute(session, `
      INSERT (:Person {name: 'Alice', age: 30}),
             (:Person {name: 'Bob', age: 25})
    `);
    
    // Query data
    const result = await db.query(session, `
      MATCH (p:Person)
      RETURN p.name, p.age
      ORDER BY p.age DESC
    `);
    
    console.log(`Found ${result.rowCount} rows`);
    for (const row of result.rows) {
      console.log(`Name: ${row['p.name']}, Age: ${row['p.age']}`);
    }
    
    // Close session
    await db.closeSession(session);
  } finally {
    // Close database
    db.close();
  }
}

main().catch(console.error);
```

## API Reference

### GraphLite Class

#### Constructor

```javascript
const db = new GraphLite(path);
```

Opens a GraphLite database at the specified path.

**Parameters:**
- `path` (string): Path to database directory

**Throws:** `GraphLiteError` if database cannot be opened

#### createSession(username)

```javascript
const sessionId = db.createSession('admin');
```

Creates a new session for the given user.

**Parameters:**
- `username` (string): Username for the session

**Returns:** Session ID string

**Throws:** `GraphLiteError` if session creation fails

#### query(sessionId, query)

```javascript
const result = await db.query(sessionId, 'MATCH (n) RETURN n');
```

Executes a GQL query and returns results.

**Parameters:**
- `sessionId` (string): Session ID from `createSession()`
- `query` (string): GQL query string

**Returns:** Promise that resolves to `QueryResult`

**Throws:** `GraphLiteError` if query execution fails

#### execute(sessionId, statement)

```javascript
await db.execute(sessionId, 'CREATE SCHEMA myschema');
```

Executes a statement without returning results. Useful for DDL/DML statements.

**Parameters:**
- `sessionId` (string): Session ID from `createSession()`
- `statement` (string): GQL statement to execute

**Returns:** Promise that resolves when complete

**Throws:** `GraphLiteError` if execution fails

#### closeSession(sessionId)

```javascript
await db.closeSession(sessionId);
```

Closes a session.

**Parameters:**
- `sessionId` (string): Session ID to close

**Returns:** Promise that resolves when complete

**Throws:** `GraphLiteError` if session close fails

#### close()

```javascript
db.close();
```

Closes the database and all open sessions.

#### static version()

```javascript
const version = GraphLite.version();
console.log(`GraphLite version: ${version}`);
```

Returns the GraphLite version string.

#### isOpen()

```javascript
if (db.isOpen()) {
  console.log('Database is open');
}
```

Returns `true` if the database is open, `false` otherwise.

#### getSessionCount()

```javascript
const count = db.getSessionCount();
console.log(`Open sessions: ${count}`);
```

Returns the number of open sessions.

### QueryResult Class

The `QueryResult` class provides convenient methods for accessing query results.

#### Properties

- `variables` (Array\<string\>): Column/variable names
- `rows` (Array\<Object\>): Array of row objects
- `rowCount` (number): Number of rows

#### first()

```javascript
const firstRow = result.first();
if (firstRow) {
  console.log(firstRow);
}
```

Returns the first row or `null` if no rows.

#### column(name)

```javascript
const names = result.column('p.name');
console.log(names); // ['Alice', 'Bob']
```

Returns all values from a specific column.

#### select(...columnNames)

```javascript
const subset = result.select('p.name', 'p.age');
```

Returns array of objects with only the specified columns.

#### filter(predicate)

```javascript
const adults = result.filter(row => row['p.age'] >= 18);
```

Returns filtered rows based on predicate function.

#### map(transformer)

```javascript
const names = result.map(row => row['p.name']);
```

Maps rows using a transformer function.

#### toArray()

```javascript
const allRows = result.toArray();
```

Converts result to an array of rows.

#### isEmpty()

```javascript
if (result.isEmpty()) {
  console.log('No results found');
}
```

Returns `true` if result has no rows.

#### Iteration

QueryResult is iterable:

```javascript
for (const row of result) {
  console.log(row);
}
```

### Error Handling

```javascript
const { GraphLite, GraphLiteError, ErrorCode } = require('graphlite');

async function queryWithErrorHandling() {
  const db = new GraphLite('./mydb');
  
  try {
    const session = db.createSession('user1');
    const result = await db.query(session, 'MATCH (n) RETURN n');
    return result.rows;
  } catch (error) {
    if (error instanceof GraphLiteError) {
      console.error(`GraphLite error (${error.errorName}): ${error.message}`);
      console.error(`Error code: ${error.code}`);
      
      if (error.isCode(ErrorCode.QUERY_ERROR)) {
        console.error('Query syntax error');
      }
    } else {
      console.error('Unexpected error:', error);
    }
  } finally {
    db.close();
  }
}
```

#### ErrorCode Enum

- `SUCCESS` (0): Operation succeeded
- `NULL_POINTER` (1): Null pointer was passed
- `INVALID_UTF8` (2): Invalid UTF-8 string
- `DATABASE_OPEN_ERROR` (3): Failed to open database
- `SESSION_ERROR` (4): Failed to create or manage session
- `QUERY_ERROR` (5): Query execution failed
- `PANIC_ERROR` (6): Internal panic occurred
- `JSON_ERROR` (7): JSON serialization/deserialization failed

## Usage Examples

### Basic CRUD Operations

```javascript
const { GraphLite } = require('graphlite');

async function basicCrud() {
  const db = new GraphLite('./mydb');
  const session = db.createSession('admin');
  
  try {
    // Setup
    await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS app');
    await db.execute(session, 'SESSION SET SCHEMA app');
    await db.execute(session, 'CREATE GRAPH IF NOT EXISTS data');
    await db.execute(session, 'SESSION SET GRAPH data');
    
    // Create
    await db.execute(session, `
      INSERT (:User {id: 1, name: 'Alice', email: 'alice@example.com'}),
             (:User {id: 2, name: 'Bob', email: 'bob@example.com'})
    `);
    
    // Read
    const users = await db.query(session, 'MATCH (u:User) RETURN u');
    console.log('Users:', users.rows);
    
    // Query with WHERE
    const result = await db.query(session, `
      MATCH (u:User)
      WHERE u.name = 'Alice'
      RETURN u.name, u.email
    `);
    console.log('Alice:', result.first());
    
  } finally {
    await db.closeSession(session);
    db.close();
  }
}
```

### Complex Queries

```javascript
async function complexQueries() {
  const db = new GraphLite('./mydb');
  const session = db.createSession('admin');
  
  try {
    // Aggregation
    const stats = await db.query(session, `
      MATCH (u:User)
      RETURN count(u) as total, avg(u.age) as avg_age
    `);
    const { total, avg_age } = stats.first();
    console.log(`Total users: ${total}, Average age: ${avg_age}`);
    
    // Ordering
    const sorted = await db.query(session, `
      MATCH (u:User)
      RETURN u.name, u.age
      ORDER BY u.age DESC
      LIMIT 10
    `);
    
    // Pattern matching
    const friends = await db.query(session, `
      MATCH (a:User)-[:FRIENDS_WITH]->(b:User)
      WHERE a.name = 'Alice'
      RETURN b.name
    `);
    
  } finally {
    await db.closeSession(session);
    db.close();
  }
}
```

### TypeScript Support

```typescript
import { GraphLite, QueryResult, GraphLiteError, ErrorCode } from 'graphlite';

async function typescriptExample(): Promise<void> {
  const db: GraphLite = new GraphLite('./mydb');
  
  try {
    const session: string = db.createSession('admin');
    const result: QueryResult = await db.query(session, 'MATCH (n) RETURN n');
    
    const rows: Array<Record<string, any>> = result.rows;
    const firstRow: Record<string, any> | null = result.first();
    
    await db.closeSession(session);
  } catch (error) {
    if (error instanceof GraphLiteError) {
      console.error(`Error ${error.code}: ${error.message}`);
    }
  } finally {
    db.close();
  }
}
```

## Environment Variables

- `GRAPHLITE_FFI_LIB`: Path to the GraphLite FFI library (optional)
  ```bash
  export GRAPHLITE_FFI_LIB=/path/to/libgraphlite_ffi.dylib
  ```

## Platform Support

- **macOS**: `libgraphlite_ffi.dylib`
- **Linux**: `libgraphlite_ffi.so`
- **Windows**: `graphlite_ffi.dll`

## Troubleshooting

### Library Not Found

If you get an error about the FFI library not being found:

1. Build the FFI library:
   ```bash
   cargo build --release -p graphlite-ffi
   ```

2. Set the `GRAPHLITE_FFI_LIB` environment variable:
   ```bash
   export GRAPHLITE_FFI_LIB=/path/to/GraphLite/target/release/libgraphlite_ffi.dylib
   ```

### FFI Module Installation Issues

If `ffi-napi` fails to install:

```bash
npm install --build-from-source
```

Or install prerequisites:
- **macOS**: `xcode-select --install`
- **Linux**: `sudo apt-get install build-essential`
- **Windows**: Install Visual Studio Build Tools

## Development

### Running Tests

```bash
npm test
```

### Linting

```bash
npm run lint
```

### Formatting

```bash
npm run format
```

## Architecture

The JavaScript bindings follow this architecture:

```
┌─────────────────────────────┐
│   Your JavaScript App       │
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│   GraphLite JS Bindings     │
│   (ffi-napi)                │
└─────────────────────────────┘
              │ FFI calls
              ▼
┌─────────────────────────────┐
│   graphlite-ffi             │
│   (C-compatible API)        │
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│   GraphLite Core (Rust)     │
└─────────────────────────────┘
```

## License

Apache-2.0

## Links

- [GraphLite GitHub](https://github.com/deepgraph/graphlite)
- [Documentation](https://github.com/deepgraph/graphlite/tree/main/bindings/javascript)
- [Issue Tracker](https://github.com/deepgraph/graphlite/issues)
