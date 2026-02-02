/**
 * Error handling example for GraphLite JavaScript bindings
 * 
 * Demonstrates:
 * - Catching and handling GraphLiteError
 * - Checking error codes
 * - Proper error recovery
 * - Cleanup in error scenarios
 */

const { GraphLite, GraphLiteError, ErrorCode } = require('..');

async function demonstrateErrorHandling() {
  console.log('GraphLite Error Handling Example');
  console.log('=================================\n');

  // Example 1: Database open error
  console.log('1. Testing database open with invalid path...');
  try {
    const db = new GraphLite('/invalid/path/that/does/not/exist');
    db.close();
  } catch (error) {
    if (error instanceof GraphLiteError) {
      console.log(`   ✓ Caught GraphLiteError: ${error.errorName}`);
      console.log(`   ✓ Error code: ${error.code}`);
      console.log(`   ✓ Message: ${error.message}\n`);
    }
  }

  // Example 2: Invalid query
  console.log('2. Testing invalid query...');
  const db = new GraphLite('./test_db');
  
  try {
    const session = db.createSession('user1');
    
    // Try an invalid query
    try {
      await db.query(session, 'THIS IS NOT VALID GQL');
    } catch (error) {
      if (error instanceof GraphLiteError) {
        console.log(`   ✓ Caught GraphLiteError: ${error.errorName}`);
        console.log(`   ✓ Is query error: ${error.isCode(ErrorCode.QUERY_ERROR)}`);
        console.log(`   ✓ Message: ${error.message.substring(0, 100)}...\n`);
      }
    }

    // Example 3: Handling specific error codes
    console.log('3. Testing error code checking...');
    try {
      await db.query(session, 'MATCH (n:NonExistent) RETURN n.property');
    } catch (error) {
      if (error instanceof GraphLiteError) {
        if (error.isCode(ErrorCode.QUERY_ERROR)) {
          console.log('   ✓ Handled query error specifically');
        } else if (error.isCode(ErrorCode.SESSION_ERROR)) {
          console.log('   ✓ Handled session error');
        } else {
          console.log(`   ✓ Other error: ${error.errorName}`);
        }
      }
    }
    console.log();

    // Example 4: Graceful degradation
    console.log('4. Testing graceful degradation...');
    try {
      await db.execute(session, 'SESSION SET SCHEMA example');
      await db.execute(session, 'SESSION SET GRAPH social');
      
      const result = await db.query(session, 'MATCH (n) RETURN n LIMIT 5');
      console.log(`   ✓ Query succeeded: ${result.rowCount} rows`);
    } catch (error) {
      console.log('   ✓ Query failed, using fallback behavior');
      // Implement fallback logic here
    }
    console.log();

    // Example 5: Error recovery
    console.log('5. Testing error recovery...');
    let retries = 3;
    let success = false;
    
    while (retries > 0 && !success) {
      try {
        await db.execute(session, 'CREATE SCHEMA IF NOT EXISTS example');
        await db.execute(session, 'SESSION SET SCHEMA example');
        await db.execute(session, 'CREATE GRAPH IF NOT EXISTS social');
        await db.execute(session, 'SESSION SET GRAPH social');
        success = true;
        console.log('   ✓ Operation succeeded');
      } catch (error) {
        retries--;
        console.log(`   ✓ Attempt failed, ${retries} retries remaining`);
        if (retries === 0) {
          console.log('   ✗ All retries exhausted');
        }
      }
    }
    console.log();

    // Example 6: Error information
    console.log('6. Testing error information extraction...');
    try {
      await db.query(session, 'INVALID SYNTAX HERE');
    } catch (error) {
      if (error instanceof GraphLiteError) {
        const errorInfo = error.toJSON();
        console.log('   Error information:');
        console.log(`   - Name: ${errorInfo.name}`);
        console.log(`   - Code: ${errorInfo.code}`);
        console.log(`   - Error Name: ${errorInfo.errorName}`);
        console.log(`   - Message: ${errorInfo.message.substring(0, 80)}...`);
      }
    }
    console.log();

    await db.closeSession(session);
    
  } catch (error) {
    console.error('Unexpected error:', error);
  } finally {
    db.close();
  }

  console.log('Error handling examples completed!');
}

// Helper function: Safe query execution
async function safeQuery(db, sessionId, query, defaultValue = null) {
  try {
    return await db.query(sessionId, query);
  } catch (error) {
    if (error instanceof GraphLiteError) {
      console.warn(`Query failed: ${error.message}`);
      return defaultValue;
    }
    throw error;
  }
}

// Helper function: Retry with exponential backoff
async function queryWithRetry(db, sessionId, query, maxRetries = 3) {
  let lastError;
  
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await db.query(sessionId, query);
    } catch (error) {
      lastError = error;
      if (!(error instanceof GraphLiteError)) {
        throw error; // Don't retry non-GraphLite errors
      }
      
      if (i < maxRetries - 1) {
        const delay = Math.pow(2, i) * 100; // Exponential backoff
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }
  }
  
  throw lastError;
}

// Run the example
if (require.main === module) {
  demonstrateErrorHandling().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

module.exports = { demonstrateErrorHandling, safeQuery, queryWithRetry };
