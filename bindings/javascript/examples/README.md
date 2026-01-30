# GraphLite JavaScript Bindings - Examples

This directory contains example code demonstrating how to use the GraphLite JavaScript bindings.

## Running Examples

First, make sure you have:
1. Built the FFI library: `cargo build --release -p graphlite-ffi`
2. Installed dependencies: `npm install`

Then run any example:

```bash
node examples/basic-usage.js
node examples/error-handling.js
node examples/complex-queries.js
```

## Examples

### basic-usage.js
Demonstrates the fundamental operations:
- Opening a database
- Creating sessions
- Setting up schema and graphs
- Inserting data
- Querying data
- Using QueryResult methods
- Proper cleanup

### error-handling.js
Shows comprehensive error handling patterns:
- Catching GraphLiteError
- Checking specific error codes
- Error recovery strategies
- Retry logic with exponential backoff
- Safe query execution helpers

### complex-queries.js
Advanced query examples:
- Pattern matching with WHERE clauses
- Aggregations (count, avg, min, max, sum)
- Grouping and ordering
- Pagination with LIMIT
- Using QueryResult utility methods
- Complex filtering

## Creating Your Own Examples

Use any example as a template. The basic pattern is:

```javascript
const { GraphLite } = require('graphlite');

async function myExample() {
  const db = new GraphLite('./mydb');
  
  try {
    const session = db.createSession('username');
    
    // Your code here
    await db.execute(session, 'CREATE SCHEMA myschema');
    const result = await db.query(session, 'MATCH (n) RETURN n');
    
    await db.closeSession(session);
  } finally {
    db.close();
  }
}

myExample().catch(console.error);
```

## Tips

- Always use `try/finally` to ensure database cleanup
- Use `async/await` for cleaner code
- Check the [README.md](../README.md) for full API documentation
- See [test/graphlite.test.js](../test/graphlite.test.js) for more usage patterns
