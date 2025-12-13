# GraphLite Updated Architecture Documentation

> **Source**: This document is based on actual source code analysis from `graphlite/src/` directory.
>
> **Last Updated**: 2025-01-26
>
> **Total Files**: 163 Rust source files

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Architectural Layers](#architectural-layers)
3. [Major Operations & Call Hierarchies](#major-operations--call-hierarchies)
4. [Module-by-Module Architecture](#module-by-module-architecture)
5. [Data Flow Diagrams](#data-flow-diagrams)
6. [Integration Points](#integration-points)

---

## System Overview

### Core Design Philosophy

GraphLite is a **pure Rust embedded graph database** implementing the **ISO GQL standard**. Unlike client-server databases, GraphLite runs in-process with your application.

**Key Characteristics**:
- **Embedded**: No daemon, direct file access via Sled key-value store
- **Single Entry Point**: All operations flow through `QueryCoordinator` (lib.rs:37)
- **Layered Architecture**: Clean separation between parsing, planning, execution, and storage
- **Thread-Safe**: Extensive use of `Arc<RwLock<T>>` for concurrent access
- **Type-Safe**: Rust's type system prevents entire classes of bugs

### Technology Stack

```
Language:          Rust 1.70+
Parser:            nom (parser combinators)
Storage Backend:   Sled (embedded B-tree key-value store)
Concurrency:       Arc<RwLock<T>>, rayon (data parallelism)
Serialization:     serde, bincode, serde_json
Temporal:          chrono, chrono-tz
CLI:               clap, rustyline
```

---

## Architectural Layers

### Layer 1: Public API Layer

**Location**: `src/lib.rs` (63 lines)

**Purpose**: Defines the public interface exposed to external users.

**Key Exports**:
```rust
// Line 37: Only public module
pub mod coordinator;

// Lines 53-56: Public API
pub use coordinator::{QueryCoordinator, QueryResult, Row, QueryInfo, QueryPlan, QueryType};
pub use storage::Value;
```

**Access Control**:
- All other modules are `pub(crate)` - internal only
- Users interact ONLY with `QueryCoordinator`
- Clean encapsulation - implementation details hidden

---

### Layer 2: Coordination Layer

**Location**: `src/coordinator/query_coordinator.rs` (300+ lines)

**Purpose**: Main orchestrator for query execution with session management.

**Key Structure** (Lines 28-33):
```rust
pub struct QueryCoordinator {
    session_manager: Arc<SessionManager>,
    executor: Arc<QueryExecutor>,
}
```

**Initialization Flow** (Lines 69-117):
```
QueryCoordinator::from_path("./mydb")
    │
    ├─ 1. Create StorageManager (Lines 73-76)
    │     └─ File: src/storage/storage_manager.rs
    │
    ├─ 2. Create CatalogManager (Lines 78-79)
    │     └─ File: src/catalog/manager.rs
    │
    ├─ 3. Create TransactionManager (Lines 82-85)
    │     └─ File: src/txn/manager.rs
    │
    ├─ 4. Create CacheManager (Lines 88-92)
    │     └─ File: src/cache/cache_manager.rs
    │
    ├─ 5. Create QueryExecutor (Lines 95-103)
    │     └─ File: src/exec/executor.rs
    │
    └─ 6. Create SessionManager (Lines 106-110)
          └─ File: src/session/manager.rs
```

**Main Method** (Lines 145-169):
```rust
pub fn process_query(&self, query_text: &str, session_id: &str) -> Result<QueryResult, String>
```

**File Relationships**:
- **Calls**: `ast/parser.rs::parse_query()` (Line 147)
- **Calls**: `exec/executor.rs::execute_query()` (Line 158-161)
- **Updates**: Session state via `session/manager.rs` (Lines 164-166)

---

### Layer 3: Query Processing Layer

This layer transforms GQL text into executable operations.

#### 3.1 AST Module (Parsing & Validation)

**Location**: `src/ast/` (6 files, ~6000 lines total)

**Files**:
1. **`lexer.rs`** - Tokenization
2. **`parser.rs`** - Parsing (largest file, ~3000+ lines)
3. **`ast.rs`** - AST node definitions (~2500 lines)
4. **`validator.rs`** - Semantic validation
5. **`pretty_printer.rs`** - AST → GQL text
6. **`mod.rs`** - Module exports

**Flow**:
```
Query Text
    ↓
lexer.rs::tokenize() → Vec<Token>
    ↓
parser.rs::parse_query() → Document { statement, location }
    ↓
validator.rs::validate() → Validated AST
```

**Key Data Structure** (`ast.rs`):
```rust
pub struct Document {
    pub statement: Statement,
    pub location: Location,
}

pub enum Statement {
    Query(Query),                          // MATCH, WITH, RETURN
    Select(SelectStatement),               // SELECT
    Call(CallStatement),                   // CALL procedure
    DataStatement(DataStatement),          // INSERT, DELETE, SET, REMOVE
    CatalogStatement(CatalogStatement),    // CREATE/DROP GRAPH/SCHEMA
    SessionStatement(SessionStatement),    // SET SESSION GRAPH/SCHEMA
    TransactionStatement(TransactionStatement), // BEGIN, COMMIT, ROLLBACK
    IndexStatement(IndexStatement),        // CREATE/DROP INDEX
    // ... more
}
```

---

#### 3.2 Planning Module (Optimization)

**Location**: `src/plan/` (9 files)

**Core Files**:
1. **`logical.rs`** - Logical plan representation
2. **`physical.rs`** - Physical execution plan
3. **`optimizer.rs`** - Logical optimizations
4. **`cost.rs`** - Cost estimation
5. **`insert_planner.rs`** - INSERT planning
6. **`pattern_optimization/`** - Graph pattern optimizations (6 subfiles)

**Transformation Pipeline**:
```
AST (from parser)
    ↓
[logical.rs] Logical Planning
    └─ LogicalNode tree (implementation-independent)
    ↓
[optimizer.rs] Logical Optimization
    ├─ Predicate pushdown
    ├─ Projection pruning
    ├─ Constant folding
    └─ Dead code elimination
    ↓
[physical.rs] Physical Planning
    └─ PhysicalNode tree (with execution algorithms)
    ↓
[cost.rs] Cost Estimation
    └─ Select cheapest physical plan
```

**Key Structures**:

**Logical Plan** (`logical.rs`, Lines 18-21):
```rust
pub struct LogicalPlan {
    pub root: LogicalNode,
    pub variables: HashMap<String, VariableInfo>,
}
```

**Physical Plan** (`physical.rs`, Lines 71-75):
```rust
pub struct PhysicalPlan {
    pub root: PhysicalNode,
    pub estimated_cost: f64,
    pub estimated_rows: usize,
}
```

**Operators**:

| Logical Operator | Physical Operators | Files |
|-----------------|-------------------|-------|
| NodeScan | NodeSeqScan, NodeIndexScan | logical.rs:95, physical.rs:81,89 |
| Expand | IndexedExpand, HashExpand | logical.rs:109, physical.rs:108,121 |
| Filter | Filter (with selectivity) | logical.rs:129, physical.rs:145 |
| Join | HashJoin, NestedLoopJoin, SortMergeJoin | logical.rs:141, physical.rs:162,174,184 |

---

#### 3.3 Execution Module (Query Execution)

**Location**: `src/exec/` (42 files, ~25,000 lines)

**Core Files**:
1. **`executor.rs`** - Main execution engine (~7000 lines)
2. **`context.rs`** - Execution context (variable bindings)
3. **`result.rs`** - Query result structures
4. **`write_stmt/`** - DML/DDL/Transaction executors (32 files)

**Main Executor Structure** (`executor.rs`, Lines 102-130):
```rust
pub struct QueryExecutor {
    storage: Arc<StorageManager>,
    function_registry: Arc<FunctionRegistry>,
    catalog_manager: Arc<RwLock<CatalogManager>>,
    transaction_manager: Arc<TransactionManager>,
    current_transaction: Arc<RwLock<Option<TransactionId>>>,
    type_inference: TypeInference,
    type_validator: TypeValidator,
    type_coercion: TypeCoercion,
    type_caster: TypeCaster,
}
```

**Entry Point** (`executor.rs`, Line 144):
```rust
pub fn execute_query(&self, request: ExecutionRequest) -> Result<QueryResult, ExecutionError>
```

**Execution Routing** (`executor.rs`, Lines 288-300+):
```rust
fn route_and_execute(...) -> Result<QueryResult> {
    match &request.statement {
        Statement::Query(_) if request.physical_plan.is_some() => {
            // Execute pre-planned query
            self.execute_physical_plan(...)
        }
        Statement::Query(query) => {
            // Parse → Plan → Execute
            self.execute_read_query(query, context, graph)
        }
        Statement::DataStatement(stmt) => {
            // INSERT, DELETE, SET, REMOVE
            // Delegates to write_stmt/data_stmt/*.rs
        }
        Statement::CatalogStatement(stmt) => {
            // CREATE/DROP GRAPH, SCHEMA, etc.
            // Delegates to catalog/manager.rs
        }
        Statement::SessionStatement(stmt) => {
            // SET SESSION GRAPH/SCHEMA
            // Updates session/models.rs::UserSession
        }
        Statement::TransactionStatement(stmt) => {
            // BEGIN, COMMIT, ROLLBACK
            // Delegates to write_stmt/transaction/*.rs
        }
        // ... more
    }
}
```

---

### Layer 4: Data Management Layer

#### 4.1 Storage Module

**Location**: `src/storage/` (18 files)

**Architecture**:
```
Application (via StorageManager)
    │
    ├─ L1: Cache (MultiGraphManager) - In-memory
    │   └─ File: multi_graph.rs, graph_cache.rs
    │
    ├─ L2: Persistent Store (DataAdapter → Sled)
    │   └─ Files: data_adapter.rs, persistent/sled.rs
    │
    └─ L3: Index Manager (Property indexes)
        └─ Files: indexes/manager.rs
```

**StorageManager** (`storage_manager.rs`, Lines 42-60):
```rust
pub struct StorageManager {
    cache: Arc<MultiGraphManager>,                    // L1: Hot cache
    storage_driver: Option<Arc<Box<dyn StorageDriver>>>, // Backend
    persistent_store: Option<Arc<DataAdapter>>,       // L2: Disk
    memory_store: Option<Arc<DataAdapter>>,           // L3: External (future)
    storage_type: StorageType,                        // Sled/RocksDB
    index_manager: Option<Arc<IndexManager>>,         // Indexes
}
```

**Storage Method** (`storage_manager.rs`, Lines 29-38):
```rust
pub enum StorageMethod {
    DiskOnly,           // Uses Sled only
    MemoryOnly,         // Uses Redis (future)
    DiskAndMemory,      // Both (future)
}
```

**Data Access Flow** (`storage_manager.rs`, Lines 153-200):
```
get_graph(name)
    │
    ├─ 1. Check cache.get_graph(name) [Lines 157-169]
    │   └─ HIT → Return immediately
    │   └─ MISS → Continue
    │
    ├─ 2. Check memory_store (if configured) [Lines 174-176]
    │   └─ Future: Redis/Valkey
    │
    └─ 3. Check persistent_store [Lines 184-200]
        └─ Load from Sled → Add to cache → Return
```

**Key Files**:
- **`storage_manager.rs`** - Orchestration (800+ lines)
- **`graph_cache.rs`** - In-memory graph representation
- **`multi_graph.rs`** - Manages multiple graphs
- **`value.rs`** - Value type system (Node, Edge, primitives)
- **`types.rs`** - Core data structures (Node, Edge)
- **`persistent/sled.rs`** - Sled backend implementation
- **`indexes/manager.rs`** - Property indexes

---

#### 4.2 Catalog Module (Metadata)

**Location**: `src/catalog/` (11 files)

**Purpose**: Manages database metadata (graphs, schemas, users, roles).

**CatalogManager** (`manager.rs`):
```rust
pub struct CatalogManager {
    graph_metadata_provider: Arc<RwLock<GraphMetadataProvider>>,
    schema_provider: Arc<RwLock<SchemaProvider>>,
    security_provider: Arc<RwLock<SecurityProvider>>,
    index_provider: Arc<RwLock<IndexProvider>>,
    system_procedures: SystemProcedures,
}
```

**Providers** (`providers/`):
1. **`graph_metadata.rs`** - Graph definitions, metadata
2. **`schema.rs`** - Schema (Graph Type) definitions
3. **`security.rs`** - Users, roles, permissions
4. **`index.rs`** - Index metadata

**Operations** (`operations.rs`):
```rust
pub enum CatalogOperation {
    CreateGraph { name, schema },
    DropGraph { name },
    CreateSchema { name },
    CreateUser { username, password },
    GrantRole { user, role },
    // ... more
}
```

---

#### 4.3 Transaction Module

**Location**: `src/txn/` (7 files)

**TransactionManager** (`manager.rs`):
```rust
pub struct TransactionManager {
    active_transactions: Arc<RwLock<HashMap<TransactionId, TransactionLog>>>,
    next_txn_id: AtomicU64,
    wal: Arc<WriteAheadLog>,
}
```

**Key Methods**:
```rust
pub fn begin(...) -> Result<TransactionId>
pub fn commit(txn_id) -> Result<()>
pub fn rollback(txn_id) -> Result<()>
pub fn log_operation(txn_id, op: UndoOperation) -> Result<()>
```

**Components**:
1. **`manager.rs`** - Transaction lifecycle
2. **`log.rs`** - Transaction log (undo operations)
3. **`wal.rs`** - Write-ahead log (durability)
4. **`isolation.rs`** - Isolation level definitions
5. **`state.rs`** - Transaction state machine
6. **`recovery.rs`** - Crash recovery (future)

---

#### 4.4 Session Module

**Location**: `src/session/` (4 files)

**SessionManager** (`manager.rs`):
```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<UserSession>>>>>,
    transaction_manager: Arc<TransactionManager>,
    storage: Arc<StorageManager>,
    catalog_manager: Arc<RwLock<CatalogManager>>,
}
```

**UserSession** (`models.rs`):
```rust
pub struct UserSession {
    pub session_id: String,
    pub username: String,
    pub roles: Vec<String>,
    pub current_graph: Option<String>,      // Session state
    pub current_schema: Option<String>,     // Session state
    pub permissions: SessionPermissionCache,
    pub created_at: DateTime<Utc>,
}
```

---

### Layer 5: Supporting Modules

#### 5.1 Schema Module (ISO GQL Graph Types)

**Location**: `src/schema/` (18 files)

**Purpose**: Implements ISO GQL Graph Type system with validation.

**Submodules**:
- **`catalog/`** - Graph type definitions
- **`parser/`** - CREATE GRAPH TYPE parser
- **`executor/`** - CREATE/ALTER/DROP GRAPH TYPE executors
- **`integration/`** - Schema validation (ingestion, runtime, index)
- **`enforcement/`** - Enforcement configuration

---

#### 5.2 Cache Module

**Location**: `src/cache/` (7 files)

**CacheManager** (`cache_manager.rs`):
```rust
pub struct CacheManager {
    l1_result_cache: Arc<RwLock<ResultCache>>,    // Hot
    l2_result_cache: Arc<RwLock<ResultCache>>,    // Warm
    l3_plan_cache: Arc<RwLock<PlanCache>>,        // Cold
    subquery_cache: Arc<RwLock<SubqueryCache>>,   // Specialized
}
```

**Cache Types**:
1. **`result_cache.rs`** - Query result caching
2. **`plan_cache.rs`** - Compiled query plan caching
3. **`subquery_cache.rs`** - Subquery result caching
4. **`invalidation.rs`** - Cache invalidation logic

---

#### 5.3 Functions Module

**Location**: `src/functions/` (12 files)

**FunctionRegistry** (`mod.rs`):
```rust
pub struct FunctionRegistry {
    functions: HashMap<String, Arc<dyn Function>>,
}
```

**Function Categories** (60+ functions):
1. **`aggregate_functions.rs`** - COUNT, SUM, AVG, MIN, MAX
2. **`string_functions.rs`** - UPPER, LOWER, TRIM, SUBSTRING
3. **`mathematical_functions.rs`** - ABS, SQRT, POWER, SIN, COS
4. **`temporal_functions.rs`** - NOW, DATETIME, DATE_ADD
5. **`graph_functions.rs`** - LABELS, TYPE, ID, PROPERTIES
6. **`list_functions.rs`** - LIST_CONTAINS, LIST_APPEND
7. **`timezone_functions.rs`** - AT_TIME_ZONE, CONVERT_TZ
8. **`null_functions.rs`** - COALESCE, NULLIF
9. **`special_functions.rs`** - ALL_DIFFERENT, SAME

---

#### 5.4 Types Module

**Location**: `src/types/` (5 files)

**Components**:
1. **`inference.rs`** - Type inference algorithm
2. **`validation.rs`** - Type compatibility checking
3. **`coercion.rs`** - Automatic type conversion
4. **`casting.rs`** - CAST expression execution

---

## Major Operations & Call Hierarchies

### Operation 1: Read Query Execution

**Example Query**: `MATCH (p:Person {name: 'Alice'}) RETURN p.name`

**Complete Call Stack**:

```
[User Application]
    │
    ├─ graphlite::QueryCoordinator::process_query(query_text, session_id)
    │   └─ File: src/coordinator/query_coordinator.rs:145
    │
    ├─ [STEP 1: PARSE]
    │   └─ crate::ast::parser::parse_query(query_text)
    │       └─ File: src/ast/parser.rs
    │       └─ Returns: Document { statement: Query(...) }
    │
    ├─ [STEP 2: GET SESSION]
    │   └─ session_manager.get_session(session_id)
    │       └─ File: src/session/manager.rs
    │       └─ Returns: Option<Arc<RwLock<UserSession>>>
    │
    ├─ [STEP 3: CREATE EXECUTION REQUEST]
    │   └─ ExecutionRequest::new(statement)
    │           .with_session(session)
    │           .with_query_text(query_text)
    │       └─ File: src/exec/executor.rs:58-99
    │
    └─ [STEP 4: EXECUTE]
        └─ executor.execute_query(request)
            └─ File: src/exec/executor.rs:144
            │
            ├─ [4.1: Resolve Graph Context]
            │   └─ resolve_graph_for_execution()
            │       └─ File: src/exec/executor.rs:235-265
            │       ├─ Priority 1: Explicit FROM clause
            │       ├─ Priority 2: Session current_graph
            │       └─ storage.get_graph(graph_path)
            │           └─ File: src/storage/storage_manager.rs:153
            │
            ├─ [4.2: Create Execution Context]
            │   └─ create_execution_context_from_session()
            │       └─ File: src/exec/executor.rs:268-286
            │       └─ Returns: ExecutionContext
            │           └─ File: src/exec/context.rs
            │
            └─ [4.3: Route and Execute]
                └─ route_and_execute(request, context, graph)
                    └─ File: src/exec/executor.rs:289-300+
                    │
                    └─ match statement:
                        │
                        ├─ Statement::Query(query) →
                        │   └─ execute_read_query(query, context, graph)
                        │       └─ File: src/exec/executor.rs (method)
                        │       │
                        │       ├─ [4.3.1: Logical Planning]
                        │       │   └─ LogicalPlanner::plan(query)
                        │       │       └─ File: src/plan/logical.rs
                        │       │       └─ Returns: LogicalPlan
                        │       │
                        │       ├─ [4.3.2: Optimization]
                        │       │   └─ LogicalOptimizer::optimize(logical_plan)
                        │       │       └─ File: src/plan/optimizer.rs
                        │       │       ├─ apply_predicate_pushdown()
                        │       │       ├─ apply_projection_pruning()
                        │       │       └─ Returns: Optimized LogicalPlan
                        │       │
                        │       ├─ [4.3.3: Physical Planning]
                        │       │   └─ PhysicalPlanner::plan(logical_plan, stats)
                        │       │       └─ File: src/plan/physical.rs
                        │       │       ├─ Choose scan methods (Seq vs Index)
                        │       │       ├─ Choose join algorithms
                        │       │       ├─ Estimate costs
                        │       │       └─ Returns: PhysicalPlan
                        │       │
                        │       └─ [4.3.4: Execute Physical Plan]
                        │           └─ execute_physical_plan(plan, context)
                        │               └─ File: src/exec/executor.rs (method)
                        │               │
                        │               └─ match plan.root:
                        │                   │
                        │                   ├─ PhysicalNode::NodeSeqScan { variable, labels, ... } →
                        │                   │   └─ storage.scan_nodes(graph, labels)
                        │                   │       └─ File: src/storage/storage_manager.rs
                        │                   │       ├─ Check cache first
                        │                   │       └─ Load from persistent store
                        │                   │           └─ File: src/storage/persistent/sled.rs
                        │                   │
                        │                   ├─ PhysicalNode::Filter { condition, input, ... } →
                        │                   │   ├─ execute_physical_plan(input) (recursive)
                        │                   │   └─ Evaluate condition for each row
                        │                   │       └─ evaluate_expression(condition)
                        │                   │           └─ Uses: src/functions/mod.rs
                        │                   │
                        │                   ├─ PhysicalNode::IndexedExpand { from_var, edge_var, to_var, ... } →
                        │                   │   └─ storage.get_neighbors(node_id, direction, labels)
                        │                   │       └─ File: src/storage/storage_manager.rs
                        │                   │
                        │                   └─ PhysicalNode::Project { expressions, input, ... } →
                        │                       ├─ execute_physical_plan(input) (recursive)
                        │                       └─ Project specified columns
                        │                           └─ evaluate_expression() for each column
                        │
                        └─ [RETURN]
                            └─ QueryResult {
                                   rows: Vec<Row>,
                                   variables: Vec<String>,
                                   execution_time_ms: u64,
                                   ...
                               }
                            └─ File: src/exec/result.rs
```

**File Interaction Summary**:

| Step | File | Purpose |
|------|------|---------|
| 1 | coordinator/query_coordinator.rs:145 | Entry point |
| 2 | ast/parser.rs | Parse GQL → AST |
| 3 | session/manager.rs | Get user session |
| 4 | exec/executor.rs:144 | Main execution |
| 5 | exec/executor.rs:235 | Resolve graph context |
| 6 | storage/storage_manager.rs:153 | Get graph from storage |
| 7 | exec/context.rs | Create execution context |
| 8 | plan/logical.rs | Generate logical plan |
| 9 | plan/optimizer.rs | Optimize logical plan |
| 10 | plan/physical.rs | Generate physical plan |
| 11 | exec/executor.rs (methods) | Execute physical operators |
| 12 | storage/storage_manager.rs | Access graph data |
| 13 | storage/persistent/sled.rs | Read from Sled |
| 14 | functions/mod.rs | Evaluate functions |
| 15 | exec/result.rs | Build query result |

---

### Operation 2: Write Operation (INSERT)

**Example Query**: `INSERT (:Person {name: 'Bob', age: 25})`

**Complete Call Stack**:

```
[User Application]
    │
    └─ QueryCoordinator::process_query(query_text, session_id)
        └─ File: src/coordinator/query_coordinator.rs:145
        │
        ├─ parse_query(query_text)
        │   └─ Returns: Document { statement: DataStatement(Insert(...)) }
        │
        └─ executor.execute_query(request)
            └─ File: src/exec/executor.rs:144
            │
            └─ route_and_execute()
                │
                └─ match Statement::DataStatement(stmt) →
                    │
                    └─ [DELEGATE TO DATA STATEMENT EXECUTOR]
                        └─ Coordinator::execute_data_statement(stmt)
                            └─ File: src/exec/write_stmt/data_stmt/coordinator.rs
                            │
                            └─ match stmt:
                                │
                                └─ DataStatement::Insert(insert_stmt) →
                                    │
                                    └─ execute_insert()
                                        └─ File: src/exec/write_stmt/data_stmt/insert.rs
                                        │
                                        ├─ [STEP 1: Parse Patterns]
                                        │   └─ Parse (:Person {name: 'Bob', age: 25})
                                        │   └─ Extract labels: ["Person"]
                                        │   └─ Extract properties: {name: "Bob", age: 25}
                                        │
                                        ├─ [STEP 2: Generate IDs]
                                        │   └─ node_id = uuid::Uuid::new_v4().to_string()
                                        │
                                        ├─ [STEP 3: Validate Schema]
                                        │   └─ If graph has schema:
                                        │       └─ IngestionValidator::validate_node()
                                        │           └─ File: src/schema/integration/ingestion_validator.rs
                                        │           ├─ Check labels match schema
                                        │           ├─ Check properties match types
                                        │           └─ Check constraints (NOT NULL, UNIQUE)
                                        │
                                        ├─ [STEP 4: Check Transaction]
                                        │   └─ transaction_manager.get_current_transaction()
                                        │       └─ File: src/txn/manager.rs
                                        │       └─ If no active txn, auto-begin
                                        │
                                        ├─ [STEP 5: Log to WAL]
                                        │   └─ transaction_manager.log_operation(
                                        │          txn_id,
                                        │          UndoOperation::InsertNode { ... }
                                        │       )
                                        │       └─ File: src/txn/manager.rs
                                        │       └─ Calls: wal.append(log_entry)
                                        │           └─ File: src/txn/wal.rs
                                        │
                                        ├─ [STEP 6: Insert to Storage]
                                        │   └─ storage.insert_node(graph_name, node)
                                        │       └─ File: src/storage/storage_manager.rs
                                        │       │
                                        │       ├─ Update cache
                                        │       │   └─ cache.add_node(node)
                                        │       │       └─ File: src/storage/graph_cache.rs
                                        │       │
                                        │       └─ Persist to disk
                                        │           └─ persistent_store.insert_node(driver, graph, node)
                                        │               └─ File: src/storage/data_adapter.rs
                                        │               └─ Calls: driver.put(tree, key, value)
                                        │                   └─ File: src/storage/persistent/sled.rs
                                        │
                                        └─ [STEP 7: Invalidate Caches]
                                            └─ cache_manager.invalidate_on_write(graph, labels)
                                                └─ File: src/cache/invalidation.rs
                                                ├─ Clear result caches
                                                └─ Clear subquery caches
```

**File Interaction Summary**:

| Step | File | Purpose |
|------|------|---------|
| 1 | coordinator/query_coordinator.rs:145 | Entry point |
| 2 | ast/parser.rs | Parse INSERT statement |
| 3 | exec/executor.rs:144 | Main execution |
| 4 | exec/write_stmt/data_stmt/coordinator.rs | Route to INSERT handler |
| 5 | exec/write_stmt/data_stmt/insert.rs | Execute INSERT |
| 6 | schema/integration/ingestion_validator.rs | Validate against schema |
| 7 | txn/manager.rs | Transaction control |
| 8 | txn/wal.rs | Write-ahead logging |
| 9 | storage/storage_manager.rs | Storage orchestration |
| 10 | storage/graph_cache.rs | Update cache |
| 11 | storage/data_adapter.rs | Persistent storage adapter |
| 12 | storage/persistent/sled.rs | Sled backend |
| 13 | cache/invalidation.rs | Cache invalidation |

---

### Operation 3: Transaction Lifecycle

**Example**: `BEGIN; INSERT ...; COMMIT;`

**Call Stack**:

```
[BEGIN TRANSACTION]
    │
    └─ executor.execute_query(request)
        └─ Statement::TransactionStatement(Begin) →
            └─ Coordinator::execute_transaction_statement()
                └─ File: src/exec/write_stmt/transaction/coordinator.rs
                │
                └─ execute_start_transaction()
                    └─ File: src/exec/write_stmt/transaction/start.rs
                    │
                    ├─ Allocate transaction ID
                    │   └─ transaction_manager.begin(session_id, isolation, access_mode)
                    │       └─ File: src/txn/manager.rs
                    │       ├─ txn_id = next_txn_id.fetch_add(1)
                    │       ├─ Create TransactionLog
                    │       └─ Write BEGIN to WAL
                    │
                    └─ Update session state
                        └─ session.transaction_state = Active(txn_id)
                            └─ File: src/session/transaction_state.rs

[INSERT (within transaction)]
    │
    └─ ... (see Operation 2)
        ├─ Uses txn_id from session
        └─ Logs operations to transaction_manager

[COMMIT]
    │
    └─ executor.execute_query(request)
        └─ Statement::TransactionStatement(Commit) →
            └─ execute_commit()
                └─ File: src/exec/write_stmt/transaction/commit.rs
                │
                ├─ [STEP 1: Validate Transaction State]
                │   └─ Check transaction is Active
                │
                ├─ [STEP 2: Flush WAL]
                │   └─ wal.flush()
                │       └─ File: src/txn/wal.rs
                │       └─ Ensures all log entries on disk
                │
                ├─ [STEP 3: Apply Changes]
                │   └─ All writes already applied during INSERT
                │
                ├─ [STEP 4: Release Locks]
                │   └─ lock_tracker.release_all(txn_id)
                │       └─ File: src/exec/lock_tracker.rs
                │
                ├─ [STEP 5: Mark Committed]
                │   └─ transaction_manager.commit(txn_id)
                │       └─ File: src/txn/manager.rs
                │       ├─ Write COMMIT to WAL
                │       ├─ Remove from active_transactions
                │       └─ Update session.transaction_state = Committed
                │
                └─ [STEP 6: Invalidate Caches]
                    └─ cache_manager.invalidate_affected_caches()
                        └─ File: src/cache/invalidation.rs
```

**File Interaction Summary**:

| Operation | File | Purpose |
|-----------|------|---------|
| BEGIN | exec/write_stmt/transaction/start.rs | Start transaction |
| BEGIN | txn/manager.rs | Allocate txn_id, log BEGIN |
| BEGIN | session/transaction_state.rs | Update session state |
| COMMIT | exec/write_stmt/transaction/commit.rs | Commit transaction |
| COMMIT | txn/wal.rs | Flush WAL to disk |
| COMMIT | exec/lock_tracker.rs | Release locks |
| COMMIT | txn/manager.rs | Mark committed |
| ROLLBACK | exec/write_stmt/transaction/rollback.rs | Rollback transaction |
| ROLLBACK | txn/log.rs | Apply undo operations |

---

### Operation 4: Session Management

**Example**: `SET SESSION GRAPH /schema_name/graph_name`

**Call Stack**:

```
[SET SESSION GRAPH]
    │
    └─ QueryCoordinator::process_query(query_text, session_id)
        └─ File: src/coordinator/query_coordinator.rs:145
        │
        ├─ parse_query("SET SESSION GRAPH /schema/graph")
        │   └─ Returns: SessionStatement(SetGraph(...))
        │
        └─ executor.execute_query(request)
            └─ File: src/exec/executor.rs:144
            │
            └─ route_and_execute()
                └─ Statement::SessionStatement(stmt) →
                    │
                    └─ Execute session statement
                        └─ Returns: QueryResult {
                               session_result: Some(SessionResult::SetGraph { ... })
                           }
                        └─ File: src/exec/result.rs
                    │
                    └─ [Back in QueryCoordinator]
                        └─ handle_session_result(session_result, session_id)
                            └─ File: src/coordinator/query_coordinator.rs:172-176
                            │
                            ├─ Get session from session_manager
                            │   └─ session_manager.get_session(session_id)
                            │       └─ File: src/session/manager.rs
                            │
                            └─ Update session.current_graph
                                └─ session.current_graph = Some(graph_path)
                                    └─ File: src/session/models.rs
```

**File Interaction Summary**:

| Step | File | Purpose |
|------|------|---------|
| 1 | coordinator/query_coordinator.rs:145 | Entry point |
| 2 | ast/parser.rs | Parse SET SESSION statement |
| 3 | exec/executor.rs:144 | Execute statement |
| 4 | exec/result.rs | Return SessionResult |
| 5 | coordinator/query_coordinator.rs:172 | Handle session result |
| 6 | session/manager.rs | Get session |
| 7 | session/models.rs | Update UserSession.current_graph |

---

### Operation 5: DDL Execution (CREATE GRAPH)

**Example**: `CREATE GRAPH /schema_name/graph_name`

**Call Stack**:

```
[CREATE GRAPH]
    │
    └─ executor.execute_query(request)
        └─ Statement::CatalogStatement(stmt) →
            │
            └─ Coordinator::execute_catalog_statement()
                └─ File: src/exec/write_stmt/ddl_stmt/coordinator.rs
                │
                └─ match stmt:
                    │
                    └─ CatalogStatement::CreateGraph { name, schema } →
                        │
                        └─ execute_create_graph()
                            └─ File: src/exec/write_stmt/ddl_stmt/create_graph.rs
                            │
                            ├─ [STEP 1: Validate Graph Name]
                            │   └─ Parse catalog path: /schema_name/graph_name
                            │
                            ├─ [STEP 2: Create in Catalog]
                            │   └─ catalog_manager.execute_operation(
                            │          CatalogOperation::CreateGraph { name, schema }
                            │       )
                            │       └─ File: src/catalog/manager.rs
                            │       │
                            │       └─ graph_metadata_provider.create_graph(name, schema)
                            │           └─ File: src/catalog/providers/graph_metadata.rs
                            │           ├─ Create GraphMetadata entry
                            │           └─ Store in catalog storage
                            │               └─ File: src/catalog/storage/mod.rs
                            │
                            └─ [STEP 3: Initialize Storage]
                                └─ storage.create_graph(name)
                                    └─ File: src/storage/storage_manager.rs
                                    ├─ Create empty GraphCache
                                    │   └─ File: src/storage/graph_cache.rs
                                    └─ Persist to disk
                                        └─ File: src/storage/persistent/sled.rs
```

**File Interaction Summary**:

| Step | File | Purpose |
|------|------|---------|
| 1 | exec/write_stmt/ddl_stmt/coordinator.rs | Route to DDL handler |
| 2 | exec/write_stmt/ddl_stmt/create_graph.rs | Execute CREATE GRAPH |
| 3 | catalog/manager.rs | Catalog operation |
| 4 | catalog/providers/graph_metadata.rs | Create metadata |
| 5 | catalog/storage/mod.rs | Persist metadata |
| 6 | storage/storage_manager.rs | Initialize graph storage |
| 7 | storage/graph_cache.rs | Create cache entry |
| 8 | storage/persistent/sled.rs | Persist to disk |

---

## Data Flow Diagrams

### Query Execution Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Query Execution Flow                     │
└─────────────────────────────────────────────────────────────────┘

Query Text (String)
    │
    ▼
┌──────────────────────────┐
│ coordinator/             │
│ query_coordinator.rs:147 │  parse_query()
└───────────┬──────────────┘
            │
            ▼
┌──────────────────────────┐
│ ast/parser.rs            │  Parser (nom combinators)
│ ast/ast.rs               │  AST structures
└───────────┬──────────────┘
            │
            ▼
Document { statement: Statement }
    │
    ▼
┌──────────────────────────┐
│ exec/executor.rs:144     │  execute_query()
└───────────┬──────────────┘
            │
    ┌───────┴───────┐
    │               │
    ▼               ▼
[Read Query]    [Write Statement]
    │               │
    ▼               ▼
┌──────────────┐  ┌──────────────────────┐
│ plan/        │  │ exec/write_stmt/     │
│ logical.rs   │  │ data_stmt/*.rs       │
│ optimizer.rs │  │ ddl_stmt/*.rs        │
│ physical.rs  │  │ transaction/*.rs     │
└──────┬───────┘  └───────┬──────────────┘
       │                  │
       ▼                  ▼
  PhysicalPlan      Direct Execution
       │                  │
       └────────┬─────────┘
                │
                ▼
┌─────────────────────────────────┐
│ exec/executor.rs                │  execute_physical_plan()
│ - NodeSeqScan                   │  or direct operation
│ - Filter                        │
│ - Expand                        │
│ - Project                       │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ storage/storage_manager.rs      │  Data access
│  ├─ cache (multi_graph.rs)      │  L1: Cache
│  ├─ persistent (data_adapter.rs)│  L2: Sled
│  └─ indexes (manager.rs)        │  Indexes
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ storage/persistent/sled.rs      │  Sled B-tree
└──────────────┬──────────────────┘
               │
               ▼
QueryResult { rows, variables, ... }
    │
    ▼
┌─────────────────────────────────┐
│ exec/result.rs                  │  Result structure
└─────────────────────────────────┘
```

---

### Transaction Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Transaction Flow                         │
└─────────────────────────────────────────────────────────────────┘

BEGIN TRANSACTION
    │
    ▼
┌──────────────────────────────────┐
│ exec/write_stmt/transaction/     │
│ start.rs                         │
└───────────┬──────────────────────┘
            │
            ▼
┌──────────────────────────────────┐
│ txn/manager.rs                   │  Allocate txn_id
│  ├─ Create TransactionLog        │
│  └─ active_transactions[txn_id]  │
└───────────┬──────────────────────┘
            │
            ▼
┌──────────────────────────────────┐
│ txn/wal.rs                       │  Write BEGIN to WAL
└───────────┬──────────────────────┘
            │
            ▼
┌──────────────────────────────────┐
│ session/transaction_state.rs     │  Update session state
│  transaction_state = Active(id)  │
└──────────────────────────────────┘

[Operations: INSERT, UPDATE, DELETE]
    │
    ▼
┌──────────────────────────────────┐
│ txn/manager.rs                   │  log_operation()
│  ├─ Add to TransactionLog        │
│  └─ UndoOperation                │
└───────────┬──────────────────────┘
            │
            ▼
┌──────────────────────────────────┐
│ txn/wal.rs                       │  Append to WAL
└───────────┬──────────────────────┘
            │
            ▼
┌──────────────────────────────────┐
│ storage/storage_manager.rs       │  Apply changes
└──────────────────────────────────┘

COMMIT or ROLLBACK
    │
    ├─ COMMIT ─────────────────┐
    │   │                      │
    │   ▼                      │
    │ ┌─────────────────────┐  │
    │ │ txn/wal.rs          │  │  Flush WAL
    │ └──────────┬──────────┘  │
    │            │             │
    │            ▼             │
    │ ┌─────────────────────┐  │
    │ │ exec/lock_tracker.rs│  │  Release locks
    │ └──────────┬──────────┘  │
    │            │             │
    │            ▼             │
    │ ┌─────────────────────┐  │
    │ │ txn/manager.rs      │  │  Mark committed
    │ │ state = Committed   │  │
    │ └─────────────────────┘  │
    │                          │
    └─ ROLLBACK ───────────────┤
        │                      │
        ▼                      │
      ┌─────────────────────┐  │
      │ txn/log.rs          │  │  Get undo ops
      └──────────┬──────────┘  │
                 │             │
                 ▼             │
      ┌─────────────────────┐  │
      │ Apply undo ops      │  │  Reverse changes
      │ (in reverse order)  │  │
      └──────────┬──────────┘  │
                 │             │
                 ▼             │
      ┌─────────────────────┐  │
      │ txn/manager.rs      │  │  Mark rolled back
      │ state = RolledBack  │  │
      └─────────────────────┘  │
                               │
                               ▼
                    ┌──────────────────────┐
                    │ cache/invalidation.rs│  Invalidate caches
                    └──────────────────────┘
```

---

## Integration Points

### How Modules Integrate

#### 1. Coordinator ↔ Executor

**Integration Point**: `coordinator/query_coordinator.rs:158-161`

```rust
let result = self.executor.execute_query(request)
```

**Purpose**: Coordinator delegates all execution to QueryExecutor.

**Data Flow**: ExecutionRequest → QueryResult

---

#### 2. Executor ↔ Storage

**Integration Point**: `exec/executor.rs` (throughout)

**Key Access Points**:
```rust
self.storage.get_graph(graph_name)              // Get graph
self.storage.scan_nodes(graph, labels)          // Scan nodes
self.storage.get_neighbors(node_id, direction)  // Traverse
self.storage.insert_node(graph, node)           // Insert
```

**Purpose**: Executor accesses all graph data through StorageManager.

---

#### 3. Executor ↔ Transaction Manager

**Integration Point**: `exec/executor.rs` (field)

```rust
pub struct QueryExecutor {
    transaction_manager: Arc<TransactionManager>,
    current_transaction: Arc<RwLock<Option<TransactionId>>>,
    // ...
}
```

**Purpose**: Executor manages transactions for ACID compliance.

**Key Operations**:
- Begin/Commit/Rollback via `exec/write_stmt/transaction/*.rs`
- Log operations via `transaction_manager.log_operation()`

---

#### 4. Storage ↔ Persistent Backend

**Integration Point**: `storage/storage_manager.rs:184-200`

```rust
persistent_store.load_graph_by_path(driver, name)
```

**Architecture**:
```
StorageManager
    └─ DataAdapter (stateless)
        └─ StorageDriver trait
            └─ SledStorage (implementation)
```

**Files**:
- `storage/data_adapter.rs` - Adapter layer
- `storage/persistent/traits.rs` - StorageDriver trait
- `storage/persistent/sled.rs` - Sled implementation

---

#### 5. Executor ↔ Functions

**Integration Point**: `exec/executor.rs` (field)

```rust
pub struct QueryExecutor {
    function_registry: Arc<FunctionRegistry>,
    // ...
}
```

**Usage**: When evaluating expressions containing function calls.

**Example**:
```rust
// In execute_physical_plan() when evaluating:
MATCH (p:Person) WHERE UPPER(p.name) = 'ALICE'

// Calls:
function_registry.get("UPPER")
    .execute(vec![Value::String(p.name)])
```

**Files**:
- `functions/mod.rs` - FunctionRegistry
- `functions/*_functions.rs` - Function implementations

---

#### 6. Executor ↔ Types

**Integration Point**: `exec/executor.rs` (fields)

```rust
pub struct QueryExecutor {
    type_inference: TypeInference,
    type_validator: TypeValidator,
    type_coercion: TypeCoercion,
    type_caster: TypeCaster,
}
```

**Purpose**: Type checking, inference, and coercion during execution.

**Files**:
- `types/inference.rs` - Infer expression types
- `types/validation.rs` - Validate type compatibility
- `types/coercion.rs` - Automatic type conversion
- `types/casting.rs` - CAST expressions

---

#### 7. Executor ↔ Catalog

**Integration Point**: `exec/executor.rs` (field)

```rust
pub struct QueryExecutor {
    catalog_manager: Arc<RwLock<CatalogManager>>,
    // ...
}
```

**Purpose**: Access metadata (graphs, schemas, users, roles).

**Key Operations**:
- CREATE/DROP GRAPH
- CREATE/DROP SCHEMA
- CREATE/DROP USER
- GRANT/REVOKE ROLE

**Files**:
- `catalog/manager.rs` - CatalogManager
- `catalog/providers/*.rs` - Metadata providers

---

#### 8. Executor ↔ Schema

**Integration Point**: Via catalog and direct validation

**Schema Validation Points**:

1. **At INSERT** (`exec/write_stmt/data_stmt/insert.rs`):
   ```rust
   IngestionValidator::validate_node(node, graph_type)
   ```
   └─ File: `schema/integration/ingestion_validator.rs`

2. **At Runtime** (during execution):
   ```rust
   RuntimeValidator::validate(...)
   ```
   └─ File: `schema/integration/runtime_validator.rs`

3. **At Index Creation**:
   ```rust
   IndexValidator::validate(...)
   ```
   └─ File: `schema/integration/index_validator.rs`

---

#### 9. Cache Integration

**Integration Point**: Throughout execution

**Cache Lookup Flow**:
```
execute_query()
    ├─ Check plan_cache for compiled plan
    ├─ Check result_cache for cached result
    └─ Execute if cache miss
        └─ Store result in caches
```

**Files**:
- `cache/cache_manager.rs` - Orchestrator
- `cache/plan_cache.rs` - Plan caching
- `cache/result_cache.rs` - Result caching
- `cache/invalidation.rs` - Cache invalidation

---

## Summary

### File Count by Module

| Module | Files | Purpose |
|--------|-------|---------|
| **ast** | 6 | Parsing & AST |
| **plan** | 9 | Query planning & optimization |
| **exec** | 42 | Query execution (largest) |
| **storage** | 18 | Data storage & persistence |
| **catalog** | 11 | Metadata management |
| **txn** | 7 | Transaction management |
| **session** | 4 | Session management |
| **schema** | 18 | Schema (Graph Type) support |
| **cache** | 7 | Caching system |
| **functions** | 12 | Built-in functions |
| **types** | 5 | Type system |
| **coordinator** | 2 | Query coordination |
| **lib.rs** | 1 | Public API |
| **Total** | **163** | |

### Key Takeaways

1. **Single Entry Point**: All operations flow through `QueryCoordinator` (lib.rs:37)
2. **Layered Architecture**: Clear separation between parsing, planning, execution, and storage
3. **Extensive Delegation**: Executor delegates to specialized modules (storage, txn, catalog, schema)
4. **Write Path Complexity**: Write operations involve multiple validators and transaction logging
5. **Cache-Aside Pattern**: Storage uses cache-aside with multi-tier fallback
6. **Transaction Integration**: All write operations integrated with transaction manager
7. **Schema Enforcement**: Multiple validation points (ingestion, runtime, index)

### Critical Files for Understanding

If you only read 10 files, read these:

1. **lib.rs** - Public API
2. **coordinator/query_coordinator.rs** - Main orchestrator
3. **exec/executor.rs** - Execution engine
4. **storage/storage_manager.rs** - Storage orchestration
5. **ast/parser.rs** - Query parsing
6. **plan/physical.rs** - Physical plan execution
7. **txn/manager.rs** - Transaction management
8. **session/manager.rs** - Session management
9. **catalog/manager.rs** - Metadata management
10. **storage/persistent/sled.rs** - Persistent backend

---

**End of Updated Architecture Documentation**
