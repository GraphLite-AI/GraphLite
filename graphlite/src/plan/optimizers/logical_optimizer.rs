// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Logical plan optimizer - applies logical optimizations to plans
//!
//! This module handles logical plan optimizations such as:
//! - Predicate pushdown
//! - Projection elimination
//! - Join reordering
//! - Subquery unnesting
//!
//! Extracted from optimizer.rs as part of Phase 3 refactoring.

use crate::ast::Expression;
use crate::plan::logical::{LogicalNode, LogicalPlan};
use crate::plan::optimizer::{OptimizationLevel, PlanningError};

/// Optimizer for logical plans
#[derive(Debug)]
pub struct LogicalOptimizer {
    optimization_level: OptimizationLevel,
}

impl LogicalOptimizer {
    /// Create a new logical optimizer
    pub fn new(level: OptimizationLevel) -> Self {
        Self {
            optimization_level: level,
        }
    }

    /// Optimize a logical plan
    /// Originally: optimizer.rs line 1470
    pub fn optimize(&self, mut plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        match self.optimization_level {
            OptimizationLevel::None => Ok(plan),

            OptimizationLevel::Basic => {
                // Apply basic optimizations
                plan = self.apply_predicate_pushdown(plan)?;
                plan = self.apply_projection_elimination(plan)?;
                plan = self.apply_subquery_unnesting(plan)?;
                Ok(plan)
            }

            OptimizationLevel::Advanced | OptimizationLevel::Aggressive => {
                // Apply all basic optimizations
                plan = self.apply_predicate_pushdown(plan)?;
                plan = self.apply_projection_elimination(plan)?;

                // Apply advanced optimizations
                plan = self.apply_subquery_unnesting(plan)?;
                plan = self.apply_join_reordering(plan)?;

                Ok(plan)
            }
        }
    }

    // ========================================================================
    // Optimization Rules (to be fully extracted from optimizer.rs)
    // ========================================================================

    /// Apply predicate pushdown optimization
    /// Originally: optimizer.rs line 1497
    fn apply_predicate_pushdown(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // Recursively optimize the logical plan tree
        let optimized_root = self.optimize_logical_node(plan.root)?;

        Ok(LogicalPlan {
            root: optimized_root,
            variables: plan.variables,
        })
    }

    /// Recursively optimize a logical node
    /// Originally: optimizer.rs line 1508
    fn optimize_logical_node(&self, node: LogicalNode) -> Result<LogicalNode, PlanningError> {
        match node {
            LogicalNode::Union { inputs, all } => {
                // For UNION queries, we need to ensure each branch is properly optimized
                let mut optimized_inputs = Vec::new();
                for input in inputs {
                    optimized_inputs.push(self.optimize_logical_node(input)?);
                }

                Ok(LogicalNode::Union {
                    inputs: optimized_inputs,
                    all,
                })
            }

            LogicalNode::Filter { condition, input } => {
                // Recursively optimize the input first
                let optimized_input = self.optimize_logical_node(*input)?;

                // Try to push the filter down if possible
                match optimized_input {
                    LogicalNode::Union { inputs, all } => {
                        // Push the filter down to both sides of the UNION
                        let mut filtered_inputs = Vec::new();
                        for union_input in inputs {
                            filtered_inputs.push(LogicalNode::Filter {
                                condition: condition.clone(),
                                input: Box::new(union_input),
                            });
                        }

                        Ok(LogicalNode::Union {
                            inputs: filtered_inputs,
                            all,
                        })
                    }

                    LogicalNode::Join {
                        left,
                        right,
                        join_type,
                        condition: join_condition,
                    } => {
                        // For joins, we need to analyze which side the filter applies to
                        // For now, keep the filter above the join
                        Ok(LogicalNode::Filter {
                            condition,
                            input: Box::new(LogicalNode::Join {
                                left: Box::new(self.optimize_logical_node(*left)?),
                                right: Box::new(self.optimize_logical_node(*right)?),
                                join_type,
                                condition: join_condition,
                            }),
                        })
                    }

                    other => {
                        // For other node types, keep the filter as is but optimize the input
                        Ok(LogicalNode::Filter {
                            condition,
                            input: Box::new(other),
                        })
                    }
                }
            }

            LogicalNode::Join {
                left,
                right,
                join_type,
                condition,
            } => Ok(LogicalNode::Join {
                left: Box::new(self.optimize_logical_node(*left)?),
                right: Box::new(self.optimize_logical_node(*right)?),
                join_type,
                condition,
            }),

            LogicalNode::Project { expressions, input } => Ok(LogicalNode::Project {
                expressions,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Aggregate {
                group_by,
                aggregates,
                input,
            } => Ok(LogicalNode::Aggregate {
                group_by,
                aggregates,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Sort { expressions, input } => Ok(LogicalNode::Sort {
                expressions,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Limit {
                count,
                offset,
                input,
            } => Ok(LogicalNode::Limit {
                count,
                offset,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            } => Ok(LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input,
            } => Ok(LogicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Having { condition, input } => Ok(LogicalNode::Having {
                condition,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Distinct { input } => Ok(LogicalNode::Distinct {
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::GenericFunction {
                function_name,
                arguments,
                input,
            } => Ok(LogicalNode::GenericFunction {
                function_name,
                arguments,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Intersect { left, right, all } => Ok(LogicalNode::Intersect {
                left: Box::new(self.optimize_logical_node(*left)?),
                right: Box::new(self.optimize_logical_node(*right)?),
                all,
            }),

            LogicalNode::Except { left, right, all } => Ok(LogicalNode::Except {
                left: Box::new(self.optimize_logical_node(*left)?),
                right: Box::new(self.optimize_logical_node(*right)?),
                all,
            }),

            // Leaf nodes and complex nodes that don't need recursion for now
            LogicalNode::NodeScan { .. }
            | LogicalNode::EdgeScan { .. }
            | LogicalNode::SingleRow
            | LogicalNode::Insert { .. }
            | LogicalNode::Delete { .. }
            | LogicalNode::Update { .. }
            | LogicalNode::ExistsSubquery { .. }
            | LogicalNode::NotExistsSubquery { .. }
            | LogicalNode::InSubquery { .. }
            | LogicalNode::NotInSubquery { .. }
            | LogicalNode::ScalarSubquery { .. }
            | LogicalNode::WithQuery { .. }
            | LogicalNode::Unwind { .. } => Ok(node),
        }
    }

    /// Apply projection elimination optimization
    /// Originally: optimizer.rs line 1697
    fn apply_projection_elimination(
        &self,
        plan: LogicalPlan,
    ) -> Result<LogicalPlan, PlanningError> {
        // TODO: Extract implementation from optimizer.rs
        Ok(plan)
    }

    /// Apply join reordering optimization
    /// Originally: optimizer.rs line 1707
    fn apply_join_reordering(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // TODO: Extract implementation from optimizer.rs
        Ok(plan)
    }

    /// Apply subquery unnesting optimization
    /// Originally: optimizer.rs line 1714
    fn apply_subquery_unnesting(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        let unnested_root = self.unnest_subqueries_in_node(plan.root)?;
        Ok(LogicalPlan::new(unnested_root))
    }

    /// Recursively unnest subqueries in a logical node
    fn unnest_subqueries_in_node(&self, node: LogicalNode) -> Result<LogicalNode, PlanningError> {
        use crate::plan::logical::JoinType;

        match node {
            // EXISTS subquery can be converted to LEFT SEMI JOIN
            LogicalNode::ExistsSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                if self.can_unnest_exists_subquery(&subquery, &outer_variables) {
                    self.unnest_exists_subquery(*subquery, outer_variables)
                } else {
                    // Keep as subquery but unnest any nested subqueries
                    let unnested_subquery = Box::new(self.unnest_subqueries_in_node(*subquery)?);
                    Ok(LogicalNode::ExistsSubquery {
                        subquery: unnested_subquery,
                        outer_variables,
                    })
                }
            }

            // NOT EXISTS subquery can be converted to LEFT ANTI JOIN
            LogicalNode::NotExistsSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                if self.can_unnest_not_exists_subquery(&subquery, &outer_variables) {
                    self.unnest_not_exists_subquery(*subquery, outer_variables)
                } else {
                    let unnested_subquery = Box::new(self.unnest_subqueries_in_node(*subquery)?);
                    Ok(LogicalNode::NotExistsSubquery {
                        subquery: unnested_subquery,
                        outer_variables,
                    })
                }
            }

            // IN subquery can sometimes be converted to INNER JOIN
            LogicalNode::InSubquery {
                expression,
                subquery,
                outer_variables,
                ..
            } => {
                if self.can_unnest_in_subquery(&subquery, &outer_variables, &expression) {
                    self.unnest_in_subquery(*subquery, outer_variables, expression)
                } else {
                    let unnested_subquery = Box::new(self.unnest_subqueries_in_node(*subquery)?);
                    Ok(LogicalNode::InSubquery {
                        expression,
                        subquery: unnested_subquery,
                        outer_variables,
                    })
                }
            }

            // Recursively process nodes with inputs
            LogicalNode::Filter { condition, input } => {
                let unnested_input = Box::new(self.unnest_subqueries_in_node(*input)?);
                Ok(LogicalNode::Filter {
                    condition,
                    input: unnested_input,
                })
            }

            LogicalNode::Project { expressions, input } => {
                let unnested_input = Box::new(self.unnest_subqueries_in_node(*input)?);
                Ok(LogicalNode::Project {
                    expressions,
                    input: unnested_input,
                })
            }

            LogicalNode::Join {
                join_type,
                condition,
                left,
                right,
            } => {
                let unnested_left = Box::new(self.unnest_subqueries_in_node(*left)?);
                let unnested_right = Box::new(self.unnest_subqueries_in_node(*right)?);
                Ok(LogicalNode::Join {
                    join_type,
                    condition,
                    left: unnested_left,
                    right: unnested_right,
                })
            }

            // For all other nodes, return as-is (base case for recursion)
            _ => Ok(node),
        }
    }

    /// Check if EXISTS subquery can be unnested
    fn can_unnest_exists_subquery(
        &self,
        subquery: &LogicalNode,
        outer_variables: &[String],
    ) -> bool {
        // Basic unnesting is possible if:
        // 1. Subquery doesn't contain aggregation (would need HAVING)
        // 2. Subquery references outer variables (correlated)
        // 3. Subquery doesn't contain LIMIT/OFFSET

        !self.contains_aggregation(subquery)
            && !outer_variables.is_empty()
            && !self.contains_limit(subquery)
    }

    /// Check if NOT EXISTS subquery can be unnested
    fn can_unnest_not_exists_subquery(
        &self,
        subquery: &LogicalNode,
        outer_variables: &[String],
    ) -> bool {
        // Same conditions as EXISTS
        self.can_unnest_exists_subquery(subquery, outer_variables)
    }

    /// Check if IN subquery can be unnested
    fn can_unnest_in_subquery(
        &self,
        subquery: &LogicalNode,
        outer_variables: &[String],
        _expression: &Expression,
    ) -> bool {
        // IN subquery can be unnested if:
        // 1. No aggregation
        // 2. Correlated (references outer variables)
        // 3. No LIMIT/OFFSET
        // 4. Subquery returns unique values (to avoid duplicate results)

        !self.contains_aggregation(subquery)
            && !outer_variables.is_empty()
            && !self.contains_limit(subquery)
            && self.returns_unique_values(subquery)
    }

    /// Unnest EXISTS subquery to LEFT SEMI JOIN
    fn unnest_exists_subquery(
        &self,
        subquery: LogicalNode,
        outer_variables: Vec<String>,
    ) -> Result<LogicalNode, PlanningError> {
        use crate::plan::logical::JoinType;

        // Convert to LEFT SEMI JOIN with correlation conditions
        let join_condition = self.build_correlation_condition(&outer_variables)?;

        // Create a placeholder for the outer query (would be provided by caller)
        // For now, create a simple node scan as the left side
        let outer_scan = LogicalNode::NodeScan {
            variable: "outer".to_string(),
            labels: vec![],
            properties: None,
        };

        Ok(LogicalNode::Join {
            join_type: JoinType::LeftSemi,
            condition: Some(join_condition),
            left: Box::new(outer_scan),
            right: Box::new(subquery),
        })
    }

    /// Unnest NOT EXISTS subquery to LEFT ANTI JOIN
    fn unnest_not_exists_subquery(
        &self,
        subquery: LogicalNode,
        outer_variables: Vec<String>,
    ) -> Result<LogicalNode, PlanningError> {
        use crate::plan::logical::JoinType;

        let join_condition = self.build_correlation_condition(&outer_variables)?;

        let outer_scan = LogicalNode::NodeScan {
            variable: "outer".to_string(),
            labels: vec![],
            properties: None,
        };

        Ok(LogicalNode::Join {
            join_type: JoinType::LeftAnti,
            condition: Some(join_condition),
            left: Box::new(outer_scan),
            right: Box::new(subquery),
        })
    }

    /// Unnest IN subquery to INNER JOIN
    fn unnest_in_subquery(
        &self,
        subquery: LogicalNode,
        outer_variables: Vec<String>,
        expression: Expression,
    ) -> Result<LogicalNode, PlanningError> {
        use crate::ast::{BinaryExpression, Operator};
        use crate::plan::logical::JoinType;

        // Build join condition combining correlation and IN expression
        let correlation_condition = self.build_correlation_condition(&outer_variables)?;
        let in_condition = self.build_in_join_condition(expression)?;

        // Combine conditions with AND
        let combined_condition = Expression::Binary(BinaryExpression {
            left: Box::new(correlation_condition),
            operator: Operator::And,
            right: Box::new(in_condition),
            location: crate::ast::Location::default(),
        });

        let outer_scan = LogicalNode::NodeScan {
            variable: "outer".to_string(),
            labels: vec![],
            properties: None,
        };

        Ok(LogicalNode::Join {
            join_type: JoinType::Inner,
            condition: Some(combined_condition),
            left: Box::new(outer_scan),
            right: Box::new(subquery),
        })
    }

    /// Build correlation condition from outer variables
    fn build_correlation_condition(
        &self,
        outer_variables: &[String],
    ) -> Result<Expression, PlanningError> {
        use crate::ast::{BinaryExpression, Operator, Variable};

        if outer_variables.is_empty() {
            return Err(PlanningError::InvalidQuery(
                "No correlation variables for join".to_string(),
            ));
        }

        // For simplicity, create an equality condition on the first outer variable
        // In practice, this would be more sophisticated
        let var_name = &outer_variables[0];
        Ok(Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Variable(Variable {
                name: format!("outer.{}", var_name),
                location: crate::ast::Location::default(),
            })),
            operator: Operator::Equal,
            right: Box::new(Expression::Variable(Variable {
                name: format!("inner.{}", var_name),
                location: crate::ast::Location::default(),
            })),
            location: crate::ast::Location::default(),
        }))
    }

    /// Build join condition for IN expression
    fn build_in_join_condition(&self, expression: Expression) -> Result<Expression, PlanningError> {
        // Convert IN expression to equality for join
        // This is a simplified implementation
        Ok(expression)
    }

    /// Check if node contains aggregation
    fn contains_aggregation(&self, node: &LogicalNode) -> bool {
        match node {
            LogicalNode::Aggregate { .. } => true,
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Sort { input, .. }
            | LogicalNode::Distinct { input, .. }
            | LogicalNode::Limit { input, .. } => self.contains_aggregation(input),
            LogicalNode::Join { left, right, .. } => {
                self.contains_aggregation(left) || self.contains_aggregation(right)
            }
            _ => false,
        }
    }

    /// Check if node contains LIMIT
    fn contains_limit(&self, node: &LogicalNode) -> bool {
        match node {
            LogicalNode::Limit { .. } => true,
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Sort { input, .. }
            | LogicalNode::Distinct { input, .. } => self.contains_limit(input),
            LogicalNode::Join { left, right, .. } => {
                self.contains_limit(left) || self.contains_limit(right)
            }
            _ => false,
        }
    }

    /// Check if node returns unique values
    fn returns_unique_values(&self, node: &LogicalNode) -> bool {
        match node {
            LogicalNode::Distinct { .. } => true,
            LogicalNode::NodeScan { .. } => true, // Assume node scans return unique nodes
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Sort { input, .. }
            | LogicalNode::Limit { input, .. } => self.returns_unique_values(input),
            _ => false, // Conservative approach
        }
    }
}

impl Default for LogicalOptimizer {
    fn default() -> Self {
        Self::new(OptimizationLevel::Basic)
    }
}
