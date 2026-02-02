# GraphLite Node.js SDK - Implementation Plan

**Date:** February 1, 2026  
**Version:** 0.1.0  
**Status:** Planning Phase

## Executive Summary

This document outlines the implementation plan for the **GraphLite Node.js SDK** - a high-level, ergonomic SDK that wraps the low-level FFI bindings (already implemented in `bindings/javascript/`) with a developer-friendly API following patterns from successful embedded databases.

## Goals

### Primary Goals
1. **High-Level Ergonomic API** - Developer-friendly interface following Node.js best practices
2. **Transaction Support** - ACID guarantees with automatic rollback (context pattern)
3. **Query Builder** - Fluent API for constructing type-safe GQL queries
4. **TypeScript-First** - Full type safety with excellent IDE integration
5. **Zero External Dependencies** - Only dependency is the FFI bindings layer
6. **Production Ready** - Comprehensive testing, documentation, and error handling

### Secondary Goals
- **Connection Pooling** - Efficient concurrent access patterns (future)
- **Async/Await Native** - Modern Promise-based API throughout
- **ORM Features** - Optional decorators for mapping classes to graph entities (future)
- **Query Validation** - Runtime query validation (future)

## Architecture

```text
┌─────────────────────────────────────────────────┐
│   Your Application (Node.js/TypeScript)        │
└─────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│  GraphLite SDK (sdk-nodejs) - NEW               │
│  ├── GraphLite (main connection API)            │
│  ├── Session (session management)               │
│  ├── Transaction (ACID support)                 │
│  ├── QueryBuilder (fluent query API)            │
│  └── TypedResult (deserialization)              │
└─────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│  GraphLite Bindings (bindings/javascript)       │
│  ├── GraphLite (FFI wrapper class)              │
│  ├── QueryResult (result handling)              │
│  ├── GraphLiteError (error types)               │
│  └── FFI Loader (koffi integration)             │
└─────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│  GraphLite FFI (Rust)                           │
│  libgraphlite_ffi.{dylib,so,dll}                │
└─────────────────────────────────────────────────┘
```

## Comparison with Existing SDKs

### Python SDK (sdk-python)
- **Strengths**: Clean API, comprehensive transaction support, query builder, typed results
- **Patterns to Adopt**: Transaction context managers (as async functions), QueryBuilder fluent API
- **Differences**: Python uses dataclasses, we'll use TypeScript classes/interfaces

### Rust SDK (sdk-rust)
- **Strengths**: Zero-cost abstractions, RAII transaction pattern, excellent type safety
- **Patterns to Adopt**: Connection/Session separation, query builder chaining
- **Differences**: Rust has Drop trait (RAII), we'll use async/await and try/finally

### Key Similarities
- Both follow SQLite/rusqlite patterns adapted for graphs
- Both provide transaction support with automatic rollback
- Both have query builders for fluent query construction
- Both separate connection from session management

## Module Structure

```
sdk-nodejs/
├── package.json                    # NPM package configuration
├── tsconfig.json                   # TypeScript configuration
├── jest.config.js                  # Test configuration
├── README.md                       # SDK documentation
├── IMPLEMENTATION_PLAN.md          # This document
├── src/
│   ├── index.ts                    # Main exports
│   ├── connection.ts               # GraphLite class
│   ├── session.ts                  # Session class
│   ├── transaction.ts              # Transaction class with auto-rollback
│   ├── query-builder.ts            # QueryBuilder fluent API
│   ├── typed-result.ts             # TypedResult with deserialization
│   ├── errors.ts                   # SDK-specific error classes
│   └── types.ts                    # TypeScript type definitions
├── examples/
│   ├── basic-usage.ts              # Basic usage example
│   ├── transactions.ts             # Transaction patterns
│   ├── query-builder.ts            # Query builder examples
│   └── typed-results.ts            # Typed deserialization examples
├── test/
│   ├── connection.test.ts          # Connection tests
│   ├── session.test.ts             # Session tests
│   ├── transaction.test.ts         # Transaction tests
│   ├── query-builder.test.ts       # Query builder tests
│   └── typed-result.test.ts        # Typed result tests
└── docs/
    ├── API.md                      # API reference
    ├── MIGRATION.md                # Migration from bindings to SDK
    └── EXAMPLES.md                 # Comprehensive examples
```

## Core Components

### 1. GraphLite (Connection)

Main entry point for database operations.

```typescript
class GraphLite {
    // Static factory method (follows SQLite pattern)
    static open(path: string): GraphLite;
    
    // Create a session for a user
    session(username: string): Session;
    
    // Close database and cleanup
    close(): void;
    
    // Get version information
    static version(): string;
}
```

**Implementation Notes:**
- Wraps the low-level GraphLite binding from `bindings/javascript`
- Manages database lifecycle
- Factory pattern for initialization
- Auto-cleanup on process exit (optional)

### 2. Session

Represents a user session with permission context.

```typescript
class Session {
    // Query execution
    async query(query: string): Promise<QueryResult>;
    
    // Execute without results (DDL/DML)
    async execute(statement: string): Promise<void>;
    
    // Begin a transaction
    transaction(): Transaction;
    
    // Create query builder
    queryBuilder(): QueryBuilder;
    
    // Session metadata
    id(): string;
    username(): string;
}
```

**Implementation Notes:**
- Wraps session ID from bindings
- Provides convenience methods
- Transaction factory
- Query builder factory

### 3. Transaction

ACID transaction support with automatic rollback.

```typescript
class Transaction {
    // Execute statement in transaction
    async execute(statement: string): Promise<void>;
    
    // Query in transaction
    async query(query: string): Promise<QueryResult>;
    
    // Commit changes
    async commit(): Promise<void>;
    
    // Rollback changes
    async rollback(): Promise<void>;
    
    // Auto-rollback support (using try/finally or Symbol.asyncDispose)
    [Symbol.asyncDispose](): Promise<void>;
}
```

**Implementation Notes:**
- Follows rusqlite pattern: auto-rollback unless committed
- Uses `BEGIN TRANSACTION` / `COMMIT` / `ROLLBACK` GQL commands
- Supports both explicit and automatic rollback
- TypeScript 5.2+ `using` keyword support (Symbol.asyncDispose)
- Fallback to manual try/finally for older TypeScript

**Usage Pattern:**
```typescript
// Modern TypeScript 5.2+ with 'using'
await using tx = session.transaction();
await tx.execute("INSERT ...");
await tx.commit(); // Required to persist

// Legacy pattern (always works)
const tx = session.transaction();
try {
    await tx.execute("INSERT ...");
    await tx.commit();
} finally {
    // Auto-rollback if not committed
    await tx[Symbol.asyncDispose]?.();
}
```

### 4. QueryBuilder

Fluent API for building GQL queries.

```typescript
class QueryBuilder {
    // Fluent methods (all return this)
    match(pattern: string): this;
    where(condition: string): this;
    with(clause: string): this;
    return(clause: string): this;
    orderBy(clause: string): this;
    skip(n: number): this;
    limit(n: number): this;
    
    // Terminal operations
    build(): string;
    async execute(): Promise<QueryResult>;
}
```

**Implementation Notes:**
- Chainable fluent API
- Builds GQL query string internally
- Terminal operations: `build()` returns string, `execute()` runs query
- Type-safe with TypeScript generics (future enhancement)

**Usage Pattern:**
```typescript
const result = await session
    .queryBuilder()
    .match("(p:Person)")
    .where("p.age > 25")
    .return("p.name, p.age")
    .orderBy("p.age DESC")
    .limit(10)
    .execute();
```

### 5. TypedResult

Result wrapper with typed deserialization.

```typescript
class TypedResult<T = any> {
    constructor(result: QueryResult);
    
    // Deserialize all rows
    deserializeRows<T>(type: { new(...args: any[]): T }): T[];
    
    // Deserialize first row
    first<T>(type: { new(...args: any[]): T }): T | null;
    
    // Get scalar value (first row, first column)
    scalar(): any;
    
    // Raw access
    rows(): Array<Record<string, any>>;
    rowCount(): number;
    columnNames(): string[];
    isEmpty(): boolean;
}
```

**Implementation Notes:**
- Wraps QueryResult from bindings
- Provides deserialization to TypeScript classes
- Supports both class constructors and factory functions
- Uses constructor parameter matching or property assignment

**Usage Pattern:**
```typescript
class Person {
    constructor(public name: string, public age: number) {}
}

const result = await session.query("MATCH (p:Person) RETURN p.name, p.age");
const typed = new TypedResult(result);
const people = typed.deserializeRows(Person);
```

### 6. Error Handling

SDK-specific error hierarchy.

```typescript
// Base SDK error
class GraphLiteSDKError extends Error {
    constructor(message: string, cause?: Error);
}

// Specific error types
class ConnectionError extends GraphLiteSDKError {}
class SessionError extends GraphLiteSDKError {}
class QueryError extends GraphLiteSDKError {}
class TransactionError extends GraphLiteSDKError {}
class DeserializationError extends GraphLiteSDKError {}
```

**Implementation Notes:**
- Wraps errors from bindings layer
- Adds SDK-specific context
- Preserves original error as `cause`
- Follows Node.js error conventions

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1)
**Goal:** Basic SDK structure with connection and session management

**Tasks:**
- [ ] Set up project structure (package.json, tsconfig.json, etc.)
- [ ] Implement `GraphLite` class (connection management)
- [ ] Implement `Session` class (wraps binding session)
- [ ] Implement error classes hierarchy
- [ ] Basic TypeScript type definitions
- [ ] Unit tests for connection and session
- [ ] Basic README with installation instructions

**Deliverables:**
- Working `GraphLite.open()` and `session()` API
- Basic query execution via `session.query()` and `session.execute()`
- Error handling foundation
- ~60% test coverage

### Phase 2: Transaction Support (Week 2)
**Goal:** ACID transaction support with automatic rollback

**Tasks:**
- [ ] Implement `Transaction` class
- [ ] Automatic rollback using Symbol.asyncDispose
- [ ] Fallback pattern for older TypeScript
- [ ] Transaction state tracking (committed/rolled back)
- [ ] Nested transaction prevention
- [ ] Transaction tests (commit, rollback, auto-rollback)
- [ ] Transaction examples

**Deliverables:**
- Full transaction support with auto-rollback
- `using` keyword support (TypeScript 5.2+)
- Legacy try/finally pattern
- ~70% test coverage

### Phase 3: Query Builder (Week 3)
**Goal:** Fluent API for query construction

**Tasks:**
- [ ] Implement `QueryBuilder` class
- [ ] All GQL clauses (MATCH, WHERE, WITH, RETURN, etc.)
- [ ] Query building and validation
- [ ] Integration with Session
- [ ] Query builder tests
- [ ] Query builder examples
- [ ] Documentation for query patterns

**Deliverables:**
- Full query builder API
- Method chaining support
- Query validation (basic)
- ~75% test coverage

### Phase 4: Typed Results (Week 4)
**Goal:** Type-safe result deserialization

**Tasks:**
- [ ] Implement `TypedResult` class
- [ ] Class-based deserialization
- [ ] Factory function support
- [ ] Scalar and first() helpers
- [ ] Type inference support
- [ ] Typed result tests
- [ ] Deserialization examples

**Deliverables:**
- TypedResult with deserialization
- Class constructor mapping
- Property assignment fallback
- ~80% test coverage

### Phase 5: Documentation & Polish (Week 5)
**Goal:** Production-ready SDK with comprehensive docs

**Tasks:**
- [ ] Complete API documentation
- [ ] Migration guide from bindings to SDK
- [ ] Comprehensive examples
- [ ] Performance benchmarks
- [ ] Integration tests
- [ ] CI/CD setup
- [ ] NPM package preparation

**Deliverables:**
- Complete API reference
- Migration guide
- 10+ examples
- ~90% test coverage
- Ready for NPM publish

## API Design Principles

### 1. Follow Node.js Conventions
- Async/await everywhere (no callbacks)
- Promise-based APIs
- Error-first patterns
- EventEmitter where appropriate

### 2. TypeScript-First Design
- Full type safety
- Generics for type inference
- Discriminated unions for complex types
- JSDoc comments for IDE hints

### 3. SQLite Patterns (Adapted for Graphs)
- `open()` instead of `connect()` (embedded DB)
- Sessions instead of direct connection
- Transaction auto-rollback (rusqlite pattern)
- Simple, intuitive API

### 4. Error Handling
- Specific error types for different failures
- Preserve error chain with `cause`
- Clear error messages with context
- Fail-fast for developer errors

### 5. Performance
- Minimal overhead over bindings
- Lazy evaluation where possible
- No unnecessary copies
- Connection pooling (future)

## Testing Strategy

### Unit Tests
- Each module in isolation
- Mock bindings layer
- Edge cases and error conditions
- ~80% code coverage target

### Integration Tests
- End-to-end scenarios
- Real database operations
- Transaction semantics
- Query builder correctness

### Performance Tests
- Benchmark against bindings
- Memory leak detection
- Connection overhead
- Query throughput

### Example Tests
- All examples must run successfully
- Examples serve as integration tests
- Keep examples simple and focused

## Documentation Structure

### README.md
- Quick start (30 seconds to first query)
- Installation instructions
- Basic examples
- Links to detailed docs

### API.md
- Complete API reference
- All classes, methods, parameters
- Return types and errors
- Code examples for each method

### EXAMPLES.md
- Comprehensive usage examples
- Common patterns and idioms
- Best practices
- Performance tips

### MIGRATION.md
- Guide for users of bindings
- API differences
- Migration checklist
- Code transformation examples

## Success Criteria

### Functional Requirements
- ✅ All core features implemented (connection, session, transaction, query builder, typed results)
- ✅ Comprehensive error handling
- ✅ Full TypeScript support
- ✅ Works on Node.js 14+ (or 16+)

### Quality Requirements
- ✅ 80%+ test coverage
- ✅ All tests passing
- ✅ No memory leaks
- ✅ API documentation complete

### Performance Requirements
- ✅ <10% overhead vs bindings layer
- ✅ Handles 1000+ queries/sec
- ✅ Memory usage <50MB for typical workload

### Developer Experience
- ✅ Clear error messages
- ✅ Excellent TypeScript IntelliSense
- ✅ Easy to get started (<5 minutes)
- ✅ Comprehensive examples

## Dependencies

### Required
- `graphlite` (from `bindings/javascript/`) - Low-level FFI bindings
- TypeScript 4.7+ (for development)
- Node.js 14+ or 16+ (runtime)

### Development Dependencies
- Jest (testing)
- TypeScript (compilation)
- ESLint (linting)
- Prettier (formatting)
- TypeDoc (API docs generation)

### Zero External Runtime Dependencies
- SDK should only depend on the bindings
- No lodash, no axios, no unnecessary bloat
- Keep bundle size minimal

## Risk Analysis

### Technical Risks

**Risk 1: Transaction Auto-Rollback Complexity**
- **Impact:** High
- **Probability:** Medium
- **Mitigation:** Implement fallback pattern, extensive testing, clear documentation

**Risk 2: TypeScript Compatibility**
- **Impact:** Medium
- **Probability:** Low
- **Mitigation:** Support multiple TypeScript versions, test on TS 4.7, 5.0, 5.2+

**Risk 3: Performance Overhead**
- **Impact:** Medium
- **Probability:** Low
- **Mitigation:** Benchmark early, optimize hot paths, minimize abstractions

**Risk 4: Query Builder Complexity**
- **Impact:** Low
- **Probability:** Medium
- **Mitigation:** Start simple, iterate based on feedback, comprehensive tests

### Project Risks

**Risk 1: Scope Creep**
- **Impact:** High
- **Probability:** Medium
- **Mitigation:** Strict phase boundaries, defer advanced features

**Risk 2: Documentation Lag**
- **Impact:** Medium
- **Probability:** High
- **Mitigation:** Write docs alongside code, examples as tests

## Future Enhancements

### Version 0.2.0
- **Connection Pooling** - Efficient multi-threaded access
- **Query Validation** - Runtime query validation before execution
- **Enhanced Type Inference** - Better TypeScript generics for query builder

### Version 0.3.0
- **ORM Features** - Decorators for entity mapping
- **Graph Algorithms** - Built-in shortest path, centrality, etc.
- **Schema Validation** - Runtime schema validation

### Version 1.0.0
- **Async Iterators** - Stream large result sets
- **Worker Thread Support** - Database in worker threads
- **Migration Tools** - Database schema migrations
- **Admin Tools** - Database introspection and management

## Appendix A: Code Examples

### Basic Usage

```typescript
import { GraphLite } from '@graphlite/sdk';

// Open database
const db = GraphLite.open('./mydb');

// Create session
const session = db.session('admin');

// Execute query
const result = await session.query('MATCH (p:Person) RETURN p.name, p.age');

for (const row of result.rows) {
    console.log(`${row['p.name']}: ${row['p.age']}`);
}

// Cleanup
db.close();
```

### Transaction Usage

```typescript
// TypeScript 5.2+ with 'using'
await using tx = session.transaction();
await tx.execute("INSERT (:Person {name: 'Alice', age: 30})");
await tx.execute("INSERT (:Person {name: 'Bob', age: 25})");
await tx.commit(); // Required to persist

// Legacy pattern
const tx = session.transaction();
try {
    await tx.execute("INSERT (:Person {name: 'Alice', age: 30})");
    await tx.execute("INSERT (:Person {name: 'Bob', age: 25})");
    await tx.commit();
} finally {
    // Auto-rollback if not committed
}
```

### Query Builder Usage

```typescript
const result = await session
    .queryBuilder()
    .match("(p:Person)")
    .where("p.age > 25")
    .where("p.city = 'NYC'")
    .return("p.name, p.age, p.city")
    .orderBy("p.age DESC")
    .limit(10)
    .execute();
```

### Typed Results Usage

```typescript
class Person {
    constructor(
        public name: string,
        public age: number,
        public city: string
    ) {}
}

const result = await session.query(
    "MATCH (p:Person) RETURN p.name, p.age, p.city"
);

const typed = new TypedResult(result);
const people = typed.deserializeRows(Person);

for (const person of people) {
    console.log(`${person.name}, ${person.age}, ${person.city}`);
}
```

## Appendix B: Comparison with Bindings

### Before (Low-Level Bindings)

```typescript
import { GraphLite, QueryResult } from 'graphlite';

const db = new GraphLite('./mydb');
const sessionId = db.createSession('admin');

// Manual transaction management
await db.execute(sessionId, 'BEGIN TRANSACTION');
try {
    await db.execute(sessionId, "INSERT (:Person {name: 'Alice'})");
    await db.execute(sessionId, "INSERT (:Person {name: 'Bob'})");
    await db.execute(sessionId, 'COMMIT');
} catch (e) {
    await db.execute(sessionId, 'ROLLBACK');
    throw e;
}

const result = await db.query(sessionId, 'MATCH (p:Person) RETURN p');
for (const row of result.rows) {
    // Manual data extraction
    const name = row['p.name'];
    console.log(name);
}
```

### After (High-Level SDK)

```typescript
import { GraphLite } from '@graphlite/sdk';

const db = GraphLite.open('./mydb');
const session = db.session('admin');

// Automatic transaction management
await using tx = session.transaction();
await tx.execute("INSERT (:Person {name: 'Alice'})");
await tx.execute("INSERT (:Person {name: 'Bob'})");
await tx.commit();

// Type-safe results
class Person {
    constructor(public name: string) {}
}

const result = await session.query('MATCH (p:Person) RETURN p.name');
const typed = new TypedResult(result);
const people = typed.deserializeRows(Person);

for (const person of people) {
    console.log(person.name);
}
```

## Appendix C: Implementation Timeline

**Total Duration:** 5 weeks

| Phase | Duration | Start | End | Key Milestone |
|-------|----------|-------|-----|---------------|
| 1. Core Infrastructure | 1 week | Week 1 | Week 1 | Connection & Session working |
| 2. Transaction Support | 1 week | Week 2 | Week 2 | Auto-rollback working |
| 3. Query Builder | 1 week | Week 3 | Week 3 | Full query builder API |
| 4. Typed Results | 1 week | Week 4 | Week 4 | Deserialization working |
| 5. Documentation & Polish | 1 week | Week 5 | Week 5 | Production ready |

**Milestones:**
- Week 1: First query executes via SDK
- Week 2: First transaction commits
- Week 3: First query built with builder
- Week 4: First typed deserialization
- Week 5: NPM package published

## Sign-Off

**Created By:** GitHub Copilot (Claude Sonnet 4.5)  
**Date:** February 1, 2026  
**Status:** Awaiting Approval

**Approval Required From:**
- [ ] Technical Lead
- [ ] Product Owner
- [ ] Engineering Team

**Next Steps:**
1. Review and approve this plan
2. Set up project structure
3. Begin Phase 1 implementation
4. Weekly progress reviews
