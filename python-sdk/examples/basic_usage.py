"""
GraphLite Python SDK - Basic Usage Example

This example demonstrates the core features of the GraphLite Python SDK:
- Opening a database
- Creating sessions
- Executing queries
- Using transactions
- Query builder API
- Typed result deserialization

Run with: python3 examples/basic_usage.py
"""

from dataclasses import dataclass
from typing import Optional
import tempfile
import shutil
import sys
from pathlib import Path

# Add parent directory to path so we can import src
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.connection import GraphLite
from src.error import GraphLiteError


@dataclass
class Person:
    """Person entity for typed deserialization"""
    name: str
    age: float


def main() -> int:
    """Run the basic usage example"""
    print("=== GraphLite SDK Basic Usage Example ===\n")

    # Use temporary directory for demo
    db_path = tempfile.mkdtemp(prefix="graphlite_sdk_example_")
    
    try:
        # 1. Open a database
        print("1. Opening database...")
        db = GraphLite.open(db_path)
        print(f"   ✓ Database opened at {db_path}\n")

        # 2. Create a session
        print("2. Creating session...")
        session = db.session("admin")
        print("   ✓ Session created for user 'admin'\n")

        # 3. Execute DDL statements
        print("3. Creating schema and graph...")
        session.execute("CREATE SCHEMA IF NOT EXISTS /example")
        session.execute("SESSION SET SCHEMA /example")
        session.execute("CREATE GRAPH IF NOT EXISTS social")
        session.execute("SESSION SET GRAPH social")
        print("   ✓ Schema and graph created\n")

        # 4. Insert data using transactions
        print("4. Inserting data with transaction...")
        with session.transaction() as tx:
            tx.execute("CREATE (p:Person {name: 'Alice', age: 30})")
            tx.execute("CREATE (p:Person {name: 'Bob', age: 25})")
            tx.execute("CREATE (p:Person {name: 'Charlie', age: 35})")
            tx.commit()
        print("   ✓ Inserted 3 persons\n")

        # 5. Query data directly
        print("5. Querying data...")
        result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")
        print(f"   Found {len(result.rows)} persons:")
        for row in result.rows:
            name = row.get("name")
            age = row.get("age")
            if name is not None and age is not None:
                print(f"   - Name: {name}, Age: {age}")
        print()

        # 6. Use query builder
        print("6. Using query builder...")
        result = (session.query_builder()
            .match_pattern("(p:Person)")
            .where_clause("p.age > 25")
            .return_clause("p.name as name, p.age as age")
            .order_by("p.age DESC")
            .execute())
        print(f"   Found {len(result.rows)} persons over 25:")
        for row in result.rows:
            name = row.get("name")
            age = row.get("age")
            if name is not None and age is not None:
                print(f"   - Name: {name}, Age: {age}")
        print()

        # # 7. Typed deserialization
        # print("7. Using typed deserialization...")
        # result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")
        # typed_result = result.to_typed()
        # people = typed_result.deserialize_rows(Person)
        # print(f"   Deserialized {len(people)} persons:")
        # for person in people:
        #     print(f"   - {person}")
        # print()

        # # 8. Transaction with rollback
        # print("8. Demonstrating transaction rollback...")
        # try:
        #     with session.transaction() as tx:
        #         tx.execute("CREATE (p:Person {name: 'David', age: 40})")
        #         print("   Created person 'David' in transaction")
        #         # Transaction is NOT committed - will auto-rollback
        #         # (by not calling tx.commit())
        # except Exception:
        #     pass  # Expected - rollback on exception
        # print("   Transaction rolled back (David not persisted)\n")

        # # 9. Verify rollback
        # result = session.query("MATCH (p:Person) RETURN count(p) as count")
        # if result.rows:
        #     count = result.rows[0].get("count")
        #     print(f"   Person count after rollback: {count}\n")

        print("=== Example completed successfully ===")
        return 0

    except GraphLiteError as e:
        print(f"\n❌ GraphLite Error: {e}")
        return 1

    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return 1

    finally:
        # Cleanup temporary directory
        shutil.rmtree(db_path, ignore_errors=True)


if __name__ == "__main__":
    exit(main())
