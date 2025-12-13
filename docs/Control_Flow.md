# GraphLite Control Flow Documentation

> **Complete guide to all operations and their precise control flows through architectural layers**
>
> **Source**: Direct analysis of `graphlite/src/` codebase
>
> **Last Updated**: 2025-01-26

---

## Table of Contents

1. [Main Operations Overview](#main-operations-overview)
2. [Architectural Layers](#architectural-layers)
3. [Operation 1: QUERY (MATCH...RETURN)](#operation-1-query-matchreturn)
4. [Operation 2: INSERT (Data Modification)](#operation-2-insert-data-modification)
5. [Operation 3: Transaction Control](#operation-3-transaction-control)
6. [Operation 4: Session Management](#operation-4-session-management)
5. [Operation 5: DDL (CREATE GRAPH)](#operation-5-ddl-create-graph)
6. [Operation 6: SELECT](#operation-6-select-sql-style)
7. [Operation 7: CALL](#operation-7-call-procedure)
8. [Complete Operations Summary Table](#complete-operations-summary-table)
9. [Key Architectural Patterns](#key-architectural-patterns)

---

## Main Operations Overview

GraphLite supports **13 statement types** (from `ast/ast.rs:25-38`):

### **Category 1: Query Operations (Read)**

| Operation | Statement | Example | Planning Required |
|-----------|-----------|---------|-------------------|
| **Query** | `Query(...)` | `MATCH (n:Person) RETURN n` | ✅ Yes |
| **Select** | `Select(...)` | `SELECT * FROM graph MATCH (n)` | ✅ Yes |
| **Call** | `Call(...)` | `CALL gql.list_graphs()` | ❌ No |

### **Category 2: Data Modification (Write)**

From `ast/ast.rs:882-891`:

```rust
pub enum DataStatement {
    Insert(InsertStatement),              // INSERT (:Person {name: 'Alice'})
    MatchInsert(MatchInsertStatement),    // MATCH ... INSERT ...
    Set(SetStatement),                    // SET properties
    MatchSet(MatchSetStatement),          // MATCH ... SET ...
    Remove(RemoveStatement),              // REMOVE properties
    MatchRemove(MatchRemoveStatement),    // MATCH ... REMOVE ...
    Delete(DeleteStatement),              // DELETE entities
    MatchDelete(MatchDeleteStatement),    // MATCH ... DELETE ...
}
```

### **Category 3: Schema/Metadata (DDL)**

From `ast/ast.rs:626-644`:

```rust
pub enum CatalogStatement {
    CreateSchema,      // CREATE SCHEMA /myschema
    DropSchema,        // DROP SCHEMA /myschema
    CreateGraph,       // CREATE GRAPH /schema/graph
    DropGraph,         // DROP GRAPH /schema/graph
    TruncateGraph,     // TRUNCATE GRAPH /schema/graph
    ClearGraph,        // CLEAR GRAPH /schema/graph
    CreateGraphType,   // CREATE GRAPH TYPE ...
    DropGraphType,     // DROP GRAPH TYPE ...
    AlterGraphType,    // ALTER GRAPH TYPE ...
    CreateUser,        // CREATE USER alice WITH PASSWORD 'secret'
    DropUser,          // DROP USER alice
    CreateRole,        // CREATE ROLE admin
    DropRole,          // DROP ROLE admin
    GrantRole,         // GRANT ROLE admin TO alice
    RevokeRole,        // REVOKE ROLE admin FROM alice
    CreateProcedure,   // CREATE PROCEDURE ... (future)
    DropProcedure,     // DROP PROCEDURE ... (future)
}
```

### **Category 4: Session Management**

From `ast/ast.rs:993-997`:

```rust
pub enum SessionStatement {
    Set(SessionSetStatement),      // SESSION SET GRAPH /schema/graph
    Reset(SessionResetStatement),  // SESSION RESET GRAPH
    Close(SessionCloseStatement),  // SESSION CLOSE
}
```

### **Category 5: Transaction Control**

From `ast/ast.rs:1622-1627`:

```rust
pub enum TransactionStatement {
    StartTransaction,                    // BEGIN TRANSACTION
    Commit,                              // COMMIT
    Rollback,                            // ROLLBACK
    SetTransactionCharacteristics,       // SET TRANSACTION ISOLATION LEVEL ...
}
```

---

## Architectural Layers

GraphLite has **7 main architectural layers** plus **4 cross-cutting concerns**:

### **Main Layers**

```
┌─────────────────────────────────────────────────────────────────┐
│ Layer 7: USER INTERFACE                                        │
│   • CLI (graphlite-cli)                                        │
│   • REPL Console                                               │
│   • Application embedding                                      │
├─────────────────────────────────────────────────────────────────┤
│ Layer 6: QUERY COORDINATOR                                     │
│   • Entry point for all operations                            │
│   • File: coordinator/query_coordinator.rs                     │
├─────────────────────────────────────────────────────────────────┤
│ Layer 5: SESSION MANAGER                                       │
│   • User sessions and authentication                           │
│   • File: session/manager.rs                                   │
├─────────────────────────────────────────────────────────────────┤
│ Layer 4: QUERY EXECUTOR                                        │
│   • Statement routing and execution                            │
│   • File: exec/executor.rs                                     │
├─────────────────────────────────────────────────────────────────┤
│ Layer 3: PLANNER (Optional - Read Queries Only)               │
│   • Logical Planning: plan/logical.rs                          │
│   • Optimization: plan/optimizer.rs                            │
│   • Physical Planning: plan/physical.rs                        │
├─────────────────────────────────────────────────────────────────┤
│ Layer 2: STORAGE MANAGER                                       │
│   • Multi-tier storage orchestration                           │
│   • File: storage/storage_manager.rs                           │
├─────────────────────────────────────────────────────────────────┤
│ Layer 1: PERSISTENT BACKEND                                    │
│   • Sled embedded database                                     │
│   • File: storage/persistent/sled.rs                           │
└─────────────────────────────────────────────────────────────────┘
```

### **Cross-Cutting Concerns**

```
┌─────────────────────────────────────────────────────────────────┐
│ TRANSACTION MANAGER (Integrates with Layers 4 & 2)            │
│   • File: txn/manager.rs                                       │
│   • Provides: ACID guarantees, WAL, transaction logs           │
├─────────────────────────────────────────────────────────────────┤
│ CATALOG MANAGER (Integrates with Layer 4)                      │
│   • File: catalog/manager.rs                                   │
│   • Provides: Metadata (graphs, schemas, users, roles)         │
├─────────────────────────────────────────────────────────────────┤
│ CACHE MANAGER (Integrates with Layer 2)                        │
│   • File: cache/cache_manager.rs                               │
│   • Provides: Multi-level caching (L1/L2/L3)                   │
├─────────────────────────────────────────────────────────────────┤
│ FUNCTION REGISTRY (Integrates with Layer 4)                    │
│   • File: functions/mod.rs                                     │
│   • Provides: 60+ built-in functions                           │
└─────────────────────────────────────────────────────────────────┘
```

---

## Operation 1: QUERY (MATCH...RETURN)

### **Example Query**
```gql
MATCH (p:Person {name: 'Alice'})
RETURN p.name, p.age
```

### **Complete Control Flow Diagram**

```
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 7: USER INTERFACE                                        │
├─────────────────────────────────────────────────────────────────┤
│ graphlite-cli/src/cli/gqlcli.rs                                │
│   • handle_query() - One-shot execution                        │
│   • handle_gql() - REPL console                                │
└────────────────────────┬────────────────────────────────────────┘
                         │ query_text: "MATCH (p:Person ...",
                         │ session_id: "uuid-string"
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 6: QUERY COORDINATOR                                     │
├─────────────────────────────────────────────────────────────────┤
│ QueryCoordinator::process_query(query_text, session_id)        │
│ File: coordinator/query_coordinator.rs:145                     │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 1: PARSE QUERY                                         │ │
│ │ parse_query(query_text) → Document                          │ │
│ │ └─ Calls: ast/parser.rs::parse_query()                      │ │
│ │ └─ Returns: Document {                                      │ │
│ │      statement: Statement::Query(                            │ │
│ │        Query::Basic(BasicQuery {                             │ │
│ │          match_clauses: [...],                               │ │
│ │          return_clause: Some(...)                            │ │
│ │        })                                                    │ │
│ │      )                                                       │ │
│ │    }                                                         │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 2: GET SESSION                                         │ │
│ │ session_manager.get_session(session_id)                     │ │
│ │ └─ File: session/manager.rs                                 │ │
│ │ └─ Returns: Arc<RwLock<UserSession>>                        │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 3: CREATE EXECUTION REQUEST                            │ │
│ │ ExecutionRequest::new(statement)                            │ │
│ │   .with_session(session)                                    │ │
│ │   .with_query_text(Some(query_text))                        │ │
│ │ └─ File: exec/executor.rs:58-99                             │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 4: EXECUTE                                             │ │
│ │ executor.execute_query(request)                             │ │
│ │ └─ Delegates to Layer 4                                     │ │
│ └─────────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────┘
                         │ ExecutionRequest
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 5: SESSION MANAGER (Parallel Context Lookup)             │
├─────────────────────────────────────────────────────────────────┤
│ SessionManager::get_session(session_id)                        │
│ File: session/manager.rs                                       │
│                                                                 │
│ Returns: UserSession {                                         │
│   session_id: String,                                          │
│   username: String,                                            │
│   roles: Vec<String>,                                          │
│   current_graph: Option<String>, ← CRITICAL: Used below        │
│   current_schema: Option<String>,                              │
│   permissions: SessionPermissionCache,                         │
│   ...                                                          │
│ }                                                              │
└────────────────────────┬────────────────────────────────────────┘
                         │ Session provides graph context
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 4: QUERY EXECUTOR                                        │
├─────────────────────────────────────────────────────────────────┤
│ QueryExecutor::execute_query(request)                          │
│ File: exec/executor.rs:144                                     │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ PHASE 1: RESOLVE GRAPH CONTEXT                              │ │
│ │ resolve_graph_for_execution(request)                        │ │
│ │ File: exec/executor.rs:235-265                              │ │
│ │                                                              │ │
│ │ Graph Resolution Priority:                                  │ │
│ │ 1. Explicit FROM clause in query                            │ │
│ │    └─ Example: MATCH (n) FROM /schema/graph                 │ │
│ │ 2. Session.current_graph                                    │ │
│ │    └─ Example: SESSION SET GRAPH /schema/graph              │ │
│ │ 3. Error if needed but not available                        │ │
│ │                                                              │ │
│ │ Calls: storage.get_graph(graph_path)                        │ │
│ │ └─ File: storage/storage_manager.rs:153                     │ │
│ │ └─ Returns: Arc<GraphCache>                                 │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ PHASE 2: CREATE EXECUTION CONTEXT                           │ │
│ │ create_execution_context_from_session(session)              │ │
│ │ File: exec/executor.rs:268-286                              │ │
│ │                                                              │ │
│ │ Creates: ExecutionContext {                                 │ │
│ │   session_id: String,                                       │ │
│ │   bindings: HashMap<String, Vec<Value>>,                    │ │
│ │   current_graph: Some(graph),                               │ │
│ │   function_registry: Arc<FunctionRegistry>,                 │ │
│ │   ...                                                        │ │
│ │ }                                                            │ │
│ │ File: exec/context.rs                                       │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ PHASE 3: ROUTE STATEMENT                                    │ │
│ │ route_and_execute(request, context, graph)                  │ │
│ │ File: exec/executor.rs:289                                  │ │
│ │                                                              │ │
│ │ match &request.statement {                                  │ │
│ │   Statement::Query(query) => {                              │ │
│ │     execute_read_query(query, context, graph)               │ │
│ │   }                                                          │ │
│ │   // ... other statement types                              │ │
│ │ }                                                            │ │
│ └─────────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────┘
                         │ Statement::Query
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 3: PLANNER                                               │
├─────────────────────────────────────────────────────────────────┤
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3.1: LOGICAL PLANNING                                 ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ LogicalPlanner::plan(query)                                    │
│ File: plan/logical.rs                                          │
│                                                                 │
│ Input: Query AST (from parser)                                 │
│                                                                 │
│ Transforms to: LogicalPlan {                                   │
│   root: LogicalNode,                                           │
│   variables: HashMap<String, VariableInfo>                     │
│ }                                                              │
│                                                                 │
│ Example Logical Tree for our query:                            │
│                                                                 │
│   LogicalNode::NodeScan {                                      │
│     variable: "p",                                             │
│     labels: ["Person"],                                        │
│     properties: Some({name: Literal("Alice")})                 │
│   }                                                            │
│     ↓                                                          │
│   LogicalNode::Filter {                                        │
│     condition: BinaryOp(PropertyAccess(p.name), Eq, "Alice"),  │
│     input: Box<NodeScan>                                       │
│   }                                                            │
│     ↓                                                          │
│   LogicalNode::Project {                                       │
│     expressions: [                                             │
│       PropertyAccess { object: p, property: "name" },          │
│       PropertyAccess { object: p, property: "age" }            │
│     ],                                                         │
│     input: Box<Filter>                                         │
│   }                                                            │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3.2: LOGICAL OPTIMIZATION                             ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ LogicalOptimizer::optimize(logical_plan)                       │
│ File: plan/optimizer.rs                                        │
│                                                                 │
│ Optimization Rules Applied:                                    │
│                                                                 │
│ 1. PREDICATE PUSHDOWN                                          │
│    Move filters closer to data sources                         │
│    Before: Scan → Filter                                       │
│    After:  Scan(with filter predicate)                         │
│                                                                 │
│ 2. PROJECTION PRUNING                                          │
│    Only fetch needed properties                                │
│    Before: Fetch all properties                                │
│    After:  Fetch only p.name, p.age                            │
│                                                                 │
│ 3. CONSTANT FOLDING                                            │
│    Evaluate constants at compile-time                          │
│    Before: WHERE 2 + 3 > x                                     │
│    After:  WHERE 5 > x                                         │
│                                                                 │
│ 4. DEAD CODE ELIMINATION                                       │
│    Remove always-true/false conditions                         │
│    Before: Filter(true) → ...                                  │
│    After:  ... (filter removed)                                │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3.3: PHYSICAL PLANNING                                ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ PhysicalPlanner::plan(logical_plan, statistics)                │
│ File: plan/physical.rs                                         │
│                                                                 │
│ Output: PhysicalPlan {                                         │
│   root: PhysicalNode,                                          │
│   estimated_cost: f64,                                         │
│   estimated_rows: usize                                        │
│ }                                                              │
│                                                                 │
│ Physical Decisions Made:                                       │
│                                                                 │
│ Decision 1: Scan Method                                        │
│   LogicalNode::NodeScan → PhysicalNode::?                      │
│   Options:                                                     │
│   • NodeSeqScan (sequential scan)                              │
│     └─ Cost: O(n) where n = total nodes                        │
│   • NodeIndexScan (index-based)                                │
│     └─ Cost: O(log n + k) where k = matching nodes             │
│   Choice: NodeIndexScan (if index on Person.name exists)       │
│                                                                 │
│ Decision 2: Filter Selectivity                                 │
│   Estimate: 10% of Person nodes match name='Alice'             │
│   selectivity = 0.1                                            │
│                                                                 │
│ Decision 3: Project Cost                                       │
│   Extract 2 properties per row                                 │
│   estimated_cost = rows * 2 * PROPERTY_ACCESS_COST             │
│                                                                 │
│ Example Physical Plan:                                         │
│                                                                 │
│   PhysicalNode::NodeIndexScan {                                │
│     variable: "p",                                             │
│     labels: ["Person"],                                        │
│     properties: Some({name: "Alice"}),                         │
│     estimated_rows: 10,                                        │
│     estimated_cost: 15.0                                       │
│   }                                                            │
│     ↓                                                          │
│   PhysicalNode::Filter {                                       │
│     condition: p.name = 'Alice',                               │
│     selectivity: 0.1,                                          │
│     estimated_rows: 1,                                         │
│     estimated_cost: 20.0,                                      │
│     input: Box<NodeIndexScan>                                  │
│   }                                                            │
│     ↓                                                          │
│   PhysicalNode::Project {                                      │
│     expressions: [p.name, p.age],                              │
│     estimated_rows: 1,                                         │
│     estimated_cost: 25.0,                                      │
│     input: Box<Filter>                                         │
│   }                                                            │
└────────────────────────┬────────────────────────────────────────┘
                         │ PhysicalPlan
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 4: QUERY EXECUTOR (EXECUTION)                            │
├─────────────────────────────────────────────────────────────────┤
│ QueryExecutor::execute_physical_plan(plan, context)            │
│ File: exec/executor.rs (methods)                               │
│                                                                 │
│ Execution Strategy: Recursive Tree Walk (Volcano Model)        │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ EXECUTE: PhysicalNode::NodeIndexScan                        │ │
│ │                                                              │ │
│ │ match plan.root {                                           │ │
│ │   PhysicalNode::NodeIndexScan {                             │ │
│ │     variable, labels, properties, ...                       │ │
│ │   } => {                                                    │ │
│ │     // Call storage layer                                   │ │
│ │     nodes = storage.scan_nodes(graph, labels)?              │ │
│ │     └─ Goes to LAYER 2 ──────────────┐                      │ │
│ │                                      │                      │ │
│ │     // Bind to execution context     │                      │ │
│ │     context.bind(variable, nodes)    │                      │ │
│ │   }                                  │                      │ │
│ │ }                                    │                      │ │
│ └──────────────────────────────────────┼──────────────────────┘ │
│                                        │                        │
│ ┌──────────────────────────────────────┼──────────────────────┐ │
│ │ EXECUTE: PhysicalNode::Filter        │                      │ │
│ │                                      │                      │ │
│ │ match PhysicalNode::Filter {         │                      │ │
│ │   condition, input, ...              │                      │ │
│ │ } => {                               │                      │ │
│ │   // Execute input (recursive)       │                      │ │
│ │   rows = execute_physical_plan(input, context)?             │ │
│ │                                      │                      │ │
│ │   // Filter rows                     │                      │ │
│ │   filtered = rows.into_iter()        │                      │ │
│ │     .filter(|row| {                  │                      │ │
│ │       // Evaluate condition          │                      │ │
│ │       evaluate_expression(condition, row, context)          │ │
│ │       └─ Uses: functions/mod.rs (FunctionRegistry)          │ │
│ │     })                               │                      │ │
│ │     .collect()                       │                      │ │
│ │ }                                    │                      │ │
│ └──────────────────────────────────────┼──────────────────────┘ │
│                                        │                        │
│ ┌──────────────────────────────────────┼──────────────────────┐ │
│ │ EXECUTE: PhysicalNode::Project       │                      │ │
│ │                                      │                      │ │
│ │ match PhysicalNode::Project {        │                      │ │
│ │   expressions, input, ...            │                      │ │
│ │ } => {                               │                      │ │
│ │   // Execute input (recursive)       │                      │ │
│ │   rows = execute_physical_plan(input, context)?             │ │
│ │                                      │                      │ │
│ │   // Project columns                 │                      │ │
│ │   projected = rows.into_iter()       │                      │ │
│ │     .map(|row| {                     │                      │ │
│ │       Row {                          │                      │ │
│ │         values: expressions.iter()   │                      │ │
│ │           .map(|expr| {              │                      │ │
│ │             evaluate_expression(expr, row, context)         │ │
│ │           })                         │                      │ │
│ │           .collect()                 │                      │ │
│ │       }                              │                      │ │
│ │     })                               │                      │ │
│ │     .collect()                       │                      │ │
│ │ }                                    │                      │ │
│ └──────────────────────────────────────┼──────────────────────┘ │
│                                        │                        │
│ ┌──────────────────────────────────────┼──────────────────────┐ │
│ │ BUILD RESULT                         │                      │ │
│ │                                      │                      │ │
│ │ QueryResult {                        │                      │ │
│ │   rows: Vec<Row>,                    │                      │ │
│ │   variables: ["p.name", "p.age"],    │                      │ │
│ │   execution_time_ms: u64,            │                      │ │
│ │   rows_affected: 0,                  │                      │ │
│ │   warnings: Vec::new(),              │                      │ │
│ │ }                                    │                      │ │
│ │ File: exec/result.rs                 │                      │ │
│ └──────────────────────────────────────┼──────────────────────┘ │
└────────────────────────────────────────┼───────────────────────┘
                                         │
                         ┌───────────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 2: STORAGE MANAGER                                       │
├─────────────────────────────────────────────────────────────────┤
│ StorageManager::scan_nodes(graph_name, labels)                 │
│ File: storage/storage_manager.rs                               │
│                                                                 │
│ Multi-Tier Storage Access:                                     │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ L1: CHECK CACHE (Fast Path - Microseconds)                  │ │
│ │                                                              │ │
│ │ cache.get_nodes_by_label(label)                             │ │
│ │ File: storage/graph_cache.rs                                │ │
│ │                                                              │ │
│ │ GraphCache Structure:                                       │ │
│ │   nodes: HashMap<String, Node>,                             │ │
│ │   nodes_by_label: BTreeMap<String, Vec<String>>,            │ │
│ │   ─────────────────────────────────────                     │ │
│ │   Key: "Person"                                             │ │
│ │   Value: [node_id_1, node_id_2, ...]                        │ │
│ │                                                              │ │
│ │ If HIT:  Return nodes immediately ✓                         │ │
│ │ If MISS: Continue to L2 ↓                                   │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ L2: CHECK PERSISTENT STORE (Medium - Milliseconds)          │ │
│ │                                                              │ │
│ │ persistent_store.scan_nodes(driver, graph, labels)          │ │
│ │ File: storage/data_adapter.rs                               │ │
│ │                                                              │ │
│ │ Delegates to LAYER 1 ────────────┐                          │ │
│ └──────────────────────────────────┼──────────────────────────┘ │
│                                    │                            │
│ ┌──────────────────────────────────┼──────────────────────────┐ │
│ │ AFTER LOAD: POPULATE CACHE       │                          │ │
│ │                                  │                          │ │
│ │ cache.add_nodes(nodes)           │                          │ │
│ │ File: storage/graph_cache.rs     │                          │ │
│ │ └─ Cache for future queries      │                          │ │
│ └──────────────────────────────────┼──────────────────────────┘ │
│                                    │                            │
│ Returns: Vec<Node>                 │                            │
└────────────────────────────────────┼───────────────────────────┘
                                     │
                         ┌───────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 1: PERSISTENT BACKEND                                    │
├─────────────────────────────────────────────────────────────────┤
│ SledStorage::scan_nodes_by_label(graph, labels)                │
│ File: storage/persistent/sled.rs                               │
│                                                                 │
│ Sled B-Tree Operations:                                        │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 1: Open Tree                                           │ │
│ │ tree_name = "graph_name:nodes_by_label"                     │ │
│ │ tree = db.open_tree(tree_name)?                             │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 2: Scan Prefix                                         │ │
│ │ prefix = "Person:"                                          │ │
│ │ iter = tree.scan_prefix(prefix)                             │ │
│ │                                                              │ │
│ │ Sled Tree Structure:                                        │ │
│ │   Key              → Value                                  │ │
│ │   "Person:uuid-1"  → node_id_1                              │ │
│ │   "Person:uuid-2"  → node_id_2                              │ │
│ │   ...                                                        │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 3: Load Nodes                                          │ │
│ │ for (key, value) in iter {                                  │ │
│ │   node_id = deserialize(value)                              │ │
│ │   node_data = tree.get("nodes", node_id)?                   │ │
│ │   node = deserialize::<Node>(node_data)                     │ │
│ │   nodes.push(node)                                          │ │
│ │ }                                                            │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 4: Return                                              │ │
│ │ Ok(nodes: Vec<Node>)                                        │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ Sled Guarantees:                                               │
│ • B-tree structure for efficient range scans                   │
│ • Crash-safe via built-in WAL                                  │
│ • ACID transactions                                            │
│ • Automatic compression                                        │
└─────────────────────────────────────────────────────────────────┘
```

### **Visual Summary: Query Data Flow**

```
Query Text
    ↓
┌──────────────────┐
│ LAYER 7: CLI     │  User interface
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 6: COORD   │  Parse → Create Request
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 5: SESSION │  Lookup session → Get current_graph
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 4: EXEC    │  Resolve graph → Create context → Route
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 3: PLAN    │  Logical → Optimize → Physical
│   • logical.rs   │  (Only for Query/Select!)
│   • optimizer.rs │
│   • physical.rs  │
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 4: EXEC    │  Execute PhysicalPlan (recursive)
│ (execution)      │  • NodeScan → storage.scan_nodes()
└────────┬─────────┘  • Filter → evaluate conditions
         ↓            • Project → extract properties
┌──────────────────┐
│ LAYER 2: STORAGE │  Check cache → Load from disk
│   • cache        │  L1: GraphCache (hot)
│   • persistent   │  L2: Sled (cold)
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 1: SLED    │  B-tree scan → Deserialize nodes
└────────┬─────────┘
         ↓
    QueryResult {
      rows: Vec<Row>,
      variables: Vec<String>,
      execution_time_ms: u64
    }
```

### **Key Characteristics of Query Operation**

| Aspect | Detail |
|--------|--------|
| **Layers Touched** | All 7 layers |
| **Planning** | ✅ Yes (Logical → Physical) |
| **Transaction** | ❌ No (read-only) |
| **Cache Interaction** | ✅ Read from cache, no invalidation |
| **Files Involved** | 15+ files across 6 modules |
| **Performance** | Optimized via cost-based planner |

---

## Operation 2: INSERT (Data Modification)

### **Example Query**
```gql
INSERT (:Person {name: 'Bob', age: 25})-[:KNOWS]->(:Person {name: 'Carol'})
```

### **Complete Control Flow Diagram**

```
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 7: USER INTERFACE                                        │
│ Same as Query operation                                        │
└────────────────────────┬────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 6: QUERY COORDINATOR                                     │
│ QueryCoordinator::process_query(query_text, session_id)        │
│                                                                 │
│ parse_query(...) → Document {                                  │
│   statement: Statement::DataStatement(                          │
│     DataStatement::Insert(InsertStatement {                     │
│       graph_patterns: [                                         │
│         PathPattern { ... }  // (:Person {...})-[:KNOWS]->(...) │
│       ]                                                         │
│     })                                                          │
│   )                                                            │
│ }                                                              │
└────────────────────────┬────────────────────────────────────────┘
                         │ ExecutionRequest
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 4: QUERY EXECUTOR (NO PLANNING!)                         │
├─────────────────────────────────────────────────────────────────┤
│ QueryExecutor::execute_query(request)                          │
│ File: exec/executor.rs:144                                     │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ PHASE 1: RESOLVE GRAPH (Same as Query)                      │ │
│ │ resolve_graph_for_execution(request)                        │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ PHASE 2: ROUTE STATEMENT                                    │ │
│ │                                                              │ │
│ │ match Statement::DataStatement(stmt) =>                     │ │
│ │   Coordinator::execute_data_statement(stmt, ...)            │ │
│ │   File: exec/write_stmt/data_stmt/coordinator.rs            │ │
│ │   │                                                          │ │
│ │   └─ match DataStatement::Insert(insert) =>                 │ │
│ │       execute_insert(                                        │ │
│ │         insert, storage, txn_manager, catalog,              │ │
│ │         schema_validator, context                            │ │
│ │       )                                                      │ │
│ │       File: exec/write_stmt/data_stmt/insert.rs             │ │
│ └─────────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ INSERT EXECUTOR (Direct Execution - No Planner!)               │
├─────────────────────────────────────────────────────────────────┤
│ execute_insert(...)                                            │
│ File: exec/write_stmt/data_stmt/insert.rs                      │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 1: PARSE PATTERNS                                     ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ for pattern in insert.graph_patterns:                          │
│                                                                 │
│   Pattern 1: (:Person {name: 'Bob', age: 25})                  │
│   ├─ Element type: Node                                        │
│   ├─ Labels: ["Person"]                                        │
│   └─ Properties: {name: "Bob", age: 25}                        │
│                                                                 │
│   Pattern 2: -[:KNOWS]->                                       │
│   ├─ Element type: Edge                                        │
│   ├─ Label: "KNOWS"                                            │
│   ├─ Direction: Outgoing                                       │
│   └─ Properties: {}                                            │
│                                                                 │
│   Pattern 3: (:Person {name: 'Carol'})                         │
│   ├─ Element type: Node                                        │
│   ├─ Labels: ["Person"]                                        │
│   └─ Properties: {name: "Carol"}                               │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 2: GENERATE IDS                                       ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ use uuid::Uuid;                                                │
│                                                                 │
│ node1_id = Uuid::new_v4().to_string()                          │
│   └─ Example: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"           │
│                                                                 │
│ edge_id = Uuid::new_v4().to_string()                           │
│   └─ Example: "b2c3d4e5-f6a7-8901-bcde-f12345678901"           │
│                                                                 │
│ node2_id = Uuid::new_v4().to_string()                          │
│   └─ Example: "c3d4e5f6-a7b8-9012-cdef-123456789012"           │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3: SCHEMA VALIDATION (If Graph Has Schema)            ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ if let Some(graph_type) = get_graph_type(graph_name) {         │
│   IngestionValidator::validate_node(node, graph_type)?         │
│   File: schema/integration/ingestion_validator.rs              │
│                                                                 │
│   Validation Checks:                                           │
│   ✓ Labels exist in schema                                     │
│     └─ Check: "Person" in graph_type.node_types                │
│   ✓ Property types match schema                                │
│     └─ Check: name is STRING, age is INTEGER                   │
│   ✓ NOT NULL constraints                                       │
│     └─ Check: Required properties present                      │
│   ✓ UNIQUE constraints                                         │
│     └─ Check: No duplicate unique values                       │
│   ✓ CHECK constraints                                          │
│     └─ Evaluate: CHECK (age >= 0)                              │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 4: TRANSACTION CHECK                                  ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ txn_id = transaction_manager.get_current_transaction(session)? │
│ File: txn/manager.rs                                           │
│                                                                 │
│ if txn_id.is_none() {                                          │
│   // Auto-begin transaction                                    │
│   txn_id = transaction_manager.begin(                          │
│     session_id,                                                │
│     IsolationLevel::ReadCommitted,                             │
│     AccessMode::ReadWrite                                      │
│   )?                                                           │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 5: LOG TO WAL (Write-Ahead Log)                       ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ For each entity (node1, edge, node2):                          │
│   transaction_manager.log_operation(                           │
│     txn_id,                                                    │
│     UndoOperation::InsertNode {                                │
│       graph: graph_name.clone(),                               │
│       node_id: node_id.clone()                                 │
│     }                                                          │
│   )?                                                           │
│   File: txn/manager.rs → txn/log.rs                           │
│                                                                 │
│ WAL Operations:                                                │
│ File: txn/wal.rs                                               │
│                                                                 │
│ wal.append(LogEntry {                                          │
│   txn_id: txn_id,                                              │
│   lsn: next_lsn(),                                             │
│   operation: WalOperation::Write {                             │
│     graph: graph_name,                                         │
│     entity_type: EntityType::Node,                             │
│     entity_id: node_id,                                        │
│     data: serialize(node)                                      │
│   },                                                           │
│   timestamp: Utc::now()                                        │
│ })                                                             │
│                                                                 │
│ wal.flush()  // fsync to disk (DURABILITY!)                    │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 6: INSERT TO STORAGE                                  ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ For each entity:                                               │
│                                                                 │
│   // Insert node1                                              │
│   storage.insert_node(graph_name, Node {                       │
│     id: node1_id,                                              │
│     labels: vec!["Person"],                                    │
│     properties: hashmap! {                                     │
│       "name" => Value::String("Bob"),                          │
│       "age" => Value::Number(25.0)                             │
│     }                                                          │
│   })?                                                          │
│                                                                 │
│   // Insert edge                                               │
│   storage.insert_edge(graph_name, Edge {                       │
│     id: edge_id,                                               │
│     from: node1_id,                                            │
│     to: node2_id,                                              │
│     label: "KNOWS",                                            │
│     properties: HashMap::new()                                 │
│   })?                                                          │
│                                                                 │
│   // Insert node2                                              │
│   storage.insert_node(graph_name, Node {                       │
│     id: node2_id,                                              │
│     labels: vec!["Person"],                                    │
│     properties: hashmap! {                                     │
│       "name" => Value::String("Carol")                         │
│     }                                                          │
│   })?                                                          │
│                                                                 │
│   └─ Goes to LAYER 2 ─────────────────┐                        │
│                                       │                        │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┻━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 7: INVALIDATE CACHES          │                       ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┳━━━━━━━━━━━━━━━━━━━━━┛ │
│                                       │                        │
│ cache_manager.invalidate_on_write(    │                        │
│   graph_name,                         │                        │
│   EntityType::Node,                   │                        │
│   labels: vec!["Person"]              │                        │
│ )                                     │                        │
│ File: cache/invalidation.rs           │                        │
│                                       │                        │
│ Invalidation Actions:                 │                        │
│ • Clear L1 result_cache (hot)         │                        │
│ • Clear L2 result_cache (warm)        │                        │
│ • Clear subquery_cache                │                        │
│ • Keep L3 plan_cache (still valid)    │                        │
│                                       │                        │
│ ┌─────────────────────────────────────┼──────────────────────┐ │
│ │ RETURN                              │                      │ │
│ │                                     │                      │ │
│ │ QueryResult {                       │                      │ │
│ │   rows: Vec::new(),                 │                      │ │
│ │   variables: Vec::new(),            │                      │ │
│ │   execution_time_ms: elapsed,       │                      │ │
│ │   rows_affected: 3,  ← 2 nodes + 1 edge                   │ │
│ │   warnings: Vec::new()              │                      │ │
│ │ }                                   │                      │ │
│ │ File: exec/result.rs                │                      │ │
│ └─────────────────────────────────────┼──────────────────────┘ │
└─────────────────────────────────────────┼──────────────────────┘
                                          │
                         ┌────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 2: STORAGE MANAGER (Write Path)                          │
├─────────────────────────────────────────────────────────────────┤
│ StorageManager::insert_node(graph_name, node)                  │
│ File: storage/storage_manager.rs                               │
│                                                                 │
│ Write Strategy: Write-Through Cache                            │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ L1: UPDATE CACHE (Write-Through)                            │ │
│ │                                                              │ │
│ │ cache.add_node(node.clone())                                │ │
│ │ File: storage/graph_cache.rs                                │ │
│ │                                                              │ │
│ │ GraphCache Updates:                                         │ │
│ │ 1. nodes.insert(node.id, node)                              │ │
│ │ 2. For each label in node.labels:                           │ │
│ │      nodes_by_label[label].push(node.id)                    │ │
│ │ 3. If node has edges:                                       │ │
│ │      adjacency_out[node.id] = edge_ids                      │ │
│ │      adjacency_in[target_id].push(edge_id)                  │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ L2: PERSIST TO DISK (Durability)                            │ │
│ │                                                              │ │
│ │ persistent_store.insert_node(driver, graph, node)           │ │
│ │ File: storage/data_adapter.rs                               │ │
│ │                                                              │ │
│ │ Delegates to LAYER 1 ────────────┐                          │ │
│ └──────────────────────────────────┼──────────────────────────┘ │
│                                    │                            │
│ Returns: Result<(), StorageError>  │                            │
└────────────────────────────────────┼───────────────────────────┘
                                     │
                         ┌───────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ LAYER 1: PERSISTENT BACKEND (Write)                            │
├─────────────────────────────────────────────────────────────────┤
│ SledStorage::put_node(graph, node)                             │
│ File: storage/persistent/sled.rs                               │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 1: SERIALIZE NODE                                      │ │
│ │                                                              │ │
│ │ node_bytes = bincode::serialize(&node)?                     │ │
│ │ └─ Binary serialization for efficiency                      │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 2: WRITE TO NODES TREE                                 │ │
│ │                                                              │ │
│ │ tree = driver.open_tree("graph_name:nodes")?                │ │
│ │ tree.insert(node.id.as_bytes(), node_bytes)?                │ │
│ │                                                              │ │
│ │ Sled Tree Structure:                                        │ │
│ │   Tree: "mygraph:nodes"                                     │ │
│ │   ─────────────────────────────────                         │ │
│ │   Key                    → Value                            │ │
│ │   "uuid-node1"           → <serialized Node>                │ │
│ │   "uuid-node2"           → <serialized Node>                │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 3: WRITE TO LABEL INDEX                                │ │
│ │                                                              │ │
│ │ for label in &node.labels {                                 │ │
│ │   tree = driver.open_tree("graph_name:nodes_by_label")?    │ │
│ │   key = format!("{}:{}", label, node.id)                    │ │
│ │   tree.insert(key.as_bytes(), &[])?                         │ │
│ │ }                                                            │ │
│ │                                                              │ │
│ │ Sled Tree Structure:                                        │ │
│ │   Tree: "mygraph:nodes_by_label"                            │ │
│ │   ────────────────────────────────────                      │ │
│ │   Key                    → Value                            │ │
│ │   "Person:uuid-node1"    → ()                               │ │
│ │   "Person:uuid-node2"    → ()                               │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 4: FLUSH (Sled handles this automatically)             │ │
│ │                                                              │ │
│ │ Sled's built-in WAL ensures durability                      │ │
│ │ └─ Combined with our txn/wal.rs for double safety           │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ Returns: Ok(())                                                │
│                                                                 │
│ ACID Guarantees:                                               │
│ • Atomicity: Sled transaction + our TransactionManager         │
│ • Consistency: Schema validation before write                  │
│ • Isolation: Read-committed (from txn/isolation.rs)            │
│ • Durability: Sled WAL + our WAL (txn/wal.rs)                  │
└─────────────────────────────────────────────────────────────────┘
```

### **Visual Summary: INSERT Data Flow**

```
Query Text
    ↓
┌──────────────────┐
│ LAYER 6: COORD   │  Parse INSERT statement
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 4: EXEC    │  Route to INSERT executor
└────────┬─────────┘  (NO PLANNER - Direct execution!)
         ↓
┌──────────────────┐
│ INSERT EXECUTOR  │  Parse patterns → Generate IDs
│ (write_stmt/     │  ↓
│  data_stmt/      │  Validate schema
│  insert.rs)      │  ↓
└────────┬─────────┘  Check/Begin transaction
         ↓
┌──────────────────┐
│ TXN MANAGER      │  Log undo operation
│ (txn/manager.rs) │  ↓
│ + WAL            │  Write to WAL (durability)
│ (txn/wal.rs)     │  ↓
└────────┬─────────┘  fsync() to disk
         ↓
┌──────────────────┐
│ LAYER 2: STORAGE │  Update cache (write-through)
│ (storage_        │  ↓
│  manager.rs)     │  Persist to disk
└────────┬─────────┘
         ↓
┌──────────────────┐
│ LAYER 1: SLED    │  Serialize → Insert to trees
│ (persistent/     │  • nodes tree
│  sled.rs)        │  • nodes_by_label index
└────────┬─────────┘  • adjacency lists (for edges)
         ↓
┌──────────────────┐
│ CACHE MANAGER    │  Invalidate result caches
│ (cache/          │  (Data changed - results stale)
│  invalidation.rs)│
└──────────────────┘
         ↓
    QueryResult {
      rows_affected: 3
    }
```

### **Key Differences from Query Operation**

| Aspect | Query | INSERT |
|--------|-------|--------|
| **Planning** | ✅ Logical → Physical | ❌ None (direct execution) |
| **Transaction** | ❌ Optional | ✅ Required (auto or explicit) |
| **WAL** | ❌ No | ✅ Yes (durability) |
| **Undo Log** | ❌ No | ✅ Yes (rollback support) |
| **Schema Validation** | ❌ No | ✅ Yes (before write) |
| **Cache** | Read only | Write-through + Invalidation |
| **Files Touched** | 15+ | 12+ |

---

## Operation 3: Transaction Control

### **Example**
```gql
BEGIN TRANSACTION;
INSERT (:Person {name: 'Alice'});
COMMIT;
```

### **Complete Control Flow Diagram**

```
┌═════════════════════════════════════════════════════════════════┐
║ OPERATION 3A: BEGIN TRANSACTION                                 ║
└═════════════════════════════════════════════════════════════════┘

LAYER 6: QueryCoordinator
  └─ parse_query("BEGIN TRANSACTION")
      └─ Returns: TransactionStatement(StartTransaction(...))

LAYER 4: QueryExecutor
  └─ route_and_execute()
      └─ match Statement::TransactionStatement(stmt) =>
          Coordinator::execute_transaction_statement(stmt)
          File: exec/write_stmt/transaction/coordinator.rs
          │
          └─ match TransactionStatement::StartTransaction =>
              execute_start_transaction(stmt, txn_mgr, session)
              File: exec/write_stmt/transaction/start.rs

┌─────────────────────────────────────────────────────────────────┐
│ TRANSACTION MANAGER: BEGIN                                     │
├─────────────────────────────────────────────────────────────────┤
│ TransactionManager::begin(session_id, isolation, access_mode)  │
│ File: txn/manager.rs                                           │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 1: ALLOCATE TRANSACTION ID                            ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ txn_id = next_txn_id.fetch_add(1, Ordering::SeqCst)            │
│ File: txn/state.rs                                             │
│                                                                 │
│ pub struct TransactionManager {                                │
│   next_txn_id: AtomicU64,  ← Monotonically increasing          │
│   ...                                                          │
│ }                                                              │
│                                                                 │
│ Example: txn_id = 42                                           │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 2: CREATE TRANSACTION LOG                             ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ log = TransactionLog {                                         │
│   transaction_id: txn_id,                                      │
│   operations: Vec::new(),  ← Empty initially                   │
│   timestamp: Utc::now(),                                       │
│   isolation_level: IsolationLevel::ReadCommitted,              │
│   access_mode: AccessMode::ReadWrite                           │
│ }                                                              │
│ File: txn/log.rs                                               │
│                                                                 │
│ pub enum UndoOperation {                                       │
│   InsertNode { graph: String, node_id: String },               │
│   DeleteNode { graph: String, node: Node },  ← Stores data     │
│   UpdateNode { graph: String, node_id: String,                 │
│                old_properties: HashMap<String, Value> },       │
│   InsertEdge { graph: String, edge_id: String },               │
│   DeleteEdge { graph: String, edge: Edge },                    │
│   UpdateEdge { graph: String, edge_id: String,                 │
│                old_properties: HashMap<String, Value> },       │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3: REGISTER TRANSACTION                               ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ active_transactions.insert(txn_id, log)                        │
│ File: txn/manager.rs                                           │
│                                                                 │
│ pub struct TransactionManager {                                │
│   active_transactions: Arc<RwLock<HashMap<TransactionId,       │
│                                           TransactionLog>>>,   │
│   ...                                                          │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 4: WRITE BEGIN TO WAL                                 ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ wal.append(LogEntry {                                          │
│   txn_id: txn_id,                                              │
│   lsn: next_lsn(),  ← Log Sequence Number                      │
│   operation: WalOperation::Begin {                             │
│     isolation: IsolationLevel::ReadCommitted                   │
│   },                                                           │
│   timestamp: Utc::now()                                        │
│ })                                                             │
│ File: txn/wal.rs                                               │
│                                                                 │
│ wal.flush()  ← fsync() to disk                                 │
│                                                                 │
│ pub struct WriteAheadLog {                                     │
│   log_file: Arc<RwLock<File>>,                                 │
│   buffer: Arc<RwLock<Vec<LogEntry>>>,                          │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 5: UPDATE SESSION STATE                               ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ session.transaction_state = TransactionState::Active(txn_id)   │
│ File: session/transaction_state.rs                             │
│                                                                 │
│ pub enum TransactionState {                                    │
│   NotStarted,                                                  │
│   Active(TransactionId),      ← Current state                  │
│   Committed,                                                   │
│   RolledBack,                                                  │
│ }                                                              │
│                                                                 │
│ Returns: Ok(txn_id)                                            │
└─────────────────────────────────────────────────────────────────┘

┌═════════════════════════════════════════════════════════════════┐
║ OPERATION 3B: INSERT (Within Transaction)                       ║
└═════════════════════════════════════════════════════════════════┘

[See Operation 2: INSERT for details]

Additional Transaction Integration:
  └─ transaction_manager.log_operation(
       txn_id,
       UndoOperation::InsertNode {
         graph: "mygraph",
         node_id: "uuid-alice"
       }
     )
     File: txn/manager.rs
     │
     └─ Adds to: active_transactions[txn_id].operations
         File: txn/log.rs

This allows rollback if needed!

┌═════════════════════════════════════════════════════════════════┐
║ OPERATION 3C: COMMIT                                            ║
└═════════════════════════════════════════════════════════════════┘

LAYER 6: QueryCoordinator
  └─ parse_query("COMMIT")
      └─ Returns: TransactionStatement(Commit)

LAYER 4: QueryExecutor
  └─ route_and_execute()
      └─ match TransactionStatement::Commit =>
          execute_commit(stmt, txn_mgr, session)
          File: exec/write_stmt/transaction/commit.rs

┌─────────────────────────────────────────────────────────────────┐
│ TRANSACTION MANAGER: COMMIT                                    │
├─────────────────────────────────────────────────────────────────┤
│ TransactionManager::commit(txn_id)                             │
│ File: txn/manager.rs                                           │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 1: VALIDATE TRANSACTION STATE                         ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ if !active_transactions.contains_key(txn_id) {                 │
│   return Err("Transaction not found or already completed")     │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 2: FLUSH WAL (CRITICAL FOR DURABILITY!)               ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ wal.append(LogEntry {                                          │
│   txn_id,                                                      │
│   operation: WalOperation::Commit,                             │
│   timestamp: Utc::now()                                        │
│ })                                                             │
│ File: txn/wal.rs                                               │
│                                                                 │
│ wal.flush()  ← CRITICAL: fsync() to disk                       │
│                                                                 │
│ At this point, transaction is durable!                         │
│ Even if system crashes, changes can be recovered from WAL.     │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3: APPLY CHANGES                                      ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ ✓ Already applied during INSERT!                               │
│   (Write-through strategy)                                     │
│                                                                 │
│ GraphLite uses eager application:                              │
│ • Changes applied immediately during operation                 │
│ • Undo log allows rollback if needed                           │
│ • Alternative: buffered writes + apply on commit               │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 4: RELEASE LOCKS                                      ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ lock_tracker.release_all(txn_id)                               │
│ File: exec/lock_tracker.rs                                     │
│                                                                 │
│ pub struct LockTracker {                                       │
│   locks: HashMap<EntityId, LockInfo>,                          │
│ }                                                              │
│                                                                 │
│ For each lock held by txn_id:                                  │
│   └─ Remove from locks HashMap                                 │
│   └─ Wake up waiting transactions                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 5: MARK TRANSACTION COMMITTED                         ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ active_transactions.remove(txn_id)                             │
│ └─ Transaction log discarded (no longer needed)                │
│                                                                 │
│ session.transaction_state = TransactionState::Committed        │
│ File: session/transaction_state.rs                             │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 6: INVALIDATE CACHES                                  ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ cache_manager.invalidate_affected_caches()                     │
│ File: cache/invalidation.rs                                    │
│ • Clear result caches (data changed)                           │
│ • Clear subquery caches                                        │
│ • Keep plan cache (still valid)                                │
│                                                                 │
│ Returns: Ok(())                                                │
└─────────────────────────────────────────────────────────────────┘

┌═════════════════════════════════════════════════════════════════┐
║ OPERATION 3D: ROLLBACK (Alternative to COMMIT)                  ║
└═════════════════════════════════════════════════════════════════┘

TransactionManager::rollback(txn_id)
File: exec/write_stmt/transaction/rollback.rs

┌─────────────────────────────────────────────────────────────────┐
│ ROLLBACK PROCESS                                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 1: GET TRANSACTION LOG                                ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ log = active_transactions.get(txn_id)?                         │
│ File: txn/log.rs                                               │
│                                                                 │
│ Example log.operations:                                        │
│   [                                                            │
│     UndoOperation::InsertNode {                                │
│       graph: "mygraph",                                        │
│       node_id: "uuid-alice"                                    │
│     },                                                         │
│     UndoOperation::InsertEdge {                                │
│       graph: "mygraph",                                        │
│       edge_id: "uuid-edge-1"                                   │
│     }                                                          │
│   ]                                                            │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 2: APPLY UNDO OPERATIONS (In REVERSE Order!)          ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ for op in log.operations.into_iter().rev() {                   │
│   match op {                                                   │
│     UndoOperation::InsertNode { graph, node_id } => {          │
│       // Undo insert by deleting                               │
│       storage.delete_node(graph, node_id)?                     │
│     }                                                          │
│                                                                 │
│     UndoOperation::DeleteNode { graph, node } => {             │
│       // Undo delete by re-inserting                           │
│       storage.insert_node(graph, node)?                        │
│     }                                                          │
│                                                                 │
│     UndoOperation::UpdateNode { graph, node_id, old_props } => { │
│       // Undo update by restoring old properties               │
│       storage.update_node(graph, node_id, old_props)?          │
│     }                                                          │
│                                                                 │
│     // ... similar for edges                                   │
│   }                                                            │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3: WRITE ROLLBACK TO WAL                              ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ wal.append(WalOperation::Rollback { txn_id })                  │
│ wal.flush()                                                    │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 4: RELEASE LOCKS                                      ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ lock_tracker.release_all(txn_id)                               │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 5: MARK ROLLED BACK                                   ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ active_transactions.remove(txn_id)                             │
│ session.transaction_state = TransactionState::RolledBack       │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 6: INVALIDATE CACHES                                  ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ cache_manager.invalidate_affected_caches()                     │
│                                                                 │
│ Returns: Ok(())                                                │
└─────────────────────────────────────────────────────────────────┘
```

### **Transaction State Machine**

```
┌──────────────┐
│ NotStarted   │  Initial state
└──────┬───────┘
       │ BEGIN TRANSACTION
       │ File: exec/write_stmt/transaction/start.rs
       ▼
┌──────────────┐
│ Active(txn)  │  Executing operations
└──┬────────┬──┘  File: session/transaction_state.rs
   │        │
   │ COMMIT │ ROLLBACK
   │        │
   ▼        ▼
┌─────┐  ┌──────────┐
│Comm │  │RolledBack│
│itted│  └──────────┘
└─────┘
```

### **Two-Log System**

GraphLite uses **two separate logs** for ACID:

```
┌─────────────────────────────────────────────────────────────────┐
│ UNDO LOG (Transaction Log)                                     │
│ File: txn/log.rs                                               │
│ Purpose: ROLLBACK support                                      │
│                                                                 │
│ Stores: Before-images of modified data                         │
│ • InsertNode → Store node_id (to delete on rollback)           │
│ • DeleteNode → Store entire node (to re-insert on rollback)    │
│ • UpdateNode → Store old properties (to restore on rollback)   │
│                                                                 │
│ Lifetime: Active transaction only                              │
│ Discarded: On COMMIT or ROLLBACK completion                    │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ REDO LOG (Write-Ahead Log)                                     │
│ File: txn/wal.rs                                               │
│ Purpose: DURABILITY and crash RECOVERY                         │
│                                                                 │
│ Stores: After-images and operations                            │
│ • BEGIN                                                        │
│ • WRITE (entity data)                                          │
│ • COMMIT                                                       │
│ • ROLLBACK                                                     │
│                                                                 │
│ Lifetime: Permanent (until checkpoint)                         │
│ Used for: Crash recovery, replaying committed transactions     │
└─────────────────────────────────────────────────────────────────┘

Together: Full ACID compliance
• Atomicity: Undo log + all-or-nothing commit
• Consistency: Schema validation
• Isolation: Lock tracker + isolation levels (txn/isolation.rs)
• Durability: WAL with fsync()
```

---

## Operation 4: Session Management

### **Example**
```gql
SESSION SET GRAPH /myschema/mygraph
```

### **Complete Control Flow Diagram**

```
┌─────────────────────────────────────────────────────────────────┐
│ SESSION OPERATION: SET GRAPH                                   │
└─────────────────────────────────────────────────────────────────┘

LAYER 7: User Interface
  └─ User executes: "SESSION SET GRAPH /myschema/mygraph"

┌─────────────────────────────────────────────────────────────────┐
│ LAYER 6: QUERY COORDINATOR                                     │
├─────────────────────────────────────────────────────────────────┤
│ QueryCoordinator::process_query(query_text, session_id)        │
│ File: coordinator/query_coordinator.rs:145                     │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 1: PARSE                                               │ │
│ │                                                              │ │
│ │ parse_query("SESSION SET GRAPH /myschema/mygraph")          │ │
│ │ File: ast/parser.rs                                         │ │
│ │                                                              │ │
│ │ Returns: Document {                                         │ │
│ │   statement: Statement::SessionStatement(                   │ │
│ │     SessionStatement::Set(SessionSetStatement {             │ │
│ │       clause: SessionSetClause::Graph {                     │ │
│ │         graph_expression: GraphExpression::Reference(       │ │
│ │           CatalogPath {                                     │ │
│ │             segments: ["myschema", "mygraph"]               │ │
│ │           }                                                 │ │
│ │         )                                                   │ │
│ │       }                                                     │ │
│ │     })                                                      │ │
│ │   )                                                         │ │
│ │ }                                                           │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 2: EXECUTE                                             │ │
│ │                                                              │ │
│ │ executor.execute_query(request)                             │ │
│ │ File: exec/executor.rs:144                                  │ │
│ │                                                              │ │
│ │ Returns: QueryResult {                                      │ │
│ │   session_result: Some(SessionResult::SetGraph {            │ │
│ │     graph_expression: GraphExpression::Reference(...)       │ │
│ │   })                                                        │ │
│ │ }                                                           │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ STEP 3: HANDLE SESSION RESULT                               │ │
│ │                                                              │ │
│ │ handle_session_result(session_result, session_id)           │ │
│ │ File: coordinator/query_coordinator.rs:172-176              │ │
│ └─────────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ HANDLE SESSION RESULT: Update Session State                    │
├─────────────────────────────────────────────────────────────────┤
│ File: coordinator/query_coordinator.rs:172                     │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 1: GET SESSION                                        ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ session_arc = session_manager.get_session(session_id)?         │
│ File: session/manager.rs                                       │
│                                                                 │
│ Returns: Arc<RwLock<UserSession>>                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 2: ACQUIRE WRITE LOCK                                 ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ session = session_arc.write()                                  │
│ └─ Blocks other readers/writers (exclusive access)             │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 3: EXTRACT GRAPH PATH                                 ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ graph_path = match graph_expression {                          │
│   GraphExpression::Reference(catalog_path) => {                │
│     match catalog_path.segments.len() {                        │
│       2 => {                                                   │
│         // Full path: /schema_name/graph_name                  │
│         format!("/{}", catalog_path.segments.join("/"))        │
│         // Result: "/myschema/mygraph"                         │
│       }                                                        │
│       1 => {                                                   │
│         // Short path: /graph_name                             │
│         // Use session.current_schema                          │
│         format!("/{}/{}", session.current_schema?, segments[0])│
│       }                                                        │
│       _ => return Err("Invalid graph path")                    │
│     }                                                          │
│   }                                                            │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 4: VALIDATE GRAPH EXISTS                              ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ storage.get_graph(graph_path)?                                 │
│ File: storage/storage_manager.rs:153                           │
│                                                                 │
│ If graph doesn't exist:                                        │
│   └─ return Err("Graph '/myschema/mygraph' not found")         │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 5: UPDATE SESSION STATE                               ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ session.current_graph = Some("/myschema/mygraph".to_string())  │
│ File: session/models.rs                                        │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ STEP 6: RELEASE LOCK                                       ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ └─ Automatic when session goes out of scope                    │
│                                                                 │
│ Returns: Ok(())                                                │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ SESSION MODEL: UserSession                                     │
├─────────────────────────────────────────────────────────────────┤
│ File: session/models.rs                                        │
│                                                                 │
│ pub struct UserSession {                                       │
│   pub session_id: String,           // UUID                    │
│   pub username: String,             // "alice"                 │
│   pub roles: Vec<String>,           // ["user", "analyst"]     │
│                                                                 │
│   pub current_graph: Option<String>,  ← UPDATED!               │
│   // Before: None                                              │
│   // After:  Some("/myschema/mygraph")                         │
│                                                                 │
│   pub current_schema: Option<String>, // "/myschema"           │
│   pub home_graph: Option<String>,     // Default graph         │
│                                                                 │
│   pub permissions: SessionPermissionCache,                     │
│   pub created_at: DateTime<Utc>,                               │
│   pub last_accessed: DateTime<Utc>,                            │
│ }                                                              │
│                                                                 │
│ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓ │
│ ┃ IMPACT ON SUBSEQUENT QUERIES                               ┃ │
│ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ │
│                                                                 │
│ After SET SESSION GRAPH, all queries use this graph by default:│
│                                                                 │
│ Query: MATCH (n:Person) RETURN n                               │
│ └─ No FROM clause needed!                                      │
│ └─ Uses session.current_graph = "/myschema/mygraph"            │
│                                                                 │
│ Resolution happens in:                                         │
│ exec/executor.rs::resolve_graph_for_execution()                │
│ └─ Priority 1: Explicit FROM clause                            │
│ └─ Priority 2: session.current_graph ← HERE!                   │
│ └─ Priority 3: Error if needed but not set                     │
└─────────────────────────────────────────────────────────────────┘
```

### **Other Session Operations**

```
┌─────────────────────────────────────────────────────────────────┐
│ SESSION SET SCHEMA /myschema                                   │
├─────────────────────────────────────────────────────────────────┤
│ Updates: session.current_schema = Some("/myschema")            │
│                                                                 │
│ Impact:                                                        │
│ • Default schema for short graph paths                         │
│ • Example: "SESSION SET GRAPH /mygraph"                        │
│   └─ Resolves to: "/myschema/mygraph"                          │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ SESSION RESET GRAPH                                            │
├─────────────────────────────────────────────────────────────────┤
│ Sets: session.current_graph = session.home_graph               │
│                                                                 │
│ Purpose: Reset to user's default graph                         │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ SESSION RESET SCHEMA                                           │
├─────────────────────────────────────────────────────────────────┤
│ Sets: session.current_schema = None                            │
│                                                                 │
│ Purpose: Clear schema context                                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ SESSION CLOSE                                                  │
├─────────────────────────────────────────────────────────────────┤
│ 1. Rollback any active transaction                             │
│ 2. Release all locks                                           │
│ 3. session_manager.delete_session(session_id)                  │
│                                                                 │
│ Purpose: Clean up session resources                            │
└─────────────────────────────────────────────────────────────────┘
```

---

## Operation 5: DDL (CREATE GRAPH)

### **Example**
```gql
CREATE GRAPH /myschema/mygraph
```

### **Complete Control Flow** (see Updated_Architecture.md for full details)

This operation flows through:
1. **Layer 6**: Parse → CreateGraph AST
2. **Layer 4**: Route to DDL coordinator
3. **Catalog Manager**: Create metadata entry
4. **Layer 2**: Initialize storage structures
5. **Layer 1**: Create Sled trees

Files involved:
- `exec/write_stmt/ddl_stmt/create_graph.rs`
- `catalog/manager.rs`
- `catalog/providers/graph_metadata.rs`
- `storage/storage_manager.rs`
- `storage/persistent/sled.rs`

---

## Operation 6: SELECT (SQL-Style)

### **Example**
```gql
SELECT p.name, p.age
FROM /myschema/mygraph
MATCH (p:Person)
WHERE p.age > 25
```

### **Key Difference from MATCH...RETURN**

SELECT has explicit FROM clause, not session-based graph resolution.

**Internal Transformation**:
```
SELECT ... FROM graph MATCH ... WHERE ...
        ↓
MATCH ... FROM graph WHERE ... RETURN ...
```

Then executes as normal Query operation through planner.

---

## Operation 7: CALL (Procedure)

### **Example**
```gql
CALL gql.list_graphs()
```

### **Control Flow**

```
LAYER 6: Parse → CallStatement

LAYER 4: Executor
  └─ Check if system procedure:
      ├─ is_system_procedure("gql.list_graphs")?
      │   File: catalog/system_procedures.rs
      │
      ├─ YES: Execute built-in
      │   └─ match procedure_name:
      │       ├─ "gql.list_graphs" → Return graph list from catalog
      │       ├─ "gql.list_schemas" → Return schema list
      │       ├─ "gql.cache_stats" → Return cache statistics
      │       ├─ "gql.version" → Return version info
      │       └─ ... more
      │
      └─ NO: Look up user-defined procedure (future feature)
```

---

## Complete Operations Summary Table

| Operation | Statement Type | Layers | Planner? | Transaction? | Cache | Key Files |
|-----------|---------------|--------|----------|--------------|-------|-----------|
| **MATCH...RETURN** | Query | 7 | ✅ Logical→Physical | ❌ Optional | Read only | parser, logical, optimizer, physical, executor, storage |
| **INSERT** | DataStatement | 6 | ❌ Direct exec | ✅ Required | Write-through + Invalidate | insert.rs, txn/manager, storage, wal |
| **DELETE** | DataStatement | 6 | ❌ Direct exec | ✅ Required | Invalidate | delete.rs, txn/manager, storage |
| **SET** | DataStatement | 6 | ❌ Direct exec | ✅ Required | Invalidate | set.rs, txn/manager, storage |
| **REMOVE** | DataStatement | 6 | ❌ Direct exec | ✅ Required | Invalidate | remove.rs, txn/manager, storage |
| **SELECT** | Select | 7 | ✅ Yes (converts to Query) | ❌ Optional | Read only | Transforms to Query internally |
| **CALL** | Call | 4 | ❌ No | ❌ No | None | system_procedures.rs |
| **CREATE GRAPH** | CatalogStatement | 5 | ❌ No | ❌ No | None | create_graph.rs, catalog/manager |
| **DROP GRAPH** | CatalogStatement | 5 | ❌ No | ❌ No | Invalidate all | drop_graph.rs, catalog/manager |
| **CREATE SCHEMA** | CatalogStatement | 5 | ❌ No | ❌ No | None | create_schema.rs, catalog/manager |
| **CREATE USER** | CatalogStatement | 4 | ❌ No | ❌ No | None | create_user.rs, security provider |
| **GRANT ROLE** | CatalogStatement | 4 | ❌ No | ❌ No | None | grant_role.rs, security provider |
| **CREATE INDEX** | IndexStatement | 5 | ❌ No | ❌ No | None | index_operations.rs, indexes/manager |
| **BEGIN** | TransactionStatement | 4 | ❌ No | N/A (starts txn) | None | start.rs, txn/manager, wal |
| **COMMIT** | TransactionStatement | 4 | ❌ No | N/A (ends txn) | Invalidate | commit.rs, txn/manager, wal |
| **ROLLBACK** | TransactionStatement | 4 | ❌ No | N/A (aborts txn) | Invalidate | rollback.rs, txn/log |
| **SESSION SET** | SessionStatement | 3 | ❌ No | ❌ No | None | coordinator, session/manager |

---

## Key Architectural Patterns

### **Pattern 1: Read vs Write Paths**

```
READ PATH (Query):
┌──────┐   ┌────────┐   ┌──────────┐   ┌─────────┐   ┌──────────┐
│Parser│ → │Logical │ → │Optimizer │ → │Physical │ → │ Executor │
└──────┘   │Planner │   └──────────┘   │Planner  │   └─────┬────┘
           └────────┘                   └─────────┘         │
                                                            ▼
                                                     ┌──────────┐
                                                     │ Storage  │
                                                     │ (Read)   │
                                                     └──────────┘

WRITE PATH (INSERT/DELETE/SET/REMOVE):
┌──────┐   ┌─────────────┐   ┌──────┐   ┌──────────┐   ┌──────────┐
│Parser│ → │Direct Exec  │ → │ Txn  │ → │ Storage  │ → │ Cache    │
└──────┘   │(no planner!)│   │ Log  │   │ (Write)  │   │Invalidate│
           └─────────────┘   └──────┘   └──────────┘   └──────────┘
```

**Why Different?**
- **Reads**: May touch lots of data → Need optimization
- **Writes**: Point operations → Know exactly what to write → No optimization needed

### **Pattern 2: Transaction Integration**

All write operations follow this pattern:

```
1. Check/Begin Transaction
   └─ txn/manager.rs::begin() or get_current_transaction()

2. Log Undo Operation
   └─ txn/manager.rs::log_operation()
   └─ txn/log.rs::add to operations vector

3. Write to WAL
   └─ txn/wal.rs::append()
   └─ fsync() to disk

4. Apply to Storage
   └─ storage/storage_manager.rs::insert/update/delete

5. Commit or Rollback
   └─ COMMIT: Flush WAL, release locks, discard undo log
   └─ ROLLBACK: Apply undo ops, release locks, discard log
```

### **Pattern 3: Session Context Flow**

```
User → Session ID → SessionManager → UserSession {
                                        current_graph,   ← Graph resolution
                                        current_schema,
                                        permissions,     ← Access control
                                        transaction_state ← Txn tracking
                                      }
```

Every query execution resolves graph:
1. **Priority 1**: Explicit FROM clause in query
2. **Priority 2**: `session.current_graph`
3. **Error**: If needed but neither available

### **Pattern 4: Cache Strategy**

```
READ OPERATIONS:
  └─ Check cache → If hit: return
                 → If miss: load from storage → populate cache

WRITE OPERATIONS:
  └─ Write-through: Update cache + disk simultaneously
  └─ Then: Invalidate result caches (data changed)
  └─ Keep: Plan cache (still valid)
```

---

**End of Control Flow Documentation**
