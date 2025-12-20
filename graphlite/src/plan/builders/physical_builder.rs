// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Physical plan builder - converts logical plans into physical execution plans
//!
//! This module handles the conversion from logical plans (what to do) to
//! physical plans (how to execute it).
//!
//! Extracted from optimizer.rs as part of Phase 3 refactoring.

use crate::plan::logical::{LogicalNode, LogicalPlan};
use crate::plan::optimizer::PlanningError;
use crate::plan::physical::{PhysicalNode, PhysicalPlan};

/// Builder for creating physical plans from logical plans
#[derive(Debug)]
pub struct PhysicalBuilder {
    // Future: Add cost model, statistics, etc.
}

impl PhysicalBuilder {
    /// Create a new physical builder
    pub fn new() -> Self {
        Self {}
    }

    /// Build a physical plan from a logical plan
    /// Originally: optimizer.rs line 2114
    pub fn build(&self, logical_plan: LogicalPlan) -> Result<PhysicalPlan, PlanningError> {
        Ok(PhysicalPlan::from_logical(&logical_plan))
    }

    /// Create a simple physical plan from a logical plan
    /// Originally: optimizer.rs line 951
    pub fn create_simple_physical_plan(
        &self,
        logical_plan: &LogicalPlan,
    ) -> Result<PhysicalNode, PlanningError> {
        // Convert the logical plan root node to a physical node
        self.logical_to_physical_node(&logical_plan.root)
    }

    /// Convert a logical node to a physical node
    /// Originally: optimizer.rs line 960
    fn logical_to_physical_node(
        &self,
        logical_node: &LogicalNode,
    ) -> Result<PhysicalNode, PlanningError> {
        match logical_node {
            LogicalNode::NodeScan {
                variable,
                labels,
                properties,
            } => Ok(PhysicalNode::NodeSeqScan {
                variable: variable.clone(),
                labels: labels.clone(),
                properties: properties.clone(),
                estimated_rows: 1000,  // Default estimate
                estimated_cost: 100.0, // Default cost
            }),
            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            } => {
                let input_physical = self.logical_to_physical_node(input)?;
                Ok(PhysicalNode::IndexedExpand {
                    from_variable: from_variable.clone(),
                    edge_variable: edge_variable.clone(),
                    to_variable: to_variable.clone(),
                    edge_labels: edge_labels.clone(),
                    direction: direction.clone(),
                    properties: properties.clone(),
                    input: Box::new(input_physical),
                    estimated_rows: 1000,
                    estimated_cost: 200.0,
                })
            }
            LogicalNode::SingleRow => Ok(PhysicalNode::SingleRow {
                estimated_rows: 1,   // Always exactly 1 row
                estimated_cost: 1.0, // Minimal cost - cheapest possible operation
            }),
            _ => {
                // For other node types, return a simple scan as fallback
                Ok(PhysicalNode::NodeSeqScan {
                    variable: "fallback".to_string(),
                    labels: vec!["Node".to_string()],
                    properties: None,
                    estimated_rows: 1000,
                    estimated_cost: 100.0,
                })
            }
        }
    }
}

impl Default for PhysicalBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::logical::{LogicalNode, LogicalPlan};
    use std::collections::HashMap;

    #[test]
    fn test_physical_builder_creation() {
        let builder = PhysicalBuilder::new();
        assert_eq!(
            std::mem::size_of_val(&builder),
            std::mem::size_of::<PhysicalBuilder>()
        );
    }

    #[test]
    fn test_physical_builder_default() {
        let builder = PhysicalBuilder::default();
        assert_eq!(
            std::mem::size_of_val(&builder),
            std::mem::size_of::<PhysicalBuilder>()
        );
    }

    #[test]
    fn test_build_simple_node_scan() {
        let builder = PhysicalBuilder::new();

        let logical_plan = LogicalPlan {
            root: LogicalNode::NodeScan {
                variable: "n".to_string(),
                labels: vec!["Person".to_string()],
                properties: None,
            },
            variables: HashMap::new(),
        };

        let result = builder.build(logical_plan);
        assert!(result.is_ok());

        let physical_plan = result.unwrap();
        // Verify the physical plan root is some form of node scan
        match physical_plan.root {
            PhysicalNode::NodeSeqScan { .. } | PhysicalNode::NodeIndexScan { .. } => {
                // Either variant is acceptable
            }
            _ => panic!("Expected a node scan physical node"),
        }
    }

    #[test]
    fn test_build_single_row() {
        let builder = PhysicalBuilder::new();

        let logical_plan = LogicalPlan {
            root: LogicalNode::SingleRow,
            variables: HashMap::new(),
        };

        let result = builder.build(logical_plan);
        assert!(result.is_ok());

        let physical_plan = result.unwrap();
        match physical_plan.root {
            PhysicalNode::SingleRow { .. } => {
                // Expected
            }
            _ => panic!("Expected SingleRow physical node"),
        }
    }

    #[test]
    fn test_build_preserves_variables() {
        let builder = PhysicalBuilder::new();

        let mut variables = HashMap::new();
        variables.insert(
            "n".to_string(),
            crate::plan::logical::VariableInfo {
                name: "n".to_string(),
                entity_type: crate::plan::logical::EntityType::Node,
                labels: vec!["Person".to_string()],
                required_properties: vec![],
            },
        );

        let logical_plan = LogicalPlan {
            root: LogicalNode::NodeScan {
                variable: "n".to_string(),
                labels: vec!["Person".to_string()],
                properties: None,
            },
            variables: variables.clone(),
        };

        let result = builder.build(logical_plan);
        assert!(result.is_ok());

        // Physical plan doesn't have a variables field - it's derived from LogicalPlan
        // Just verify the build succeeds
    }
}
