# GraphLite Swift SDK - Design Document

**Phase:** 2 (After Swift Bindings)
**Estimated Time:** 4-6 weeks
**Target Users:** iOS/macOS app developers
**Goal:** High-level, type-safe, SwiftUI-friendly API on top of Swift bindings

---

## Table of Contents

1. [Vision](#vision)
2. [Architecture](#architecture)
3. [API Design](#api-design)
4. [Core Features](#core-features)
5. [Advanced Features](#advanced-features)
6. [SwiftUI Integration](#swiftui-integration)
7. [Implementation Roadmap](#implementation-roadmap)
8. [Code Examples](#code-examples)

---

## Vision

### Problem Statement

The **Swift Bindings** (Phase 1) provide a thin wrapper over C FFI:

```swift
// Low-level bindings approach
let db = try GraphLite(path: "./mydb")
let session = try db.createSession(username: "admin")
try session.execute("CREATE SCHEMA /social")
try session.execute("SESSION SET SCHEMA /social")
try session.execute("CREATE GRAPH /social/network")
try session.execute("SESSION SET GRAPH /social/network")
try session.execute("INSERT (:Person {name: 'Alice', age: 30})")
let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")
```

**Issues:**
- Requires knowledge of GQL syntax
- String-based queries (typos not caught at compile time)
- Manual JSON parsing
- Verbose boilerplate
- No type safety
- Not idiomatic Swift

### Solution: Swift SDK

Provide a **high-level, type-safe, SwiftUI-friendly** API:

```swift
// SDK approach
let db = try GraphLiteDB(path: "./mydb")

// Type-safe model
struct Person: GraphNode {
    var name: String
    var age: Int
}

// Type-safe insertion
try db.insert(Person(name: "Alice", age: 30))

// Type-safe querying with query builder
let people: [Person] = try db.match(Person.self)
    .where(\.age, greaterThan: 25)
    .orderBy(\.age, ascending: true)
    .fetch()

// SwiftUI integration
@GraphLiteQuery("MATCH (p:Person) RETURN p")
var people: [Person]
```

**Benefits:**
-  Type-safe at compile time
-  Fluent query builder API
-  Automatic serialization/deserialization
-  SwiftUI property wrappers
-  Idiomatic Swift patterns
-  Reduced boilerplate
-  Better developer experience

---

## Architecture

### Layer Diagram

```

           SwiftUI Application Layer                 
  (@GraphLiteQuery, @Observable models)              

                      ↓

         GraphLite Swift SDK (Phase 2)               
      
    Query Builder API                             
    (match(), where(), orderBy(), fetch())        
      
    Type-Safe Models                              
    (GraphNode protocol, Codable)                 
      
    Transaction Manager                           
    (transaction { }, auto commit/rollback)       
      
    Schema & Graph Management                     
    (schema.create(), graph.use())                
      
    SwiftUI Integration                           
    (@GraphLiteQuery property wrapper)            
      

                      ↓

      GraphLite Swift Bindings (Phase 1)            
  (GraphLite, Session, execute())                    

                      ↓

           C FFI Layer (graphlite-ffi)               

```

### Module Structure

```
GraphLiteSDK/
 Core/
    GraphLiteDB.swift            # Main SDK entry point
    GraphNode.swift              # Protocol for graph nodes
    GraphRelationship.swift      # Protocol for relationships
    Transaction.swift            # Transaction management

 Query/
    QueryBuilder.swift           # Fluent query builder
    MatchBuilder.swift           # MATCH clause builder
    WhereBuilder.swift           # WHERE clause builder
    ReturnBuilder.swift          # RETURN clause builder
    Operators.swift              # Query operators

 Schema/
    SchemaManager.swift          # Schema operations
    GraphManager.swift           # Graph operations

 SwiftUI/
    GraphLiteQuery.swift         # @GraphLiteQuery property wrapper
    GraphLiteEnvironment.swift   # Environment key
    ViewModifiers.swift          # View modifiers

 Extensions/
     Codable+Graph.swift          # Codable extensions
     KeyPath+Query.swift          # KeyPath utilities
     Result+Graph.swift           # Result type extensions
```

---

## API Design

### 1. Database Connection

```swift
/// Main SDK database instance
public class GraphLiteDB {
    private let bindings: GraphLite  // Phase 1 bindings
    private var currentSession: Session?

    /// Initialize database connection
    public init(path: String, username: String = "default") throws {
        self.bindings = try GraphLite(path: path)
        self.currentSession = try bindings.createSession(username: username)
    }

    /// Get or create schema
    public func schema(_ name: String) throws -> Schema {
        return try Schema(name: name, db: self)
    }

    /// Get or create graph
    public func graph(_ name: String, in schema: String = "default") throws -> Graph {
        return try Graph(name: name, schema: schema, db: self)
    }

    /// Start a transaction
    public func transaction<T>(_ block: (Transaction) throws -> T) throws -> T {
        let tx = Transaction(session: currentSession!)
        return try tx.execute(block)
    }
}
```

### 2. Type-Safe Models

```swift
/// Protocol for graph node types
public protocol GraphNode: Codable, Identifiable {
    /// Node label (default: type name)
    static var label: String { get }

    /// Unique identifier
    var id: UUID { get }
}

extension GraphNode {
    public static var label: String {
        return String(describing: Self.self)
    }
}

/// Protocol for graph relationships
public protocol GraphRelationship: Codable {
    /// Relationship type
    static var type: String { get }

    /// Source node ID
    var from: UUID { get }

    /// Target node ID
    var to: UUID { get }
}

/// Example usage
struct Person: GraphNode {
    let id: UUID
    var name: String
    var age: Int
    var city: String
}

struct Knows: GraphRelationship {
    static let type = "KNOWS"
    let from: UUID
    let to: UUID
    var since: Date
}
```

### 3. Query Builder

```swift
/// Fluent query builder
public class QueryBuilder<T: GraphNode> {
    private let db: GraphLiteDB
    private var matchClause: String = ""
    private var whereClause: String = ""
    private var returnClause: String = ""
    private var orderByClause: String = ""
    private var limitClause: String = ""

    internal init(db: GraphLiteDB, nodeType: T.Type) {
        self.db = db
        self.matchClause = "MATCH (n:\(T.label))"
    }

    /// Add WHERE condition
    public func `where`<V>(_ keyPath: KeyPath<T, V>, _ op: QueryOperator, _ value: V) -> Self {
        let property = propertyName(from: keyPath)
        whereClause = "WHERE n.\(property) \(op.rawValue) \(formatValue(value))"
        return self
    }

    /// Add ORDER BY
    public func orderBy<V>(_ keyPath: KeyPath<T, V>, ascending: Bool = true) -> Self {
        let property = propertyName(from: keyPath)
        let direction = ascending ? "ASC" : "DESC"
        orderByClause = "ORDER BY n.\(property) \(direction)"
        return self
    }

    /// Limit results
    public func limit(_ count: Int) -> Self {
        limitClause = "LIMIT \(count)"
        return self
    }

    /// Execute query and return typed results
    public func fetch() throws -> [T] {
        let gql = buildGQL()
        let result = try db.execute(gql)
        return try result.decode([T].self)
    }

    private func buildGQL() -> String {
        return [matchClause, whereClause, "RETURN n", orderByClause, limitClause]
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}

/// Query operators
public enum QueryOperator: String {
    case equal = "="
    case notEqual = "<>"
    case greaterThan = ">"
    case lessThan = "<"
    case greaterThanOrEqual = ">="
    case lessThanOrEqual = "<="
    case `in` = "IN"
}
```

### 4. CRUD Operations

```swift
extension GraphLiteDB {
    /// Insert a node
    public func insert<T: GraphNode>(_ node: T) throws {
        let properties = try encodeProperties(node)
        let gql = "INSERT (:\(T.label) \(properties))"
        try execute(gql)
    }

    /// Insert multiple nodes
    public func insertAll<T: GraphNode>(_ nodes: [T]) throws {
        try transaction { tx in
            for node in nodes {
                try tx.insert(node)
            }
        }
    }

    /// Update a node
    public func update<T: GraphNode>(_ node: T) throws {
        let properties = try encodeProperties(node)
        let gql = """
        MATCH (n:\(T.label) {id: '\(node.id)'})
        SET n = \(properties)
        """
        try execute(gql)
    }

    /// Delete a node
    public func delete<T: GraphNode>(_ node: T) throws {
        let gql = "MATCH (n:\(T.label) {id: '\(node.id)'}) DELETE n"
        try execute(gql)
    }

    /// Find node by ID
    public func find<T: GraphNode>(_ type: T.Type, id: UUID) throws -> T? {
        let gql = "MATCH (n:\(T.label) {id: '\(id)'}) RETURN n"
        let result = try execute(gql)
        return try result.decodeFirst(T.self)
    }

    /// Create relationship
    public func createRelationship<R: GraphRelationship>(_ rel: R) throws {
        let properties = try encodeProperties(rel)
        let gql = """
        MATCH (a {id: '\(rel.from)'}), (b {id: '\(rel.to)'})
        CREATE (a)-[:\(R.type) \(properties)]->(b)
        """
        try execute(gql)
    }
}
```

### 5. Transaction Management

```swift
/// Transaction context
public class Transaction {
    private let session: Session
    private var operations: [String] = []
    private var isCommitted = false

    internal init(session: Session) {
        self.session = session
    }

    /// Execute transaction block with auto-commit/rollback
    func execute<T>(_ block: (Transaction) throws -> T) throws -> T {
        do {
            let result = try block(self)
            try commit()
            return result
        } catch {
            try? rollback()
            throw error
        }
    }

    /// Add operation to transaction
    public func execute(_ gql: String) throws {
        operations.append(gql)
    }

    /// Insert node in transaction
    public func insert<T: GraphNode>(_ node: T) throws {
        let properties = try encodeProperties(node)
        try execute("INSERT (:\(T.label) \(properties))")
    }

    /// Commit transaction
    public func commit() throws {
        guard !isCommitted else { return }

        // Execute all operations in sequence
        for operation in operations {
            try session.execute(operation)
        }

        isCommitted = true
    }

    /// Rollback transaction
    public func rollback() throws {
        // For now, just clear operations
        // Future: implement proper rollback via savepoints
        operations.removeAll()
    }
}
```

---

## Core Features

### Feature 1: Query Builder

**Design Goal:** Fluent, discoverable, type-safe

```swift
// Example: Find people over 25 in NYC
let people = try db.match(Person.self)
    .where(\.city, .equal, "NYC")
    .where(\.age, .greaterThan, 25)
    .orderBy(\.age, ascending: true)
    .limit(10)
    .fetch()

// Generated GQL:
// MATCH (n:Person)
// WHERE n.city = 'NYC' AND n.age > 25
// RETURN n
// ORDER BY n.age ASC
// LIMIT 10
```

**Implementation:**

```swift
public extension GraphLiteDB {
    /// Start a query builder
    func match<T: GraphNode>(_ type: T.Type) -> QueryBuilder<T> {
        return QueryBuilder(db: self, nodeType: type)
    }
}

public class QueryBuilder<T: GraphNode> {
    // WHERE clauses
    private var conditions: [String] = []

    public func `where`<V: Comparable>(_ keyPath: KeyPath<T, V>, _ op: QueryOperator, _ value: V) -> Self {
        let property = propertyName(from: keyPath)
        conditions.append("n.\(property) \(op.rawValue) \(formatValue(value))")
        return self
    }

    // Combine multiple WHERE conditions with AND
    private func buildWhereClause() -> String {
        guard !conditions.isEmpty else { return "" }
        return "WHERE " + conditions.joined(separator: " AND ")
    }
}
```

### Feature 2: Relationship Management

```swift
// Create relationships fluently
try db.createRelationship(
    from: alice,
    to: bob,
    type: Knows.self,
    properties: ["since": Date()]
)

// Query relationships
let friends = try db.match(Person.self, variable: "p")
    .related(to: Person.self, via: Knows.self, variable: "f")
    .where(\.name, .equal, "Alice", on: "p")
    .fetch() // Returns [Person] (friends of Alice)

// Traverse multi-hop relationships
let friendsOfFriends = try db.match(Person.self)
    .related(to: Person.self, via: Knows.self)
    .related(to: Person.self, via: Knows.self)
    .where(\.name, .equal, "Alice", depth: 0)
    .fetch()
```

**Implementation:**

```swift
extension QueryBuilder {
    public func related<Target: GraphNode, Rel: GraphRelationship>(
        to targetType: Target.Type,
        via relationship: Rel.Type,
        variable: String = "r"
    ) -> RelationshipQueryBuilder<T, Target, Rel> {
        return RelationshipQueryBuilder(
            from: self,
            targetType: targetType,
            relationType: relationship,
            variable: variable
        )
    }
}

public class RelationshipQueryBuilder<Source: GraphNode, Target: GraphNode, Rel: GraphRelationship> {
    // Build: MATCH (source)-[rel:TYPE]->(target)
}
```

### Feature 3: Schema & Graph Management

```swift
// High-level schema operations
let socialSchema = try db.schema("social")
try socialSchema.createIfNeeded()
try socialSchema.use()

// Graph operations
let network = try socialSchema.graph("network")
try network.createIfNeeded()
try network.use()

// Or fluent API
try db.schema("social")
    .createIfNeeded()
    .graph("network")
    .createIfNeeded()
    .use()
```

**Implementation:**

```swift
public class Schema {
    let name: String
    let db: GraphLiteDB

    @discardableResult
    public func createIfNeeded() throws -> Schema {
        try db.execute("CREATE SCHEMA IF NOT EXISTS /\(name)")
        return self
    }

    @discardableResult
    public func use() throws -> Schema {
        try db.execute("SESSION SET SCHEMA /\(name)")
        return self
    }

    public func graph(_ name: String) throws -> Graph {
        return Graph(name: name, schema: self.name, db: db)
    }
}

public class Graph {
    let name: String
    let schema: String
    let db: GraphLiteDB

    @discardableResult
    public func createIfNeeded() throws -> Graph {
        try db.execute("CREATE GRAPH IF NOT EXISTS /\(schema)/\(name)")
        return self
    }

    @discardableResult
    public func use() throws -> Graph {
        try db.execute("SESSION SET GRAPH /\(schema)/\(name)")
        return self
    }
}
```

### Feature 4: Batch Operations

```swift
// Batch insert
let people = [
    Person(id: UUID(), name: "Alice", age: 30, city: "NYC"),
    Person(id: UUID(), name: "Bob", age: 25, city: "SF"),
    Person(id: UUID(), name: "Carol", age: 28, city: "LA"),
]

try db.insertAll(people)

// Batch update
try db.updateAll(people)

// Batch delete
try db.deleteAll(people)
```

### Feature 5: Async/Await Support

```swift
// Async database operations
public class GraphLiteDB {
    public func insert<T: GraphNode>(_ node: T) async throws {
        try await Task {
            try self.insert(node)
        }.value
    }

    public func match<T: GraphNode>(_ type: T.Type) -> AsyncQueryBuilder<T> {
        return AsyncQueryBuilder(db: self, nodeType: type)
    }
}

// Usage
let people = try await db.match(Person.self)
    .where(\.age, .greaterThan, 25)
    .fetch()
```

---

## Advanced Features

### Feature 6: Aggregations

```swift
// Count
let count = try db.match(Person.self)
    .count()

// Average
let avgAge = try db.match(Person.self)
    .average(\.age)

// Group by
let peopleByCity = try db.match(Person.self)
    .groupBy(\.city)
    .count()
    .fetch() // Returns [(city: String, count: Int)]
```

### Feature 7: Subqueries

```swift
// Nested queries
let popularPeople = try db.match(Person.self, variable: "p")
    .where { builder in
        // Subquery: people with > 10 friends
        builder.count(
            db.match(Person.self)
                .related(to: Person.self, via: Knows.self)
                .where(\.name, .equal, "p.name", on: "p")
        ).greaterThan(10)
    }
    .fetch()
```

### Feature 8: RDF Support

```swift
// Load RDF data
try db.loadRDF(
    data: turtleString,
    format: .turtle,
    into: "knowledge_graph"
)

// Query RDF triples
let triples = try db.matchTriple()
    .subject(matching: "http://example.org/Alice")
    .predicate(RDF.type)
    .fetch()

// Export to RDF
let turtle = try db.exportRDF(
    graph: "knowledge_graph",
    format: .turtle
)
```

---

## SwiftUI Integration

### Property Wrapper: @GraphLiteQuery

```swift
@propertyWrapper
public struct GraphLiteQuery<T: GraphNode>: DynamicProperty {
    @State private var results: [T] = []
    @State private var isLoading = false
    @State private var error: Error?

    private let query: String
    private let db: GraphLiteDB

    public init(_ query: String) {
        self.query = query
        // Get db from environment
        self.db = GraphLiteEnvironment.current
    }

    public var wrappedValue: [T] {
        results
    }

    public var projectedValue: Binding<[T]> {
        Binding(
            get: { results },
            set: { results = $0 }
        )
    }

    public func update() {
        Task {
            isLoading = true
            do {
                let result = try await db.execute(query)
                results = try result.decode([T].self)
                error = nil
            } catch {
                self.error = error
            }
            isLoading = false
        }
    }
}
```

### Usage in SwiftUI Views

```swift
struct PeopleListView: View {
    @GraphLiteQuery("MATCH (p:Person) RETURN p ORDER BY p.name")
    var people: [Person]

    var body: some View {
        List(people) { person in
            VStack(alignment: .leading) {
                Text(person.name)
                    .font(.headline)
                Text("Age: \(person.age)")
                    .font(.caption)
            }
        }
    }
}

// Environment setup
struct MyApp: App {
    @StateObject var db = try! GraphLiteDB(path: "./app.db")

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(db)
        }
    }
}
```

### Query Builder in SwiftUI

```swift
struct PersonSearchView: View {
    @State private var minAge = 18
    @State private var city = ""
    @StateObject private var db = GraphLiteEnvironment.current

    var people: [Person] {
        get async throws {
            try await db.match(Person.self)
                .where(\.age, .greaterThanOrEqual, minAge)
                .where(\.city, .equal, city)
                .fetch()
        }
    }

    var body: some View {
        VStack {
            HStack {
                TextField("City", text: $city)
                Stepper("Min Age: \(minAge)", value: $minAge, in: 0...100)
            }

            List {
                AsyncContentView(source: people) { people in
                    ForEach(people) { person in
                        PersonRow(person: person)
                    }
                }
            }
        }
    }
}
```

### Observable Models

```swift
@Observable
class PersonViewModel {
    private let db: GraphLiteDB
    var people: [Person] = []
    var isLoading = false
    var error: Error?

    init(db: GraphLiteDB) {
        self.db = db
    }

    func loadPeople() async {
        isLoading = true
        do {
            people = try await db.match(Person.self)
                .orderBy(\.name)
                .fetch()
            error = nil
        } catch {
            self.error = error
        }
        isLoading = false
    }

    func addPerson(_ person: Person) async throws {
        try await db.insert(person)
        await loadPeople()
    }

    func deletePerson(_ person: Person) async throws {
        try await db.delete(person)
        await loadPeople()
    }
}

// Usage
struct ContentView: View {
    @State private var viewModel: PersonViewModel

    init(db: GraphLiteDB) {
        _viewModel = State(initialValue: PersonViewModel(db: db))
    }

    var body: some View {
        List {
            ForEach(viewModel.people) { person in
                PersonRow(person: person)
            }
            .onDelete { indexSet in
                Task {
                    for index in indexSet {
                        try await viewModel.deletePerson(viewModel.people[index])
                    }
                }
            }
        }
        .task {
            await viewModel.loadPeople()
        }
    }
}
```

---

## Implementation Roadmap

### Week 1-2: Core SDK Foundation

**Days 1-3: Type System**
- [ ] Implement `GraphNode` protocol
- [ ] Implement `GraphRelationship` protocol
- [ ] Create Codable extensions
- [ ] Property encoding/decoding
- [ ] Unit tests

**Days 4-5: Database API**
- [ ] Implement `GraphLiteDB` class
- [ ] Wrap Phase 1 bindings
- [ ] Session management
- [ ] Error handling
- [ ] Integration tests

### Week 3: Query Builder

**Days 6-8: Basic Query Builder**
- [ ] Implement `QueryBuilder` class
- [ ] WHERE clause builder
- [ ] ORDER BY support
- [ ] LIMIT/OFFSET support
- [ ] Type-safe property access via KeyPath
- [ ] Unit tests

**Days 9-10: Advanced Queries**
- [ ] Aggregations (COUNT, AVG, SUM)
- [ ] GROUP BY support
- [ ] HAVING clause
- [ ] Subquery support
- [ ] Unit tests

### Week 4: CRUD & Relationships

**Days 11-12: CRUD Operations**
- [ ] insert() method
- [ ] insertAll() batch insertion
- [ ] update() method
- [ ] delete() method
- [ ] find() method
- [ ] Unit tests

**Days 13-14: Relationships**
- [ ] createRelationship() method
- [ ] deleteRelationship() method
- [ ] Relationship query builder
- [ ] Multi-hop traversal
- [ ] Unit tests

### Week 5: Transactions & Schema

**Days 15-16: Transactions**
- [ ] Transaction class
- [ ] Auto-commit/rollback
- [ ] Nested transactions
- [ ] Savepoint support (future)
- [ ] Unit tests

**Days 17-18: Schema Management**
- [ ] Schema class
- [ ] Graph class
- [ ] Fluent API
- [ ] createIfNeeded() helpers
- [ ] Unit tests

### Week 6: SwiftUI Integration

**Days 19-21: Property Wrappers**
- [ ] @GraphLiteQuery implementation
- [ ] Environment setup
- [ ] ObservableObject support
- [ ] Automatic refresh
- [ ] SwiftUI examples

**Days 22-23: Async/Await**
- [ ] Async query methods
- [ ] Task-based API
- [ ] Error handling
- [ ] Performance optimization
- [ ] Documentation

**Days 24-25: Polish & Release**
- [ ] Documentation
- [ ] Example apps
- [ ] Performance benchmarks
- [ ] API review
- [ ] Release preparation

---

## Code Examples

### Example 1: Social Network App

```swift
// Models
struct Person: GraphNode {
    let id: UUID
    var name: String
    var bio: String
    var avatarURL: URL?
}

struct Follow: GraphRelationship {
    static let type = "FOLLOWS"
    let from: UUID
    let to: UUID
    let since: Date
}

// ViewModel
@Observable
class SocialViewModel {
    let db: GraphLiteDB
    var currentUser: Person?
    var following: [Person] = []
    var followers: [Person] = []

    func loadUserData(userId: UUID) async throws {
        // Load current user
        currentUser = try await db.find(Person.self, id: userId)

        // Load who user is following
        following = try await db.match(Person.self, variable: "f")
            .related(from: currentUser!, via: Follow.self)
            .fetch()

        // Load followers
        followers = try await db.match(Person.self, variable: "f")
            .related(to: currentUser!, via: Follow.self)
            .fetch()
    }

    func followUser(_ user: Person) async throws {
        guard let current = currentUser else { return }

        let follow = Follow(
            from: current.id,
            to: user.id,
            since: Date()
        )

        try await db.createRelationship(follow)
        await loadUserData(userId: current.id)
    }
}

// SwiftUI View
struct ProfileView: View {
    @State var viewModel: SocialViewModel
    let userId: UUID

    var body: some View {
        ScrollView {
            if let user = viewModel.currentUser {
                VStack {
                    AsyncImage(url: user.avatarURL)
                        .frame(width: 100, height: 100)
                        .clipShape(Circle())

                    Text(user.name)
                        .font(.title)

                    Text(user.bio)
                        .foregroundColor(.secondary)

                    HStack {
                        VStack {
                            Text("\(viewModel.following.count)")
                                .font(.headline)
                            Text("Following")
                                .font(.caption)
                        }

                        VStack {
                            Text("\(viewModel.followers.count)")
                                .font(.headline)
                            Text("Followers")
                                .font(.caption)
                        }
                    }
                }
            }
        }
        .task {
            try? await viewModel.loadUserData(userId: userId)
        }
    }
}
```

### Example 2: Knowledge Graph Viewer

```swift
// Models
struct Concept: GraphNode {
    let id: UUID
    var name: String
    var description: String
    var category: String
}

struct RelatedTo: GraphRelationship {
    static let type = "RELATED_TO"
    let from: UUID
    let to: UUID
    var strength: Double // 0.0 to 1.0
}

// View
struct KnowledgeGraphView: View {
    @State private var db: GraphLiteDB
    @State private var concepts: [Concept] = []
    @State private var selectedConcept: Concept?

    var body: some View {
        HStack {
            // Concept list
            List(concepts, selection: $selectedConcept) { concept in
                VStack(alignment: .leading) {
                    Text(concept.name)
                        .font(.headline)
                    Text(concept.category)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            .frame(width: 300)

            // Detail view
            if let selected = selectedConcept {
                ConceptDetailView(concept: selected, db: db)
            }
        }
        .task {
            concepts = try! await db.match(Concept.self)
                .orderBy(\.name)
                .fetch()
        }
    }
}

struct ConceptDetailView: View {
    let concept: Concept
    let db: GraphLiteDB

    @State private var relatedConcepts: [Concept] = []

    var body: some View {
        VStack(alignment: .leading) {
            Text(concept.name)
                .font(.largeTitle)

            Text(concept.description)
                .padding()

            Text("Related Concepts")
                .font(.headline)

            ForEach(relatedConcepts) { related in
                NavigationLink(value: related) {
                    Text(related.name)
                }
            }
        }
        .task {
            relatedConcepts = try! await db.match(Concept.self)
                .related(from: concept, via: RelatedTo.self)
                .orderBy(\.name)
                .fetch()
        }
    }
}
```

### Example 3: Task Dependency Graph

```swift
// Models
struct Task: GraphNode {
    let id: UUID
    var title: String
    var status: TaskStatus
    var priority: Int
    var assignee: String?
}

enum TaskStatus: String, Codable {
    case todo
    case inProgress = "in_progress"
    case done
    case blocked
}

struct DependsOn: GraphRelationship {
    static let type = "DEPENDS_ON"
    let from: UUID  // dependent task
    let to: UUID    // dependency
}

// ViewModel
@Observable
class TaskViewModel {
    let db: GraphLiteDB
    var tasks: [Task] = []

    func loadTasks() async throws {
        tasks = try await db.match(Task.self)
            .orderBy(\.priority, ascending: false)
            .fetch()
    }

    func addTask(_ task: Task, dependsOn dependencies: [Task] = []) async throws {
        // Insert task
        try await db.transaction { tx in
            try tx.insert(task)

            // Create dependency relationships
            for dependency in dependencies {
                try tx.createRelationship(
                    DependsOn(from: task.id, to: dependency.id)
                )
            }
        }

        await loadTasks()
    }

    func canStart(_ task: Task) async throws -> Bool {
        // Check if all dependencies are completed
        let dependencies = try await db.match(Task.self)
            .related(from: task, via: DependsOn.self)
            .fetch()

        return dependencies.allSatisfy { $0.status == .done }
    }

    func blockingTasks(for task: Task) async throws -> [Task] {
        // Find incomplete dependencies
        return try await db.match(Task.self)
            .related(from: task, via: DependsOn.self)
            .where(\.status, .notEqual, TaskStatus.done)
            .fetch()
    }
}

// View
struct TaskListView: View {
    @State var viewModel: TaskViewModel

    var body: some View {
        List {
            ForEach(viewModel.tasks) { task in
                TaskRow(task: task, viewModel: viewModel)
            }
        }
        .task {
            try? await viewModel.loadTasks()
        }
    }
}

struct TaskRow: View {
    let task: Task
    let viewModel: TaskViewModel

    @State private var canStart = false
    @State private var blockingTasks: [Task] = []

    var body: some View {
        VStack(alignment: .leading) {
            HStack {
                Text(task.title)
                    .font(.headline)

                Spacer()

                TaskStatusBadge(status: task.status)
            }

            if !blockingTasks.isEmpty {
                Text("Blocked by: \(blockingTasks.map(\.title).joined(separator: ", "))")
                    .font(.caption)
                    .foregroundColor(.red)
            }
        }
        .opacity(canStart ? 1.0 : 0.5)
        .task {
            canStart = try! await viewModel.canStart(task)
            blockingTasks = try! await viewModel.blockingTasks(for: task)
        }
    }
}
```

---

## Performance Considerations

### Query Optimization

```swift
// Bad: N+1 query problem
for person in people {
    let friends = try db.match(Person.self)
        .related(from: person, via: Follow.self)
        .fetch()
}

// Good: Single query with relationships
let peopleWithFriends = try db.match(Person.self)
    .includeRelated(Follow.self)
    .fetch()
```

### Caching Strategy

```swift
public class GraphLiteDB {
    private var cache: [String: Any] = [:]

    public func cached<T>(_ key: String, fetch: () throws -> T) throws -> T {
        if let cached = cache[key] as? T {
            return cached
        }

        let value = try fetch()
        cache[key] = value
        return value
    }
}

// Usage
let people = try db.cached("all_people") {
    try db.match(Person.self).fetch()
}
```

### Batch Operations

```swift
// Batch insert with single transaction
try db.transaction { tx in
    for person in largeBatchOfPeople {
        try tx.insert(person)
    }
}
```

---

## Comparison: Bindings vs SDK

| Feature | Phase 1 Bindings | Phase 2 SDK |
|---------|-----------------|-------------|
| **API Style** | Low-level, string queries | High-level, fluent builders |
| **Type Safety** | Runtime (JSON) | Compile-time (generics) |
| **Learning Curve** | Steep (GQL knowledge) | Gentle (discoverable) |
| **Code Volume** | ~300 lines | ~2000-3000 lines |
| **Implementation Time** | 1-2 weeks | 4-6 weeks |
| **Target Users** | Expert developers | App developers |
| **SwiftUI Integration** | Manual | Property wrappers |
| **Transactions** | Manual | Automatic |
| **Query Builder** |  |  |
| **Type-safe Models** |  |  |
| **IDE Autocomplete** | Limited | Full |
| **Performance** | Minimal overhead | Slight overhead |
| **Maintenance** | Low | Medium |

---

## Next Steps

### Phase 1: Swift Bindings (1-2 weeks)
- Thin wrapper over C FFI
- Validates approach
- Quick iOS/macOS support

### Phase 2: Swift SDK (4-6 weeks)
- Build on top of bindings
- High-level abstractions
- SwiftUI integration

### Phase 3: Advanced Features (2-3 weeks)
- RDF support
- Complex queries
- Performance optimization

### Phase 4: Production Ready (1-2 weeks)
- Documentation
- Examples
- Performance tuning
- Release

**Total: 8-13 weeks** from start to production-ready SDK

---

## Decision: When to Build SDK?

**Build SDK if:**
-  Phase 1 bindings working well
-  User demand for higher-level API
-  Target is iOS/macOS app developers
-  Have 4-6 weeks to invest
-  Want to compete with native Swift graph libraries

**Stick with Bindings if:**
-  Target is Rust developers using Swift
-  Want minimal maintenance
-  Limited development time
-  Users prefer low-level control

---

**Document Version:** 1.0
**Last Updated:** 2025-11-25
**Status:** Design Phase
**Maintained By:** GraphLite Contributors
