// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Pattern Connectivity Analysis
//!
//! Data structures for analyzing connectivity between MATCH patterns
//! to determine optimal execution strategies.

use petgraph::Graph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ast::ast::{Edge, PathPattern};

/// Analysis result for a set of patterns, capturing their connectivity
///
/// **Planned Feature** - Pattern connectivity analysis for optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PatternConnectivity {
    /// Original patterns being analyzed
    pub patterns: Vec<PathPattern>,
    /// Map of variable names to pattern indices where they appear
    pub shared_variables: HashMap<String, Vec<usize>>,
    /// Graph representing pattern connectivity (nodes = pattern indices, edges = shared variables)
    pub connectivity_graph: Graph<usize, String>,
}

/// A linear sequence of connected patterns that can be executed via path traversal
///
/// **Planned Feature** - Linear path detection for optimized traversal
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearPath {
    /// The starting pattern (first in the sequence)
    pub start_pattern: PathPattern,
    /// Steps in the traversal sequence
    pub steps: Vec<TraversalStep>,
    /// All variables involved in the path
    pub variables: Vec<String>,
    /// Estimated selectivity of the entire path
    pub estimated_selectivity: f64,
}

/// A single step in a path traversal
///
/// **Planned Feature** - Traversal step definition for path optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalStep {
    /// Variable name we're traversing from
    pub from_var: String,
    /// The relationship pattern being traversed
    pub relationship: Edge,
    /// Variable name we're traversing to
    pub to_var: String,
    /// Estimated selectivity of this step (0.0 to 1.0)
    pub selectivity: f64,
    /// Index of the pattern this step corresponds to
    pub pattern_index: usize,
}

/// Strategy for executing connected patterns
///
/// **Planned Feature** - Pattern execution strategy selection
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PatternPlanStrategy {
    /// Execute as a single path traversal (most efficient for linear paths)
    PathTraversal(LinearPath),
    /// Execute using hash joins
    HashJoin {
        patterns: Vec<PathPattern>,
        join_order: Vec<JoinStep>,
        estimated_cost: f64,
    },
    /// Execute using nested loop joins  
    NestedLoopJoin {
        patterns: Vec<PathPattern>,
        estimated_cost: f64,
    },
    /// Execute as Cartesian product (when no shared variables)
    CartesianProduct {
        patterns: Vec<PathPattern>,
        estimated_cost: f64,
    },
}

/// A join step between two patterns
///
/// **Planned Feature** - Join step optimization for pattern execution
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinStep {
    /// Index of left pattern
    pub left_pattern_idx: usize,
    /// Index of right pattern  
    pub right_pattern_idx: usize,
    /// Variables to join on
    pub join_variables: Vec<String>,
    /// Type of join to use
    pub join_type: JoinType,
    /// Estimated cost of this join
    pub estimated_cost: f64,
}

/// Types of joins available
///
/// **Planned Feature** - Join type selection for pattern optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JoinType {
    /// Hash join - build hash table on smaller side
    Hash,
    /// Nested loop join - for small datasets
    NestedLoop,
    /// Index lookup join - when indexes are available
    IndexLookup,
}

impl PatternConnectivity {
    /// Create a new pattern connectivity analysis
    #[allow(dead_code)] // ROADMAP v0.4.0 - Advanced pattern optimization with connectivity analysis
    pub fn new(patterns: Vec<PathPattern>) -> Self {
        PatternConnectivity {
            patterns,
            shared_variables: HashMap::new(),
            connectivity_graph: Graph::new(),
        }
    }

    /// Check if patterns have any shared variables
    #[allow(dead_code)] // ROADMAP v0.4.0 - Pattern connectivity detection for join optimization
    pub fn has_shared_variables(&self) -> bool {
        !self.shared_variables.is_empty()
    }

    /// Get the number of connected components in the pattern graph
    #[allow(dead_code)] // ROADMAP v0.4.0 - Multi-component pattern analysis for parallel execution
    pub fn connected_components(&self) -> usize {
        use petgraph::algo::connected_components;
        connected_components(&self.connectivity_graph)
    }

    /// Check if all patterns are connected (single component)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Connected pattern detection for Cartesian product avoidance
    pub fn is_fully_connected(&self) -> bool {
        self.connected_components() <= 1
    }
}

impl LinearPath {
    /// Create a new linear path
    pub fn new(start_pattern: PathPattern, steps: Vec<TraversalStep>) -> Self {
        let variables = Self::extract_variables(&start_pattern, &steps);
        let estimated_selectivity = steps.iter().map(|s| s.selectivity).product();

        LinearPath {
            start_pattern,
            steps,
            variables,
            estimated_selectivity,
        }
    }

    /// Extract all variables involved in this path
    fn extract_variables(start_pattern: &PathPattern, steps: &[TraversalStep]) -> Vec<String> {
        let mut variables = Vec::new();

        // Add variables from start pattern
        for element in &start_pattern.elements {
            match element {
                crate::ast::ast::PatternElement::Node(node) => {
                    if let Some(ref id) = node.identifier {
                        if !variables.contains(id) {
                            variables.push(id.clone());
                        }
                    }
                }
                crate::ast::ast::PatternElement::Edge(edge) => {
                    if let Some(ref id) = edge.identifier {
                        if !variables.contains(id) {
                            variables.push(id.clone());
                        }
                    }
                }
            }
        }

        // Add variables from steps
        for step in steps {
            if !variables.contains(&step.from_var) {
                variables.push(step.from_var.clone());
            }
            if !variables.contains(&step.to_var) {
                variables.push(step.to_var.clone());
            }
        }

        variables
    }

    /// Get the length of this path (number of traversal steps)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Path metrics for selectivity estimation
    pub fn length(&self) -> usize {
        self.steps.len()
    }

    /// Check if this is a simple two-step path
    #[allow(dead_code)] // ROADMAP v0.4.0 - Simple path detection for index usage
    pub fn is_simple_path(&self) -> bool {
        self.steps.len() <= 2
    }

    /// Get the final variable in the path
    #[allow(dead_code)] // ROADMAP v0.4.0 - Path endpoint tracking for join planning
    pub fn end_variable(&self) -> Option<&String> {
        self.steps.last().map(|step| &step.to_var)
    }
}

impl PatternPlanStrategy {
    /// Get a human-readable name for this strategy
    #[allow(dead_code)] // ROADMAP v0.4.0 - Strategy naming for query plan explanation
    pub fn name(&self) -> &'static str {
        match self {
            PatternPlanStrategy::PathTraversal(_) => "PathTraversal",
            PatternPlanStrategy::HashJoin { .. } => "HashJoin",
            PatternPlanStrategy::NestedLoopJoin { .. } => "NestedLoopJoin",
            PatternPlanStrategy::CartesianProduct { .. } => "CartesianProduct",
        }
    }

    /// Get the estimated cost of this strategy
    #[allow(dead_code)] // ROADMAP v0.4.0 - Cost-based strategy selection
    pub fn estimated_cost(&self) -> f64 {
        match self {
            PatternPlanStrategy::PathTraversal(path) => {
                // Cost is roughly proportional to path selectivity and length
                let base_cost = 100.0;
                base_cost * (1.0 / path.estimated_selectivity.max(0.001)) * (path.length() as f64)
            }
            PatternPlanStrategy::HashJoin { estimated_cost, .. } => *estimated_cost,
            PatternPlanStrategy::NestedLoopJoin { estimated_cost, .. } => *estimated_cost,
            PatternPlanStrategy::CartesianProduct { estimated_cost, .. } => *estimated_cost,
        }
    }

    /// Check if this is a path traversal strategy
    #[allow(dead_code)] // ROADMAP v0.4.0 - Strategy type checking for optimization rules
    pub fn is_path_traversal(&self) -> bool {
        matches!(self, PatternPlanStrategy::PathTraversal(_))
    }

    /// Check if this is a join-based strategy
    #[allow(dead_code)] // ROADMAP v0.4.0 - Join strategy detection for plan transformation
    pub fn is_join_based(&self) -> bool {
        matches!(
            self,
            PatternPlanStrategy::HashJoin { .. } | PatternPlanStrategy::NestedLoopJoin { .. }
        )
    }
}

impl JoinStep {
    /// Create a new join step
    #[allow(dead_code)] // ROADMAP v0.4.0 - Join step construction for multi-pattern queries
    pub fn new(
        left_idx: usize,
        right_idx: usize,
        join_vars: Vec<String>,
        join_type: JoinType,
    ) -> Self {
        JoinStep {
            left_pattern_idx: left_idx,
            right_pattern_idx: right_idx,
            join_variables: join_vars,
            join_type,
            estimated_cost: 0.0, // Will be filled by cost estimator
        }
    }

    /// Check if this is a hash join
    #[allow(dead_code)] // ROADMAP v0.4.0 - Hash join type detection for plan optimization
    pub fn is_hash_join(&self) -> bool {
        matches!(self.join_type, JoinType::Hash)
    }

    /// Check if this is an index lookup join
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index join detection for index-aware optimization
    pub fn is_index_join(&self) -> bool {
        matches!(self.join_type, JoinType::IndexLookup)
    }
}

impl JoinType {
    /// Get the string representation of this join type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Join type display for query plan explanation
    pub fn as_str(&self) -> &'static str {
        match self {
            JoinType::Hash => "Hash",
            JoinType::NestedLoop => "NestedLoop",
            JoinType::IndexLookup => "IndexLookup",
        }
    }

    /// Check if this join type is suitable for large datasets
    #[allow(dead_code)] // ROADMAP v0.4.0 - Scalability assessment for large graph queries
    pub fn is_scalable(&self) -> bool {
        matches!(self, JoinType::Hash | JoinType::IndexLookup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ast::{Location, Node};

    #[test]
    fn test_pattern_connectivity_creation() {
        let patterns = vec![];
        let connectivity = PatternConnectivity::new(patterns);

        assert!(!connectivity.has_shared_variables());
        assert_eq!(connectivity.connected_components(), 0);
    }

    #[test]
    fn test_linear_path_variables() {
        use crate::ast::ast::{Location, PatternElement};

        // Create a simple pattern: (a)-[:R]->(b)
        let start_pattern = PathPattern {
            assignment: None,
            path_type: None,
            elements: vec![
                PatternElement::Node(Node {
                    identifier: Some("a".to_string()),
                    labels: vec![],
                    properties: None,
                    location: Location::default(),
                }),
                PatternElement::Edge(Edge {
                    identifier: Some("r".to_string()),
                    labels: vec!["R".to_string()],
                    properties: None,
                    direction: crate::ast::ast::EdgeDirection::Outgoing,
                    quantifier: None,
                    location: Location::default(),
                }),
                PatternElement::Node(Node {
                    identifier: Some("b".to_string()),
                    labels: vec![],
                    properties: None,
                    location: Location::default(),
                }),
            ],
            location: Location::default(),
        };

        let steps = vec![TraversalStep {
            from_var: "b".to_string(),
            relationship: Edge {
                identifier: Some("r2".to_string()),
                labels: vec!["R2".to_string()],
                properties: None,
                direction: crate::ast::ast::EdgeDirection::Outgoing,
                quantifier: None,
                location: Location::default(),
            },
            to_var: "c".to_string(),
            selectivity: 0.1,
            pattern_index: 1,
        }];

        let path = LinearPath::new(start_pattern, steps);

        assert_eq!(path.length(), 1);
        assert!(path.is_simple_path());
        assert_eq!(path.end_variable(), Some(&"c".to_string()));
        assert!(path.variables.contains(&"a".to_string()));
        assert!(path.variables.contains(&"b".to_string()));
        assert!(path.variables.contains(&"c".to_string()));
    }

    #[test]
    fn test_join_step_creation() {
        let join_step = JoinStep::new(0, 1, vec!["x".to_string()], JoinType::Hash);

        assert!(join_step.is_hash_join());
        assert!(!join_step.is_index_join());
        assert_eq!(join_step.join_variables, vec!["x".to_string()]);
    }

    #[test]
    fn test_pattern_strategy_properties() {
        let path = LinearPath {
            start_pattern: PathPattern {
                assignment: None,
                path_type: None,
                elements: vec![],
                location: Location::default(),
            },
            steps: vec![TraversalStep {
                from_var: "n".to_string(),
                relationship: Edge {
                    identifier: Some("r".to_string()),
                    labels: vec![],
                    properties: None,
                    direction: crate::ast::ast::EdgeDirection::Outgoing,
                    quantifier: None,
                    location: Location::default(),
                },
                to_var: "m".to_string(),
                selectivity: 0.5,
                pattern_index: 0,
            }],
            variables: vec!["n".to_string(), "m".to_string()],
            estimated_selectivity: 0.1,
        };

        let strategy = PatternPlanStrategy::PathTraversal(path);

        assert!(strategy.is_path_traversal());
        assert!(!strategy.is_join_based());
        assert_eq!(strategy.name(), "PathTraversal");
        assert!(strategy.estimated_cost() > 0.0);
    }
}
