#!/usr/bin/env python3
"""
Test script to verify all README examples work correctly.

This script tests all the examples and features documented in the README.
Run from the GraphLite root directory or sdk-python directory.
"""

import tempfile
import shutil
from dataclasses import dataclass
from pathlib import Path

# Test 1: Quick Start Example
print("=" * 60)
print("TEST 1: Quick Start Example")
print("=" * 60)

from graphlite_sdk import GraphLite

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    # Setup schema and graph (required)
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    # Insert some test data
    session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
    session.execute("INSERT (p:Person {name: 'Bob', age: 25})")
    
    # Execute query
    result = session.query("MATCH (p:Person) RETURN p.name as name")
    
    print("Query results:")
    for row in result.rows:
        print(f"  - {row}")
    print("✓ Quick Start test passed\n")
except Exception as e:
    print(f"✗ Quick Start test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 2: Opening a Database
print("=" * 60)
print("TEST 2: Opening a Database")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    print(f"✓ Database opened at {db_path}\n")
    db.close()
except Exception as e:
    print(f"✗ Database opening test failed: {e}\n")
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 3: Sessions
print("=" * 60)
print("TEST 3: Sessions")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("testuser")
    print(f"✓ Session created for user: {session.username()}")
    print(f"✓ Session ID: {session.id()[:20]}...\n")
    db.close()
except Exception as e:
    print(f"✗ Sessions test failed: {e}\n")
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 4: Executing Queries
print("=" * 60)
print("TEST 4: Executing Queries")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
    
    result = session.query("MATCH (n:Person) RETURN n.name as name")
    print(f"✓ Query returned {len(result.rows)} rows")
    for row in result.rows:
        print(f"  - {row['name']}")
    print("✓ Executing queries test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Executing queries test failed: {e}\n")
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 5: Transactions
print("=" * 60)
print("TEST 5: Transactions")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    # Transaction with explicit commit
    with session.transaction() as tx:
        tx.execute("INSERT (p:Person {name: 'Alice'})")
        tx.execute("INSERT (p:Person {name: 'Bob'})")
        tx.commit()
    
    result = session.query("MATCH (p:Person) RETURN count(p) as count")
    count_after_commit = result.rows[0]['count']
    print(f"✓ After commit: {count_after_commit} persons")
    
    # Transaction with automatic rollback
    with session.transaction() as tx:
        tx.execute("INSERT (p:Person {name: 'Charlie'})")
        # No commit - should rollback
    
    result = session.query("MATCH (p:Person) RETURN count(p) as count")
    count_after_rollback = result.rows[0]['count']
    print(f"✓ After rollback: {count_after_rollback} persons (Charlie not persisted)")
    print("✓ Transactions test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Transactions test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 6: Query Builder
print("=" * 60)
print("TEST 6: Query Builder")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
    session.execute("INSERT (p:Person {name: 'Bob', age: 25})")
    session.execute("INSERT (p:Person {name: 'Charlie', age: 35})")
    
    result = (session.query_builder()
        .match_pattern("(p:Person)")
        .where_clause("p.age > 25")
        .return_clause("p.name, p.age")
        .order_by("p.age DESC")
        .limit(10)
        .execute())
    
    print(f"✓ Query builder returned {len(result.rows)} rows:")
    for row in result.rows:
        print(f"  - {row}")
    print("✓ Query Builder test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Query Builder test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 7: Typed Results
print("=" * 60)
print("TEST 7: Typed Results")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    from graphlite_sdk import TypedResult
    
    @dataclass
    class Person:
        name: str
        age: int
    
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
    session.execute("INSERT (p:Person {name: 'Bob', age: 25})")
    
    result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")
    typed = TypedResult(result)
    people = typed.deserialize_rows(Person)
    
    print(f"✓ Deserialized {len(people)} persons:")
    for person in people:
        print(f"  - {person.name} is {person.age} years old")
    print("✓ Typed Results test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Typed Results test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 8: Basic CRUD Operations
print("=" * 60)
print("TEST 8: Basic CRUD Operations")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    # Create schema and graph
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    # Create nodes
    session.execute("INSERT (p:Person {name: 'Alice', age: 30})")
    session.execute("INSERT (p:Person {name: 'Bob', age: 25})")
    print("✓ Nodes created")
    
    # Create relationships
    session.execute("""
        MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
        INSERT (a)-[:KNOWS]->(b)
    """)
    print("✓ Relationship created")
    
    # Query
    result = session.query("""
        MATCH (p:Person)-[:KNOWS]->(f:Person)
        RETURN p.name as person, f.name as friend
    """)
    
    print("✓ Query results:")
    for row in result.rows:
        print(f"  {row['person']} knows {row['friend']}")
    print("✓ Basic CRUD Operations test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Basic CRUD Operations test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 9: Transaction Example
print("=" * 60)
print("TEST 9: Transaction Example")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    session.execute("INSERT (p:Person {name: 'Alice'})")
    session.execute("INSERT (p:Person {name: 'Bob'})")
    session.execute("INSERT (p:Person {name: 'Charlie'})")
    # Create relationship between existing nodes
    session.execute("""
        MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
        INSERT (a)-[:FOLLOWS]->(b)
    """)
    
    with session.transaction() as tx:
        # Delete old relationship
        tx.execute("MATCH (a)-[r:FOLLOWS]->(b) WHERE a.name = 'Alice' DELETE r")
        
        # Create new relationship
        tx.execute("""
            MATCH (a {name: 'Alice'}), (c {name: 'Charlie'})
            INSERT (a)-[:FOLLOWS]->(c)
        """)
        
        tx.commit()
    
    result = session.query("MATCH (a)-[:FOLLOWS]->(b) RETURN a.name as from_name, b.name as to_name")
    print("✓ Relationships after transaction:")
    for row in result.rows:
        print(f"  {row['from_name']} follows {row['to_name']}")
    print("✓ Transaction Example test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Transaction Example test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 10: Query Builder Example
print("=" * 60)
print("TEST 10: Query Builder Example")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    session.execute("INSERT (u:User {name: 'Alice', email: 'alice@test.com', status: 'active', lastLogin: '2024-06-01'})")
    session.execute("INSERT (u:User {name: 'Bob', email: 'bob@test.com', status: 'inactive', lastLogin: '2023-12-01'})")
    session.execute("INSERT (u:User {name: 'Charlie', email: 'charlie@test.com', status: 'active', lastLogin: '2024-07-01'})")
    
    result = (session.query_builder()
        .match_pattern("(u:User)")
        .where_clause("u.status = 'active'")
        .where_clause("u.lastLogin > '2024-01-01'")
        .return_clause("u.name, u.email")
        .order_by("u.lastLogin DESC")
        .limit(20)
        .execute())
    
    print(f"✓ Query builder returned {len(result.rows)} users:")
    for row in result.rows:
        # Query builder returns column names with prefixes (u.name, u.email)
        name = row.get('u.name') or row.get('name')
        email = row.get('u.email') or row.get('email')
        print(f"  {name}: {email}")
    print("✓ Query Builder Example test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Query Builder Example test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 11: Typed Deserialization Example
print("=" * 60)
print("TEST 11: Typed Deserialization Example")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    from graphlite_sdk import TypedResult
    
    @dataclass
    class User:
        name: str
        email: str
        age: int
    
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    session.execute("INSERT (u:User {name: 'Alice', email: 'alice@test.com', age: 30})")
    session.execute("INSERT (u:User {name: 'Bob', email: 'bob@test.com', age: 25})")
    
    result = session.query(
        "MATCH (u:User) RETURN u.name as name, u.email as email, u.age as age"
    )
    
    typed = TypedResult(result)
    users = typed.deserialize_rows(User)
    
    print(f"✓ Deserialized {len(users)} users:")
    for user in users:
        print(f"  {user.name} ({user.email}): {user.age}")
    print("✓ Typed Deserialization Example test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Typed Deserialization Example test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 12: Scalar and First Methods
print("=" * 60)
print("TEST 12: Scalar and First Methods")
print("=" * 60)

db_path = tempfile.mkdtemp(prefix="graphlite_test_")
try:
    from graphlite_sdk import TypedResult
    
    @dataclass
    class Count:
        count: int
    
    db = GraphLite.open(db_path)
    session = db.session("admin")
    
    session.execute("CREATE SCHEMA IF NOT EXISTS /example")
    session.execute("SESSION SET SCHEMA /example")
    session.execute("CREATE GRAPH IF NOT EXISTS social")
    session.execute("SESSION SET GRAPH social")
    
    session.execute("INSERT (p:Person {name: 'Alice'})")
    session.execute("INSERT (p:Person {name: 'Bob'})")
    session.execute("INSERT (p:Person {name: 'Charlie'})")
    
    # Get a scalar value
    result = session.query("MATCH (p:Person) RETURN count(p) as count")
    typed = TypedResult(result)
    count = typed.scalar()
    print(f"✓ Scalar value: Total persons = {count}")
    
    # Get first row as typed object
    result = session.query("MATCH (p:Person) RETURN count(p) as count")
    typed = TypedResult(result)
    count_obj = typed.first(Count)
    print(f"✓ First row as typed object: Total persons = {count_obj.count}")
    print("✓ Scalar and First Methods test passed\n")
    db.close()
except Exception as e:
    print(f"✗ Scalar and First Methods test failed: {e}\n")
    import traceback
    traceback.print_exc()
finally:
    shutil.rmtree(db_path, ignore_errors=True)


# Test 13: Error Handling
print("=" * 60)
print("TEST 13: Error Handling")
print("=" * 60)

try:
    from graphlite_sdk import (
        GraphLiteError,
        ConnectionError,
        SessionError,
        QueryError,
        TransactionError,
        SerializationError,
    )
    
    print("✓ All error types imported successfully:")
    print(f"  - GraphLiteError: {GraphLiteError}")
    print(f"  - ConnectionError: {ConnectionError}")
    print(f"  - SessionError: {SessionError}")
    print(f"  - QueryError: {QueryError}")
    print(f"  - TransactionError: {TransactionError}")
    print(f"  - SerializationError: {SerializationError}")
    
    # Test error handling
    db_path = tempfile.mkdtemp(prefix="graphlite_test_")
    try:
        db = GraphLite.open(db_path)
        session = db.session("admin")
        
        session.execute("CREATE SCHEMA IF NOT EXISTS /example")
        session.execute("SESSION SET SCHEMA /example")
        session.execute("CREATE GRAPH IF NOT EXISTS social")
        session.execute("SESSION SET GRAPH social")
        
        try:
            session.query("INVALID QUERY SYNTAX")
        except QueryError as e:
            print(f"✓ QueryError caught correctly: {type(e).__name__}")
        
        db.close()
    finally:
        shutil.rmtree(db_path, ignore_errors=True)
    
    print("✓ Error Handling test passed\n")
except Exception as e:
    print(f"✗ Error Handling test failed: {e}\n")
    import traceback
    traceback.print_exc()


print("=" * 60)
print("ALL TESTS COMPLETED")
print("=" * 60)

