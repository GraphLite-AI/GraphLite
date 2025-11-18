// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Abstract Syntax Tree (AST) structures for GQL graph language

use serde::{Deserialize, Serialize};

/// Location information for AST nodes
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

/// Main AST node representing a complete GQL program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub statement: Statement,
    pub location: Location,
}

/// Top-level statement types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement {
    Query(Query),
    Select(SelectStatement),
    Call(CallStatement),
    CatalogStatement(CatalogStatement),
    DataStatement(DataStatement),
    SessionStatement(SessionStatement),
    TransactionStatement(TransactionStatement),
    IndexStatement(IndexStatement),
    Declare(DeclareStatement),
    Let(LetStatement),
    Next(NextStatement),
    AtLocation(AtLocationStatement),
    ProcedureBody(ProcedureBodyStatement),
}

/// SELECT statement: SELECT [DISTINCT|ALL] (* | return_items) [FROM graph_expression [match_statement]] [WHERE] [GROUP BY] [HAVING] [ORDER BY] [LIMIT]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectStatement {
    pub distinct: DistinctQualifier,
    pub return_items: SelectItems,
    pub from_clause: Option<FromClause>,
    pub where_clause: Option<WhereClause>,
    pub group_clause: Option<GroupClause>,
    pub having_clause: Option<HavingClause>,
    pub order_clause: Option<OrderClause>,
    pub limit_clause: Option<LimitClause>,
    pub location: Location,
}

/// Select items: either * wildcard or explicit list of return items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectItems {
    Wildcard { location: Location },
    Explicit { items: Vec<ReturnItem>, location: Location },
}

/// FROM clause with graph expression and optional match statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromClause {
    pub graph_expressions: Vec<FromGraphExpression>,
    pub location: Location,
}

/// Graph expression in FROM clause with optional match statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromGraphExpression {
    pub graph_expression: GraphExpression,
    pub match_statement: Option<MatchClause>,
    pub location: Location,
}

/// CALL statement for procedure invocation: CALL procedure_name(args...) [YIELD ...] [WHERE ...]
///
/// NOTE: WHERE clause on CALL is a GraphLite extension for convenience.
/// ISO GQL standard requires using NEXT FILTER WHERE pattern instead:
///   CALL proc() YIELD col NEXT FILTER WHERE col = 'value'
/// GraphLite allows the simpler form:
///   CALL proc() YIELD col WHERE col = 'value'
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStatement {
    pub procedure_name: String,
    pub arguments: Vec<Expression>,
    pub yield_clause: Option<YieldClause>,
    pub where_clause: Option<WhereClause>,  // GraphLite extension (not in ISO GQL call-statement)
    pub location: Location,
}

/// YIELD clause for CALL statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldClause {
    pub items: Vec<YieldItem>,
    pub location: Location,
}

/// YIELD item: column_name [AS alias]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldItem {
    pub column_name: String,
    pub alias: Option<String>,
    pub location: Location,
}

/// Query can be either a basic query or a set operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    Basic(BasicQuery),
    SetOperation(SetOperation),
    /// Query with top-level ORDER BY and/or LIMIT clauses
    Limited {
        query: Box<Query>,
        order_clause: Option<OrderClause>,
        limit_clause: Option<LimitClause>,
    },
    /// Query with WITH clauses creating a pipeline of query segments
    WithQuery(WithQuery),
    /// Query with WITH/UNWIND clauses ending in mutation (REMOVE/SET/DELETE)
    MutationPipeline(MutationPipeline),
    /// LET statement for variable bindings
    Let(LetStatement),
    /// FOR statement for iteration
    For(ForStatement),
    /// FILTER statement for conditional filtering
    Filter(FilterStatement),
    /// Standalone RETURN query without MATCH
    Return(ReturnQuery),
    /// UNWIND statement for expanding lists into rows
    Unwind(UnwindStatement),
}

/// Query with WITH clauses: MATCH ... WITH ... MATCH ... RETURN ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithQuery {
    pub segments: Vec<QuerySegment>,
    pub final_return: ReturnClause,
    pub group_clause: Option<GroupClause>,
    pub having_clause: Option<HavingClause>,
    pub order_clause: Option<OrderClause>,
    pub limit_clause: Option<LimitClause>,
    pub location: Location,
}

/// Query with mutation pipeline: MATCH ... WITH ... [UNWIND ...] REMOVE/SET/DELETE ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationPipeline {
    pub segments: Vec<QuerySegment>,
    pub final_mutation: FinalMutation,
    pub location: Location,
}

/// Final mutation action in a pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinalMutation {
    Remove(Vec<RemoveItem>),
    Set(Vec<SetItem>),
    Delete { expressions: Vec<Expression>, detach: bool },
}

/// A segment of a WITH-based query: MATCH [WHERE] [WITH] [WHERE] [ORDER BY] [LIMIT]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySegment {
    pub match_clause: MatchClause,
    pub where_clause: Option<WhereClause>,
    pub with_clause: Option<WithClause>,
    pub unwind_clause: Option<UnwindClause>,
    pub post_unwind_where: Option<WhereClause>,
    pub location: Location,
}

/// Basic GQL Query structure: MATCH [WHERE] RETURN [ORDER BY] [LIMIT]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicQuery {
    pub match_clause: MatchClause,
    pub where_clause: Option<WhereClause>,
    pub return_clause: ReturnClause,
    pub group_clause: Option<GroupClause>,
    pub having_clause: Option<HavingClause>,
    pub order_clause: Option<OrderClause>,
    pub limit_clause: Option<LimitClause>,
    pub location: Location,
}

/// Standalone RETURN Query structure: RETURN [DISTINCT|ALL] items [GROUP BY]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnQuery {
    pub return_clause: ReturnClause,
    pub group_clause: Option<GroupClause>,
    pub having_clause: Option<HavingClause>,
    pub order_clause: Option<OrderClause>,
    pub limit_clause: Option<LimitClause>,
    pub location: Location,
}

/// Set operation types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SetOperationType {
    Union,
    UnionAll,
    Intersect,
    IntersectAll,
    Except,
    ExceptAll,
}

/// Set operation between two queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOperation {
    pub left: Box<Query>,
    pub operation: SetOperationType,
    pub right: Box<Query>,
    pub limit_clause: Option<LimitClause>,
    pub order_clause: Option<OrderClause>,
    pub location: Location,
}

/// LET statement: LET variable = expression [, variable = expression]*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetStatement {
    pub variable_definitions: Vec<VariableDefinition>,
    pub location: Location,
}

/// Variable definition: variable = expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub variable_name: String,
    pub expression: Expression,
    pub location: Location,
}

/// FOR statement: FOR [alias:] variable IN expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForStatement {
    pub variable: String,
    pub alias: Option<String>,
    pub expression: Expression,
    pub location: Location,
}

/// FILTER statement: FILTER [WHERE] expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStatement {
    pub where_clause: WhereClause,
    pub location: Location,
}

/// UNWIND statement: UNWIND expression AS variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnwindStatement {
    pub expression: Expression,
    pub variable: String,
    pub location: Location,
}

/// MATCH clause with path patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchClause {
    pub patterns: Vec<PathPattern>,
    pub location: Location,
}

/// Path type constraints for graph traversal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PathType {
    /// WALK - allows repeated vertices and edges (most permissive)
    Walk,
    /// TRAIL - allows repeated vertices but not repeated edges
    Trail,
    /// SIMPLE PATH - no repeated vertices or edges
    SimplePath,
    /// ACYCLIC PATH - no repeated vertices or edges, strictest cycle detection
    AcyclicPath,
}

impl Default for PathType {
    fn default() -> Self {
        PathType::Walk // Default to most permissive
    }
}

impl PathType {
    /// Check if this path type allows repeated vertices
    pub fn allows_repeated_vertices(&self) -> bool {
        matches!(self, PathType::Walk | PathType::Trail)
    }
    
    /// Check if this path type allows repeated edges
    pub fn allows_repeated_edges(&self) -> bool {
        matches!(self, PathType::Walk)
    }
    
    /// Get the string representation for queries
    pub fn as_str(&self) -> &'static str {
        match self {
            PathType::Walk => "WALK",
            PathType::Trail => "TRAIL", 
            PathType::SimplePath => "SIMPLE PATH",
            PathType::AcyclicPath => "ACYCLIC PATH",
        }
    }
}

/// Path pattern: [identifier =] [path_type] node (edge node)* with optional path type constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPattern {
    pub assignment: Option<String>, // Optional path variable assignment
    pub path_type: Option<PathType>, // None means default (WALK)
    pub elements: Vec<PatternElement>,
    pub location: Location,
}

/// Pattern element (node or edge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternElement {
    Node(Node),
    Edge(Edge),
}

/// Node pattern: (identifier? :label? {properties}?)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub identifier: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<PropertyMap>,
    pub location: Location,
}

/// Edge pattern: -[:label {properties}]-
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub identifier: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<PropertyMap>,
    pub direction: EdgeDirection,
    pub quantifier: Option<PathQuantifier>,
    pub location: Location,
}

/// Edge direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeDirection {
    Outgoing,    // ->
    Incoming,    // <-
    Both,        // <->
    Undirected,  // -
}

/// Path quantifier for specifying repetition patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PathQuantifier {
    /// Optional pattern: ?
    Optional,
    /// Exact count: {n}
    Exact(u32),
    /// Range: {min, max}
    Range { min: u32, max: u32 },
    /// At least: {min,}
    AtLeast(u32),
    /// At most: {,max}
    AtMost(u32),
}

/// Property map: {key: value, ...}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyMap {
    pub properties: Vec<Property>,
    pub location: Location,
}

/// Property: key: value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: Expression,
    pub location: Location,
}

/// WHERE clause with conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereClause {
    pub condition: Expression,
    pub location: Location,
}

/// DISTINCT/ALL qualifier for RETURN clause
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DistinctQualifier {
    None,     // No qualifier specified  
    Distinct, // DISTINCT specified
    All,      // ALL specified
}

/// RETURN clause with return items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnClause {
    pub distinct: DistinctQualifier,
    pub items: Vec<ReturnItem>,
    pub location: Location,
}

/// Return item: expression [AS alias]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnItem {
    pub expression: Expression,
    pub alias: Option<String>,
    pub location: Location,
}

/// WITH clause: WITH expr [AS alias] [, expr [AS alias]]* [WHERE condition] [ORDER BY ...] [LIMIT ...]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithClause {
    pub distinct: DistinctQualifier,
    pub items: Vec<WithItem>,
    pub where_clause: Option<WhereClause>,
    pub order_clause: Option<OrderClause>,
    pub limit_clause: Option<LimitClause>,
    pub location: Location,
}

/// WITH item: expression [AS alias]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithItem {
    pub expression: Expression,
    pub alias: Option<String>,
    pub location: Location,
}

/// UNWIND clause: UNWIND expression AS variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnwindClause {
    pub expression: Expression,
    pub variable: String,
    pub location: Location,
}

/// ORDER BY clause: ORDER BY expr [ASC|DESC] [, expr [ASC|DESC]]*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderClause {
    pub items: Vec<OrderItem>,
    pub location: Location,
}

/// Order item: expression [ASC|DESC] [NULLS FIRST|LAST]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub expression: Expression,
    pub direction: OrderDirection,
    pub nulls_ordering: Option<NullsOrdering>,
    pub location: Location,
}

/// Order direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderDirection {
    Ascending,
    Descending,
}

/// NULLS ordering
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NullsOrdering {
    First,
    Last,
}

/// LIMIT clause: LIMIT count [OFFSET offset]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitClause {
    pub count: usize,
    pub offset: Option<usize>,
    pub location: Location,
}

/// GROUP BY clause: GROUP BY expression, expression, ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupClause {
    pub expressions: Vec<Expression>,
    pub location: Location,
}

/// HAVING clause: HAVING expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HavingClause {
    pub condition: Expression,
    pub location: Location,
}

/// Expression types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    Binary(BinaryExpression),
    Unary(UnaryExpression),
    FunctionCall(FunctionCall),
    PropertyAccess(PropertyAccess),
    Variable(Variable),
    Parameter(Parameter),
    Literal(Literal),
    Case(CaseExpression),
    PathConstructor(PathConstructor),
    Cast(CastExpression),
    Subquery(SubqueryExpression),
    ExistsSubquery(ExistsSubqueryExpression),
    NotExistsSubquery(NotExistsSubqueryExpression),
    InSubquery(InSubqueryExpression),
    NotInSubquery(NotInSubqueryExpression),
    QuantifiedComparison(QuantifiedComparisonExpression),
    IsPredicate(IsPredicateExpression),
    Pattern(PatternExpression),
    ArrayIndex(ArrayIndexExpression),
}

/// Binary expression: left op right
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryExpression {
    pub left: Box<Expression>,
    pub operator: Operator,
    pub right: Box<Expression>,
    pub location: Location,
}

/// Unary expression: op expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryExpression {
    pub operator: Operator,
    pub expression: Box<Expression>,
    pub location: Location,
}

/// Function call: name(args...)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub distinct: DistinctQualifier,
    pub arguments: Vec<Expression>,
    pub location: Location,
}

/// Property access: object.property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyAccess {
    pub object: String,
    pub property: String,
    pub location: Location,
}

/// Variable: $name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub location: Location,
}

/// Parameter: $identifier (ISO GQL compliant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub location: Location,
}

/// Operators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Operator {
    // Arithmetic
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    
    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    Regex,
    
    // Logical
    And,
    Or,
    Not,
    Xor,
    
    // String
    In,
    NotIn,
    Contains,
    Starts,
    Ends,
    Exists,
    Like,
    Matches,      // Pattern matching operator
    FuzzyEqual,   // Fuzzy/approximate equality (~=)
    Concat,
    
    // Temporal
    Within,
}

/// Literal values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
    DateTime(String),
    Duration(String),
    TimeWindow(String),
    Vector(Vec<f64>),
    List(Vec<Literal>),
}

/// Catalog statements (DDL operations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CatalogStatement {
    CreateSchema(CreateSchemaStatement),
    DropSchema(DropSchemaStatement),
    CreateGraph(CreateGraphStatement),
    DropGraph(DropGraphStatement),
    TruncateGraph(TruncateGraphStatement),
    ClearGraph(ClearGraphStatement),
    CreateGraphType(CreateGraphTypeStatement),
    DropGraphType(DropGraphTypeStatement),
    AlterGraphType(AlterGraphTypeStatement),
    CreateUser(CreateUserStatement),
    DropUser(DropUserStatement),
    CreateRole(CreateRoleStatement),
    DropRole(DropRoleStatement),
    GrantRole(GrantRoleStatement),
    RevokeRole(RevokeRoleStatement),
    CreateProcedure(CreateProcedureStatement),
    DropProcedure(DropProcedureStatement),
}

/// CREATE SCHEMA statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSchemaStatement {
    pub schema_path: CatalogPath,
    pub if_not_exists: bool,
    pub location: Location,
}

/// DROP SCHEMA statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropSchemaStatement {
    pub schema_path: CatalogPath,
    pub if_exists: bool,
    pub cascade: bool,
    pub location: Location,
}

/// CREATE GRAPH statement  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGraphStatement {
    pub graph_path: CatalogPath,
    pub graph_type_spec: Option<GraphTypeSpec>,
    pub if_not_exists: bool,
    pub or_replace: bool,
    pub as_query: Option<Box<Query>>,
    pub location: Location,
}

/// DROP GRAPH statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropGraphStatement {
    pub graph_path: CatalogPath,
    pub if_exists: bool,
    pub cascade: bool,
    pub location: Location,
}

/// TRUNCATE GRAPH statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncateGraphStatement {
    pub graph_path: CatalogPath,
    pub location: Location,
}

/// CLEAR GRAPH statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearGraphStatement {
    pub graph_path: Option<CatalogPath>, // None for current session graph
    pub location: Location,
}

/// CREATE GRAPH TYPE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGraphTypeStatement {
    pub graph_type_path: CatalogPath,
    pub copy_of: Option<CatalogPath>,
    pub graph_type_spec: GraphTypeSpec,
    pub if_not_exists: bool,
    pub or_replace: bool,
    pub location: Location,
}

/// DROP GRAPH TYPE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropGraphTypeStatement {
    pub graph_type_path: CatalogPath,
    pub if_exists: bool,
    pub cascade: bool,
    pub location: Location,
}

/// ALTER GRAPH TYPE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterGraphTypeStatement {
    pub name: String,
    pub location: Location,
}

/// Catalog path for referencing objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPath {
    pub segments: Vec<String>,
    pub location: Location,
}

impl CatalogPath {
    pub fn new(segments: Vec<String>, location: Location) -> Self {
        Self { segments, location }
    }
    
    pub fn to_string(&self) -> String {
        format!("/{}", self.segments.join("/"))
    }
    
    /// Get the last segment as the name
    pub fn name(&self) -> Option<&String> {
        self.segments.last()
    }
}

/// Graph type specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]  
pub struct GraphTypeSpec {
    pub vertex_types: Vec<VertexTypeSpec>,
    pub edge_types: Vec<EdgeTypeSpec>,
    pub location: Location,
}

/// Vertex type specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VertexTypeSpec {
    pub identifier: Option<String>,
    pub labels: Option<LabelExpression>,
    pub properties: Option<PropertyTypeList>,
    pub location: Location,
}

/// Edge type specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeTypeSpec {
    pub identifier: Option<String>,
    pub labels: Option<LabelExpression>,
    pub properties: Option<PropertyTypeList>,
    pub source_vertex: Option<String>,
    pub destination_vertex: Option<String>,
    pub location: Location,
}


/// Label expression for type matching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabelExpression {
    pub terms: Vec<LabelTerm>,
    pub location: Location,
}

/// Label term (union of label factors)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabelTerm {
    pub factors: Vec<LabelFactor>,
    pub location: Location,
}

/// Label factor (basic label element)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LabelFactor {
    Identifier(String),
    Wildcard, // %
    Parenthesized(Box<LabelExpression>),
}

/// Property type list
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertyTypeList {
    pub properties: Vec<PropertyTypeDecl>,
    pub location: Location,
}

/// Property type declaration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertyTypeDecl {
    pub name: String,
    pub type_spec: TypeSpec,
    pub location: Location,
}

/// Type specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TypeSpec {
    Boolean,
    String { max_length: Option<usize> },
    Bytes { max_length: Option<usize> },
    Decimal { precision: Option<u8>, scale: Option<u8> },
    Integer,
    BigInt,
    SmallInt,
    Int128,
    Int256,
    Float { precision: Option<u8> },
    Float32,  // Dedicated f32 type for vectors
    Real,
    Double,
    Vector { dimension: Option<usize> }, // Vector type for embeddings/vectors
    Date,
    Time { precision: Option<u8>, with_timezone: bool },
    Timestamp { precision: Option<u8>, with_timezone: bool },
    ZonedTime { precision: Option<u8> },
    ZonedDateTime { precision: Option<u8> },
    LocalTime { precision: Option<u8> },
    LocalDateTime { precision: Option<u8> },
    Duration { precision: Option<u8> },
    Reference { target_type: Option<Box<TypeSpec>> },
    Path,
    List { element_type: Box<TypeSpec>, max_length: Option<usize> },
    Record,
    Graph { graph_type_spec: Option<Box<GraphTypeSpec>> },
    BindingTable,
}

/// Data modification statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataStatement {
    Insert(InsertStatement),
    MatchInsert(MatchInsertStatement),
    Set(SetStatement),
    MatchSet(MatchSetStatement),
    Remove(RemoveStatement),
    MatchRemove(MatchRemoveStatement),
    Delete(DeleteStatement),
    MatchDelete(MatchDeleteStatement),
}

/// INSERT statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertStatement {
    pub graph_patterns: Vec<PathPattern>,
    pub location: Location,
}

/// MATCH INSERT statement: combines MATCH for binding variables with INSERT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchInsertStatement {
    pub match_clause: MatchClause,
    pub with_clause: Option<WithClause>,
    pub where_clause: Option<WhereClause>,
    pub insert_graph_patterns: Vec<PathPattern>,
    pub location: Location,
}

/// SET statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetStatement {
    pub items: Vec<SetItem>,
    pub location: Location,
}

/// SET item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SetItem {
    PropertyAssignment {
        property: PropertyAccess,
        value: Expression,
    },
    VariableAssignment {
        variable: String,
        value: Expression,
    },
    LabelAssignment {
        variable: String,
        labels: LabelExpression,
    },
}

/// REMOVE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveStatement {
    pub items: Vec<RemoveItem>,
    pub location: Location,
}

/// REMOVE item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoveItem {
    Property(PropertyAccess),
    Label {
        variable: String,
        labels: LabelExpression,
    },
    Variable(String),
}

/// DELETE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStatement {
    pub expressions: Vec<Expression>,
    pub detach: bool,
    pub location: Location,
}

/// MATCH SET statement: combines MATCH for binding variables with SET
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchSetStatement {
    pub match_clause: MatchClause,
    pub with_clause: Option<WithClause>,
    pub where_clause: Option<WhereClause>,
    pub items: Vec<SetItem>,
    pub location: Location,
}

/// MATCH REMOVE statement: combines MATCH for binding variables with REMOVE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRemoveStatement {
    pub match_clause: MatchClause,
    pub with_clause: Option<WithClause>,
    pub where_clause: Option<WhereClause>,
    pub items: Vec<RemoveItem>,
    pub location: Location,
}

/// MATCH DELETE statement: combines MATCH for binding variables with DELETE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchDeleteStatement {
    pub match_clause: MatchClause,
    pub with_clause: Option<WithClause>,
    pub where_clause: Option<WhereClause>,
    pub expressions: Vec<Expression>,
    pub detach: bool,
    pub location: Location,
}

/// Session statements for managing session state and variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatement {
    SessionSet(SessionSetStatement),
    SessionReset(SessionResetStatement),
    SessionClose(SessionCloseStatement),
}

/// SESSION SET statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSetStatement {
    pub clause: SessionSetClause,
    pub location: Location,
}

/// SESSION SET clauses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionSetClause {
    Schema { 
        schema_reference: CatalogPath 
    },
    Graph { 
        graph_expression: GraphExpression 
    },
    TimeZone { 
        time_zone: String 
    },
    GraphParameter { 
        parameter: String, 
        graph_initializer: GraphExpression,
        if_not_exists: bool 
    },
    BindingTableParameter { 
        parameter: String, 
        binding_table_initializer: Box<Query>,
        if_not_exists: bool 
    },
    ValueParameter { 
        parameter: String, 
        value_initializer: Expression,
        if_not_exists: bool 
    },
}

/// SESSION RESET statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResetStatement {
    pub args: Option<SessionResetArgs>,
    pub location: Location,
}

/// SESSION RESET arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionResetArgs {
    All { target: SessionResetTarget },
    Schema,
    Graph,
    TimeZone,
    Parameter { parameter: String },
}

/// Session reset targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionResetTarget {
    Parameters,
    Characteristics,
}

/// SESSION CLOSE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCloseStatement {
    pub location: Location,
}

/// DECLARE statement: DECLARE variable type [= initial_value] [, variable type [= initial_value]]*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclareStatement {
    pub variable_declarations: Vec<VariableDeclaration>,
    pub location: Location,
}

/// Variable declaration with type and optional initial value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclaration {
    pub variable_name: String,
    pub type_spec: TypeSpec,
    pub initial_value: Option<Expression>,
    pub location: Location,
}

/// NEXT statement for multi-statement procedure execution chaining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStatement {
    pub target_statement: Option<Box<Statement>>,
    pub yield_clause: Option<YieldClause>,
    pub location: Location,
}

/// Procedure body with chained statements using NEXT
/// Based on ISO GQL: <procedure-body> ::= [<variable-definition>+] <statement> ("NEXT" [<yield-clause>] <statement>)*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureBodyStatement {
    pub variable_definitions: Vec<DeclareStatement>,
    pub initial_statement: Box<Statement>,
    pub chained_statements: Vec<ChainedStatement>,
    pub location: Location,
}

/// A statement chained with NEXT keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainedStatement {
    pub yield_clause: Option<YieldClause>,
    pub statement: Box<Statement>,
    pub location: Location,
}

/// AT location statement for procedure execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtLocationStatement {
    pub location_path: CatalogPath,
    pub statements: Vec<Statement>,
    pub location: Location,
}

/// Graph expressions for session context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphExpression {
    Reference(CatalogPath),
    Union { left: Box<GraphExpression>, right: Box<GraphExpression>, all: bool },
    /// NON-STANDARD EXTENSION: Marker to use current session graph (for FROM MATCH syntax)
    CurrentGraph,
}

/// CREATE USER statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserStatement {
    pub username: String,
    pub password: Option<String>, // None if password will be prompted
    pub roles: Vec<String>,
    pub if_not_exists: bool,
    pub location: Location,
}

/// DROP USER statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropUserStatement {
    pub username: String,
    pub if_exists: bool,
    pub location: Location,
}

/// CREATE ROLE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleStatement {
    pub role_name: String,
    pub description: Option<String>,
    pub permissions: Vec<PermissionSpec>,
    pub if_not_exists: bool,
    pub location: Location,
}

/// DROP ROLE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropRoleStatement {
    pub role_name: String,
    pub if_exists: bool,
    pub location: Location,
}

/// GRANT ROLE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantRoleStatement {
    pub role_name: String,
    pub username: String,
    pub location: Location,
}

/// REVOKE ROLE statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeRoleStatement {
    pub role_name: String,
    pub username: String,
    pub location: Location,
}

/// Permission specification for roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSpec {
    pub resource_type: String,
    pub resource_name: Option<String>,
    pub action: String,
    pub location: Location,
}

/// CREATE PROCEDURE statement (ISO GQL)
/// Syntax: CREATE [OR REPLACE] PROCEDURE [IF NOT EXISTS] procedure_name ([param_list]) procedure_body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProcedureStatement {
    pub procedure_name: String,
    pub parameters: Vec<ProcedureParameter>,
    pub procedure_body: ProcedureBodyStatement,
    pub or_replace: bool,
    pub if_not_exists: bool,
    pub location: Location,
}

/// DROP PROCEDURE statement (ISO GQL)
/// Syntax: DROP PROCEDURE [IF EXISTS] procedure_name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropProcedureStatement {
    pub procedure_name: String,
    pub if_exists: bool,
    pub location: Location,
}

/// Procedure parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureParameter {
    pub name: String,
    pub type_spec: TypeSpec,
    pub default_value: Option<Expression>,
    pub location: Location,
}

/// CASE expression supporting both simple and searched forms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseExpression {
    pub case_type: CaseType,
    pub location: Location,
}

/// PATH constructor: PATH[expr1, expr2, ...]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConstructor {
    pub elements: Vec<Expression>,
    pub location: Location,
}

/// CAST expression: CAST(expr AS type-spec)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastExpression {
    pub expression: Box<Expression>,
    pub target_type: TypeSpec,
    pub location: Location,
}

/// Subquery expression: (subquery) in general expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubqueryExpression {
    pub query: Box<Query>,
    pub location: Location,
}

/// EXISTS subquery expression: EXISTS(subquery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistsSubqueryExpression {
    pub query: Box<Query>,
    pub location: Location,
}

/// NOT EXISTS subquery expression: NOT EXISTS(subquery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotExistsSubqueryExpression {
    pub query: Box<Query>,
    pub location: Location,
}

/// IN subquery expression: expr IN (subquery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InSubqueryExpression {
    pub expression: Box<Expression>,
    pub query: Box<Query>,
    pub location: Location,
}

/// NOT IN subquery expression: expr NOT IN (subquery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotInSubqueryExpression {
    pub expression: Box<Expression>,
    pub query: Box<Query>,
    pub location: Location,
}

/// Types of CASE expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseType {
    /// Simple CASE: CASE expr WHEN value1 THEN result1 WHEN value2 THEN result2 [ELSE default] END
    Simple(SimpleCaseExpression),
    /// Searched CASE: CASE WHEN condition1 THEN result1 WHEN condition2 THEN result2 [ELSE default] END
    Searched(SearchedCaseExpression),
}

/// Simple CASE expression with a test expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleCaseExpression {
    pub test_expression: Box<Expression>,
    pub when_branches: Vec<SimpleWhenBranch>,
    pub else_expression: Option<Box<Expression>>,
}

/// WHEN branch for simple CASE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleWhenBranch {
    pub when_values: Vec<Expression>, // Multiple values like WHEN "active", "verified"
    pub then_expression: Box<Expression>,
    pub location: Location,
}

/// Searched CASE expression with conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchedCaseExpression {
    pub when_branches: Vec<SearchedWhenBranch>,
    pub else_expression: Option<Box<Expression>>,
}

/// WHEN branch for searched CASE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchedWhenBranch {
    pub condition: Box<Expression>,
    pub then_expression: Box<Expression>,
    pub location: Location,
}

/// Quantifier type for ALL/ANY/SOME expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Quantifier {
    All,
    Any,
    Some,
}

/// Quantified comparison expression: value = ALL/ANY/SOME (subquery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantifiedComparisonExpression {
    pub left: Box<Expression>,
    pub operator: Operator,
    pub quantifier: Quantifier,
    pub subquery: Box<Expression>,
    pub location: Location,
}


impl std::fmt::Display for TypeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeSpec::Boolean => write!(f, "BOOLEAN"),
            TypeSpec::String { max_length } => match max_length {
                Some(len) => write!(f, "STRING({})", len),
                None => write!(f, "STRING"),
            },
            TypeSpec::Bytes { max_length } => match max_length {
                Some(len) => write!(f, "BYTES({})", len),
                None => write!(f, "BYTES"),
            },
            TypeSpec::Decimal { precision, scale } => match (precision, scale) {
                (Some(p), Some(s)) => write!(f, "DECIMAL({},{})", p, s),
                (Some(p), None) => write!(f, "DECIMAL({})", p),
                _ => write!(f, "DECIMAL"),
            },
            TypeSpec::Integer => write!(f, "INTEGER"),
            TypeSpec::BigInt => write!(f, "BIGINT"),
            TypeSpec::SmallInt => write!(f, "SMALLINT"),
            TypeSpec::Int128 => write!(f, "INT128"),
            TypeSpec::Int256 => write!(f, "INT256"),
            TypeSpec::Float { precision } => match precision {
                Some(p) => write!(f, "FLOAT({})", p),
                None => write!(f, "FLOAT"),
            },
            TypeSpec::Float32 => write!(f, "FLOAT32"),
            TypeSpec::Real => write!(f, "REAL"),
            TypeSpec::Double => write!(f, "DOUBLE"),
            TypeSpec::Vector { dimension } => match dimension {
                Some(d) => write!(f, "VECTOR[{}]", d),
                None => write!(f, "VECTOR"),
            },
            TypeSpec::Date => write!(f, "DATE"),
            TypeSpec::Time { precision, with_timezone } => {
                let base = match precision {
                    Some(p) => format!("TIME({})", p),
                    None => "TIME".to_string(),
                };
                if *with_timezone {
                    write!(f, "{} WITH TIME ZONE", base)
                } else {
                    write!(f, "{}", base)
                }
            },
            TypeSpec::Timestamp { precision, with_timezone } => {
                let base = match precision {
                    Some(p) => format!("TIMESTAMP({})", p),
                    None => "TIMESTAMP".to_string(),
                };
                if *with_timezone {
                    write!(f, "{} WITH TIME ZONE", base)
                } else {
                    write!(f, "{}", base)
                }
            },
            TypeSpec::ZonedTime { precision } => match precision {
                Some(p) => write!(f, "TIME({}) WITH TIME ZONE", p),
                None => write!(f, "TIME WITH TIME ZONE"),
            },
            TypeSpec::ZonedDateTime { precision } => match precision {
                Some(p) => write!(f, "TIMESTAMP({}) WITH TIME ZONE", p),
                None => write!(f, "TIMESTAMP WITH TIME ZONE"),
            },
            TypeSpec::LocalTime { precision } => match precision {
                Some(p) => write!(f, "TIME({})", p),
                None => write!(f, "TIME"),
            },
            TypeSpec::LocalDateTime { precision } => match precision {
                Some(p) => write!(f, "TIMESTAMP({})", p),
                None => write!(f, "TIMESTAMP"),
            },
            TypeSpec::Duration { precision } => match precision {
                Some(p) => write!(f, "DURATION({})", p),
                None => write!(f, "DURATION"),
            },
            TypeSpec::Reference { target_type } => match target_type {
                Some(t) => write!(f, "REF({})", t),
                None => write!(f, "REF"),
            },
            TypeSpec::Path => write!(f, "PATH"),
            TypeSpec::List { element_type, max_length } => match max_length {
                Some(len) => write!(f, "{}[{}]", element_type, len),
                None => write!(f, "{}[]", element_type),
            },
            TypeSpec::Record => write!(f, "RECORD"),
            TypeSpec::Graph { .. } => write!(f, "GRAPH"),
            TypeSpec::BindingTable => write!(f, "BINDING_TABLE"),
        }
    }
}

impl TypeSpec {
    /// Check if this type is nullable
    pub fn is_nullable(&self) -> bool {
        match self {
            TypeSpec::Reference { target_type } => target_type.is_none(),
            TypeSpec::List { .. } => false, // Lists themselves are not nullable by default
            _ => false, // Most types are not nullable by default
        }
    }
    
    /// Check if this type is a scalar type
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            TypeSpec::Boolean
                | TypeSpec::String { .. }
                | TypeSpec::Bytes { .. }
                | TypeSpec::Decimal { .. }
                | TypeSpec::Integer
                | TypeSpec::BigInt
                | TypeSpec::SmallInt
                | TypeSpec::Int128
                | TypeSpec::Int256
                | TypeSpec::Float { .. }
                | TypeSpec::Real
                | TypeSpec::Double
                | TypeSpec::Date
                | TypeSpec::Time { .. }
                | TypeSpec::Timestamp { .. }
                | TypeSpec::ZonedTime { .. }
                | TypeSpec::ZonedDateTime { .. }
                | TypeSpec::LocalTime { .. }
                | TypeSpec::LocalDateTime { .. }
                | TypeSpec::Duration { .. }
        )
    }
    
    /// Check if this type is a collection type
    pub fn is_collection(&self) -> bool {
        matches!(self, TypeSpec::List { .. })
    }
    
    /// Check if this type is a temporal type
    pub fn is_temporal(&self) -> bool {
        matches!(
            self,
            TypeSpec::Date
                | TypeSpec::Time { .. }
                | TypeSpec::Timestamp { .. }
                | TypeSpec::ZonedTime { .. }
                | TypeSpec::ZonedDateTime { .. }
                | TypeSpec::LocalTime { .. }
                | TypeSpec::LocalDateTime { .. }
        )
    }
    
    /// Check if this type is a numeric type
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            TypeSpec::Decimal { .. }
                | TypeSpec::Integer
                | TypeSpec::BigInt
                | TypeSpec::SmallInt
                | TypeSpec::Int128
                | TypeSpec::Int256
                | TypeSpec::Float { .. }
                | TypeSpec::Real
                | TypeSpec::Double
        )
    }
    
    /// Check if this is an exact numeric type (integer or decimal)
    pub fn is_exact_numeric(&self) -> bool {
        matches!(
            self,
            TypeSpec::Integer
                | TypeSpec::BigInt
                | TypeSpec::SmallInt
                | TypeSpec::Int128
                | TypeSpec::Int256
                | TypeSpec::Decimal { .. }
        )
    }
    
    /// Check if this is an approximate numeric type (float, real, double)
    pub fn is_approximate_numeric(&self) -> bool {
        matches!(
            self,
            TypeSpec::Float { .. } | TypeSpec::Real | TypeSpec::Double
        )
    }
    
    /// Get the element type if this is a collection
    pub fn element_type(&self) -> Option<&TypeSpec> {
        match self {
            TypeSpec::List { element_type, .. } => Some(element_type),
            _ => None,
        }
    }
    
    /// Check if a temporal type has timezone information
    pub fn has_timezone(&self) -> bool {
        match self {
            TypeSpec::Time { with_timezone, .. } => *with_timezone,
            TypeSpec::Timestamp { with_timezone, .. } => *with_timezone,
            TypeSpec::ZonedTime { .. } | TypeSpec::ZonedDateTime { .. } => true,
            TypeSpec::LocalTime { .. } | TypeSpec::LocalDateTime { .. } => false,
            TypeSpec::Date => false,
            _ => false,
        }
    }
    
    /// Check if a temporal type has time component
    pub fn has_time_component(&self) -> bool {
        match self {
            TypeSpec::Date => false,
            TypeSpec::Time { .. }
            | TypeSpec::Timestamp { .. }
            | TypeSpec::ZonedTime { .. }
            | TypeSpec::ZonedDateTime { .. }
            | TypeSpec::LocalTime { .. }
            | TypeSpec::LocalDateTime { .. } => true,
            _ => false,
        }
    }
    
    /// Check if a temporal type has date component
    pub fn has_date_component(&self) -> bool {
        match self {
            TypeSpec::Date
            | TypeSpec::Timestamp { .. }
            | TypeSpec::ZonedDateTime { .. }
            | TypeSpec::LocalDateTime { .. } => true,
            TypeSpec::Time { .. }
            | TypeSpec::ZonedTime { .. }
            | TypeSpec::LocalTime { .. } => false,
            _ => false,
        }
    }
}

/// IS predicate expression: expression IS [NOT] predicate_type [target]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsPredicateExpression {
    pub subject: Box<Expression>,
    pub predicate_type: IsPredicateType,
    pub negated: bool,
    pub target: Option<Box<Expression>>,     // For SOURCE OF, DESTINATION OF
    pub type_spec: Option<TypeSpec>,         // For IS TYPED
    pub location: Location,
}

/// Array index expression: array[index]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayIndexExpression {
    pub array: Box<Expression>,
    pub index: Box<Expression>,
    pub location: Location,
}

/// IS predicate types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IsPredicateType {
    Null,
    True,
    False,
    Unknown,
    Normalized,
    Directed,
    Source,
    Destination,
    Typed,
    Label(LabelExpression),
}

/// Pattern expression for WHERE clauses: (node)-[relationship]->(node)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternExpression {
    pub pattern: PathPattern,
    pub location: Location,
}

/// Transaction control statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatement {
    StartTransaction(StartTransactionStatement),
    Commit(CommitStatement),
    Rollback(RollbackStatement),
    SetTransactionCharacteristics(SetTransactionCharacteristicsStatement),
}

/// START TRANSACTION statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTransactionStatement {
    pub characteristics: Option<TransactionCharacteristics>,
    pub location: Location,
}

/// COMMIT [WORK] statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStatement {
    pub work: bool,
    pub location: Location,
}

/// ROLLBACK [WORK] statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackStatement {
    pub work: bool,
    pub location: Location,
}

/// SET TRANSACTION statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTransactionCharacteristicsStatement {
    pub characteristics: TransactionCharacteristics,
    pub location: Location,
}

/// Transaction characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCharacteristics {
    pub isolation_level: Option<IsolationLevel>,
    pub access_mode: Option<AccessMode>,
    pub location: Location,
}

/// Transaction isolation levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

/// Transaction access modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

impl IsolationLevel {
    /// Get string representation for display
    pub fn as_str(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }
}

impl AccessMode {
    /// Get string representation for display
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessMode::ReadOnly => "READ ONLY",
            AccessMode::ReadWrite => "READ WRITE",
        }
    }
}

// =============================================================================
// INDEX DDL STATEMENTS
// =============================================================================

/// Index DDL statement types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexStatement {
    CreateIndex(CreateIndexStatement),
    DropIndex(DropIndexStatement),
    AlterIndex(AlterIndexStatement),
    OptimizeIndex(OptimizeIndexStatement),
    ReindexIndex(ReindexStatement),
}

/// CREATE GRAPH INDEX statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIndexStatement {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub index_type: IndexTypeSpecifier,
    pub options: IndexOptions,
    pub if_not_exists: bool,
    pub location: Location,
}

/// DROP INDEX statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropIndexStatement {
    pub name: String,
    pub if_exists: bool,
    pub location: Location,
}

/// ALTER INDEX statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterIndexStatement {
    pub name: String,
    pub operation: AlterIndexOperation,
    pub location: Location,
}

/// OPTIMIZE INDEX statement  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeIndexStatement {
    pub name: String,
    pub location: Location,
}

/// REINDEX statement - rebuilds an index from existing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReindexStatement {
    pub name: String,
    pub location: Location,
}

/// ALTER INDEX operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlterIndexOperation {
    Rebuild,
    SetOption(String, Value),
    Optimize,
}

/// Index type specifier in DDL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexTypeSpecifier {
    Graph(GraphIndexTypeSpecifier),
}

/// Graph index type specifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphIndexTypeSpecifier {
    AdjacencyList,
    PathIndex,
    ReachabilityIndex,
    PatternIndex,
}

/// Index options for CREATE INDEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexOptions {
    pub parameters: std::collections::HashMap<String, Value>,
    pub location: Location,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            parameters: std::collections::HashMap::new(),
            location: Location::default(),
        }
    }
}

/// Value type for index parameters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Array(Vec<Value>),
    Null,
}
