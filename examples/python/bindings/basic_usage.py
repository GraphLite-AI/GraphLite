#!/usr/bin/env python3
"""
GraphLite Python Bindings - Basic Usage Example

This example demonstrates how to use GraphLite from Python.
"""

from graphlite import GraphLite, GraphLiteError
import tempfile
import shutil


def main():
    print("=== GraphLite Python Bindings Example ===\n")

    # Use temporary directory for demo
    temp_dir = tempfile.mkdtemp(prefix="graphlite_python_")
    print(f"Using temporary database: {temp_dir}\n")

    try:
        # 1. Open database
        print("1. Opening database...")
        db = GraphLite(temp_dir)
        print(f"   ✓ GraphLite version: {GraphLite.version()}\n")

        # 2. Create session
        print("2. Creating session...")
        session = db.create_session("admin")
        print(f"   ✓ Session created: {session[:20]}...\n")

        # 3. Create schema and graph
        print("3. Setting up schema and graph...")
        db.execute(session, "CREATE SCHEMA IF NOT EXISTS example")
        db.execute(session, "SESSION SET SCHEMA example")
        db.execute(session, "CREATE GRAPH IF NOT EXISTS social")
        db.execute(session, "SESSION SET GRAPH social")
        print("   ✓ Schema and graph created\n")

        # 4. Insert data
        print("4. Inserting data...")
        db.execute(session, "CREATE (p:Person {name: 'Alice', age: 30})")
        db.execute(session, "CREATE (p:Person {name: 'Bob', age: 25})")
        db.execute(session, "CREATE (p:Person {name: 'Charlie', age: 35})")
        print("   ✓ Inserted 3 persons\n")

        # 5. Query data
        print("5. Querying data...")
        result = db.query(session, "MATCH (p:Person) RETURN p.name as name, p.age as age")
        print(f"   Found {result.row_count} persons:")
        for row in result.rows:
            print(f"   - {row['name']}: {row['age']} years old")
        print()

        # 6. Filter with WHERE
        print("6. Filtering with WHERE clause...")
        result = db.query(
            session,
            "MATCH (p:Person) WHERE p.age > 25 RETURN p.name as name, p.age as age ORDER BY p.age DESC"
        )
        print(f"   Found {result.row_count} persons over 25:")
        for row in result.rows:
            print(f"   - {row['name']}: {row['age']} years old")
        print()

        # 7. Aggregation
        print("7. Aggregation query...")
        result = db.query(session, "MATCH (p:Person) RETURN count(p) as total, avg(p.age) as avg_age")
        if result.rows:
            row = result.first()
            print(f"   Total persons: {row['total']}")
            print(f"   Average age: {row['avg_age']:.1f}")
        print()

        # 8. Get column values
        print("8. Extracting column values...")
        result = db.query(session, "MATCH (p:Person) RETURN p.name as name")
        names = result.column('name')
        print(f"   All names: {names}\n")

        # 9. Close session
        print("9. Closing session...")
        db.close_session(session)
        print("   ✓ Session closed\n")

        # 10. Close database
        print("10. Closing database...")
        db.close()
        print("   ✓ Database closed\n")

        print("=== Example completed successfully ===")

    except GraphLiteError as e:
        print(f"\n❌ GraphLite Error: {e}")
        return 1

    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
        return 1

    finally:
        # Cleanup
        shutil.rmtree(temp_dir, ignore_errors=True)

    return 0


if __name__ == "__main__":
    exit(main())
