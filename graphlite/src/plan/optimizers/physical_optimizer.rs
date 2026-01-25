// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Physical plan optimizer - applies physical optimizations to plans
//!
//! This module handles physical plan optimizations such as:
//! - Index scan vs sequential scan selection
//! - Operator selection (hash vs nested loop join)
//! - Parallel execution planning
//!
//! Extracted from optimizer.rs as part of Phase 3 refactoring.

use crate::plan::optimizer::PlanningError;
use crate::plan::physical::{PhysicalNode, PhysicalPlan};

/// Optimizer for physical plans
#[derive(Debug)]
pub struct PhysicalOptimizer {
    avoid_index_scan: bool,
}

impl PhysicalOptimizer {
    /// Create a new physical optimizer
    pub fn new(avoid_index_scan: bool) -> Self {
        Self { avoid_index_scan }
    }

    /// Optimize a physical plan
    /// Originally: optimizer.rs line 2122
    pub fn optimize(&self, plan: PhysicalPlan) -> Result<PhysicalPlan, PlanningError> {
        let mut optimized_plan = plan;

        // Apply index scan optimization based on setting
        if self.avoid_index_scan {
            optimized_plan = self.disable_index_scans(optimized_plan)?;
        }

        // TODO: Implement other physical optimizations like:
        // - Operator selection (hash vs nested loop join)
        // - Parallel execution planning
        Ok(optimized_plan)
    }

    /// Disable index scans in the physical plan
    /// Originally: optimizer.rs line 2137
    pub fn disable_index_scans(&self, plan: PhysicalPlan) -> Result<PhysicalPlan, PlanningError> {
        let transformed_root = self.transform_node_disable_indexes(plan.root)?;
        Ok(PhysicalPlan::new(transformed_root))
    }

    /// Recursively transform physical nodes to disable index scans
    /// Originally: optimizer.rs line 2143
    fn transform_node_disable_indexes(
        &self,
        node: PhysicalNode,
    ) -> Result<PhysicalNode, PlanningError> {
        match node {
            // Replace NodeIndexScan with NodeSeqScan
            PhysicalNode::NodeIndexScan {
                variable,
                labels,
                properties,
                estimated_rows,
                ..
            } => {
                // Sequential scan typically has higher cost than index scan
                let estimated_cost = estimated_rows as f64 * 0.1;
                Ok(PhysicalNode::NodeSeqScan {
                    variable,
                    labels,
                    properties,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Replace IndexedExpand with HashExpand (non-indexed expansion)
            PhysicalNode::IndexedExpand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
                estimated_rows,
                ..
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                let estimated_cost = estimated_rows as f64 * 0.3; // Higher cost without index
                Ok(PhysicalNode::HashExpand {
                    from_variable,
                    edge_variable,
                    to_variable,
                    edge_labels,
                    direction,
                    properties,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Recursively transform nodes with single input
            PhysicalNode::Filter {
                condition,
                input,
                selectivity,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Filter {
                    condition,
                    input: transformed_input,
                    selectivity,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Having {
                condition,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Having {
                    condition,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Project {
                expressions,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Project {
                    expressions,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::HashAggregate {
                group_by,
                aggregates,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::HashAggregate {
                    group_by,
                    aggregates,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::SortAggregate {
                group_by,
                aggregates,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::SortAggregate {
                    group_by,
                    aggregates,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::ExternalSort {
                expressions,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::ExternalSort {
                    expressions,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::InMemorySort {
                expressions,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::InMemorySort {
                    expressions,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Limit {
                count,
                offset,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Limit {
                    count,
                    offset,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Distinct {
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Distinct {
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Transform nodes with two inputs (joins)
            PhysicalNode::HashJoin {
                join_type,
                condition,
                build_keys,
                probe_keys,
                build,
                probe,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_build = Box::new(self.transform_node_disable_indexes(*build)?);
                let transformed_probe = Box::new(self.transform_node_disable_indexes(*probe)?);
                Ok(PhysicalNode::HashJoin {
                    join_type,
                    condition,
                    build_keys,
                    probe_keys,
                    build: transformed_build,
                    probe: transformed_probe,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::NestedLoopJoin {
                join_type,
                condition,
                left,
                right,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::NestedLoopJoin {
                    join_type,
                    condition,
                    left: transformed_left,
                    right: transformed_right,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::SortMergeJoin {
                join_type,
                left_keys,
                right_keys,
                left,
                right,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::SortMergeJoin {
                    join_type,
                    left_keys,
                    right_keys,
                    left: transformed_left,
                    right: transformed_right,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Transform nodes with multiple inputs
            PhysicalNode::UnionAll {
                inputs,
                all,
                estimated_rows,
                estimated_cost,
            } => {
                let mut transformed_inputs = Vec::new();
                for input in inputs {
                    transformed_inputs.push(self.transform_node_disable_indexes(input)?);
                }
                Ok(PhysicalNode::UnionAll {
                    inputs: transformed_inputs,
                    all,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Transform PathTraversal input
            PhysicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::PathTraversal {
                    path_type,
                    from_variable,
                    to_variable,
                    path_elements,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Nodes that are already non-indexed (no transformation needed)
            PhysicalNode::NodeSeqScan { .. }
            | PhysicalNode::EdgeSeqScan { .. }
            | PhysicalNode::HashExpand { .. }
            | PhysicalNode::GenericFunction { .. }
            | PhysicalNode::SingleRow { .. } => {
                Ok(node) // Already using appropriate scans / no transformation needed
            }

            // Handle subqueries recursively
            PhysicalNode::ExistsSubquery {
                subplan,
                estimated_rows,
                estimated_cost,
                optimized,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::ExistsSubquery {
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                    optimized,
                })
            }

            PhysicalNode::NotExistsSubquery {
                subplan,
                estimated_rows,
                estimated_cost,
                optimized,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::NotExistsSubquery {
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                    optimized,
                })
            }

            PhysicalNode::InSubquery {
                expression,
                subplan,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::InSubquery {
                    expression,
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::NotInSubquery {
                expression,
                subplan,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::NotInSubquery {
                    expression,
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::ScalarSubquery {
                subplan,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::ScalarSubquery {
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::WithQuery {
                original_query,
                estimated_rows,
                estimated_cost,
            } => {
                // WITH queries don't use index scans in their current implementation
                // Just return them as-is
                Ok(PhysicalNode::WithQuery {
                    original_query,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Unwind {
                variable,
                expression,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = if let Some(input_node) = input {
                    Some(Box::new(self.transform_node_disable_indexes(*input_node)?))
                } else {
                    None
                };
                Ok(PhysicalNode::Unwind {
                    variable,
                    expression,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Data modification operations (no transformation needed)
            PhysicalNode::Insert { .. }
            | PhysicalNode::Update { .. }
            | PhysicalNode::Delete { .. } => Ok(node),

            // Set operations with two inputs
            PhysicalNode::Intersect {
                left,
                right,
                all,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::Intersect {
                    left: transformed_left,
                    right: transformed_right,
                    all,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Except {
                left,
                right,
                all,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::Except {
                    left: transformed_left,
                    right: transformed_right,
                    all,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Graph-specific operations (keep as-is or may need specialized handling)
            PhysicalNode::GraphIndexScan { .. } => Ok(node),
            PhysicalNode::IndexJoin { .. } => Ok(node),
        }
    }
}

impl Default for PhysicalOptimizer {
    fn default() -> Self {
        Self::new(true) // By default, avoid index scans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expression;
    use crate::plan::physical::{PhysicalNode, PhysicalPlan};

    #[test]
    fn test_physical_optimizer_creation() {
        let optimizer = PhysicalOptimizer::new(true);
        assert_eq!(
            std::mem::size_of_val(&optimizer),
            std::mem::size_of::<PhysicalOptimizer>()
        );
    }

    #[test]
    fn test_physical_optimizer_default() {
        let optimizer = PhysicalOptimizer::default();
        assert_eq!(
            std::mem::size_of_val(&optimizer),
            std::mem::size_of::<PhysicalOptimizer>()
        );
    }

    #[test]
    fn test_optimize_without_index_scan_avoidance() {
        let optimizer = PhysicalOptimizer::new(false);

        let plan = PhysicalPlan::new(PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 100,
            estimated_cost: 10.0,
        });

        let result = optimizer.optimize(plan);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimize_with_index_scan_avoidance() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::NodeIndexScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 100,
            estimated_cost: 5.0,
        });

        let result = optimizer.optimize(plan);
        assert!(result.is_ok());

        // Should convert NodeIndexScan to NodeSeqScan
        let optimized = result.unwrap();
        match optimized.root {
            PhysicalNode::NodeSeqScan { .. } => {
                // Expected - index scan was disabled
            }
            _ => panic!("Expected NodeIndexScan to be converted to NodeSeqScan"),
        }
    }

    #[test]
    fn test_disable_index_scans_converts_node_index_scan() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::NodeIndexScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 100,
            estimated_cost: 5.0,
        });

        let result = optimizer.disable_index_scans(plan);
        assert!(result.is_ok());

        let transformed = result.unwrap();
        match transformed.root {
            PhysicalNode::NodeSeqScan {
                variable, labels, ..
            } => {
                assert_eq!(variable, "n");
                assert_eq!(labels, vec!["Person"]);
            }
            _ => panic!("Expected NodeSeqScan after disabling index scans"),
        }
    }

    #[test]
    fn test_disable_index_scans_converts_indexed_expand() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::IndexedExpand {
            from_variable: "n".to_string(),
            edge_variable: Some("e".to_string()),
            to_variable: "m".to_string(),
            edge_labels: vec!["KNOWS".to_string()],
            direction: crate::ast::EdgeDirection::Outgoing,
            properties: None,
            input: Box::new(PhysicalNode::NodeSeqScan {
                variable: "n".to_string(),
                labels: vec![],
                properties: None,
                estimated_rows: 10,
                estimated_cost: 1.0,
            }),
            estimated_rows: 100,
            estimated_cost: 10.0,
        });

        let result = optimizer.disable_index_scans(plan);
        assert!(result.is_ok());

        let transformed = result.unwrap();
        match transformed.root {
            PhysicalNode::HashExpand {
                from_variable,
                edge_variable,
                to_variable,
                ..
            } => {
                assert_eq!(from_variable, "n");
                assert_eq!(edge_variable, Some("e".to_string()));
                assert_eq!(to_variable, "m");
            }
            _ => panic!("Expected HashExpand after disabling index scans"),
        }
    }

    #[test]
    fn test_disable_index_scans_preserves_seq_scan() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 100,
            estimated_cost: 10.0,
        });

        let result = optimizer.disable_index_scans(plan);
        assert!(result.is_ok());

        let transformed = result.unwrap();
        match transformed.root {
            PhysicalNode::NodeSeqScan { .. } => {
                // Expected - seq scan is already non-indexed
            }
            _ => panic!("Expected NodeSeqScan to be preserved"),
        }
    }

    #[test]
    fn test_disable_index_scans_processes_filter_recursively() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::Filter {
            condition: Expression::Literal(crate::ast::Literal::Boolean(true)),
            input: Box::new(PhysicalNode::NodeIndexScan {
                variable: "n".to_string(),
                labels: vec![],
                properties: None,
                estimated_rows: 100,
                estimated_cost: 5.0,
            }),
            selectivity: 0.5,
            estimated_rows: 50,
            estimated_cost: 5.0,
        });

        let result = optimizer.disable_index_scans(plan);
        assert!(result.is_ok());

        // Verify the nested NodeIndexScan was converted
        let transformed = result.unwrap();
        match transformed.root {
            PhysicalNode::Filter { input, .. } => match *input {
                PhysicalNode::NodeSeqScan { .. } => {
                    // Expected - nested index scan was converted
                }
                _ => panic!("Expected nested NodeSeqScan"),
            },
            _ => panic!("Expected Filter node at root"),
        }
    }

    #[test]
    fn test_disable_index_scans_processes_join_recursively() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::HashJoin {
            join_type: crate::plan::logical::JoinType::Inner,
            condition: None,
            build_keys: vec![],
            probe_keys: vec![],
            build: Box::new(PhysicalNode::NodeIndexScan {
                variable: "n".to_string(),
                labels: vec![],
                properties: None,
                estimated_rows: 100,
                estimated_cost: 5.0,
            }),
            probe: Box::new(PhysicalNode::NodeIndexScan {
                variable: "m".to_string(),
                labels: vec![],
                properties: None,
                estimated_rows: 100,
                estimated_cost: 5.0,
            }),
            estimated_rows: 1000,
            estimated_cost: 100.0,
        });

        let result = optimizer.disable_index_scans(plan);
        assert!(result.is_ok());

        // Verify both sides of the join were converted
        let transformed = result.unwrap();
        match transformed.root {
            PhysicalNode::HashJoin { build, probe, .. } => {
                match (*build, *probe) {
                    (PhysicalNode::NodeSeqScan { .. }, PhysicalNode::NodeSeqScan { .. }) => {
                        // Expected - both sides converted
                    }
                    _ => panic!("Expected both join sides to have NodeSeqScan"),
                }
            }
            _ => panic!("Expected HashJoin node at root"),
        }
    }

    #[test]
    fn test_disable_index_scans_preserves_single_row() {
        let optimizer = PhysicalOptimizer::new(true);

        let plan = PhysicalPlan::new(PhysicalNode::SingleRow {
            estimated_rows: 1,
            estimated_cost: 0.0,
        });

        let result = optimizer.disable_index_scans(plan);
        assert!(result.is_ok());

        let transformed = result.unwrap();
        match transformed.root {
            PhysicalNode::SingleRow { .. } => {
                // Expected - SingleRow is preserved
            }
            _ => panic!("Expected SingleRow to be preserved"),
        }
    }
}
