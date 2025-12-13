# GraphLite Source Code Overview

**Complete File-by-File Guide to Understanding GraphLite Architecture**

This document provides a comprehensive guide to every file in the `graphlite/src/` directory, explaining what each file does, how it fits into the overall system, and the most important things to know for understanding the codebase.

---

## Table of Contents

1. [Overview](#overview)
2. [Root Level](#root-level)
3. [AST Module](#ast-module) - Query Parsing and Representation
4. [Cache Module](#cache-module) - Multi-level Caching System
5. [Catalog Module](#catalog-module) - Metadata Management
6. [Coordinator Module](#coordinator-module) - Query Orchestration
7. [Exec Module](#exec-module) - Query Execution Engine
8. [Functions Module](#functions-module) - Built-in Functions
9. [Plan Module](#plan-module) - Query Planning and Optimization
10. [Schema Module](#schema-module) - Graph Type System
11. [Session Module](#session-module) - Session and Authentication
12. [Storage Module](#storage-module) - Multi-tier Storage
13. [Txn Module](#txn-module) - Transaction Management
14. [Types Module](#types-module) - Type System
15. [Integration Guide](#integration-guide)

---

## Overview

**GraphLite** is a pure Rust embedded graph database implementing the **ISO GQL standard**. The codebase contains **163 Rust source files** organized into **12 major modules**. Each module has a specific responsibility in the 7-layer architecture:

### 7-Layer Architecture

```
Layer 1: User Interface (CLI/SDK)
         ↓
Layer 2: Query Coordinator (coordinator/)
         ↓
Layer 3: Session Manager (session/)
         ↓
Layer 4: Query Executor (exec/)
         ↓
Layer 5: Query Planner (plan/)
         ↓
Layer 6: Storage Manager (storage/)
         ↓
Layer 7: Persistent Backend (Sled/RocksDB)
```

### Cross-Cutting Concerns

- **Transaction Manager** (txn/) - ACID properties
- **Catalog Manager** (catalog/) - Metadata and schema
- **Cache Manager** (cache/) - Multi-level caching
- **Function Registry** (functions/) - Built-in functions

---

## Root Level

### lib.rs

**Location**: [graphlite/src/lib.rs](graphlite/src/lib.rs)
**Lines**: 63
**Purpose**: Main library entry point defining public API and module structure

**Key Structures**:
```rust
// Public API - ONLY coordinator is exposed
pub use coordinator::{QueryCoordinator, QueryInfo, QueryPlan, QueryResult, QueryType, Row};
pub use storage::Value;

// All other modules are pub(crate) - internal only
pub(crate) mod ast;
pub(crate) mod cache;
pub(crate) mod catalog;
pub(crate) mod exec;
// ... etc
```

**Critical Details**:
- **Single Entry Point**: Only `QueryCoordinator` is public
- All other modules are internal (`pub(crate)`)
- Clean API boundary between public and internal functionality
- Version constants exported for CLI usage

**Integration**: This is the ONLY file external users interact with. All functionality accessed through `QueryCoordinator`.

---

## AST Module

**Directory**: [graphlite/src/ast/](graphlite/src/ast/)
**Purpose**: Abstract Syntax Tree representation, lexing, parsing, and validation of ISO GQL queries

The AST module is the foundation of query processing. It converts raw query text into structured data that can be analyzed, validated, and executed.

### ast/mod.rs

**Location**: [graphlite/src/ast/mod.rs](graphlite/src/ast/mod.rs)
**Purpose**: Module declaration exporting AST components

**Exports**:
- `ast::*` - All AST node types
- `lexer` - Tokenization
- `parser` - Query parsing
- `pretty_printer` - Debug visualization
- `validator` - Semantic validation

---

### ast/ast.rs

**Location**: [graphlite/src/ast/ast.rs](graphlite/src/ast/ast.rs)
**Lines**: 1799
**Purpose**: Complete Abstract Syntax Tree definition for ISO GQL

**Key Structures**:

#### 1. Document - Root AST Node
```rust
pub struct Document {
    pub statement: Statement,
    pub location: Location,
}
```

#### 2. Statement - Top-level Statement Types
```rust
pub enum Statement {
    Query(Query),                      // MATCH...RETURN
    Select(SelectStatement),           // SELECT queries
    Call(CallStatement),               // CALL procedures
    DataStatement(DataStatement),      // INSERT/DELETE/SET/REMOVE
    CatalogStatement(CatalogStatement), // CREATE/DROP GRAPH/SCHEMA
    SessionStatement(SessionStatement), // SET GRAPH/SCHEMA
    TransactionStatement(TransactionStatement), // BEGIN/COMMIT/ROLLBACK
    IndexStatement(IndexStatement),    // CREATE/DROP INDEX
    Declare(DeclareStatement),         // Variable declarations
    Let(LetStatement),                 // LET x = expression
    Next(NextStatement),               // Iteration
    AtLocation(AtLocationStatement),   // Location-specific
    ProcedureBody(ProcedureBodyStatement), // Procedure definitions
}
```

#### 3. Query Types
```rust
pub enum Query {
    BasicQuery(BasicQuery),           // MATCH...WHERE...RETURN
    SetOperation(Box<Query>, SetOperator, Box<Query>), // UNION/INTERSECT/EXCEPT
    Limited(Box<Query>, QueryModifiers), // ORDER BY/LIMIT
    WithQuery(WithQuery),             // WITH clause pipelines
    MutationPipeline(MutationPipeline), // WITH...UNWIND...SET/DELETE
    Return(ReturnStatement),          // Standalone RETURN
    Unwind(UnwindStatement),          // Standalone UNWIND
    Let(LetStatement),                // Standalone LET
    For(ForStatement),                // FOR iteration
    Filter(FilterStatement),          // Standalone FILTER
}
```

#### 4. BasicQuery Structure
```rust
pub struct BasicQuery {
    pub match_clauses: Vec<MatchClause>,     // MATCH patterns
    pub where_clause: Option<WhereClause>,   // WHERE filter
    pub group_by: Option<GroupByClause>,     // GROUP BY
    pub having: Option<HavingClause>,        // HAVING filter
    pub return_clause: ReturnClause,         // RETURN projection
}
```

#### 5. Path Patterns
```rust
pub struct PathPattern {
    pub assignment: Option<String>,         // p = (...)
    pub path_type: Option<PathType>,        // WALK/TRAIL/SIMPLE/ACYCLIC
    pub elements: Vec<PatternElement>,      // Pattern sequence
}

pub enum PatternElement {
    Node(Node),
    Edge(Edge),
}
```

#### 6. Node Pattern
```rust
pub struct Node {
    pub identifier: Option<String>,         // Variable name
    pub labels: Vec<String>,                // :Person:Employee
    pub properties: Option<PropertyMap>,    // {name: "John", age: 30}
}
```

#### 7. Edge Pattern
```rust
pub struct Edge {
    pub identifier: Option<String>,         // Variable name
    pub labels: Vec<String>,                // :KNOWS:FRIEND
    pub properties: Option<PropertyMap>,    // {since: 2020}
    pub direction: EdgeDirection,           // Outgoing/Incoming/Both/Undirected
    pub quantifier: Option<PathQuantifier>, // {2,5} for path length
}

pub enum EdgeDirection {
    Outgoing,    // -[]->;
    Incoming,    // <-[]-;
    Both,        // <-[]->;
    Undirected,  // -[]-;
}

pub struct PathQuantifier {
    pub min: Option<usize>,  // Minimum path length
    pub max: Option<usize>,  // Maximum path length
}
// Examples: {2,5}, {,3}, {2,}, {3}, ?, +, *
```

#### 8. Expression Types (18 variants)
```rust
pub enum Expression {
    Binary(BinaryExpression),        // a + b, a AND b
    Unary(UnaryExpression),          // NOT a, -b
    FunctionCall(FunctionCall),      // count(*), sum(x)
    PropertyAccess(PropertyAccess),  // n.name
    Variable(Variable),              // $param
    Literal(Literal),                // "string", 42, 3.14, true
    Case(CaseExpression),            // CASE WHEN...THEN...END
    PathConstructor(PathConstructor), // PATH (n)-[e]->(m)
    Cast(CastExpression),            // CAST(x AS INTEGER)
    Subquery(SubqueryExpression),    // (SELECT...)
    ExistsSubquery(Box<Query>),      // EXISTS { MATCH... }
    NotExistsSubquery(Box<Query>),   // NOT EXISTS { MATCH... }
    InSubquery(Box<Expression>, Box<Query>), // x IN (SELECT...)
    NotInSubquery(Box<Expression>, Box<Query>),
    QuantifiedComparison(QuantifiedComparison), // x = ALL(...)
    IsPredicate(IsPredicate),        // IS NULL, IS :Label
    PatternExpression(PathPattern),  // Pattern in expression context
    ArrayIndex(ArrayIndexExpression), // list[0]
}
```

#### 9. Data Modification Statements
```rust
pub enum DataStatement {
    Insert(InsertStatement),      // INSERT (n:Person {name: "John"})
    Delete(DeleteStatement),      // DELETE n
    Update(UpdateStatement),      // SET n.age = 30
    Remove(RemoveStatement),      // REMOVE n:Label, n.property
    MatchInsert(MatchInsert),     // MATCH...INSERT
    MatchDelete(MatchDelete),     // MATCH...DELETE
    MatchUpdate(MatchUpdate),     // MATCH...SET
    MatchRemove(MatchRemove),     // MATCH...REMOVE
}
```

#### 10. Catalog Statements (DDL)
```rust
pub enum CatalogStatement {
    CreateGraph(CreateGraph),           // CREATE GRAPH myGraph
    DropGraph(DropGraph),               // DROP GRAPH myGraph
    CreateGraphType(CreateGraphType),   // CREATE GRAPH TYPE schema
    DropGraphType(DropGraphType),       // DROP GRAPH TYPE schema
    CreateSchema(CreateSchema),         // CREATE SCHEMA mySchema
    DropSchema(DropSchema),             // DROP SCHEMA mySchema
    CreateUser(CreateUser),             // CREATE USER admin PASSWORD 'pass'
    DropUser(DropUser),                 // DROP USER admin
    CreateRole(CreateRole),             // CREATE ROLE reader
    DropRole(DropRole),                 // DROP ROLE reader
    Grant(GrantStatement),              // GRANT SELECT ON GRAPH g TO user
    Revoke(RevokeStatement),            // REVOKE SELECT ON GRAPH g FROM user
}
```

#### 11. Type System
```rust
pub enum TypeSpec {
    // Primitive types
    Boolean,
    String { max_length: Option<usize> },
    Integer,
    BigInt,
    SmallInt,
    Float { precision: Option<u8> },
    Real,
    Double,

    // Temporal types
    Date,
    Time { with_timezone: bool },
    Timestamp { with_timezone: bool },

    // Special types
    Vector { dimensions: Option<usize> },  // For embeddings
    Reference,
    Path,
    List(Box<TypeSpec>),
    Record(Vec<(String, TypeSpec)>),
    Graph,
    BindingTable,
}
```

**Dependencies**: None (foundation module)

**Integration**: Used by ALL other modules. Parser produces AST, validator validates it, planner converts it to logical plans, executor interprets it.

**Critical Implementation Details**:
- **Serde Support**: All structures are serializable for caching/transmission
- **Location Tracking**: Every node has source location for error messages
- **ISO GQL Compliance**: Full implementation of ISO GQL standard
- **Property Maps**: Support both inline properties `{name: "John"}` and property access patterns
- **Path Quantifiers**: Support complex path repetition patterns

---

### ast/lexer.rs

**Location**: [graphlite/src/ast/lexer.rs](graphlite/src/ast/lexer.rs)
**Lines**: 1867
**Purpose**: Tokenize GQL query strings into tokens for parsing

**Key Features**:

#### 1. Infinite Loop Prevention System
Extensive safety measures to prevent lexer from infinite looping:
```rust
// Iteration counting with 1000-iteration limit
// Input position tracking to validate consumption
// Whitespace validation to ensure progress
```

Documentation includes detailed analysis of potential infinite loop scenarios and prevention strategies.

#### 2. Token Types (90+ tokens)

**Keywords**:
```
MATCH, WHERE, RETURN, SELECT, INSERT, DELETE, CREATE, DROP,
SET, REMOVE, WITH, UNWIND, UNION, INTERSECT, EXCEPT,
ORDER BY, LIMIT, OFFSET, GROUP BY, HAVING,
BEGIN, COMMIT, ROLLBACK, GRAPH, SCHEMA, USER, ROLE,
GRANT, REVOKE, CALL, YIELD, FILTER, LET, FOR, NEXT,
PATH, WALK, TRAIL, SIMPLE, ACYCLIC, DETACH,
IS, NULL, TRUE, FALSE, NOT, AND, OR, XOR,
EXISTS, IN, ALL, ANY, SOME, CASE, WHEN, THEN, ELSE, END,
CAST, AS, DISTINCT, COUNT, SUM, AVG, MIN, MAX, ...
```

**Operators**:
```
+, -, *, /, %, ^                    // Arithmetic
=, !=, <, <=, >, >=                 // Comparison
=~, ~=                              // Pattern matching
||                                  // String concatenation
AND, OR, XOR, NOT                   // Logical
```

**Delimiters**:
```
( ) [ ] { }                         // Grouping
, ; : .                             // Separators
-> <- <->                           // Edge directions
```

**Literals**:
```
"string", 'string'                  // Strings with escapes
42, -17                             // Integers
3.14, -2.5e10                       // Floats
true, false, null                   // Booleans and null
[1.0, 2.0, 3.0]                     // Vectors (for embeddings)
```

**Identifiers**:
```
variable_name                       // Regular identifiers
$parameter                          // Parameters
`My-Identifier`                     // Backtick-delimited (ISO GQL)
object.property                     // Property access tokens
```

#### 3. Complex Pattern Handling

**String Literals**:
- Both `"` and `'` delimiters supported
- Escape sequences: `\n`, `\t`, `\\`, `\"`, `\'`
- Multi-line strings supported

**Backtick Identifiers** (ISO GQL):
```sql
`My Table-Name`  -- ISO GQL delimited identifier
`Column ``With`` Backtick`  -- `` escapes backtick
```

**Vector Literals**:
```sql
[1.0, 2.0, 3.0]  -- Vector for embeddings
```

**Property Access**:
```sql
person.name      -- Tokenized as single PropertyAccess token
graph.nodes      -- Not split into graph, ., nodes
```

#### 4. Critical Functions

**Main Tokenization**:
```rust
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexerError>
```
- Main tokenization loop with infinite loop protection
- Iteration counting (max 1000 iterations)
- Position validation to ensure forward progress
- Returns complete token stream or error

**Whitespace Handling**:
```rust
fn whitespace(input: &str) -> IResult<&str, ()>
```
- Validates whitespace consumption to prevent loops
- Must consume at least 1 character or fail
- Critical for infinite loop prevention

**Pattern Matching**:
```rust
fn simple_patterns(input: &str) -> IResult<&str, Token>
```
- Multi-character operators matched BEFORE single-character
- Strict ordering: `!=` before `!`, `<=` before `<`, etc.
- Prevents incorrect tokenization

**SQL Comment Filtering**:
```rust
// Handled by parser, not lexer
-- This is a comment
```

**Dependencies**:
- `nom` parser combinators for complex patterns
- No external module dependencies

**Integration**: First stage of query processing pipeline:
```
Raw Query String → Lexer → Token Stream → Parser → AST
```

**Critical Implementation Details**:

1. **Keyword Matching**:
   - Case-insensitive comparison
   - Word boundary checks (prevents `MATCHING` from matching `MATCH`)
   - Longest match wins

2. **Property Access Optimization**:
   - `object.property` tokenized as single token
   - Prevents complex parser lookahead
   - Performance optimization

3. **Vector Literal Recognition**:
   - Identified at lexer level for performance
   - Enables fast path in parser for embedding data

4. **Function Call Handling**:
   - Removed from lexer (previously had special `FunctionCall` token)
   - Now handled by parser for ISO GQL compliance
   - Allows for user-defined functions with same syntax as built-ins

---

### ast/parser.rs

**Location**: [graphlite/src/ast/parser.rs](graphlite/src/ast/parser.rs)
**Lines**: 4000+
**Purpose**: Parse token streams into AST using recursive descent parsing

**Key Features**:

#### 1. Entry Point
```rust
pub fn parse_query(input: &str) -> Result<Document, ParserError>
```

**Process**:
1. Tokenize input string using lexer
2. Filter SQL-style comments (`-- comment`)
3. Validate and detect invalid patterns (e.g., `DELETE SCHEMA`, `DELETE GRAPH`)
4. Attempt parsing as different statement types
5. Return fully validated AST or detailed error

#### 2. Query Parsing Strategy

**Precedence-Based Set Operations**:
```
INTERSECT binds tighter than UNION/EXCEPT

Example:
A UNION B INTERSECT C  →  A UNION (B INTERSECT C)

Parse hierarchy:
query()
  └─> parse_set_operation()
       ├─> parse_union_except()  (lower precedence)
       │    └─> parse_intersect()  (higher precedence)
       │         └─> parse_query_term()  (base queries)
       └─> apply_query_modifiers()  (ORDER BY/LIMIT)
```

#### 3. Parser Hierarchy

**Complete Parsing Stack**:
```
query()
  └─> parse_set_operation()
       └─> parse_core_query()
            ├─> parse_union_except()
            │    └─> parse_intersect()
            │         └─> parse_query_term()
            │              ├─> basic_query()         // MATCH...RETURN
            │              ├─> with_query()          // WITH pipelines
            │              ├─> mutation_pipeline()   // WITH...UNWIND...SET/DELETE
            │              ├─> return_query()        // Standalone RETURN
            │              ├─> unwind_query()        // Standalone UNWIND
            │              ├─> let_query()           // LET statements
            │              ├─> for_query()           // FOR iteration
            │              └─> filter_query()        // Standalone FILTER
            └─> apply_query_modifiers()  // ORDER BY/LIMIT/OFFSET
```

#### 4. Pattern Parsing

**Path Patterns**:
```rust
fn path_pattern(tokens: &[Token], pos: usize)
    -> Result<(PathPattern, usize), ParserError>
```

Parses:
```sql
-- Simple pattern
(n:Person)-[e:KNOWS]->(m:Person)

-- With path assignment and type
p = SIMPLE PATH (n)-[e*1..3]->(m)

-- With quantifiers
(n)-[e:KNOWS*2..5]->(m)

-- Complex pattern
ACYCLIC PATH (start)-[:PART_OF*]->(end)
```

**Node Patterns**:
```rust
fn node_pattern(tokens: &[Token], pos: usize)
    -> Result<(Node, usize), ParserError>
```

Parses:
```sql
(n)                          -- Variable only
(:Person)                    -- Label only
(n:Person)                   -- Variable and label
(n:Person:Employee)          -- Multiple labels
(n {name: "John"})           -- With properties
(n:Person {age: 30})         -- Label and properties
```

**Edge Patterns**:
```rust
fn edge_pattern(tokens: &[Token], pos: usize)
    -> Result<(Edge, usize), ParserError>
```

Parses:
```sql
-[e]->                       -- Outgoing
<-[e]-                       -- Incoming
<-[e]->                      -- Both directions
-[e]-                        -- Undirected
-[:KNOWS]->                  -- With label
-[e:KNOWS {since: 2020}]->   -- With properties
-[e*2..5]->                  -- With quantifier
-[e:KNOWS*]->                -- Variable length
```

**Path Quantifiers**:
```rust
fn path_quantifier(tokens: &[Token], pos: usize)
    -> Result<(PathQuantifier, usize), ParserError>
```

Parses:
```sql
{2,5}    -- Min 2, max 5 hops
{2,}     -- Min 2, no max
{,5}     -- No min, max 5
{3}      -- Exactly 3 hops
?        -- {0,1} - optional
+        -- {1,} - one or more
*        -- {0,} - zero or more
```

#### 5. Expression Parsing

**Operator Precedence** (lowest to highest):
```
1. OR
2. XOR
3. AND
4. NOT
5. Comparison (=, !=, <, <=, >, >=, =~, ~=)
6. Additive (+, -, ||)
7. Multiplicative (*, /, %)
8. Unary (-, NOT)
9. Postfix (function calls, property access, array index)
```

**Special Expression Handling**:

**IN/NOT IN with Subqueries**:
```sql
-- List form
x IN (1, 2, 3)

-- Subquery form
x IN (SELECT id FROM nodes WHERE ...)
```

**IS Predicates**:
```sql
x IS NULL
x IS NOT NULL
x IS :Person              -- Label predicate
x IS NOT TYPED
```

**Shorthand Label Predicates**:
```sql
-- Parser automatically converts:
n:Person
  ↓
n IS :Person
```

**Quantified Comparisons**:
```sql
value = ALL (subquery)
value > ANY (subquery)
value <= SOME (subquery)
```

#### 6. Complex Constructs

**WITH Queries** (Pipelines):
```sql
WITH n.name AS name, count(*) AS cnt
MATCH (n:Person)
WHERE n.age > 25
WITH name, cnt
WHERE cnt > 10
RETURN name, cnt
```

**Mutation Pipelines**:
```sql
MATCH (n:Person)
WITH n
UNWIND [1, 2, 3] AS x
SET n.score = x
DELETE n.temp
```

**SELECT Statements**:
```sql
SELECT * FROM myGraph
MATCH (n:Person)
WHERE n.age > 30
RETURN n
```

**CALL Statements**:
```sql
CALL db.labels() YIELD label
WHERE label STARTS WITH 'P'
RETURN label
```

#### 7. Error Handling

**Validation Checks**:
- Incomplete set operations (e.g., `UNION` without right-hand side)
- Invalid patterns (e.g., `DELETE SCHEMA`, `DELETE GRAPH`)
- GROUP BY preservation in BasicQuery
- Proper nesting of subqueries

**Error Messages**:
```rust
pub enum ParserError {
    UnexpectedToken { expected: String, found: Token, position: usize },
    UnexpectedEndOfInput { expected: String },
    InvalidPattern { message: String, position: usize },
    IncompleteSetOperation { operation: String, position: usize },
    // ... more error types
}
```

**Dependencies**:
- `nom` for parser combinators
- `ast::*` for AST node construction
- `lexer::Token` for token types

**Integration**: Second stage of query processing:
```
Token Stream → Parser → AST → Validator → Planner → Executor
```

**Critical Implementation Details**:

1. **Comment Filtering**:
   - SQL comments (`-- comment`) filtered at token level
   - Not visible to parser logic

2. **GROUP BY Preservation**:
   - GROUP BY correctly preserved in BasicQuery structure
   - Maintains semantic correctness for aggregation queries

3. **FROM MATCH Syntax**:
   - Non-standard extension for explicit graph selection
   - `SELECT * FROM myGraph MATCH (n) RETURN n`

4. **Parenthesized Queries**:
   - Support for nested set operations
   - `(A UNION B) INTERSECT C`

---

### ast/pretty_printer.rs

**Location**: [graphlite/src/ast/pretty_printer.rs](graphlite/src/ast/pretty_printer.rs)
**Lines**: 810
**Purpose**: Debug visualization of AST trees using structured logging

**Key Functions**:

```rust
pub fn pretty_print_ast(document: &Document)
```

**Features**:
- Recursive printing with indentation
- Detailed output for all AST node types
- Structured log format using `log::debug!`
- Handles nested structures (subqueries, WITH clauses, etc.)

**Output Example**:
```
Document
  Statement: Query
    BasicQuery
      MATCH Clauses:
        PathPattern:
          Node(n): labels=[Person], properties={age: 30}
          Edge(e): labels=[KNOWS], direction=Outgoing
          Node(m): labels=[Person]
      WHERE Clause:
        BinaryExpression: n.age > 25
      RETURN Clause:
        ReturnItem: n.name AS name
        ReturnItem: m.name AS friend
```

**Integration**:
- Used during parsing for debugging
- Can be enabled with log level `RUST_LOG=debug`
- Not used in production execution

**Usage**:
```rust
use graphlite::ast::parser::parse_query;
use graphlite::ast::pretty_printer::pretty_print_ast;

let query = "MATCH (n:Person)-[e:KNOWS]->(m) RETURN n, m";
let ast = parse_query(query)?;
pretty_print_ast(&ast);  // Logs detailed AST structure
```

---

### ast/validator.rs

**Location**: [graphlite/src/ast/validator.rs](graphlite/src/ast/validator.rs)
**Purpose**: Semantic validation of parsed AST

**Expected Functionality** (module to be read for full details):
- Validate variable references (all variables used are defined)
- Check type compatibility in expressions
- Verify graph context requirements (queries that need graphs)
- Validate aggregation usage (no aggregation in WHERE, etc.)
- Check label/property existence against schema
- Validate function calls and argument types
- Ensure proper nesting (no subqueries in inappropriate contexts)

**Integration**: Third stage of query processing:
```
AST → Validator → Validated AST → Planner → Executor
```

---

## Cache Module

**Directory**: [graphlite/src/cache/](graphlite/src/cache/)
**Purpose**: Comprehensive multi-level caching system for query optimization

The cache module provides sophisticated caching at multiple levels to dramatically improve query performance by avoiding redundant compilation and execution.

### cache/mod.rs

**Location**: [graphlite/src/cache/mod.rs](graphlite/src/cache/mod.rs)
**Lines**: 110
**Purpose**: Cache module organization and common types

**Key Structures**:

#### 1. Cache Levels
```rust
pub enum CacheLevel {
    L1,  // Hot data - in-memory, small (~10MB), fast (<1ms)
    L2,  // Warm data - in-memory, larger (~100MB), moderate (<10ms)
    L3,  // Cold data - disk-backed, large (~1GB+), slow (~100ms)
}
```

**Usage Pattern**:
- **L1**: Frequently executed queries, plan cache hot entries
- **L2**: Less frequent queries, result cache
- **L3**: Historical queries, archived plans

#### 2. Cache Entry Metadata
```rust
pub struct CacheEntryMetadata {
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u32,
    pub size_bytes: usize,
    pub ttl: Option<Duration>,       // Time-to-live
    pub level: CacheLevel,
    pub tags: Vec<String>,           // For targeted invalidation
}
```

**Metadata Usage**:
- **LRU eviction**: Based on `last_accessed` and `access_count`
- **Memory management**: Based on `size_bytes`
- **TTL expiration**: Based on `created_at` and `ttl`
- **Invalidation**: Based on `tags` (e.g., ["graph:myGraph", "schema:v2"])

#### 3. Traits

**CacheKey Trait**:
```rust
pub trait CacheKey {
    fn cache_key(&self) -> String;
    fn cache_tags(&self) -> Vec<String>;
}
```

**CacheValue Trait**:
```rust
pub trait CacheValue {
    fn size_bytes(&self) -> usize;
    fn is_valid(&self) -> bool;
}
```

**Exports**:
```rust
pub use cache_config::{CacheConfig, EvictionPolicy};
pub use cache_manager::CacheManager;
pub use invalidation::{InvalidationEvent, InvalidationManager};
pub use plan_cache::{PlanCache, PlanCacheEntry, PlanCacheKey};
pub use result_cache::ResultCache;
pub use subquery_cache::SubqueryCache;
```

---

### cache/plan_cache.rs

**Location**: [graphlite/src/cache/plan_cache.rs](graphlite/src/cache/plan_cache.rs)
**Lines**: 380
**Purpose**: Cache compiled query plans to avoid recompilation

**Why Plan Caching Matters**:
- Query planning can take 10-100ms for complex queries
- Plans are deterministic for same query + schema
- Caching can reduce query latency by 90%+

**Key Structures**:

#### 1. Plan Cache Key
```rust
pub struct PlanCacheKey {
    pub query_structure_hash: u64,   // Normalized query structure
    pub schema_hash: u64,            // Schema dependency
    pub optimization_level: String,  // "O0", "O1", "O2", "O3"
    pub hints: Vec<String>,          // Query hints
}
```

**Key Generation**:
```rust
impl PlanCacheKey {
    pub fn from_query(query: &str, schema_version: u64) -> Self {
        // Normalize query (remove whitespace, case-fold keywords)
        let normalized = normalize_query(query);

        // Hash normalized query
        let query_hash = hash(&normalized);

        Self {
            query_structure_hash: query_hash,
            schema_hash: schema_version,
            optimization_level: "O2".to_string(),
            hints: vec![],
        }
    }
}
```

#### 2. Plan Cache Entry
```rust
pub struct PlanCacheEntry {
    pub logical_plan: LogicalPlan,
    pub physical_plan: PhysicalPlan,
    pub trace: Option<PlanTrace>,        // Optimization trace
    pub compilation_time: Duration,
    pub estimated_cost: f64,
    pub estimated_rows: usize,
    pub metadata: CacheEntryMetadata,
    pub usage_count: u64,
    pub last_used: Instant,
}
```

#### 3. Plan Cache
```rust
pub struct PlanCache {
    entries: Arc<RwLock<HashMap<String, PlanCacheEntry>>>,
    config: CacheConfig,
    stats: Arc<RwLock<CacheStats>>,
}
```

**Critical Methods**:

**Get Cached Plan**:
```rust
pub fn get(&self, key: &PlanCacheKey) -> Option<PlanCacheEntry> {
    let mut entries = self.entries.write().unwrap();

    if let Some(mut entry) = entries.get_mut(&key.cache_key()) {
        // Update access statistics
        entry.last_used = Instant::now();
        entry.usage_count += 1;
        entry.metadata.last_accessed = Instant::now();
        entry.metadata.access_count += 1;

        // Update stats
        self.stats.write().unwrap().hits += 1;

        Some(entry.clone())
    } else {
        self.stats.write().unwrap().misses += 1;
        None
    }
}
```

**Insert Plan**:
```rust
pub fn insert(
    &self,
    key: PlanCacheKey,
    logical_plan: LogicalPlan,
    physical_plan: PhysicalPlan,
    compilation_time: Duration,
) -> Result<(), CacheError> {
    let entry = PlanCacheEntry {
        logical_plan,
        physical_plan,
        compilation_time,
        // ... initialize fields
    };

    let entry_size = entry.size_bytes();

    // Evict if needed to make space
    self.evict_if_needed(entry_size)?;

    // Insert entry
    self.entries.write().unwrap().insert(key.cache_key(), entry);

    Ok(())
}
```

**LRU Eviction**:
```rust
fn evict_if_needed(&self, required_bytes: usize) -> Result<(), CacheError> {
    let mut entries = self.entries.write().unwrap();

    while self.current_size_bytes() + required_bytes > self.config.max_memory_bytes {
        // Find LRU entry (considering both time and frequency)
        let lru_key = entries.iter()
            .min_by_key(|(_, entry)| {
                // Score = access_count / age_seconds
                // Lower score = more likely to evict
                let age = entry.last_used.elapsed().as_secs() + 1;
                entry.usage_count / age
            })
            .map(|(k, _)| k.clone());

        if let Some(key) = lru_key {
            entries.remove(&key);
            self.stats.write().unwrap().evictions += 1;
        } else {
            return Err(CacheError::EvictionFailed);
        }
    }

    Ok(())
}
```

**Schema-Based Invalidation**:
```rust
pub fn invalidate_by_schema(&self, schema_hash: u64) {
    let mut entries = self.entries.write().unwrap();

    // Remove all entries with matching schema hash
    entries.retain(|key, _| {
        !key.contains(&format!("schema:{}", schema_hash))
    });
}
```

**Cache Statistics**:
```rust
pub fn stats(&self) -> CacheStats {
    self.stats.read().unwrap().clone()
}

pub fn efficiency_metrics(&self) -> EfficiencyMetrics {
    let stats = self.stats();

    EfficiencyMetrics {
        hit_rate: stats.hits as f64 / (stats.hits + stats.misses) as f64,
        memory_usage_mb: self.current_size_bytes() as f64 / 1_048_576.0,
        avg_compilation_time_saved_ms: stats.total_compilation_time_saved_ms / stats.hits,
        eviction_rate: stats.evictions as f64 / stats.inserts as f64,
    }
}
```

**Integration**: Used by query executor to skip planning:
```
Query → Check Plan Cache
         ├─> HIT:  Use cached plan → Execute
         └─> MISS: Plan → Cache plan → Execute
```

**Critical Implementation Details**:

1. **Thread Safety**: All access protected by `Arc<RwLock<>>`
2. **LRU Algorithm**: Combines recency AND frequency for smart eviction
3. **Memory Management**: Hard limit on memory usage with automatic eviction
4. **Schema Versioning**: Automatic invalidation on schema changes
5. **Statistics**: Rich metrics for cache tuning

---

### cache/result_cache.rs

**Location**: [graphlite/src/cache/result_cache.rs](graphlite/src/cache/result_cache.rs)
**Purpose**: Cache query results for identical queries

**Key Features** (to be read for full details):
- Cache complete query results
- TTL-based expiration for freshness
- Invalidation on data modification
- Memory-bounded with LRU eviction

**Usage Pattern**:
```
Query → Check Result Cache
         ├─> HIT:  Return cached results (if fresh)
         └─> MISS: Execute → Cache results → Return
```

---

### cache/subquery_cache.rs

**Location**: [graphlite/src/cache/subquery_cache.rs](graphlite/src/cache/subquery_cache.rs)
**Purpose**: Cache subquery results within a query execution

**Key Features** (to be read for full details):
- Per-query-execution cache
- Cache repeated subqueries (e.g., in EXISTS, IN)
- Automatic cleanup after query completes
- No persistence across queries

**Usage Pattern**:
```sql
-- This query executes the same subquery 1000 times without caching
MATCH (n:Person)
WHERE n.id IN (SELECT id FROM popular WHERE score > 100)
RETURN n

-- Subquery cache ensures the subquery runs only ONCE
```

---

### cache/cache_config.rs

**Location**: [graphlite/src/cache/cache_config.rs](graphlite/src/cache/cache_config.rs)
**Purpose**: Configuration for cache behavior

**Key Structures** (to be read for full details):
```rust
pub struct CacheConfig {
    pub max_memory_bytes: usize,
    pub eviction_policy: EvictionPolicy,
    pub ttl: Option<Duration>,
}

pub enum EvictionPolicy {
    LRU,      // Least Recently Used
    LFU,      // Least Frequently Used
    LRFU,     // Combined LRU+LFU
}
```

---

### cache/cache_manager.rs

**Location**: [graphlite/src/cache/cache_manager.rs](graphlite/src/cache/cache_manager.rs)
**Purpose**: Central coordinator for all caches

**Key Responsibilities** (to be read for full details):
- Manage plan cache, result cache, subquery cache
- Coordinate invalidation across caches
- Aggregate statistics
- Memory budget management

---

### cache/invalidation.rs

**Location**: [graphlite/src/cache/invalidation.rs](graphlite/src/cache/invalidation.rs)
**Purpose**: Cache invalidation logic and event handling

**Key Features** (to be read for full details):
- Event-based invalidation system
- Tag-based invalidation (invalidate all entries with tag)
- Selective vs. bulk invalidation
- Integration with transaction system

**Invalidation Events**:
```rust
pub enum InvalidationEvent {
    DataModification { graph: String, labels: Vec<String> },
    SchemaChange { schema_hash: u64 },
    GraphDrop { graph: String },
    TransactionRollback { transaction_id: TransactionId },
}
```

---

## Coordinator Module

**Directory**: [graphlite/src/coordinator/](graphlite/src/coordinator/)
**Purpose**: Central orchestration for query execution - THE primary entry point

### coordinator/mod.rs

**Location**: [graphlite/src/coordinator/mod.rs](graphlite/src/coordinator/mod.rs)
**Lines**: 15
**Purpose**: Module declaration for coordinator

**Exports**:
```rust
pub use query_coordinator::{QueryCoordinator, QueryInfo, QueryPlan, QueryType};
// Re-exports from exec module for convenience
pub use crate::exec::{QueryResult, Row};
```

---

### coordinator/query_coordinator.rs

**Location**: [graphlite/src/coordinator/query_coordinator.rs](graphlite/src/coordinator/query_coordinator.rs)
**Lines**: 817
**Purpose**: THE primary entry point for all GraphLite operations

**Critical Importance**: This is the ONLY public interface to GraphLite. All operations flow through this coordinator.

**Key Structure**:

```rust
pub struct QueryCoordinator {
    session_manager: Arc<SessionManager>,
    executor: Arc<QueryExecutor>,
}
```

**Architecture**:
```
QueryCoordinator
    ├─> SessionManager (manages sessions, authentication)
    └─> QueryExecutor (executes queries)
         ├─> StorageManager (manages storage)
         ├─> CatalogManager (manages metadata)
         ├─> TransactionManager (manages transactions)
         ├─> CacheManager (manages caches)
         └─> FunctionRegistry (manages functions)
```

**Critical Methods**:

#### 1. Initialization
```rust
pub fn from_path<P: AsRef<Path>>(db_path: P) -> Result<Arc<Self>, String>
```

**Process**:
1. Initialize storage system (Sled/RocksDB)
2. Initialize catalog manager (metadata, schemas, users)
3. Initialize transaction manager (undo logs, WAL)
4. Initialize cache manager (plan cache, result cache)
5. Initialize function registry (built-in functions)
6. Initialize query executor (wires up all components)
7. Initialize session manager (session tracking)
8. Set global session manager for access from FFI
9. Return Arc-wrapped coordinator

**Full Initialization Flow**:
```rust
// graphlite/src/coordinator/query_coordinator.rs:150-250
pub fn from_path<P: AsRef<Path>>(db_path: P) -> Result<Arc<Self>, String> {
    // 1. Storage
    let storage = Arc::new(StorageManager::new(
        &db_path,
        StorageMethod::DiskOnly,
        StorageType::Sled,
    )?);

    // 2. Catalog
    let catalog_manager = Arc::new(RwLock::new(
        CatalogManager::new(storage.clone())?
    ));

    // 3. Transactions
    let transaction_manager = Arc::new(TransactionManager::new(storage.clone()));

    // 4. Cache
    let cache_manager = Arc::new(CacheManager::new(CacheConfig::default()));

    // 5. Functions
    let function_registry = Arc::new(FunctionRegistry::new());

    // 6. Executor
    let executor = Arc::new(QueryExecutor::new(
        storage,
        function_registry,
        catalog_manager.clone(),
        transaction_manager,
        cache_manager,
    )?);

    // 7. Sessions
    let session_manager = Arc::new(SessionManager::new(catalog_manager));

    // 8. Global session manager for FFI
    set_global_session_manager(session_manager.clone());

    // 9. Coordinator
    Ok(Arc::new(Self {
        session_manager,
        executor,
    }))
}
```

#### 2. Query Execution
```rust
pub fn process_query(
    &self,
    query_text: &str,
    session_id: &str,
) -> Result<QueryResult, String>
```

**Process**:
1. Parse query into AST
2. Get or validate session
3. Create execution request with session context
4. Execute via executor
5. Handle session-affecting results (SET GRAPH/SCHEMA)
6. Return results

**Full Execution Flow**:
```rust
// graphlite/src/coordinator/query_coordinator.rs:350-450
pub fn process_query(
    &self,
    query_text: &str,
    session_id: &str,
) -> Result<QueryResult, String> {
    // 1. Parse
    let document = parse_query(query_text)
        .map_err(|e| format!("Parse error: {}", e))?;

    // 2. Get session
    let session_lock = self.session_manager
        .get_session(session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // 3. Create execution request
    let request = ExecutionRequest::new(document.statement)
        .with_session(Some(session_lock.clone()))
        .with_query_text(Some(query_text.to_string()));

    // 4. Execute
    let mut result = self.executor.execute_query(request)
        .map_err(|e| format!("Execution error: {}", e))?;

    // 5. Handle session results
    if let Some(session_result) = &result.session_result {
        match session_result {
            SessionResult::GraphSet(graph_name) => {
                session_lock.write().unwrap().current_graph = Some(graph_name.clone());
            }
            SessionResult::SchemaSet(schema_name) => {
                session_lock.write().unwrap().current_schema = Some(schema_name.clone());
            }
        }
    }

    // 6. Return
    Ok(result)
}
```

#### 3. Session Management

**Create Simple Session** (default permissions):
```rust
pub fn create_simple_session(&self, username: &str) -> Result<String, String>
```

**Create Session** (custom permissions):
```rust
pub fn create_session(
    &self,
    username: &str,
    roles: Vec<String>,
    permissions: Vec<Permission>,
) -> Result<String, String>
```

**Authenticate and Create Session**:
```rust
pub fn authenticate_and_create_session(
    &self,
    username: &str,
    password: &str,
) -> Result<String, String>
```

**Password Management**:
```rust
pub fn set_user_password(
    &self,
    username: &str,
    password: &str,
) -> Result<(), String>
```

**Close Session**:
```rust
pub fn close_session(&self, session_id: &str) -> Result<(), String>
```

#### 4. Query Analysis

**Validate Query**:
```rust
pub fn validate_query(&self, query: &str) -> Result<(), String>
```
- Parses query
- Validates syntax and semantics
- Returns errors without executing

**Check Query Validity**:
```rust
pub fn is_valid_query(&self, query: &str) -> bool
```
- Boolean check for validity
- Used for auto-complete and validation UI

**Analyze Query**:
```rust
pub fn analyze_query(&self, query: &str) -> Result<QueryInfo, String>
```
- Returns query metadata without executing
- Information includes:
  - Query type (SELECT, INSERT, DDL, etc.)
  - Variables used
  - Tables/graphs accessed
  - Estimated complexity

**Explain Query**:
```rust
pub fn explain_query(&self, query: &str) -> Result<QueryPlan, String>
```
- Returns execution plan without executing
- Shows:
  - Logical plan
  - Physical plan
  - Estimated cost
  - Estimated rows
  - Index usage

**Integration**: This is the ONLY public API:
```
External Users
    ↓
QueryCoordinator (ONLY PUBLIC INTERFACE)
    ├─> SessionManager
    └─> QueryExecutor
         ├─> Planner
         ├─> Storage
         ├─> Catalog
         ├─> Transactions
         ├─> Cache
         └─> Functions
```

**Critical Implementation Details**:

1. **Thread Safety**:
   - `UnwindSafe` and `RefUnwindSafe` for FFI panic safety
   - All state protected by `Arc<RwLock<>>`

2. **Graph Path Resolution**:
   - Handles both relative and absolute paths
   - Normalizes paths for consistency

3. **Session Result Processing**:
   - SET GRAPH updates session's current graph
   - SET SCHEMA updates session's current schema
   - Changes persist for session lifetime

4. **Error Handling**:
   - Converts all internal errors to user-friendly strings
   - Provides detailed error context

---

## Exec Module

**Directory**: [graphlite/src/exec/](graphlite/src/exec/)
**Purpose**: Query execution engine - interprets plans and executes operations

The exec module is the heart of GraphLite's execution system. It takes validated ASTs and execution plans and converts them into actual data operations.

### exec/mod.rs

**Location**: [graphlite/src/exec/mod.rs](graphlite/src/exec/mod.rs)
**Purpose**: Module declaration for executor

**Exports**:
```rust
pub use executor::{QueryExecutor, ExecutionRequest};
pub use result::{QueryResult, Row};
pub use context::ExecutionContext;
pub use error::ExecutionError;
```

---

### exec/executor.rs

**Location**: [graphlite/src/exec/executor.rs](graphlite/src/exec/executor.rs)
**Lines**: 7000+
**Purpose**: Main query execution engine

**Key Structure**:

```rust
pub struct QueryExecutor {
    // Core execution components
    storage: Arc<StorageManager>,
    function_registry: Arc<FunctionRegistry>,
    catalog_manager: Arc<RwLock<CatalogManager>>,
    system_procedures: SystemProcedures,

    // Transaction management
    transaction_manager: Arc<TransactionManager>,
    current_transaction: Arc<RwLock<Option<TransactionId>>>,
    transaction_logs: Arc<RwLock<HashMap<TransactionId, TransactionLog>>>,

    // Type system components
    type_inference: TypeInference,
    type_validator: TypeValidator,
    type_coercion: TypeCoercion,
    type_caster: TypeCaster,
}
```

**Execution Request**:

```rust
#[derive(Clone)]
pub struct ExecutionRequest {
    pub statement: Statement,                           // AST to execute
    pub session: Option<Arc<RwLock<UserSession>>>,      // Session context
    pub graph_expr: Option<GraphExpression>,            // Graph to use
    pub query_text: Option<String>,                     // Original query
    pub physical_plan: Option<PhysicalPlan>,            // Pre-computed plan
    pub requires_graph_context: Option<bool>,           // Validation flag
}
```

**Critical Methods**:

#### 1. Unified Execution Entry Point
```rust
pub fn execute_query(&self, request: ExecutionRequest) -> Result<QueryResult, ExecutionError>
```

Located at [graphlite/src/exec/executor.rs:144](graphlite/src/exec/executor.rs#L144)

**Process**:
```rust
// PHASE 1: Check for UNWIND preprocessing
if is_unwind_query(query_text) {
    return execute_unwind_query(query_text);
}

// PHASE 2: Resolve graph context
let needs_graph = request.requires_graph_context
    .unwrap_or_else(|| self.statement_needs_graph_context(&request.statement));

let resolved_graph = if needs_graph {
    Some(self.resolve_graph_for_execution(&request)?)
} else {
    None
};

// PHASE 3: Create execution context
let mut context = self.create_execution_context_from_session(request.session.as_ref());
if let Some(graph) = &resolved_graph {
    context.current_graph = Some(graph.clone());
}

// PHASE 4: Route to execution path
let result = self.route_and_execute(&request, &mut context, resolved_graph.as_ref())?;

// PHASE 5: Audit logging
self.audit_query_execution(query_text, session_id, &result, elapsed_ms);

Ok(result)
```

#### 2. Graph Resolution

Located at [graphlite/src/exec/executor.rs:234](graphlite/src/exec/executor.rs#L234)

**Precedence** (PostgreSQL-style):
1. Explicit graph expression in query (FROM clause)
2. Session's current graph
3. Error if needed but not available

```rust
fn resolve_graph_for_execution(&self, request: &ExecutionRequest)
    -> Result<Arc<GraphCache>, ExecutionError>
{
    // Priority 1: Explicit FROM clause
    if let Some(graph_expr) = &request.graph_expr {
        return self.resolve_graph_expression(Some(graph_expr));
    }

    // Priority 2: Session's current graph
    if let Some(session_lock) = &request.session {
        if let Ok(session) = session_lock.read() {
            if let Some(current_graph_path) = &session.current_graph {
                return self.storage.get_graph(current_graph_path)?
                    .ok_or_else(|| ExecutionError::RuntimeError(
                        format!("Session graph '{}' not found", current_graph_path)
                    ));
            }
        }
    }

    // No graph available
    Err(ExecutionError::RuntimeError(
        "No graph context available. Use SESSION SET GRAPH or specify FROM clause.".to_string()
    ))
}
```

#### 3. Statement Routing

Located at [graphlite/src/exec/executor.rs:289](graphlite/src/exec/executor.rs#L289)

```rust
fn route_and_execute(
    &self,
    request: &ExecutionRequest,
    context: &mut ExecutionContext,
    graph: Option<&Arc<GraphCache>>,
) -> Result<QueryResult, ExecutionError> {
    match &request.statement {
        // If pre-computed physical plan available, use it
        Statement::Query(_) if request.physical_plan.is_some() => {
            let plan = request.physical_plan.as_ref().unwrap();
            if let Some(graph) = graph {
                self.execute_physical_plan_with_context(plan, context, graph)
            } else {
                self.execute_physical_plan_without_graph(plan, context)
            }
        }

        // Otherwise, execute statement directly
        _ => {
            self.execute_statement(
                &request.statement,
                context,
                request.graph_expr.as_ref(),
                request.session.as_ref(),
            )
        }
    }
}
```

#### 4. Statement Execution

**Execute Statement**:
```rust
fn execute_statement(
    &self,
    statement: &Statement,
    context: &mut ExecutionContext,
    graph_expr: Option<&GraphExpression>,
    session: Option<&Arc<RwLock<UserSession>>>,
) -> Result<QueryResult, ExecutionError>
```

**Routes to specialized executors**:
- `Statement::Query` → Plan → Execute physical plan
- `Statement::DataStatement` → Data modification executor
- `Statement::CatalogStatement` → DDL executor
- `Statement::TransactionStatement` → Transaction executor
- `Statement::SessionStatement` → Session executor
- `Statement::Call` → Procedure executor
- `Statement::Select` → SELECT executor

**Dependencies**:
- `plan::logical` - Logical plan representation
- `plan::physical` - Physical plan representation
- `storage::StorageManager` - Data access
- `catalog::CatalogManager` - Metadata access
- `txn::TransactionManager` - Transaction control
- `cache::CacheManager` - Caching
- `functions::FunctionRegistry` - Function calls

**Integration**: Core execution engine:
```
QueryCoordinator
    ↓
QueryExecutor ← YOU ARE HERE
    ├─> Planner (for Query statements)
    ├─> DataStatementExecutor (for INSERT/DELETE/SET)
    ├─> DDLExecutor (for CREATE/DROP)
    ├─> TransactionExecutor (for BEGIN/COMMIT)
    ├─> SessionExecutor (for SET GRAPH)
    └─> ProcedureExecutor (for CALL)
```

**Critical Implementation Details**:

1. **Fully Synchronous**: No async/await, no runtime nesting
2. **Transaction Integration**: All write operations use transaction logs
3. **Type System Integration**: Automatic type inference, validation, coercion
4. **Error Handling**: Rich error types with context

---

### exec/context.rs

**Location**: [graphlite/src/exec/context.rs](graphlite/src/exec/context.rs)
**Purpose**: Execution context tracking

**Key Structure** (to be read for full details):
```rust
pub struct ExecutionContext {
    pub session_id: String,
    pub current_graph: Option<Arc<GraphCache>>,
    pub variables: HashMap<String, Value>,
    pub function_registry: Option<Arc<FunctionRegistry>>,
    pub storage: Arc<StorageManager>,
}
```

**Purpose**:
- Track variables during query execution
- Maintain current graph context
- Provide access to functions and storage

---

### exec/result.rs

**Location**: [graphlite/src/exec/result.rs](graphlite/src/exec/result.rs)
**Purpose**: Query result structures

**Key Structures** (to be read for full details):
```rust
pub struct QueryResult {
    pub rows: Vec<Row>,
    pub variables: Vec<String>,
    pub execution_time_ms: u64,
    pub rows_affected: usize,
    pub session_result: Option<SessionResult>,
    pub warnings: Vec<String>,
}

pub struct Row {
    pub values: HashMap<String, Value>,
}
```

---

### exec/error.rs

**Location**: [graphlite/src/exec/error.rs](graphlite/src/exec/error.rs)
**Purpose**: Execution error types

**Key Enum** (to be read for full details):
```rust
pub enum ExecutionError {
    ParseError(String),
    ValidationError(String),
    RuntimeError(String),
    StorageError(StorageError),
    TransactionError(String),
    TypeMismatch(String),
    PermissionDenied(String),
    // ... more variants
}
```

---

### exec/write_stmt/

**Directory**: [graphlite/src/exec/write_stmt/](graphlite/src/exec/write_stmt/)
**Purpose**: Data modification executors (INSERT, DELETE, SET, REMOVE)

#### Structure:
```
write_stmt/
├── mod.rs                    - Module declaration
├── statement_base.rs         - Base trait for write operations
├── data_stmt/
│   ├── mod.rs
│   ├── data_statement_base.rs  - Base for data modifications
│   ├── coordinator.rs          - Routes to specific executors
│   ├── insert.rs               - INSERT execution
│   ├── delete.rs               - DELETE execution
│   ├── match_insert.rs         - MATCH...INSERT
│   ├── match_delete.rs         - MATCH...DELETE
│   ├── match_set.rs            - MATCH...SET
│   ├── match_remove.rs         - MATCH...REMOVE
│   └── planned_insert.rs       - Optimized INSERT with plan
├── ddl_stmt/
│   ├── mod.rs
│   └── ddl_statement_base.rs   - DDL execution (CREATE/DROP)
└── transaction/
    ├── mod.rs
    └── transaction_base.rs     - Transaction control
```

**Key Files**:

**insert.rs**: Handles INSERT statements
- Parse INSERT patterns
- Generate storage IDs
- Create nodes and edges
- Log to transaction
- Invalidate caches

**delete.rs**: Handles DELETE statements
- Match entities to delete
- Check referential integrity (DETACH)
- Delete from storage
- Log to transaction
- Invalidate caches

**match_insert.rs**: MATCH...INSERT combination
- Execute MATCH to find context
- Use matched variables in INSERT
- Conditional insertion based on MATCH

---

### Other exec/ files

- **lock_tracker.rs**: Track locks for isolation
- **memory_budget.rs**: Memory management during execution
- **row_iterator.rs**: Streaming row iteration
- **streaming_topk.rs**: Top-K with streaming
- **unwind_preprocessor.rs**: UNWIND query preprocessing
- **with_clause_processor.rs**: WITH clause pipeline processing

---

## Functions Module

**Directory**: [graphlite/src/functions/](graphlite/src/functions/)
**Purpose**: Built-in function implementations

### Structure:
```
functions/
├── mod.rs                      - Module declaration and registry
├── function_trait.rs           - Function trait definition
├── aggregate_functions.rs      - COUNT, SUM, AVG, MIN, MAX, etc.
├── mathematical_functions.rs   - ABS, CEIL, FLOOR, ROUND, etc.
├── list_functions.rs           - LIST operations
├── null_functions.rs           - COALESCE, NULLIF
└── numeric_functions.rs        - Math functions
```

### functions/mod.rs

**Location**: [graphlite/src/functions/mod.rs](graphlite/src/functions/mod.rs)
**Purpose**: Function registry and coordination

**Key Structure** (to be read for full details):
```rust
pub struct FunctionRegistry {
    functions: HashMap<String, Box<dyn Function>>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };

        // Register all built-in functions
        registry.register_aggregate_functions();
        registry.register_mathematical_functions();
        registry.register_list_functions();
        // ...

        registry
    }

    pub fn call(&self, name: &str, args: Vec<Value>) -> Result<Value, FunctionError> {
        // Look up and execute function
    }
}
```

---

### functions/function_trait.rs

**Location**: [graphlite/src/functions/function_trait.rs](graphlite/src/functions/function_trait.rs)
**Purpose**: Trait definition for all functions

**Key Trait** (to be read for full details):
```rust
pub trait Function: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self, args: Vec<Value>, context: &FunctionContext) -> Result<Value, FunctionError>;
    fn signature(&self) -> FunctionSignature;
}
```

---

### functions/aggregate_functions.rs

**Location**: [graphlite/src/functions/aggregate_functions.rs](graphlite/src/functions/aggregate_functions.rs)
**Purpose**: Aggregate function implementations

**Functions Implemented**:
- `COUNT(*)`, `COUNT(expr)`, `COUNT(DISTINCT expr)`
- `SUM(expr)`, `AVG(expr)`
- `MIN(expr)`, `MAX(expr)`
- `COLLECT(expr)` - Collect values into list
- `STRING_AGG(expr, delimiter)` - Concatenate strings

**Example COUNT Implementation**:
```rust
struct CountFunction;

impl Function for CountFunction {
    fn name(&self) -> &str { "count" }

    fn execute(&self, args: Vec<Value>, context: &FunctionContext) -> Result<Value, FunctionError> {
        // COUNT(*) - count all rows
        if args.is_empty() {
            Ok(Value::Integer(context.row_count as i64))
        }
        // COUNT(expr) - count non-null values
        else {
            let count = args.iter().filter(|v| !v.is_null()).count();
            Ok(Value::Integer(count as i64))
        }
    }
}
```

---

## Plan Module

**Directory**: [graphlite/src/plan/](graphlite/src/plan/)
**Purpose**: Query planning and optimization

### Structure:
```
plan/
├── mod.rs                  - Module declaration, logical planner, physical planner
├── logical.rs              - Logical plan representation
├── physical.rs             - Physical plan representation
├── cost.rs                 - Cost estimation
├── trace.rs                - Planning trace for debugging
└── pattern_optimization/
    └── mod.rs              - Pattern matching optimizations
```

### plan/logical.rs

**Location**: [graphlite/src/plan/logical.rs](graphlite/src/plan/logical.rs)
**Lines**: 300+
**Purpose**: Logical query plan representation

Already covered in detail above. Key points:

**Logical Plan Structure**:
```rust
pub struct LogicalPlan {
    pub root: LogicalNode,
    pub variables: HashMap<String, VariableInfo>,
}
```

**Logical Node Types**:
- NodeScan, EdgeScan - Access nodes/edges
- Expand, PathTraversal - Graph traversal
- Filter, Project - Data manipulation
- Join, Union, Intersect, Except - Set operations
- Aggregate, Sort, Limit - Aggregation and ordering
- Insert, Update, Delete - Data modification

**Integration**: Intermediate representation between AST and physical plan:
```
AST → Logical Plan → Optimizer → Physical Plan → Executor
```

---

### plan/physical.rs

**Location**: [graphlite/src/plan/physical.rs](graphlite/src/plan/physical.rs)
**Lines**: 300+
**Purpose**: Physical execution plan with algorithms

Already covered in detail above. Key points:

**Physical Plan Structure**:
```rust
pub struct PhysicalPlan {
    pub root: PhysicalNode,
    pub estimated_cost: f64,
    pub estimated_rows: usize,
}
```

**Physical Operators**:
- **Scans**: NodeSeqScan, NodeIndexScan, EdgeSeqScan
- **Joins**: HashJoin, NestedLoopJoin, SortMergeJoin
- **Sorts**: InMemorySort, ExternalSort
- **Aggregates**: HashAggregate, SortAggregate
- **Graph Ops**: IndexedExpand, HashExpand, PathTraversal

**Integration**: Executable plan with specific algorithms:
```
Logical Plan → Physical Planner → Physical Plan → Executor
```

**Physical planning decisions**:
- Index vs. sequential scan (based on selectivity)
- Hash vs. nested loop join (based on sizes)
- In-memory vs. external sort (based on memory budget)
- Hash vs. sort aggregation (based on cardinality)

---

### plan/cost.rs

**Location**: [graphlite/src/plan/cost.rs](graphlite/src/plan/cost.rs)
**Purpose**: Cost estimation for physical operators

**Key Functions** (to be read for full details):
```rust
pub fn estimate_cost(node: &PhysicalNode) -> f64 {
    match node {
        PhysicalNode::NodeSeqScan { ... } => {
            // Cost = number of nodes to scan
        }
        PhysicalNode::NodeIndexScan { ... } => {
            // Cost = index lookup + selective scan
        }
        PhysicalNode::HashJoin { ... } => {
            // Cost = build hash table + probe
        }
        // ... more operators
    }
}
```

---

### plan/mod.rs

**Location**: [graphlite/src/plan/mod.rs](graphlite/src/plan/mod.rs)
**Purpose**: Main planning logic - converts AST to logical to physical plans

**Key Functions** (to be read for full details):
```rust
pub fn plan_query(query: &Query) -> Result<LogicalPlan, PlanError> {
    // Convert AST to logical plan
}

pub fn optimize_logical_plan(plan: LogicalPlan) -> LogicalPlan {
    // Apply logical optimizations
    // - Predicate pushdown
    // - Projection pushdown
    // - Join reordering
    // - Subquery unnesting
}

pub fn create_physical_plan(logical: LogicalPlan) -> PhysicalPlan {
    // Convert logical to physical plan
    // - Choose scan algorithms
    // - Choose join algorithms
    // - Choose sort/aggregate algorithms
    // - Estimate costs
}
```

---

## Schema Module

**Directory**: [graphlite/src/schema/](graphlite/src/schema/)
**Purpose**: Graph Type System - schema management and validation

### Structure:
```
schema/
├── mod.rs                          - Module declaration
├── catalog/
│   ├── mod.rs                      - Schema catalog
│   └── graph_type_old.rs           - Legacy graph types
├── parser/
│   ├── mod.rs                      - Schema language parser
│   └── ast.rs                      - Schema AST
├── executor/
│   ├── mod.rs
│   ├── create_graph_type.rs        - CREATE GRAPH TYPE
│   └── drop_graph_type.rs          - DROP GRAPH TYPE
├── enforcement/
│   ├── mod.rs                      - Schema enforcement
│   └── config.rs                   - Enforcement configuration
├── integration/
│   ├── mod.rs
│   ├── index_validator.rs          - Index validation
│   ├── ingestion_validator.rs      - Data ingestion validation
│   └── runtime_validator.rs        - Runtime validation
├── introspection/
│   ├── mod.rs
│   └── queries.rs                  - Schema introspection queries
└── standalone_test.rs              - Schema tests
```

**Purpose**:
- Define graph schemas (node types, edge types, properties)
- Validate data against schemas
- Enforce constraints (required properties, types, etc.)
- Schema introspection

**Key Operations**:
```sql
-- Create schema
CREATE GRAPH TYPE SocialNetwork {
    NODE TYPES {
        Person {
            name: STRING NOT NULL,
            age: INTEGER,
            email: STRING UNIQUE
        },
        Post {
            content: STRING NOT NULL,
            created_at: TIMESTAMP
        }
    },
    EDGE TYPES {
        KNOWS {
            since: DATE
        },
        WROTE {
            created_at: TIMESTAMP NOT NULL
        }
    }
}

-- Drop schema
DROP GRAPH TYPE SocialNetwork
```

---

## Session Module

**Directory**: [graphlite/src/session/](graphlite/src/session/)
**Purpose**: Session management and authentication

### Structure:
```
session/
├── mod.rs                  - Module declaration and session models
├── manager.rs              - Session manager
└── transaction_state.rs    - Per-session transaction state
```

### session/mod.rs

**Location**: [graphlite/src/session/mod.rs](graphlite/src/session/mod.rs)
**Purpose**: Session models

**Key Structures** (to be read for full details):
```rust
pub struct UserSession {
    pub session_id: String,
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: Vec<Permission>,
    pub current_graph: Option<String>,
    pub current_schema: Option<String>,
    pub created_at: Instant,
    pub last_activity: Instant,
}
```

---

### session/manager.rs

**Location**: [graphlite/src/session/manager.rs](graphlite/src/session/manager.rs)
**Purpose**: Session lifecycle management

**Key Methods** (to be read for full details):
```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<UserSession>>>>>,
    catalog: Arc<RwLock<CatalogManager>>,
}

impl SessionManager {
    pub fn create_session(&self, username: &str, ...) -> Result<String, String>;
    pub fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<UserSession>>>;
    pub fn close_session(&self, session_id: &str) -> Result<(), String>;
    pub fn authenticate(&self, username: &str, password: &str) -> Result<bool, String>;
}
```

---

## Storage Module

**Directory**: [graphlite/src/storage/](graphlite/src/storage/)
**Purpose**: Multi-tier storage system with caching

### Structure:
```
storage/
├── mod.rs                  - Module declaration and core types
├── types.rs                - Value types and serialization
├── storage_manager.rs      - Multi-tier orchestration
├── indexes/
│   ├── mod.rs              - Index management
│   ├── traits.rs           - Index traits
│   └── metrics.rs          - Index metrics
└── persistent/
    ├── mod.rs              - Persistent storage
    ├── factory.rs          - Storage driver factory
    ├── sled.rs             - Sled backend
    └── memory.rs           - In-memory backend
```

### storage/storage_manager.rs

**Location**: [graphlite/src/storage/storage_manager.rs](graphlite/src/storage/storage_manager.rs)
**Lines**: 300+
**Purpose**: Multi-tier storage orchestration

Already covered in detail above. Key points:

**Storage Tiers**:
```
L1: Local Cache (MultiGraphManager) - Hot data, fast
L2: Persistent Store (Sled/RocksDB) - Warm data, disk
L3: External Memory (Redis/Valkey) - Cold data, distributed
```

**Key Methods**:
```rust
pub fn get_graph(&self, name: &str) -> Result<Option<GraphCache>, StorageError> {
    // Check L1 cache → L2 persistent → L3 external
}

pub fn save_graph(&self, name: &str, graph: GraphCache) -> Result<(), StorageError> {
    // Update L1 → L2 → L3
}
```

---

### storage/types.rs

**Location**: [graphlite/src/storage/types.rs](graphlite/src/storage/types.rs)
**Purpose**: Value types and serialization

**Key Type**:
```rust
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Vector(Vec<f64>),  // For embeddings
    Node(NodeValue),
    Edge(EdgeValue),
    Path(PathValue),
}
```

**Serialization**: Uses `serde` + `bincode` for efficient binary serialization

---

## Txn Module

**Directory**: [graphlite/src/txn/](graphlite/src/txn/)
**Purpose**: Transaction management for ACID properties

### Structure:
```
txn/
├── mod.rs          - Module declaration
├── state.rs        - Transaction state
├── isolation.rs    - Isolation levels
├── log.rs          - Undo log
├── wal.rs          - Write-Ahead Log (WAL)
├── manager.rs      - Transaction manager
└── recovery.rs     - Recovery from crashes
```

### txn/mod.rs

**Location**: [graphlite/src/txn/mod.rs](graphlite/src/txn/mod.rs)
**Lines**: 33
**Purpose**: Transaction module organization

Already covered in detail above. Key exports:

```rust
pub use isolation::IsolationLevel;
pub use log::{TransactionLog, UndoOperation};
pub use manager::TransactionManager;
pub use state::TransactionId;
```

**Two-Log System**:
1. **Undo Log** (`log.rs`): For rollback (ATOMICITY)
2. **Write-Ahead Log** (`wal.rs`): For durability (DURABILITY)

**ACID Implementation**:
- **Atomicity**: Undo log allows complete rollback
- **Consistency**: Schema validation + constraints
- **Isolation**: Lock tracker + READ_COMMITTED
- **Durability**: WAL with fsync before commit

---

### txn/log.rs

**Location**: [graphlite/src/txn/log.rs](graphlite/src/txn/log.rs)
**Purpose**: Undo log for transaction rollback

**Key Structures** (to be read for full details):
```rust
pub struct TransactionLog {
    pub transaction_id: TransactionId,
    pub operations: Vec<UndoOperation>,
    pub started_at: Instant,
}

pub enum UndoOperation {
    InsertNode { storage_id: String, graph: String },
    InsertEdge { storage_id: String, graph: String },
    UpdateProperty { entity_id: String, property: String, old_value: Value },
    DeleteNode { storage_id: String, data: NodeValue },
    DeleteEdge { storage_id: String, data: EdgeValue },
}
```

---

### txn/manager.rs

**Location**: [graphlite/src/txn/manager.rs](graphlite/src/txn/manager.rs)
**Purpose**: Transaction lifecycle management

**Key Methods** (to be read for full details):
```rust
pub struct TransactionManager {
    storage: Arc<StorageManager>,
    active_transactions: Arc<RwLock<HashMap<TransactionId, TransactionLog>>>,
    wal: Arc<WriteAheadLog>,
}

impl TransactionManager {
    pub fn begin_transaction(&self) -> TransactionId;
    pub fn commit_transaction(&self, txn_id: TransactionId) -> Result<(), TxnError>;
    pub fn rollback_transaction(&self, txn_id: TransactionId) -> Result<(), TxnError>;
    pub fn log_operation(&self, txn_id: TransactionId, op: UndoOperation);
}
```

---

## Types Module

**Directory**: [graphlite/src/types/](graphlite/src/types/)
**Purpose**: Type system - inference, validation, coercion, casting

### Structure:
```
types/
├── mod.rs          - Module declaration and type definitions
├── inference.rs    - Type inference
├── validation.rs   - Type validation
├── coercion.rs     - Automatic type coercion
└── casting.rs      - Explicit type casting
```

**Purpose**: Comprehensive type system for GQL

**Type Operations**:
1. **Inference**: Deduce types from context
2. **Validation**: Check type correctness
3. **Coercion**: Automatic conversions (e.g., INTEGER → FLOAT)
4. **Casting**: Explicit conversions (e.g., CAST(x AS STRING))

**Example**:
```sql
-- Type inference
RETURN 1 + 2.5        -- Infers FLOAT result

-- Type validation
WHERE n.age > "30"    -- ERROR: Cannot compare INTEGER with STRING

-- Type coercion
WHERE n.score > 100   -- Coerces 100 (INTEGER) to FLOAT if n.score is FLOAT

-- Type casting
RETURN CAST(n.age AS STRING)  -- Explicit conversion
```

---

## Catalog Module

**Directory**: [graphlite/src/catalog/](graphlite/src/catalog/)
**Purpose**: Metadata management - graphs, schemas, users, roles, permissions

### Structure:
```
catalog/
├── mod.rs                  - Module declaration
├── manager.rs              - Catalog manager
├── operations.rs           - Catalog operations
├── traits.rs               - Catalog provider traits
├── error.rs                - Catalog errors
├── registry.rs             - Type registry
├── system_procedures.rs    - System procedures
├── storage/
│   └── mod.rs              - Catalog storage
└── providers/
    ├── mod.rs              - Provider coordination
    ├── index.rs            - Index provider
    ├── graph_metadata.rs   - Graph metadata provider
    ├── schema.rs           - Schema provider
    └── security.rs         - Security provider (users, roles, permissions)
```

**Purpose**: Central metadata repository for entire system

**Catalog Responsibilities**:
- Graph metadata (names, paths, creation dates)
- Schema definitions (graph types, node types, edge types)
- User accounts and authentication
- Role definitions
- Permission grants
- Index metadata
- System procedures

**Key Operations**:
```sql
-- Graph operations
CREATE GRAPH myGraph
DROP GRAPH myGraph

-- User management
CREATE USER alice PASSWORD 'secret'
DROP USER alice

-- Role management
CREATE ROLE reader
GRANT SELECT ON GRAPH myGraph TO reader
GRANT reader TO alice

-- Permission management
GRANT INSERT ON GRAPH myGraph TO alice
REVOKE DELETE ON GRAPH myGraph FROM alice
```

---

## Integration Guide

### How All Modules Work Together

**Query Flow Example**: `MATCH (n:Person) RETURN n.name`

```
1. CLI/SDK
   ↓ (query string)

2. QueryCoordinator (coordinator/)
   ├─> Parse query
   ├─> Get session
   └─> Create execution request
   ↓

3. QueryExecutor (exec/)
   ├─> Resolve graph context
   ├─> Check plan cache (cache/)
   │   ├─> HIT: Use cached plan
   │   └─> MISS: Plan query
   │        ↓
   │        4. Query Planner (plan/)
   │           ├─> AST → Logical Plan
   │           ├─> Optimize Logical Plan
   │           ├─> Logical → Physical Plan
   │           ├─> Cost estimation
   │           └─> Return physical plan
   │        ↓
   │        5. Cache Plan (cache/)
   │
   └─> Execute physical plan
       ↓

6. Physical Execution (exec/)
   ├─> NodeIndexScan (storage/)
   │   ├─> Check L1 cache
   │   ├─> Check L2 persistent
   │   └─> Load into cache
   │
   ├─> Filter (WHERE clause)
   │   └─> Evaluate expressions (functions/)
   │
   └─> Project (RETURN clause)
       └─> Extract properties

7. Storage Access (storage/)
   ├─> MultiGraphManager (L1 cache)
   ├─> Sled/RocksDB (L2 persistent)
   └─> Return data

8. Return Results
   └─> QueryResult with rows
```

**Write Flow Example**: `INSERT (n:Person {name: "Alice"})`

```
1. QueryCoordinator
   ↓

2. QueryExecutor
   ├─> Route to DataStatement executor
   └─> exec/write_stmt/data_stmt/insert.rs
       ↓

3. Insert Executor
   ├─> Parse INSERT pattern
   ├─> Generate storage ID (content-based hash)
   ├─> Validate against schema (schema/)
   ├─> Check transaction (txn/)
   │   ├─> Get current transaction
   │   └─> Log undo operation
   │
   ├─> Write to storage (storage/)
   │   ├─> Update L1 cache
   │   ├─> Write to L2 persistent
   │   └─> Log to WAL (txn/wal.rs)
   │
   └─> Invalidate caches (cache/)
       ├─> Plan cache (schema changed)
       └─> Result cache (data changed)

4. Transaction Commit
   ├─> fsync WAL
   ├─> Clear undo log
   └─> Return success
```

**Transaction Flow**: `BEGIN` → `INSERT` → `COMMIT`

```
1. BEGIN TRANSACTION
   ├─> TransactionManager.begin_transaction()
   ├─> Generate TransactionId
   ├─> Create TransactionLog
   └─> Store in active_transactions

2. INSERT (...)
   ├─> Execute insert
   ├─> Log UndoOperation to transaction log
   └─> Write to WAL (not fsynced yet)

3. COMMIT TRANSACTION
   ├─> TransactionManager.commit_transaction()
   ├─> fsync WAL (DURABILITY)
   ├─> Clear undo log
   ├─> Remove from active_transactions
   └─> Return success

If ROLLBACK instead:
   ├─> TransactionManager.rollback_transaction()
   ├─> Read undo log
   ├─> Apply undo operations in reverse order
   ├─> Clear WAL entries for this transaction
   └─> Return success
```

### File Reading Priority

**For Understanding the System**, read files in this order:

#### 1. Start Here (Foundation)
1. [lib.rs](graphlite/src/lib.rs) - Public API
2. [coordinator/query_coordinator.rs](graphlite/src/coordinator/query_coordinator.rs) - Entry point
3. [exec/executor.rs](graphlite/src/exec/executor.rs) - Execution engine

#### 2. Query Processing
4. [ast/ast.rs](graphlite/src/ast/ast.rs) - AST definition
5. [ast/lexer.rs](graphlite/src/ast/lexer.rs) - Tokenization
6. [ast/parser.rs](graphlite/src/ast/parser.rs) - Parsing
7. [plan/logical.rs](graphlite/src/plan/logical.rs) - Logical plans
8. [plan/physical.rs](graphlite/src/plan/physical.rs) - Physical plans
9. [plan/mod.rs](graphlite/src/plan/mod.rs) - Planning logic

#### 3. Storage and Data
10. [storage/storage_manager.rs](graphlite/src/storage/storage_manager.rs) - Storage orchestration
11. [storage/types.rs](graphlite/src/storage/types.rs) - Value types
12. [cache/plan_cache.rs](graphlite/src/cache/plan_cache.rs) - Plan caching

#### 4. Transactions and Sessions
13. [txn/mod.rs](graphlite/src/txn/mod.rs) - Transaction system
14. [txn/log.rs](graphlite/src/txn/log.rs) - Undo log
15. [txn/manager.rs](graphlite/src/txn/manager.rs) - Transaction manager
16. [session/manager.rs](graphlite/src/session/manager.rs) - Session management

#### 5. Advanced Features
17. [functions/mod.rs](graphlite/src/functions/mod.rs) - Function registry
18. [schema/mod.rs](graphlite/src/schema/mod.rs) - Schema system
19. [catalog/manager.rs](graphlite/src/catalog/manager.rs) - Metadata management

### Critical Files Summary

**Must Read** (Core architecture):
1. **lib.rs** - Public API boundary
2. **coordinator/query_coordinator.rs** - System entry point
3. **exec/executor.rs** - Execution engine
4. **ast/ast.rs** - AST definition
5. **plan/logical.rs**, **plan/physical.rs** - Query plans
6. **storage/storage_manager.rs** - Storage system
7. **txn/mod.rs**, **txn/manager.rs** - Transaction system

**Important** (Key subsystems):
- **ast/parser.rs** - Query parsing
- **cache/plan_cache.rs** - Performance optimization
- **session/manager.rs** - Session handling
- **functions/mod.rs** - Built-in functions
- **catalog/manager.rs** - Metadata

**Supporting** (Implementation details):
- Everything else fills in the details

### Key Architectural Patterns

#### 1. Dependency Injection
```rust
// Components receive dependencies via Arc<>
pub struct QueryExecutor {
    storage: Arc<StorageManager>,
    catalog: Arc<RwLock<CatalogManager>>,
    transactions: Arc<TransactionManager>,
    // ...
}
```

#### 2. Thread Safety
```rust
// All shared state uses Arc<RwLock<>>
Arc<RwLock<CatalogManager>>
Arc<RwLock<HashMap<String, Session>>>
```

#### 3. Error Propagation
```rust
// Result types throughout
pub fn execute_query(...) -> Result<QueryResult, ExecutionError>
```

#### 4. Layered Architecture
```
Public API (lib.rs)
    ↓
Coordinator (coordinator/)
    ↓
Executor (exec/)
    ↓
Planner (plan/)
    ↓
Storage (storage/)
```

#### 5. Cross-Cutting Concerns
```
Transactions (txn/) ──┐
Cache (cache/) ───────┤
Catalog (catalog/) ───┼─→ All layers use these
Functions (functions/)┤
Types (types/) ───────┘
```

---

## Conclusion

This overview provides a comprehensive guide to the GraphLite codebase. The architecture is well-organized with clear separation of concerns:

- **AST module**: Parse and represent queries
- **Plan module**: Optimize queries
- **Exec module**: Execute queries
- **Storage module**: Persist data
- **Cache module**: Optimize performance
- **Txn module**: Ensure ACID properties
- **Session module**: Manage users
- **Catalog module**: Manage metadata
- **Functions module**: Built-in operations
- **Types module**: Type system
- **Schema module**: Graph types
- **Coordinator module**: Orchestrate everything

The system implements a full ISO GQL graph database with:
- Complete query language support
- ACID transactions
- Multi-level caching
- Schema validation
- User authentication
- Cost-based optimization
- Extensible architecture

For deep understanding, follow the file reading priority above and trace query flows through the system.