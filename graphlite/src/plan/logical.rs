// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Logical query plan representation
//!
//! Logical plans represent the structure of a query without considering
//! physical execution details. They are optimized for correctness and
//! logical equivalence transformations.

use crate::ast::{
    EdgeDirection, Expression, PathPattern, PathQuantifier, PathType, PatternElement,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logical query plan tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalPlan {
    pub root: LogicalNode,
    pub variables: HashMap<String, VariableInfo>,
}

/// Information about a variable in the query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    pub name: String,
    pub entity_type: EntityType,
    pub labels: Vec<String>,
    pub required_properties: Vec<String>,
}

/// Type of entity a variable represents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityType {
    Node,
    Edge,
}

/// Element in a path traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathElement {
    pub edge_variable: Option<String>,
    pub node_variable: String,
    pub edge_labels: Vec<String>,
    pub direction: EdgeDirection,
    pub quantifier: Option<PathQuantifier>,
}

/// Pattern for INSERT operation (resolved during planning)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsertPattern {
    /// Node to be created
    CreateNode {
        /// Computed storage ID for the node
        storage_id: String,
        /// Node labels
        labels: Vec<String>,
        /// Node properties (resolved expressions)
        properties: HashMap<String, Expression>,
        /// Original identifier from the query (if any)
        original_identifier: Option<String>,
    },
    /// Edge to be created
    CreateEdge {
        /// Computed storage ID for the edge
        storage_id: String,
        /// Source node storage ID
        from_node_id: String,
        /// Target node storage ID
        to_node_id: String,
        /// Edge label
        label: String,
        /// Edge properties (resolved expressions)
        properties: HashMap<String, Expression>,
        /// Original identifier from the query (if any)
        original_identifier: Option<String>,
    },
}

/// Node identifier for mapping user identifiers to storage IDs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentifier {
    /// Storage ID (content-based hash)
    pub storage_id: String,
    /// Node labels
    pub labels: Vec<String>,
    /// Whether this is a reference to an existing node
    pub is_reference: bool,
}

/// Logical plan node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalNode {
    /// Scan all nodes with given label(s)
    NodeScan {
        variable: String,
        labels: Vec<String>,
        properties: Option<HashMap<String, Expression>>,
    },

    /// Scan all edges with given label(s)
    EdgeScan {
        variable: String,
        labels: Vec<String>,
        properties: Option<HashMap<String, Expression>>,
    },

    /// Expand from a node along edges
    Expand {
        from_variable: String,
        edge_variable: Option<String>,
        to_variable: String,
        edge_labels: Vec<String>,
        direction: EdgeDirection,
        properties: Option<HashMap<String, Expression>>,
        input: Box<LogicalNode>,
    },

    /// Path traversal with type constraints
    PathTraversal {
        path_type: PathType,
        from_variable: String,
        to_variable: String,
        path_elements: Vec<PathElement>,
        input: Box<LogicalNode>,
    },

    /// Filter rows based on condition
    Filter {
        condition: Expression,
        input: Box<LogicalNode>,
    },

    /// Project specific columns
    Project {
        expressions: Vec<ProjectExpression>,
        input: Box<LogicalNode>,
    },

    /// Join two logical plans
    Join {
        join_type: JoinType,
        condition: Option<Expression>,
        left: Box<LogicalNode>,
        right: Box<LogicalNode>,
    },

    /// Union of multiple plans
    Union { inputs: Vec<LogicalNode>, all: bool },

    /// Apply aggregation
    Aggregate {
        group_by: Vec<Expression>,
        aggregates: Vec<AggregateExpression>,
        input: Box<LogicalNode>,
    },

    /// HAVING filter for post-aggregation filtering
    Having {
        condition: Expression,
        input: Box<LogicalNode>,
    },

    /// Sort results
    Sort {
        expressions: Vec<SortExpression>,
        input: Box<LogicalNode>,
    },

    /// Remove duplicate rows
    Distinct { input: Box<LogicalNode> },

    /// Limit number of results
    Limit {
        count: usize,
        offset: Option<usize>,
        input: Box<LogicalNode>,
    },

    /// Generic function call
    GenericFunction {
        function_name: String,
        arguments: Vec<Expression>,
        input: Box<LogicalNode>,
    },

    /// Intersect operation
    Intersect {
        left: Box<LogicalNode>,
        right: Box<LogicalNode>,
        all: bool,
    },

    /// Except operation
    Except {
        left: Box<LogicalNode>,
        right: Box<LogicalNode>,
        all: bool,
    },

    /// EXISTS subquery
    ExistsSubquery {
        subquery: Box<LogicalNode>,
        outer_variables: Vec<String>, // Variables from outer query used in subquery
    },

    /// NOT EXISTS subquery
    NotExistsSubquery {
        subquery: Box<LogicalNode>,
        outer_variables: Vec<String>,
    },

    /// IN subquery
    InSubquery {
        expression: Expression, // Left side of IN
        subquery: Box<LogicalNode>,
        outer_variables: Vec<String>,
    },

    /// NOT IN subquery
    NotInSubquery {
        expression: Expression,
        subquery: Box<LogicalNode>,
        outer_variables: Vec<String>,
    },

    /// Scalar subquery (returns single value)
    ScalarSubquery {
        subquery: Box<LogicalNode>,
        outer_variables: Vec<String>,
    },

    /// WITH query that needs special execution handling
    WithQuery {
        original_query: Box<crate::ast::WithQuery>,
    },

    /// UNWIND expression into individual rows
    Unwind {
        expression: Expression,
        variable: String,
        input: Option<Box<LogicalNode>>,
    },

    // Data Modification Operations
    /// INSERT operation for creating nodes and edges
    Insert {
        /// Patterns to insert (nodes and edges)
        patterns: Vec<InsertPattern>,
        /// Identifier mappings resolved during planning
        identifier_mappings: HashMap<String, NodeIdentifier>,
    },

    /// UPDATE operation for modifying properties
    Update {
        target_variable: String,
        properties: HashMap<String, Expression>,
        input: Box<LogicalNode>,
    },

    /// DELETE operation for removing nodes/edges
    Delete {
        target_variables: Vec<String>,
        detach: bool,
        input: Box<LogicalNode>,
    },

    /// Produces exactly one row with no data
    /// Used for LET statements and standalone RETURN queries
    SingleRow,
}

/// Join types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JoinType {
    Inner,
    LeftOuter,
    RightOuter,
    FullOuter,
    Cross,
    LeftSemi, // Used for EXISTS subquery unnesting
    LeftAnti, // Used for NOT EXISTS subquery unnesting
}

/// Project expression with optional alias
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExpression {
    pub expression: Expression,
    pub alias: Option<String>,
}

/// Aggregate expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateExpression {
    pub function: AggregateFunction,
    pub expression: Expression,
    pub alias: Option<String>,
}

/// Aggregate functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Collect,
}

/// Sort expression with order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortExpression {
    pub expression: Expression,
    pub ascending: bool,
}

impl LogicalPlan {
    /// Create a new logical plan with root node
    pub fn new(root: LogicalNode) -> Self {
        Self {
            root,
            variables: HashMap::new(),
        }
    }

    /// Add variable information to the plan
    pub fn add_variable(&mut self, name: String, info: VariableInfo) {
        self.variables.insert(name, info);
    }

    /// Get variable information
    pub fn get_variable(&self, name: &str) -> Option<&VariableInfo> {
        self.variables.get(name)
    }

    /// Get all variables used in the plan
    pub fn get_variables(&self) -> &HashMap<String, VariableInfo> {
        &self.variables
    }

    /// Convert a path pattern to logical plan nodes
    pub fn from_path_pattern(pattern: &PathPattern) -> Result<LogicalNode, String> {
        if pattern.elements.is_empty() {
            return Err("Empty path pattern".to_string());
        }

        // If path type is specified and not WALK, use PathTraversal
        if let Some(path_type) = &pattern.path_type {
            if *path_type != PathType::Walk {
                return Self::create_path_traversal(pattern, path_type.clone());
            }
        }

        let mut current_node: Option<LogicalNode> = None;
        let mut i = 0;

        while i < pattern.elements.len() {
            match &pattern.elements[i] {
                PatternElement::Node(node) => {
                    let variable = node
                        .identifier
                        .clone()
                        .unwrap_or_else(|| format!("_node_{}", i));

                    let node_scan = LogicalNode::NodeScan {
                        variable: variable.clone(),
                        labels: node.labels.clone(),
                        properties: node.properties.as_ref().map(|props| {
                            props
                                .properties
                                .iter()
                                .map(|p| (p.key.clone(), p.value.clone()))
                                .collect()
                        }),
                    };

                    if current_node.is_none() {
                        current_node = Some(node_scan);
                    }

                    // Look ahead for edge pattern
                    if i + 1 < pattern.elements.len() {
                        if let PatternElement::Edge(edge) = &pattern.elements[i + 1] {
                            // Look ahead for next node
                            if i + 2 < pattern.elements.len() {
                                if let PatternElement::Node(next_node) = &pattern.elements[i + 2] {
                                    let edge_variable = edge.identifier.clone();
                                    let to_variable = next_node
                                        .identifier
                                        .clone()
                                        .unwrap_or_else(|| format!("_node_{}", i + 2));

                                    let expand = LogicalNode::Expand {
                                        from_variable: variable,
                                        edge_variable,
                                        to_variable: to_variable.clone(),
                                        edge_labels: edge.labels.clone(),
                                        direction: edge.direction.clone(),
                                        properties: edge.properties.as_ref().map(|props| {
                                            props
                                                .properties
                                                .iter()
                                                .map(|p| (p.key.clone(), p.value.clone()))
                                                .collect()
                                        }),
                                        input: Box::new(current_node.unwrap()),
                                    };

                                    current_node = Some(expand);

                                    // Add filter for target node properties if they exist
                                    if let Some(target_props) = &next_node.properties {
                                        for property in &target_props.properties {
                                            let property_access = Expression::PropertyAccess(
                                                crate::ast::PropertyAccess {
                                                    object: to_variable.clone(),
                                                    property: property.key.clone(),
                                                    location: crate::ast::Location::default(),
                                                },
                                            );

                                            let filter_condition = Expression::Binary(
                                                crate::ast::BinaryExpression {
                                                    left: Box::new(property_access),
                                                    operator: crate::ast::Operator::Equal,
                                                    right: Box::new(property.value.clone()),
                                                    location: crate::ast::Location::default(),
                                                },
                                            );

                                            let filter = LogicalNode::Filter {
                                                condition: filter_condition,
                                                input: Box::new(current_node.unwrap()),
                                            };

                                            current_node = Some(filter);
                                        }
                                    }
                                    i += 1; // Move to edge position, will be incremented to target node at end of loop
                                }
                            }
                        }
                    }
                }
                PatternElement::Edge(_) => {
                    // Edges are handled with nodes
                }
            }
            i += 1;
        }

        current_node.ok_or_else(|| "Failed to create logical plan from pattern".to_string())
    }

    /// Create a PathTraversal node for path types that need special handling
    fn create_path_traversal(
        pattern: &PathPattern,
        path_type: PathType,
    ) -> Result<LogicalNode, String> {
        // Extract start and end nodes
        let (start_node, end_node) = Self::extract_start_end_nodes(pattern)?;

        // Extract path elements (edges and intermediate nodes)
        let path_elements = Self::extract_path_elements(pattern)?;

        // Create initial node scan for start node
        let start_variable = start_node
            .identifier
            .clone()
            .unwrap_or_else(|| "_start_node".to_string());

        let node_scan = LogicalNode::NodeScan {
            variable: start_variable.clone(),
            labels: start_node.labels.clone(),
            properties: start_node.properties.as_ref().map(|props| {
                props
                    .properties
                    .iter()
                    .map(|p| (p.key.clone(), p.value.clone()))
                    .collect()
            }),
        };

        let end_variable = end_node
            .identifier
            .clone()
            .unwrap_or_else(|| "_end_node".to_string());

        // Create PathTraversal node
        Ok(LogicalNode::PathTraversal {
            path_type,
            from_variable: start_variable,
            to_variable: end_variable,
            path_elements,
            input: Box::new(node_scan),
        })
    }

    /// Extract start and end nodes from pattern
    fn extract_start_end_nodes(
        pattern: &PathPattern,
    ) -> Result<(&crate::ast::Node, &crate::ast::Node), String> {
        let first = pattern.elements.first().ok_or("Pattern has no elements")?;
        let last = pattern.elements.last().ok_or("Pattern has no elements")?;

        match (first, last) {
            (PatternElement::Node(start), PatternElement::Node(end)) => Ok((start, end)),
            _ => Err("Pattern must start and end with nodes".to_string()),
        }
    }

    /// Extract path elements (edges and intermediate nodes) from pattern
    fn extract_path_elements(pattern: &PathPattern) -> Result<Vec<PathElement>, String> {
        let mut elements = Vec::new();
        let mut i = 1; // Skip first node

        while i < pattern.elements.len() - 1 {
            // Skip last node
            if let PatternElement::Edge(edge) = &pattern.elements[i] {
                if i + 1 < pattern.elements.len() - 1 {
                    if let PatternElement::Node(node) = &pattern.elements[i + 1] {
                        elements.push(PathElement {
                            edge_variable: edge.identifier.clone(),
                            node_variable: node
                                .identifier
                                .clone()
                                .unwrap_or_else(|| format!("_node_{}", i + 1)),
                            edge_labels: edge.labels.clone(),
                            direction: edge.direction.clone(),
                            quantifier: edge.quantifier.clone(),
                        });
                        i += 2;
                        continue;
                    }
                }
            }
            i += 1;
        }

        Ok(elements)
    }

    /// Apply filter to the plan
    pub fn apply_filter(mut self, condition: Expression) -> Self {
        self.root = LogicalNode::Filter {
            condition,
            input: Box::new(self.root),
        };
        self
    }

    /// Apply HAVING filter for post-aggregation filtering
    pub fn apply_having(mut self, condition: Expression) -> Self {
        self.root = LogicalNode::Having {
            condition,
            input: Box::new(self.root),
        };
        self
    }

    /// Apply projection to the plan
    pub fn apply_projection(mut self, expressions: Vec<ProjectExpression>) -> Self {
        self.root = LogicalNode::Project {
            expressions,
            input: Box::new(self.root),
        };
        self
    }

    /// Apply aggregation to the plan
    pub fn apply_aggregation(
        mut self,
        group_by: Vec<Expression>,
        project_expressions: Vec<ProjectExpression>,
    ) -> Self {
        // Convert project expressions to aggregate expressions, preserving order
        let mut aggregates = Vec::new();
        for expr in &project_expressions {
            if let Expression::FunctionCall(func_call) = &expr.expression {
                // Map function names to aggregate functions
                let aggregate_function = match func_call.name.to_uppercase().as_str() {
                    "COUNT" => AggregateFunction::Count,
                    "SUM" => AggregateFunction::Sum,
                    "AVG" | "AVERAGE" => AggregateFunction::Avg,
                    "MIN" => AggregateFunction::Min,
                    "MAX" => AggregateFunction::Max,
                    "COLLECT" => AggregateFunction::Collect,
                    _ => continue, // Skip non-aggregate functions
                };

                // Get the argument expression (if any)
                let arg_expr = if func_call.arguments.is_empty() {
                    Expression::Literal(crate::ast::Literal::Integer(1)) // COUNT(*) case
                } else {
                    func_call.arguments[0].clone()
                };

                aggregates.push(AggregateExpression {
                    function: aggregate_function,
                    expression: arg_expr,
                    alias: expr.alias.clone(),
                });
            }
            // For non-aggregate expressions in group context, they should be in GROUP BY
        }

        // Create aggregate node
        self.root = LogicalNode::Aggregate {
            group_by,
            aggregates,
            input: Box::new(self.root),
        };

        self
    }

    /// Apply DISTINCT to remove duplicates
    pub fn apply_distinct(mut self) -> Self {
        self.root = LogicalNode::Distinct {
            input: Box::new(self.root),
        };
        self
    }

    /// Apply sorting to the plan
    pub fn apply_sort(mut self, expressions: Vec<SortExpression>) -> Self {
        self.root = LogicalNode::Sort {
            expressions,
            input: Box::new(self.root),
        };
        self
    }

    /// Apply limit to the plan
    pub fn apply_limit(mut self, count: usize, offset: Option<usize>) -> Self {
        self.root = LogicalNode::Limit {
            count,
            offset,
            input: Box::new(self.root),
        };
        self
    }

    /// Apply union operation
    pub fn apply_union(self, right: LogicalPlan, all: bool) -> Self {
        // Merge variables from both sides of the UNION
        let mut merged_variables = self.variables;
        for (name, info) in right.variables {
            merged_variables.insert(name, info);
        }

        LogicalPlan {
            root: LogicalNode::Union {
                inputs: vec![self.root, right.root],
                all,
            },
            variables: merged_variables, // Union inherits variables from both sides
        }
    }

    /// Apply intersect operation
    pub fn apply_intersect(self, right: LogicalPlan, all: bool) -> Self {
        // Merge variables from both sides of the INTERSECT
        let mut merged_variables = self.variables;
        for (name, info) in right.variables {
            merged_variables.insert(name, info);
        }

        LogicalPlan {
            root: LogicalNode::Intersect {
                left: Box::new(self.root),
                right: Box::new(right.root),
                all,
            },
            variables: merged_variables, // Intersect inherits variables from both sides
        }
    }

    /// Apply except operation
    pub fn apply_except(self, right: LogicalPlan, all: bool) -> Self {
        // Merge variables from both sides of the EXCEPT
        let mut merged_variables = self.variables;
        for (name, info) in right.variables {
            merged_variables.insert(name, info);
        }

        LogicalPlan {
            root: LogicalNode::Except {
                left: Box::new(self.root),
                right: Box::new(right.root),
                all,
            },
            variables: merged_variables, // Except inherits variables from both sides
        }
    }

    /// Apply EXISTS subquery filter
    pub fn apply_exists_subquery(
        self,
        subquery: LogicalPlan,
        outer_variables: Vec<String>,
    ) -> Self {
        let mut variables = self.variables;
        variables.extend(subquery.variables); // Merge variables from subquery

        LogicalPlan {
            root: LogicalNode::ExistsSubquery {
                subquery: Box::new(subquery.root),
                outer_variables,
            },
            variables,
        }
    }

    /// Apply NOT EXISTS subquery filter
    pub fn apply_not_exists_subquery(
        self,
        subquery: LogicalPlan,
        outer_variables: Vec<String>,
    ) -> Self {
        let mut variables = self.variables;
        variables.extend(subquery.variables);

        LogicalPlan {
            root: LogicalNode::NotExistsSubquery {
                subquery: Box::new(subquery.root),
                outer_variables,
            },
            variables,
        }
    }

    /// Apply IN subquery filter
    pub fn apply_in_subquery(
        self,
        expression: Expression,
        subquery: LogicalPlan,
        outer_variables: Vec<String>,
    ) -> Self {
        let mut variables = self.variables;
        variables.extend(subquery.variables);

        LogicalPlan {
            root: LogicalNode::InSubquery {
                expression,
                subquery: Box::new(subquery.root),
                outer_variables,
            },
            variables,
        }
    }

    /// Apply NOT IN subquery filter  
    pub fn apply_not_in_subquery(
        self,
        expression: Expression,
        subquery: LogicalPlan,
        outer_variables: Vec<String>,
    ) -> Self {
        let mut variables = self.variables;
        variables.extend(subquery.variables);

        LogicalPlan {
            root: LogicalNode::NotInSubquery {
                expression,
                subquery: Box::new(subquery.root),
                outer_variables,
            },
            variables,
        }
    }

    /// Apply scalar subquery in projection
    pub fn apply_scalar_subquery(
        self,
        subquery: LogicalPlan,
        outer_variables: Vec<String>,
    ) -> Self {
        let mut variables = self.variables;
        variables.extend(subquery.variables);

        LogicalPlan {
            root: LogicalNode::ScalarSubquery {
                subquery: Box::new(subquery.root),
                outer_variables,
            },
            variables,
        }
    }
}

impl LogicalNode {
    /// Get all variables referenced by this node
    pub fn get_variables(&self) -> Vec<String> {
        match self {
            LogicalNode::NodeScan { variable, .. } => vec![variable.clone()],
            LogicalNode::EdgeScan { variable, .. } => vec![variable.clone()],
            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                input,
                ..
            } => {
                let mut vars = input.get_variables();
                vars.push(from_variable.clone());
                if let Some(edge_var) = edge_variable {
                    vars.push(edge_var.clone());
                }
                vars.push(to_variable.clone());
                vars
            }
            LogicalNode::Filter { input, .. } => input.get_variables(),
            LogicalNode::Project { input, .. } => input.get_variables(),
            LogicalNode::Join { left, right, .. } => {
                let mut vars = left.get_variables();
                vars.extend(right.get_variables());
                vars
            }
            LogicalNode::Union { inputs, .. } => inputs
                .iter()
                .flat_map(|input| input.get_variables())
                .collect(),
            LogicalNode::Intersect { left, right, .. } => {
                let mut vars = left.get_variables();
                vars.extend(right.get_variables());
                vars
            }
            LogicalNode::Except { left, right, .. } => {
                let mut vars = left.get_variables();
                vars.extend(right.get_variables());
                vars
            }
            LogicalNode::Aggregate { input, .. } => input.get_variables(),
            LogicalNode::Having { input, .. } => input.get_variables(),
            LogicalNode::Distinct { input, .. } => input.get_variables(),
            LogicalNode::Sort { input, .. } => input.get_variables(),
            LogicalNode::Limit { input, .. } => input.get_variables(),
            LogicalNode::GenericFunction { input, .. } => input.get_variables(),
            LogicalNode::PathTraversal {
                from_variable,
                to_variable,
                path_elements,
                input,
                ..
            } => {
                let mut vars = input.get_variables();
                vars.push(from_variable.clone());
                vars.push(to_variable.clone());
                for element in path_elements {
                    vars.push(element.node_variable.clone());
                    if let Some(edge_var) = &element.edge_variable {
                        vars.push(edge_var.clone());
                    }
                }
                vars
            }

            // Subquery cases
            LogicalNode::ExistsSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                let mut vars = subquery.get_variables();
                vars.extend(outer_variables.clone());
                vars
            }
            LogicalNode::NotExistsSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                let mut vars = subquery.get_variables();
                vars.extend(outer_variables.clone());
                vars
            }
            LogicalNode::InSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                let mut vars = subquery.get_variables();
                vars.extend(outer_variables.clone());
                vars
            }
            LogicalNode::NotInSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                let mut vars = subquery.get_variables();
                vars.extend(outer_variables.clone());
                vars
            }
            LogicalNode::ScalarSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                let mut vars = subquery.get_variables();
                vars.extend(outer_variables.clone());
                vars
            }
            LogicalNode::WithQuery { original_query, .. } => {
                // Extract variables from WITH query segments and final return
                let mut vars = Vec::new();
                // Add variables from all segments
                for segment in &original_query.segments {
                    // Add variables from MATCH clause
                    for pattern in &segment.match_clause.patterns {
                        for element in &pattern.elements {
                            match element {
                                crate::ast::PatternElement::Node(node) => {
                                    if let Some(var_name) = &node.identifier {
                                        vars.push(var_name.clone());
                                    }
                                }
                                crate::ast::PatternElement::Edge(edge) => {
                                    if let Some(var_name) = &edge.identifier {
                                        vars.push(var_name.clone());
                                    }
                                }
                            }
                        }
                    }
                    // Add variables from WITH clause (if present)
                    if let Some(with_clause) = &segment.with_clause {
                        for with_item in &with_clause.items {
                            if let Some(alias) = &with_item.alias {
                                vars.push(alias.clone());
                            }
                        }
                    }
                }
                // Add variables from final return
                for return_item in &original_query.final_return.items {
                    if let Some(alias) = &return_item.alias {
                        vars.push(alias.clone());
                    }
                }
                vars
            }
            LogicalNode::Unwind {
                variable, input, ..
            } => {
                let mut vars = vec![variable.clone()];
                if let Some(input_node) = input {
                    vars.extend(input_node.get_variables());
                }
                vars
            }

            // Data modification operations
            LogicalNode::Insert { .. } => vec![], // INSERT doesn't produce variables for queries
            LogicalNode::Update { .. } => vec![], // UPDATE doesn't produce variables for queries
            LogicalNode::Delete { .. } => vec![], // DELETE doesn't produce variables for queries
            LogicalNode::SingleRow => vec![],     // SingleRow doesn't produce variables
        }
    }

    /// Estimate the cardinality (number of rows) this node will produce
    pub fn estimate_cardinality(&self) -> usize {
        match self {
            LogicalNode::NodeScan { .. } => 1000, // Default estimate
            LogicalNode::EdgeScan { .. } => 5000, // Default estimate
            LogicalNode::Expand { input, .. } => input.estimate_cardinality() * 5, // Average fanout
            LogicalNode::Filter { input, .. } => input.estimate_cardinality() / 2, // 50% selectivity
            LogicalNode::Project { input, .. } => input.estimate_cardinality(),
            LogicalNode::Join { left, right, .. } => {
                (left.estimate_cardinality() * right.estimate_cardinality()) / 100
            }
            LogicalNode::Union { inputs, .. } => inputs
                .iter()
                .map(|input| input.estimate_cardinality())
                .sum(),
            LogicalNode::Intersect { left, right, .. } => left
                .estimate_cardinality()
                .min(right.estimate_cardinality()),
            LogicalNode::Except { left, right, .. } => {
                left.estimate_cardinality()
                    - right
                        .estimate_cardinality()
                        .min(left.estimate_cardinality())
            }
            LogicalNode::Aggregate { input, .. } => input.estimate_cardinality() / 10,
            LogicalNode::Having { input, .. } => input.estimate_cardinality() / 3, // HAVING typically filters more than WHERE
            LogicalNode::Distinct { input, .. } => input.estimate_cardinality() / 2, // Assume 50% duplicates
            LogicalNode::Sort { input, .. } => input.estimate_cardinality(),
            LogicalNode::Limit { count, input, .. } => (*count).min(input.estimate_cardinality()),
            LogicalNode::GenericFunction { .. } => 1, // Functions typically return single value
            LogicalNode::PathTraversal {
                input, path_type, ..
            } => {
                let base_cardinality = input.estimate_cardinality();
                // Path traversal can significantly multiply results depending on type
                match path_type {
                    PathType::Walk => base_cardinality * 20, // Most paths possible
                    PathType::Trail => base_cardinality * 15, // Fewer due to edge constraints
                    PathType::SimplePath => base_cardinality * 10, // Fewer due to vertex constraints
                    PathType::AcyclicPath => base_cardinality * 5, // Fewest due to strict constraints
                }
            }

            // Subquery cardinality estimates
            LogicalNode::ExistsSubquery { subquery, .. } => {
                // EXISTS is boolean, but affects outer query cardinality based on selectivity
                let subquery_card = subquery.estimate_cardinality();
                if subquery_card > 0 {
                    1
                } else {
                    0
                } // Exists is binary - 1 if subquery has results, 0 otherwise
            }
            LogicalNode::NotExistsSubquery { subquery, .. } => {
                // NOT EXISTS is opposite of EXISTS
                let subquery_card = subquery.estimate_cardinality();
                if subquery_card > 0 {
                    0
                } else {
                    1
                }
            }
            LogicalNode::InSubquery { subquery, .. } => {
                // IN subquery cardinality depends on how many outer rows match subquery results
                subquery.estimate_cardinality().min(1000) // Cap at reasonable limit
            }
            LogicalNode::NotInSubquery { subquery, .. } => {
                // NOT IN is complement of IN
                let sub_card = subquery.estimate_cardinality();
                if sub_card == 0 {
                    1000
                } else {
                    100
                } // Estimate based on typical NOT IN selectivity
            }
            LogicalNode::ScalarSubquery { subquery, .. } => {
                // Scalar subquery always returns 1 row (or null)
                subquery.estimate_cardinality().min(1)
            }
            LogicalNode::WithQuery { .. } => {
                // WITH queries can vary widely, but often result in aggregation
                // Use a reasonable default estimate
                100
            }
            LogicalNode::Unwind { input, .. } => {
                // UNWIND typically expands arrays, so multiply input cardinality by expansion factor
                let base_expansion = 10; // Default expansion factor
                if let Some(input_node) = input {
                    input_node.estimate_cardinality() * base_expansion
                } else {
                    base_expansion // Standalone UNWIND with literal array
                }
            }

            // Data modification operations
            LogicalNode::Insert { patterns, .. } => patterns.len(), // Number of patterns being inserted
            LogicalNode::Update { .. } => 1, // UPDATE operations typically affect a specific number of entities
            LogicalNode::Delete { .. } => 1, // DELETE operations typically affect a specific number of entities
            LogicalNode::SingleRow => 1,     // SingleRow always produces exactly 1 row
        }
    }
}
