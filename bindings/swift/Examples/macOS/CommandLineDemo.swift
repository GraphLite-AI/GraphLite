#!/usr/bin/env swift
//
// GraphLite macOS Command Line Demo
//
// This example demonstrates using GraphLite in a macOS command-line application.
// It shows basic database operations: create, insert, query, and relationship management.
//
// To run:
//   swift run CommandLineDemo
//
// Or compile and run:
//   swiftc -I ../../Sources -L ../../.build/debug CommandLineDemo.swift -o demo
//   ./demo

import Foundation

// Note: In a real SPM package, you would import like this:
// import GraphLite

print("========================================")
print("GraphLite macOS Command Line Demo")
print("========================================")
print()

do {
    // Create a temporary database path
    let tempDir = FileManager.default.temporaryDirectory
    let dbPath = tempDir.appendingPathComponent("graphlite_demo_\(UUID().uuidString)").path

    print("Creating database at: \(dbPath)")

    // Initialize database
    let db = try GraphLite(path: dbPath)
    print("✓ Database initialized")
    print("✓ GraphLite version: \(GraphLite.version)")
    print()

    // Create session
    let session = try db.createSession(username: "demo_user")
    print("✓ Session created")
    print()

    // Setup schema and graph
    print("Setting up schema and graph...")
    _ = try session.execute("CREATE SCHEMA /demo")
    _ = try session.execute("SESSION SET SCHEMA /demo")
    _ = try session.execute("CREATE GRAPH /demo/social")
    _ = try session.execute("SESSION SET GRAPH /demo/social")
    print("✓ Schema '/demo' and graph '/demo/social' created")
    print()

    // Insert people
    print("Inserting people...")
    _ = try session.execute("""
        INSERT (:Person {name: 'Alice', age: 30, city: 'New York'}),
               (:Person {name: 'Bob', age: 25, city: 'San Francisco'}),
               (:Person {name: 'Carol', age: 28, city: 'Los Angeles'}),
               (:Person {name: 'Dave', age: 32, city: 'New York'})
    """)
    print("✓ 4 people inserted")
    print()

    // Query all people
    print("Querying all people...")
    let allPeople = try session.execute("MATCH (p:Person) RETURN p.name, p.age, p.city ORDER BY p.age")

    print("Found \(allPeople.rowCount) people:")
    for (index, row) in allPeople.rows.enumerated() {
        if case .string(let name) = row["p.name"],
           case .integer(let age) = row["p.age"],
           case .string(let city) = row["p.city"] {
            print("  \(index + 1). \(name), age \(age), from \(city)")
        }
    }
    print()

    // Query with WHERE clause
    print("Querying people from New York...")
    let nyPeople = try session.execute("MATCH (p:Person WHERE p.city = 'New York') RETURN p.name, p.age")

    print("Found \(nyPeople.rowCount) people in New York:")
    for row in nyPeople.rows {
        if case .string(let name) = row["p.name"],
           case .integer(let age) = row["p.age"] {
            print("  - \(name), age \(age)")
        }
    }
    print()

    // Query with aggregation
    print("Calculating statistics...")
    let stats = try session.execute("""
        MATCH (p:Person)
        RETURN COUNT(p) AS total, AVG(p.age) AS avg_age, MIN(p.age) AS min_age, MAX(p.age) AS max_age
    """)

    if let row = stats.rows.first {
        if case .integer(let total) = row["total"],
           case .double(let avgAge) = row["avg_age"],
           case .integer(let minAge) = row["min_age"],
           case .integer(let maxAge) = row["max_age"] {
            print("Statistics:")
            print("  Total people: \(total)")
            print("  Average age: \(String(format: "%.1f", avgAge))")
            print("  Age range: \(minAge) - \(maxAge)")
        }
    }
    print()

    // Group by city
    print("People by city...")
    let byCity = try session.execute("""
        MATCH (p:Person)
        RETURN p.city AS city, COUNT(p) AS count
        ORDER BY count DESC
    """)

    print("City distribution:")
    for row in byCity.rows {
        if case .string(let city) = row["city"],
           case .integer(let count) = row["count"] {
            print("  \(city): \(count) \(count == 1 ? "person" : "people")")
        }
    }
    print()

    // Clean up
    session.close()
    print("✓ Session closed")

    // Clean up temporary database
    try? FileManager.default.removeItem(atPath: dbPath)
    print("✓ Database cleaned up")

    print()
    print("========================================")
    print("Demo completed successfully!")
    print("========================================")

} catch {
    print()
    print("❌ Error: \(error)")
    print()
    exit(1)
}
