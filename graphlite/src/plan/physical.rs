// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Physical query plan representation
//!
//! Physical plans represent the actual execution strategy with specific
//! algorithms and data access methods chosen for optimal performance.

use crate::ast::{EdgeDirection, Expression, PathType};
use crate::plan::logical::{
    AggregateFunction, JoinType, LogicalNode, LogicalPlan, PathElement, TextSearchType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node creation operation in physical plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCreation {
    /// Storage ID for the node
    pub storage_id: String,
    /// Node labels
    pub labels: Vec<String>,
    /// Resolved property values
    pub properties: HashMap<String, Expression>,
}

/// Edge creation operation in physical plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCreation {
    /// Storage ID for the edge
    pub storage_id: String,
    /// Source node storage ID
    pub from_node_id: String,
    /// Target node storage ID
    pub to_node_id: String,
    /// Edge label
    pub label: String,
    /// Resolved property values
    pub properties: HashMap<String, Expression>,
}

/// Graph index operations for optimized graph traversals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphIndexOperation {
    /// Find neighbors within a certain distance
    FindNeighbors {
        start_node: String,
        direction: EdgeDirection,
        max_hops: Option<usize>,
        edge_labels: Vec<String>,
    },
    /// Find shortest path between two nodes
    ShortestPath {
        start_node: String,
        end_node: String,
        max_length: Option<usize>,
    },
    /// Check reachability between nodes
    IsReachable {
        start_node: String,
        end_node: String,
        max_hops: Option<usize>,
    },
    /// Pattern matching in graph structure
    PatternMatch {
        pattern: String,
        max_results: Option<usize>,
    },
}

/// Physical query plan with execution operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalPlan {
    pub root: PhysicalNode,
    pub estimated_cost: f64,
    pub estimated_rows: usize,
}

/// Physical execution node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhysicalNode {
    /// Sequential scan of all nodes with label
    NodeSeqScan {
        variable: String,
        labels: Vec<String>,
        properties: Option<HashMap<String, Expression>>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Index scan using label index
    NodeIndexScan {
        variable: String,
        labels: Vec<String>,
        properties: Option<HashMap<String, Expression>>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Sequential scan of edges
    EdgeSeqScan {
        variable: String,
        labels: Vec<String>,
        properties: Option<HashMap<String, Expression>>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Index-based edge traversal
    IndexedExpand {
        from_variable: String,
        edge_variable: Option<String>,
        to_variable: String,
        edge_labels: Vec<String>,
        direction: EdgeDirection,
        properties: Option<HashMap<String, Expression>>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Hash-based expand for high-degree nodes
    HashExpand {
        from_variable: String,
        edge_variable: Option<String>,
        to_variable: String,
        edge_labels: Vec<String>,
        direction: EdgeDirection,
        properties: Option<HashMap<String, Expression>>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Path traversal with type constraints
    PathTraversal {
        path_type: PathType,
        from_variable: String,
        to_variable: String,
        path_elements: Vec<PathElement>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Filter with predicate pushdown
    Filter {
        condition: Expression,
        input: Box<PhysicalNode>,
        selectivity: f64,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Projection with column elimination
    Project {
        expressions: Vec<ProjectionItem>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Hash join
    HashJoin {
        join_type: JoinType,
        condition: Option<Expression>,
        build_keys: Vec<Expression>,
        probe_keys: Vec<Expression>,
        build: Box<PhysicalNode>,
        probe: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Nested loop join
    NestedLoopJoin {
        join_type: JoinType,
        condition: Option<Expression>,
        left: Box<PhysicalNode>,
        right: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Sort merge join
    SortMergeJoin {
        join_type: JoinType,
        left_keys: Vec<Expression>,
        right_keys: Vec<Expression>,
        left: Box<PhysicalNode>,
        right: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Union all (concatenation)
    UnionAll {
        inputs: Vec<PhysicalNode>,
        all: bool, // true for UNION ALL, false for UNION (with deduplication)
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Intersect operation
    Intersect {
        left: Box<PhysicalNode>,
        right: Box<PhysicalNode>,
        all: bool,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Except operation
    Except {
        left: Box<PhysicalNode>,
        right: Box<PhysicalNode>,
        all: bool,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Hash aggregation
    HashAggregate {
        group_by: Vec<Expression>,
        aggregates: Vec<AggregateItem>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Sort-based aggregation
    SortAggregate {
        group_by: Vec<Expression>,
        aggregates: Vec<AggregateItem>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// HAVING filter for post-aggregation filtering
    /// Must appear after aggregation nodes in the plan
    Having {
        condition: Expression,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// External sort for large datasets
    ExternalSort {
        expressions: Vec<SortItem>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// In-memory sort for small datasets
    InMemorySort {
        expressions: Vec<SortItem>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Remove duplicate rows
    Distinct {
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Limit with early termination
    Limit {
        count: usize,
        offset: Option<usize>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Generic function call
    GenericFunction {
        function_name: String,
        arguments: Vec<Expression>,
        input: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// EXISTS subquery evaluation
    ExistsSubquery {
        subplan: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
        optimized: bool, // Whether early termination is enabled
    },

    /// NOT EXISTS subquery evaluation
    NotExistsSubquery {
        subplan: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
        optimized: bool,
    },

    /// IN subquery evaluation
    InSubquery {
        expression: Expression,
        subplan: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// NOT IN subquery evaluation
    NotInSubquery {
        expression: Expression,
        subplan: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Scalar subquery evaluation (for RETURN clauses)
    ScalarSubquery {
        subplan: Box<PhysicalNode>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// WITH query that needs special execution handling
    WithQuery {
        original_query: Box<crate::ast::WithQuery>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// UNWIND expression into individual rows
    Unwind {
        expression: Expression,
        variable: String,
        input: Option<Box<PhysicalNode>>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Graph traversal using graph indexes
    GraphIndexScan {
        index_name: String,
        operation: GraphIndexOperation,
        parameters: HashMap<String, Expression>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Full-text search using inverted/text index
    TextIndexScan {
        index_name: String,
        field: String,
        query: String,
        search_type: TextSearchType,
        min_score: Option<f64>,
        limit: Option<usize>,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    /// Join using index for lookup
    IndexJoin {
        left: Box<PhysicalNode>,
        right: Box<PhysicalNode>,
        join_type: JoinType,
        index_name: String,
        join_condition: Expression,
        estimated_rows: usize,
        estimated_cost: f64,
    },

    // Data Modification Operations
    /// INSERT operation with resolved patterns
    Insert {
        /// Node creation operations
        node_creations: Vec<NodeCreation>,
        /// Edge creation operations
        edge_creations: Vec<EdgeCreation>,
        /// Estimated number of operations
        estimated_ops: usize,
        /// Estimated cost
        estimated_cost: f64,
    },

    /// UPDATE operation
    Update {
        target_variable: String,
        properties: HashMap<String, Expression>,
        input: Box<PhysicalNode>,
        estimated_ops: usize,
        estimated_cost: f64,
    },

    /// DELETE operation
    Delete {
        target_variables: Vec<String>,
        detach: bool,
        input: Box<PhysicalNode>,
        estimated_ops: usize,
        estimated_cost: f64,
    },

    /// Produces exactly one row with no data
    /// Used for LET statements and standalone RETURN queries
    SingleRow {
        estimated_rows: usize, // Always 1
        estimated_cost: f64,   // Minimal cost
    },
}

/// Physical operator types for optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PhysicalOperator {
    Scan,
    Filter,
    Project,
    Join,
    Aggregate,
    Sort,
    Limit,
    Union,
    Subquery,
    WithQuery,
    Unwind,
    GraphIndexScan,
    TextIndexScan,
    IndexJoin,
    Insert,
    Update,
    Delete,
    SingleRow,
}

/// Projection item with computed expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionItem {
    pub expression: Expression,
    pub alias: Option<String>,
    pub output_type: OutputType,
}

/// Aggregate item for grouping operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateItem {
    pub function: AggregateFunction,
    pub expression: Expression,
    pub alias: Option<String>,
    pub output_type: OutputType,
}

/// Sort item with ordering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortItem {
    pub expression: Expression,
    pub ascending: bool,
    pub nulls_first: bool,
}

/// Output data types for expression results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Array,
    Null,
}

impl PhysicalPlan {
    /// Create a new physical plan
    pub fn new(root: PhysicalNode) -> Self {
        let estimated_cost = root.get_cost();
        let estimated_rows = root.get_row_count();

        Self {
            root,
            estimated_cost,
            estimated_rows,
        }
    }

    /// Convert logical plan to physical plan
    pub fn from_logical(logical: &LogicalPlan) -> Self {
        let root = Self::convert_logical_node(&logical.root);
        Self::new(root)
    }

    /// Convert a logical node to physical node
    pub fn convert_logical_node(logical: &LogicalNode) -> PhysicalNode {
        match logical {
            LogicalNode::NodeScan {
                variable,
                labels,
                properties,
            } => {
                let estimated_rows = 1000; // Should use statistics
                let estimated_cost = estimated_rows as f64 * 0.1;

                // Choose between sequential and index scan based on selectivity
                if labels.is_empty() {
                    PhysicalNode::NodeSeqScan {
                        variable: variable.clone(),
                        labels: labels.clone(),
                        properties: properties.clone(),
                        estimated_rows,
                        estimated_cost,
                    }
                } else {
                    PhysicalNode::NodeIndexScan {
                        variable: variable.clone(),
                        labels: labels.clone(),
                        properties: properties.clone(),
                        estimated_rows: estimated_rows / 10, // More selective
                        estimated_cost: estimated_cost * 0.5, // Cheaper with index
                    }
                }
            }

            LogicalNode::EdgeScan {
                variable,
                labels,
                properties,
            } => {
                let estimated_rows = 5000;
                let estimated_cost = estimated_rows as f64 * 0.1;

                PhysicalNode::EdgeSeqScan {
                    variable: variable.clone(),
                    labels: labels.clone(),
                    properties: properties.clone(),
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let estimated_rows = input_rows * 5; // Average fanout
                let estimated_cost = estimated_rows as f64 * 0.2;

                // Choose expand strategy based on input size and fanout
                if input_rows > 10000 {
                    PhysicalNode::HashExpand {
                        from_variable: from_variable.clone(),
                        edge_variable: edge_variable.clone(),
                        to_variable: to_variable.clone(),
                        edge_labels: edge_labels.clone(),
                        direction: direction.clone(),
                        properties: properties.clone(),
                        input: input_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                } else {
                    PhysicalNode::IndexedExpand {
                        from_variable: from_variable.clone(),
                        edge_variable: edge_variable.clone(),
                        to_variable: to_variable.clone(),
                        edge_labels: edge_labels.clone(),
                        direction: direction.clone(),
                        properties: properties.clone(),
                        input: input_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                }
            }

            LogicalNode::Filter { condition, input } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let selectivity = 0.5; // Default selectivity
                let estimated_rows = (input_rows as f64 * selectivity) as usize;
                let estimated_cost = input_physical.get_cost() + (input_rows as f64 * 0.01);

                PhysicalNode::Filter {
                    condition: condition.clone(),
                    input: input_physical,
                    selectivity,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Project { expressions, input } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let estimated_rows = input_physical.get_row_count();
                let estimated_cost = input_physical.get_cost() + (estimated_rows as f64 * 0.005);

                PhysicalNode::Project {
                    expressions: expressions
                        .iter()
                        .map(|expr| ProjectionItem {
                            expression: expr.expression.clone(),
                            alias: expr.alias.clone(),
                            output_type: OutputType::String, // Should infer type
                        })
                        .collect(),
                    input: input_physical,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Sort { expressions, input } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let estimated_rows = input_physical.get_row_count();
                let estimated_cost = input_physical.get_cost()
                    + (estimated_rows as f64 * (estimated_rows as f64).log2() * 0.001);

                let sort_items: Vec<SortItem> = expressions
                    .iter()
                    .map(|expr| SortItem {
                        expression: expr.expression.clone(),
                        ascending: expr.ascending,
                        nulls_first: false,
                    })
                    .collect();

                // Choose sort algorithm based on data size
                if estimated_rows > 100000 {
                    PhysicalNode::ExternalSort {
                        expressions: sort_items,
                        input: input_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                } else {
                    PhysicalNode::InMemorySort {
                        expressions: sort_items,
                        input: input_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                }
            }

            LogicalNode::Distinct { input } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let estimated_rows = input_rows / 2; // Assume 50% duplicates removed
                let estimated_cost = input_physical.get_cost() + (input_rows as f64 * 0.01);

                PhysicalNode::Distinct {
                    input: input_physical,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Limit {
                count,
                offset,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let offset_val = offset.unwrap_or(0);
                let estimated_rows = (*count).min(input_rows.saturating_sub(offset_val));
                let estimated_cost =
                    input_physical.get_cost() * (estimated_rows as f64 / input_rows as f64);

                PhysicalNode::Limit {
                    count: *count,
                    offset: *offset,
                    input: input_physical,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::GenericFunction {
                function_name,
                arguments,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let _input_rows = input_physical.get_row_count();
                let estimated_rows = 1; // Functions typically return single value
                let estimated_cost = input_physical.get_cost() + (arguments.len() as f64 * 2.0);

                PhysicalNode::GenericFunction {
                    function_name: function_name.clone(),
                    arguments: arguments.clone(),
                    input: input_physical,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Aggregate {
                group_by,
                aggregates,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let estimated_rows = if group_by.is_empty() {
                    1 // No GROUP BY means single aggregate result
                } else {
                    input_rows / 10 // Estimate groups as 10% of input rows
                };
                let estimated_cost = input_physical.get_cost() + (input_rows as f64 * 0.05);

                // Convert logical aggregates to physical aggregates
                let physical_aggregates: Vec<AggregateItem> = aggregates
                    .iter()
                    .map(|agg| {
                        AggregateItem {
                            function: agg.function.clone(),
                            expression: agg.expression.clone(),
                            alias: agg.alias.clone(),
                            output_type: OutputType::Float, // Default to Float for most aggregates
                        }
                    })
                    .collect();

                // Choose aggregation strategy based on data size and group count
                if input_rows > 10000 {
                    PhysicalNode::HashAggregate {
                        group_by: group_by.clone(),
                        aggregates: physical_aggregates,
                        input: input_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                } else {
                    PhysicalNode::SortAggregate {
                        group_by: group_by.clone(),
                        aggregates: physical_aggregates,
                        input: input_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                }
            }

            LogicalNode::Having { condition, input } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let selectivity = 0.3; // HAVING typically filters more aggressively than WHERE
                let estimated_rows = (input_rows as f64 * selectivity) as usize;
                let estimated_cost = input_physical.get_cost() + (input_rows as f64 * 0.02);

                PhysicalNode::Having {
                    condition: condition.clone(),
                    input: input_physical,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();

                // Path traversal can be expensive depending on path type
                let complexity_factor = match path_type {
                    PathType::Walk => 1.0,        // Most efficient
                    PathType::Trail => 2.0,       // Track edges
                    PathType::SimplePath => 3.0,  // Track vertices
                    PathType::AcyclicPath => 4.0, // Strictest constraints
                };

                let estimated_rows = input_rows * 10; // Paths can multiply results
                let estimated_cost =
                    input_physical.get_cost() + (estimated_rows as f64 * complexity_factor * 0.1);

                PhysicalNode::PathTraversal {
                    path_type: path_type.clone(),
                    from_variable: from_variable.clone(),
                    to_variable: to_variable.clone(),
                    path_elements: path_elements.clone(),
                    input: input_physical,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::TextSearch {
                variable: _variable,
                field,
                query,
                search_type,
                min_score,
                limit,
                input,
            } => {
                // Convert TextSearch logical node into a dedicated TextIndexScan physical operator.
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let estimated_rows = (input_rows as f64 * 0.1) as usize; // assume 10% selectivity
                let estimated_cost = input_physical.get_cost() + 20.0; // fixed cost for text search

                PhysicalNode::TextIndexScan {
                    index_name: format!("text_index::{}", field),
                    field: field.clone(),
                    query: query.clone(),
                    search_type: search_type.clone(),
                    min_score: *min_score,
                    limit: *limit,
                    estimated_rows,
                    estimated_cost,
                }
            }

            // Join conversion
            LogicalNode::Join {
                join_type,
                condition,
                left,
                right,
            } => {
                let left_physical = Box::new(Self::convert_logical_node(left));
                let right_physical = Box::new(Self::convert_logical_node(right));
                let left_rows = left_physical.get_row_count();
                let right_rows = right_physical.get_row_count();

                // Estimate join cardinality based on join type
                let estimated_rows = match join_type {
                    JoinType::Inner => (left_rows * right_rows) / 1000, // Assume some selectivity
                    JoinType::LeftOuter => left_rows.max((left_rows * right_rows) / 1000),
                    JoinType::RightOuter => right_rows.max((left_rows * right_rows) / 1000),
                    JoinType::FullOuter => left_rows + right_rows,
                    JoinType::Cross => left_rows * right_rows,
                    JoinType::LeftSemi => left_rows / 2, // Semi join returns subset of left
                    JoinType::LeftAnti => left_rows / 2, // Anti join returns complement subset
                };

                let estimated_cost = left_physical.get_cost()
                    + right_physical.get_cost()
                    + (left_rows * right_rows) as f64 * 0.001; // Join cost

                // Choose physical join strategy based on sizes
                if left_rows > 10000 && right_rows > 10000 {
                    PhysicalNode::SortMergeJoin {
                        join_type: join_type.clone(),
                        left_keys: vec![],  // Would be extracted from condition
                        right_keys: vec![], // Would be extracted from condition
                        left: left_physical,
                        right: right_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                } else if right_rows < left_rows {
                    PhysicalNode::HashJoin {
                        join_type: join_type.clone(),
                        condition: condition.clone(),
                        build_keys: vec![],    // Would be extracted from condition
                        probe_keys: vec![],    // Would be extracted from condition
                        build: right_physical, // Build on smaller side
                        probe: left_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                } else {
                    PhysicalNode::NestedLoopJoin {
                        join_type: join_type.clone(),
                        condition: condition.clone(),
                        left: left_physical,
                        right: right_physical,
                        estimated_rows,
                        estimated_cost,
                    }
                }
            }

            // Subquery logical to physical conversion
            LogicalNode::ExistsSubquery { subquery, .. } => {
                let subplan = Box::new(Self::convert_logical_node(subquery));
                let estimated_rows = subplan.get_row_count();
                let estimated_cost = subplan.get_cost() + 10.0; // Add overhead for EXISTS check

                PhysicalNode::ExistsSubquery {
                    subplan,
                    estimated_rows,
                    estimated_cost,
                    optimized: true, // Enable early termination by default
                }
            }

            LogicalNode::NotExistsSubquery { subquery, .. } => {
                let subplan = Box::new(Self::convert_logical_node(subquery));
                let estimated_rows = subplan.get_row_count();
                let estimated_cost = subplan.get_cost() + 10.0;

                PhysicalNode::NotExistsSubquery {
                    subplan,
                    estimated_rows,
                    estimated_cost,
                    optimized: true,
                }
            }

            LogicalNode::InSubquery {
                expression,
                subquery,
                ..
            } => {
                let subplan = Box::new(Self::convert_logical_node(subquery));
                let estimated_rows = subplan.get_row_count();
                let estimated_cost = subplan.get_cost() + (estimated_rows as f64 * 0.1); // Cost of IN comparison

                PhysicalNode::InSubquery {
                    expression: expression.clone(),
                    subplan,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::NotInSubquery {
                expression,
                subquery,
                ..
            } => {
                let subplan = Box::new(Self::convert_logical_node(subquery));
                let estimated_rows = subplan.get_row_count();
                let estimated_cost = subplan.get_cost() + (estimated_rows as f64 * 0.1);

                PhysicalNode::NotInSubquery {
                    expression: expression.clone(),
                    subplan,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::ScalarSubquery { subquery, .. } => {
                let subplan = Box::new(Self::convert_logical_node(subquery));
                let estimated_rows = 1; // Scalar subquery returns single value
                let estimated_cost = subplan.get_cost() + 5.0; // Lower overhead for scalar

                PhysicalNode::ScalarSubquery {
                    subplan,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::WithQuery { original_query } => {
                // WITH queries need special handling - preserve original structure for executor
                let estimated_rows = 100; // Default estimate for WITH queries
                let estimated_cost = 50.0; // Base cost for WITH query execution

                PhysicalNode::WithQuery {
                    original_query: original_query.clone(),
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Unwind {
                expression,
                variable,
                input,
            } => {
                // UNWIND expands arrays/collections into multiple rows
                let base_estimated_rows = 10; // Default estimate for UNWIND expansion factor
                let estimated_cost = 5.0; // Base cost for UNWIND operation

                let input_physical = input
                    .as_ref()
                    .map(|inp| Box::new(Self::convert_logical_node(inp)));

                PhysicalNode::Unwind {
                    expression: expression.clone(),
                    variable: variable.clone(),
                    input: input_physical,
                    estimated_rows: base_estimated_rows,
                    estimated_cost,
                }
            }

            // Data Modification Operations
            LogicalNode::Insert {
                patterns,
                identifier_mappings: _,
            } => {
                let mut node_creations = Vec::new();
                let mut edge_creations = Vec::new();

                // Convert logical insert patterns to physical operations
                for pattern in patterns {
                    match pattern {
                        crate::plan::logical::InsertPattern::CreateNode {
                            storage_id,
                            labels,
                            properties,
                            ..
                        } => {
                            node_creations.push(NodeCreation {
                                storage_id: storage_id.clone(),
                                labels: labels.clone(),
                                properties: properties.clone(),
                            });
                        }
                        crate::plan::logical::InsertPattern::CreateEdge {
                            storage_id,
                            from_node_id,
                            to_node_id,
                            label,
                            properties,
                            ..
                        } => {
                            edge_creations.push(EdgeCreation {
                                storage_id: storage_id.clone(),
                                from_node_id: from_node_id.clone(),
                                to_node_id: to_node_id.clone(),
                                label: label.clone(),
                                properties: properties.clone(),
                            });
                        }
                    }
                }

                let estimated_ops = node_creations.len() + edge_creations.len();
                let estimated_cost = estimated_ops as f64 * 2.0; // Base cost per insertion

                PhysicalNode::Insert {
                    node_creations,
                    edge_creations,
                    estimated_ops,
                    estimated_cost,
                }
            }

            LogicalNode::Update {
                target_variable,
                properties,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let estimated_ops = input_rows; // One update per input row
                let estimated_cost = input_physical.get_cost() + (estimated_ops as f64 * 1.5);

                PhysicalNode::Update {
                    target_variable: target_variable.clone(),
                    properties: properties.clone(),
                    input: input_physical,
                    estimated_ops,
                    estimated_cost,
                }
            }

            LogicalNode::Delete {
                target_variables,
                detach,
                input,
            } => {
                let input_physical = Box::new(Self::convert_logical_node(input));
                let input_rows = input_physical.get_row_count();
                let estimated_ops = input_rows; // One delete per input row
                let estimated_cost = input_physical.get_cost()
                    + (estimated_ops as f64 * if *detach { 3.0 } else { 2.0 }); // DETACH DELETE is more expensive

                PhysicalNode::Delete {
                    target_variables: target_variables.clone(),
                    detach: *detach,
                    input: input_physical,
                    estimated_ops,
                    estimated_cost,
                }
            }

            LogicalNode::Union { inputs, all } => {
                // Convert all input logical nodes to physical nodes
                let physical_inputs: Vec<PhysicalNode> =
                    inputs.iter().map(Self::convert_logical_node).collect();

                // Calculate estimated rows and cost
                let estimated_rows: usize = physical_inputs
                    .iter()
                    .map(|input| input.get_row_count())
                    .sum();

                let estimated_cost: f64 = physical_inputs
                    .iter()
                    .map(|input| input.get_cost())
                    .sum::<f64>()
                    + (estimated_rows as f64 * 0.1); // Small overhead for union operation

                // Pass the 'all' flag to distinguish UNION vs UNION ALL
                PhysicalNode::UnionAll {
                    inputs: physical_inputs,
                    all: *all, // true for UNION ALL, false for UNION (needs deduplication)
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Intersect { left, right, all } => {
                let left_physical = Box::new(Self::convert_logical_node(left));
                let right_physical = Box::new(Self::convert_logical_node(right));

                // Intersect returns at most the minimum of both sides
                let estimated_rows = left_physical
                    .get_row_count()
                    .min(right_physical.get_row_count());
                let estimated_cost = left_physical.get_cost()
                    + right_physical.get_cost()
                    + (estimated_rows as f64 * 0.5); // Overhead for intersect operation

                PhysicalNode::Intersect {
                    left: left_physical,
                    right: right_physical,
                    all: *all,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::Except { left, right, all } => {
                let left_physical = Box::new(Self::convert_logical_node(left));
                let right_physical = Box::new(Self::convert_logical_node(right));

                // Except returns at most the left side minus intersection with right
                let left_rows = left_physical.get_row_count();
                let right_rows = right_physical.get_row_count();
                let estimated_rows = left_rows.saturating_sub(right_rows.min(left_rows));
                let estimated_cost = left_physical.get_cost()
                    + right_physical.get_cost()
                    + (estimated_rows as f64 * 0.5); // Overhead for except operation

                PhysicalNode::Except {
                    left: left_physical,
                    right: right_physical,
                    all: *all,
                    estimated_rows,
                    estimated_cost,
                }
            }

            LogicalNode::SingleRow => {
                PhysicalNode::SingleRow {
                    estimated_rows: 1,     // Always exactly 1 row
                    estimated_cost: 0.001, // Minimal cost
                }
            }
        }
    }

    /// Get total estimated cost
    pub fn get_estimated_cost(&self) -> f64 {
        self.estimated_cost
    }

    /// Get estimated row count
    pub fn get_estimated_rows(&self) -> usize {
        self.estimated_rows
    }

    /// Get all physical operators in the plan
    pub fn get_operators(&self) -> Vec<PhysicalOperator> {
        self.root.get_operators()
    }

    /// Extract label information from a logical node (helper for text search optimization)
    #[allow(dead_code)] // FALSE POSITIVE - Recursively called at line 1085, compiler doesn't detect recursive usage
    fn extract_label_from_logical_node(node: &LogicalNode) -> Option<String> {
        match node {
            LogicalNode::NodeScan { labels, .. } => {
                // Return the first label if available
                labels.first().cloned()
            }
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Limit { input, .. }
            | LogicalNode::Sort { input, .. } => {
                // Recursively check input
                Self::extract_label_from_logical_node(input)
            }
            _ => None,
        }
    }
}

impl PhysicalNode {
    /// Get the estimated cost of this node
    pub fn get_cost(&self) -> f64 {
        match self {
            PhysicalNode::NodeSeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NodeIndexScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::EdgeSeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::IndexedExpand { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashExpand { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::PathTraversal { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Filter { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Project { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NestedLoopJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::SortMergeJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::UnionAll { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Intersect { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Except { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashAggregate { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::SortAggregate { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Having { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::ExternalSort { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::InMemorySort { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Distinct { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Limit { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::GenericFunction { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::ExistsSubquery { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NotExistsSubquery { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::InSubquery { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NotInSubquery { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::ScalarSubquery { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::WithQuery { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Unwind { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::GraphIndexScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::TextIndexScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::IndexJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Insert { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Update { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Delete { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::SingleRow { estimated_cost, .. } => *estimated_cost,
        }
    }

    /// Get the estimated row count of this node
    pub fn get_row_count(&self) -> usize {
        match self {
            PhysicalNode::NodeSeqScan { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::NodeIndexScan { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::EdgeSeqScan { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::IndexedExpand { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::HashExpand { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::PathTraversal { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Filter { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Project { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::HashJoin { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::NestedLoopJoin { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::SortMergeJoin { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::UnionAll { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Intersect { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Except { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::HashAggregate { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::SortAggregate { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::ExternalSort { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::InMemorySort { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Distinct { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Limit { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::GenericFunction { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::ExistsSubquery { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::NotExistsSubquery { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::InSubquery { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::NotInSubquery { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::ScalarSubquery { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::WithQuery { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Having { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Unwind { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::GraphIndexScan { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::TextIndexScan { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::IndexJoin { estimated_rows, .. } => *estimated_rows,
            PhysicalNode::Insert { estimated_ops, .. } => *estimated_ops,
            PhysicalNode::Update { estimated_ops, .. } => *estimated_ops,
            PhysicalNode::Delete { estimated_ops, .. } => *estimated_ops,
            PhysicalNode::SingleRow { estimated_rows, .. } => *estimated_rows,
        }
    }

    /// Get all operators in this subtree
    pub fn get_operators(&self) -> Vec<PhysicalOperator> {
        let mut operators = vec![self.get_operator_type()];

        match self {
            PhysicalNode::IndexedExpand { input, .. }
            | PhysicalNode::HashExpand { input, .. }
            | PhysicalNode::PathTraversal { input, .. }
            | PhysicalNode::Filter { input, .. }
            | PhysicalNode::Project { input, .. }
            | PhysicalNode::HashAggregate { input, .. }
            | PhysicalNode::SortAggregate { input, .. }
            | PhysicalNode::ExternalSort { input, .. }
            | PhysicalNode::InMemorySort { input, .. }
            | PhysicalNode::Distinct { input, .. }
            | PhysicalNode::Limit { input, .. }
            | PhysicalNode::GenericFunction { input, .. } => {
                operators.extend(input.get_operators());
            }

            PhysicalNode::ExistsSubquery { subplan, .. }
            | PhysicalNode::NotExistsSubquery { subplan, .. }
            | PhysicalNode::InSubquery { subplan, .. }
            | PhysicalNode::NotInSubquery { subplan, .. }
            | PhysicalNode::ScalarSubquery { subplan, .. } => {
                operators.extend(subplan.get_operators());
            }

            PhysicalNode::WithQuery { .. } => {
                // WITH queries don't have nested subplans in the physical structure
                // The execution will be handled specially
            }

            PhysicalNode::Unwind {
                input: Some(input_node),
                ..
            } => {
                operators.extend(input_node.get_operators());
            }

            PhysicalNode::Unwind { input: None, .. } => {
                // No input node
            }

            PhysicalNode::HashJoin { build, probe, .. }
            | PhysicalNode::NestedLoopJoin {
                left: build,
                right: probe,
                ..
            }
            | PhysicalNode::SortMergeJoin {
                left: build,
                right: probe,
                ..
            } => {
                operators.extend(build.get_operators());
                operators.extend(probe.get_operators());
            }

            PhysicalNode::IndexJoin { left, right, .. } => {
                operators.extend(left.get_operators());
                operators.extend(right.get_operators());
            }

            PhysicalNode::UnionAll { inputs, .. } => {
                for input in inputs {
                    operators.extend(input.get_operators());
                }
            }

            PhysicalNode::Intersect { left, right, .. } => {
                operators.extend(left.get_operators());
                operators.extend(right.get_operators());
            }

            PhysicalNode::Except { left, right, .. } => {
                operators.extend(left.get_operators());
                operators.extend(right.get_operators());
            }

            _ => {} // Leaf nodes
        }

        operators
    }

    /// Get the operator type for this node
    fn get_operator_type(&self) -> PhysicalOperator {
        match self {
            PhysicalNode::NodeSeqScan { .. }
            | PhysicalNode::NodeIndexScan { .. }
            | PhysicalNode::EdgeSeqScan { .. } => PhysicalOperator::Scan,

            PhysicalNode::IndexedExpand { .. } | PhysicalNode::HashExpand { .. } => {
                PhysicalOperator::Join
            }

            PhysicalNode::Filter { .. } | PhysicalNode::Having { .. } => PhysicalOperator::Filter,
            PhysicalNode::Project { .. } => PhysicalOperator::Project,

            PhysicalNode::HashJoin { .. }
            | PhysicalNode::NestedLoopJoin { .. }
            | PhysicalNode::SortMergeJoin { .. } => PhysicalOperator::Join,

            PhysicalNode::UnionAll { .. } => PhysicalOperator::Union,
            PhysicalNode::Intersect { .. } => PhysicalOperator::Union,
            PhysicalNode::Except { .. } => PhysicalOperator::Union,

            PhysicalNode::HashAggregate { .. } | PhysicalNode::SortAggregate { .. } => {
                PhysicalOperator::Aggregate
            }

            PhysicalNode::ExternalSort { .. } | PhysicalNode::InMemorySort { .. } => {
                PhysicalOperator::Sort
            }

            PhysicalNode::Distinct { .. } => PhysicalOperator::Sort, // DISTINCT is like sorting with deduplication

            PhysicalNode::Limit { .. } => PhysicalOperator::Limit,
            PhysicalNode::GenericFunction { .. } => PhysicalOperator::Aggregate, // Generic functions are treated as aggregates for now
            PhysicalNode::PathTraversal { .. } => PhysicalOperator::Join, // Path traversal is similar to joins

            PhysicalNode::ExistsSubquery { .. }
            | PhysicalNode::NotExistsSubquery { .. }
            | PhysicalNode::InSubquery { .. }
            | PhysicalNode::NotInSubquery { .. }
            | PhysicalNode::ScalarSubquery { .. } => PhysicalOperator::Subquery,
            PhysicalNode::WithQuery { .. } => PhysicalOperator::WithQuery,
            PhysicalNode::Unwind { .. } => PhysicalOperator::Unwind,
            PhysicalNode::GraphIndexScan { .. } => PhysicalOperator::GraphIndexScan,
            PhysicalNode::TextIndexScan { .. } => PhysicalOperator::TextIndexScan,
            PhysicalNode::IndexJoin { .. } => PhysicalOperator::IndexJoin,
            PhysicalNode::Insert { .. } => PhysicalOperator::Insert,
            PhysicalNode::Update { .. } => PhysicalOperator::Update,
            PhysicalNode::Delete { .. } => PhysicalOperator::Delete,
            PhysicalNode::SingleRow { .. } => PhysicalOperator::SingleRow,
        }
    }
}
