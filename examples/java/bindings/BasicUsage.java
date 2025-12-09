import com.deepgraph.graphlite.GraphLite;
import com.deepgraph.graphlite.GraphLite.GraphLiteException;
import com.deepgraph.graphlite.QueryResult;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Map;

/**
 * GraphLite Java Bindings - Basic Usage Example
 *
 * This example demonstrates how to use GraphLite from Java.
 *
 * Compile and run:
 * mvn clean compile
 * mvn exec:java -Dexec.mainClass="BasicUsage"
 */
public class BasicUsage {

    public static void main(String[] args) {
        System.out.println("=== GraphLite Java Bindings Example ===\n");

        // Use temporary directory for demo
        Path tempDir = null;
        try {
            tempDir = Files.createTempDirectory("graphlite_java_");
            System.out.println("Using temporary database: " + tempDir + "\n");

            runExample(tempDir.toString());

            System.out.println("\n=== Example completed successfully ===");

        } catch (GraphLiteException e) {
            System.err.println("\n[ERROR] GraphLite Error: " + e.getMessage());
            System.err.println("Error code: " + e.getErrorCode());
            System.exit(1);

        } catch (Exception e) {
            System.err.println("\n[ERROR] Unexpected error: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);

        } finally {
            // Cleanup
            if (tempDir != null) {
                try {
                    Files.walk(tempDir)
                        .sorted(Comparator.reverseOrder())
                        .forEach(path -> {
                            try {
                                Files.delete(path);
                            } catch (IOException e) {
                                // Ignore
                            }
                        });
                } catch (IOException e) {
                    // Ignore cleanup errors
                }
            }
        }
    }

    private static void runExample(String dbPath) {
        // 1. Open database
        System.out.println("1. Opening database...");
        try (GraphLite db = GraphLite.open(dbPath)) {
            System.out.println("   [OK] GraphLite version: " + GraphLite.version() + "\n");

            // 2. Create session
            System.out.println("2. Creating session...");
            String session = db.createSession("admin");
            System.out.println("   [OK] Session created: " + session.substring(0, 20) + "...\n");

            // 3. Create schema and graph
            System.out.println("3. Setting up schema and graph...");
            db.execute(session, "CREATE SCHEMA IF NOT EXISTS /example");
            db.execute(session, "SESSION SET SCHEMA /example");
            db.execute(session, "CREATE GRAPH IF NOT EXISTS social");
            db.execute(session, "SESSION SET GRAPH social");
            System.out.println("   [OK] Schema and graph created\n");

            // 4. Insert data
            System.out.println("4. Inserting data...");
            db.execute(session, "INSERT (:Person {name: 'Alice', age: 30})");
            db.execute(session, "INSERT (:Person {name: 'Bob', age: 25})");
            db.execute(session, "INSERT (:Person {name: 'Charlie', age: 35})");
            System.out.println("   [OK] Inserted 3 persons\n");

            // 5. Query data
            System.out.println("5. Querying data...");
            QueryResult result = db.query(session,
                "MATCH (p:Person) RETURN p.name as name, p.age as age");
            System.out.println("   Found " + result.getRowCount() + " persons:");
            for (Map<String, Object> row : result.getRows()) {
                System.out.println("   - " + row.get("name") + ": " + row.get("age") + " years old");
            }
            System.out.println();

            // 6. Filter with WHERE
            System.out.println("6. Filtering with WHERE clause...");
            result = db.query(session,
                "MATCH (p:Person) WHERE p.age > 25 " +
                "RETURN p.name as name, p.age as age ORDER BY p.age DESC");
            System.out.println("   Found " + result.getRowCount() + " persons over 25:");
            for (Map<String, Object> row : result.getRows()) {
                System.out.println("   - " + row.get("name") + ": " + row.get("age") + " years old");
            }
            System.out.println();

            // 7. Aggregation
            System.out.println("7. Aggregation query...");
            result = db.query(session,
                "MATCH (p:Person) RETURN count(p) as total, avg(p.age) as avg_age");
            if (!result.isEmpty()) {
                Map<String, Object> row = result.first();
                System.out.println("   Total persons: " + row.get("total"));
                System.out.println("   Average age: " + String.format("%.1f", row.get("avg_age")));
            }
            System.out.println();

            // 8. Get column values
            System.out.println("8. Extracting column values...");
            result = db.query(session, "MATCH (p:Person) RETURN p.name as name");
            System.out.println("   All names: " + result.column("name") + "\n");

            // 9. Close session
            System.out.println("9. Closing session...");
            db.closeSession(session);
            System.out.println("   [OK] Session closed\n");

        } // Database automatically closed (try-with-resources)
        System.out.println("10. Database closed");
    }
}
