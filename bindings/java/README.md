# GraphLite Java Bindings

Java bindings for GraphLite embedded graph database using JNA (Java Native Access).

## Installation

### Prerequisites

1. **Java 11 or higher**
2. **Maven 3.6+**
3. **Build the FFI library** first:
   ```bash
   cd /path/to/GraphLite
   cargo build --release -p graphlite-ffi
   ```

### Add to Your Project

#### Maven

Add to your `pom.xml`:

```xml
<dependency>
    <groupId>com.deepgraph</groupId>
    <artifactId>graphlite</artifactId>
    <version>0.1.0</version>
</dependency>
```

#### Gradle

Add to your `build.gradle`:

```gradle
dependencies {
    implementation 'com.deepgraph:graphlite:0.1.0'
}
```

### Build from Source

```bash
cd bindings/java
mvn clean install
```

## Quick Start

```java
import com.deepgraph.graphlite.GraphLite;
import com.deepgraph.graphlite.QueryResult;

public class Example {
    public static void main(String[] args) {
        // Open database (auto-closes with try-with-resources)
        try (GraphLite db = GraphLite.open("./mydb")) {

            // Create session
            String session = db.createSession("admin");

            // Execute queries
            db.execute(session, "CREATE (p:Person {name: 'Alice', age: 30})");

            // Query data
            QueryResult result = db.query(session,
                "MATCH (p:Person) RETURN p.name, p.age");

            for (Map<String, Object> row : result.getRows()) {
                System.out.println(row);
            }

        } catch (GraphLiteException e) {
            System.err.println("Error: " + e.getMessage());
        }
    }
}
```

## Usage

### Opening a Database

```java
// Try-with-resources (recommended - auto-closes)
try (GraphLite db = GraphLite.open("./mydb")) {
    // ... use database
} // Automatically closed

// Manual management
GraphLite db = GraphLite.open("./mydb");
try {
    // ... use database
} finally {
    db.close();
}
```

### Creating Sessions

```java
GraphLite db = GraphLite.open("./mydb");

// Create session
String sessionId = db.createSession("username");

// Use session for queries
QueryResult result = db.query(sessionId, "MATCH (n) RETURN n");

// Close session when done
db.closeSession(sessionId);
```

### Executing Statements

```java
// DDL statements
db.execute(session, "CREATE SCHEMA myschema");
db.execute(session, "USE SCHEMA myschema");
db.execute(session, "CREATE GRAPH social");
db.execute(session, "USE GRAPH social");

// DML statements
db.execute(session, "CREATE (p:Person {name: 'Alice', age: 30})");
db.execute(session, "CREATE (p:Person {name: 'Bob', age: 25})");
```

### Querying Data

```java
// Simple query
QueryResult result = db.query(session,
    "MATCH (p:Person) RETURN p.name, p.age");

// Access result properties
System.out.println("Found " + result.getRowCount() + " rows");
System.out.println("Columns: " + result.getVariables());

// Iterate rows
for (Map<String, Object> row : result.getRows()) {
    System.out.println("Name: " + row.get("p.name"));
    System.out.println("Age: " + row.get("p.age"));
}

// Get first row
Map<String, Object> firstRow = result.first();
if (firstRow != null) {
    System.out.println(firstRow);
}

// Get column values
List<Object> names = result.column("p.name");
System.out.println(names);  // [Alice, Bob]
```

### Complex Queries

```java
// WHERE clause
QueryResult result = db.query(session,
    "MATCH (p:Person) WHERE p.age > 25 RETURN p.name, p.age");

// ORDER BY
result = db.query(session,
    "MATCH (p:Person) RETURN p.name, p.age ORDER BY p.age DESC");

// Aggregation
result = db.query(session,
    "MATCH (p:Person) RETURN count(p) as total, avg(p.age) as avg_age");
Map<String, Object> stats = result.first();
System.out.println("Total: " + stats.get("total"));
System.out.println("Average: " + stats.get("avg_age"));
```

### Error Handling

```java
import com.deepgraph.graphlite.GraphLite;
import com.deepgraph.graphlite.GraphLite.GraphLiteException;
import com.deepgraph.graphlite.GraphLite.ErrorCode;

try {
    GraphLite db = GraphLite.open("./mydb");
    String session = db.createSession("admin");
    QueryResult result = db.query(session, "MATCH (n) RETURN n");

} catch (GraphLiteException e) {
    System.err.println("Error: " + e.getMessage());
    System.err.println("Error code: " + e.getErrorCode());

    if (e.getErrorCode() == ErrorCode.QUERY_ERROR) {
        System.err.println("Query syntax error");
    } else if (e.getErrorCode() == ErrorCode.DATABASE_OPEN_ERROR) {
        System.err.println("Cannot open database");
    }
}
```

### Try-with-Resources Pattern

```java
// Recommended: automatic cleanup
try (GraphLite db = GraphLite.open("./mydb")) {
    String session = db.createSession("admin");

    db.execute(session, "CREATE (p:Person {name: 'Alice'})");
    QueryResult result = db.query(session, "MATCH (p:Person) RETURN p");

    for (Map<String, Object> row : result.getRows()) {
        System.out.println(row);
    }
} // Database automatically closed
```

## API Reference

### GraphLite

Main database class.

#### Static Methods

- `GraphLite open(String path)` - Open database at path
- `String version()` - Get GraphLite version

#### Instance Methods

- `String createSession(String username)` - Create session, returns session ID
- `QueryResult query(String sessionId, String query)` - Execute query, returns results
- `void execute(String sessionId, String statement)` - Execute statement without results
- `void closeSession(String sessionId)` - Close a session
- `void close()` - Close database

### QueryResult

Query result wrapper.

#### Methods

- `List<String> getVariables()` - Get column names from RETURN clause
- `List<Map<String, Object>> getRows()` - Get all result rows
- `int getRowCount()` - Get number of rows
- `Map<String, Object> first()` - Get first row or null
- `List<Object> column(String name)` - Get all values from a column
- `boolean isEmpty()` - Check if result is empty

### GraphLiteException

Exception thrown for GraphLite errors.

#### Methods

- `ErrorCode getErrorCode()` - Get error code enum
- `String getMessage()` - Get error message

### ErrorCode Enum

Error code enumeration.

#### Values

- `SUCCESS` - Operation succeeded
- `NULL_POINTER` - Null pointer error
- `INVALID_UTF8` - Invalid UTF-8 string
- `DATABASE_OPEN_ERROR` - Failed to open database
- `SESSION_ERROR` - Session operation failed
- `QUERY_ERROR` - Query execution failed
- `PANIC_ERROR` - Internal panic
- `JSON_ERROR` - JSON parsing error

## Examples

### Compile and Run Example

```bash
# Build GraphLite FFI library
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi

# Build and run Java example
cd bindings/java
mvn clean compile

# Run example (option 1 - via Maven exec plugin)
mvn exec:java -Dexec.mainClass="BasicUsage" -Dexec.classpathScope="compile"

# Run example (option 2 - compile and run manually)
javac -cp "target/classes:$(mvn dependency:build-classpath | grep -v '\[INFO\]')" \
    examples/BasicUsage.java
java -cp "target/classes:$(mvn dependency:build-classpath | grep -v '\[INFO\]'):examples" \
    BasicUsage
```

## Performance Considerations

- **JNA Overhead**: ~15-25% overhead compared to JNI
- **JSON Serialization**: Results are serialized to JSON across FFI boundary
- **Session Reuse**: Create sessions once and reuse for better performance
- **Object Creation**: QueryResult creates Java objects from JSON on each query

## Troubleshooting

### "Could not load GraphLite library"

Build the FFI library first:
```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

The Java bindings look for the library in:
- `target/release/`
- `target/debug/`
- `/usr/local/lib/`
- `/usr/lib/`
- Current directory

### Set library path explicitly

```java
System.setProperty("jna.library.path", "/path/to/GraphLite/target/release");
GraphLite db = GraphLite.open("./mydb");
```

### "Session error"

Make sure to create a session before querying:
```java
String session = db.createSession("username");
QueryResult result = db.query(session, "MATCH (n) RETURN n");
```

### Dependencies not found

Install dependencies with Maven:
```bash
mvn clean install
```

## Development

### Build Project

```bash
mvn clean compile
```

### Run Tests

```bash
mvn test
```

### Generate Javadoc

```bash
mvn javadoc:javadoc
# Open target/site/apidocs/index.html
```

### Package JAR

```bash
mvn package
# Creates target/graphlite-0.1.0.jar
```

## Comparison with Other Languages

| Feature | Java (via JNA) | Rust SDK | Python |
|---------|---------------|----------|---------|
| **Performance** | ~75-85% of native | ~100% native | ~80-90% native |
| **Ease of Use** | Easy | Easy | Very easy |
| **Type Safety** | Compile-time | Compile-time | Runtime |
| **Installation** | Maven/Gradle | Cargo | pip |
| **Overhead** | JNA + JSON | None | ctypes + JSON |

## License

Apache-2.0 - See [LICENSE](../../LICENSE) for details.
