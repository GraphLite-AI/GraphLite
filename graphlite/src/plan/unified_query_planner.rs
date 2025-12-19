// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Unified Query Planner for all data modification operations
//!
//! This planner handles MATCH/WHERE/WITH clauses for all operations (INSERT, SET, DELETE, REMOVE)
//! and generates optimizable logical plans.

use crate::ast::{Expression, MatchClause, WithClause};
use crate::plan::insert_planner::PlanningError;
use crate::plan::logical::{JoinType, LogicalNode, LogicalPlan};

/// Unified query planner that handles MATCH/WHERE/WITH for all operations
pub struct UnifiedQueryPlanner;

impl UnifiedQueryPlanner {
    /// Plan MATCH clause into NodeScan/Expand operators
    ///
    /// Converts a MATCH clause with its patterns into a logical plan tree.
    /// Handles:
    /// - Simple node patterns: (p:Person) -> NodeScan
    /// - Edge patterns: (a)-[:KNOWS]->(b) -> NodeScan + Expand
    /// - Multiple patterns: (a), (b) -> Join (Cartesian product)
    pub fn plan_match(match_clause: &MatchClause) -> Result<LogicalNode, PlanningError> {
        if match_clause.patterns.is_empty() {
            return Err(PlanningError::InvalidPattern(
                "MATCH clause has no patterns".to_string(),
            ));
        }

        // Convert first pattern
        let mut result = LogicalPlan::from_path_pattern(&match_clause.patterns[0])
            .map_err(|e| PlanningError::InvalidPattern(e))?;

        // If there are multiple patterns, join them with Cross Join (Cartesian product)
        for pattern in &match_clause.patterns[1..] {
            let right = LogicalPlan::from_path_pattern(pattern)
                .map_err(|e| PlanningError::InvalidPattern(e))?;

            result = LogicalNode::Join {
                join_type: JoinType::Cross,
                condition: None,
                left: Box::new(result),
                right: Box::new(right),
            };
        }

        Ok(result)
    }

    /// Plan WHERE clause into Filter operator
    ///
    /// Wraps the input logical node with a Filter node containing the condition.
    pub fn plan_where(
        condition: &Expression,
        input: LogicalNode,
    ) -> Result<LogicalNode, PlanningError> {
        Ok(LogicalNode::Filter {
            condition: condition.clone(),
            input: Box::new(input),
        })
    }

    /// Plan WITH clause into Aggregate or Project operator
    ///
    /// Converts a WITH clause into either:
    /// - LogicalNode::Aggregate if there are aggregate functions (COUNT, SUM, etc.)
    /// - LogicalNode::Project if only projections/aliases
    pub fn plan_with(
        with_clause: &WithClause,
        input: LogicalNode,
    ) -> Result<LogicalNode, PlanningError> {
        use crate::plan::logical::{AggregateExpression, ProjectExpression};

        // Check if any items contain aggregate functions
        let has_aggregates = with_clause.items.iter().any(|item| {
            Self::is_aggregate_expression(&item.expression)
        });

        let mut result = if has_aggregates {
            // Build aggregate node
            let mut group_by = Vec::new();
            let mut aggregates = Vec::new();

            for item in &with_clause.items {
                if let Expression::FunctionCall(func_call) = &item.expression {
                    if let Some(agg_func) = Self::get_aggregate_function(&func_call.name) {
                        // This is an aggregate function
                        let arg_expr = if func_call.arguments.is_empty() {
                            Expression::Literal(crate::ast::Literal::Integer(1)) // COUNT(*) case
                        } else {
                            func_call.arguments[0].clone()
                        };

                        aggregates.push(AggregateExpression {
                            function: agg_func,
                            expression: arg_expr,
                            alias: item.alias.clone(),
                        });
                    } else {
                        // Non-aggregate function in GROUP BY
                        group_by.push(item.expression.clone());
                    }
                } else {
                    // Non-aggregate expression goes to GROUP BY
                    group_by.push(item.expression.clone());
                }
            }

            LogicalNode::Aggregate {
                group_by,
                aggregates,
                input: Box::new(input),
            }
        } else {
            // No aggregates, use projection
            let expressions = with_clause.items.iter().map(|item| {
                ProjectExpression {
                    expression: item.expression.clone(),
                    alias: item.alias.clone(),
                }
            }).collect();

            LogicalNode::Project {
                expressions,
                input: Box::new(input),
            }
        };

        // Add WHERE clause as filter if present
        if let Some(where_clause) = &with_clause.where_clause {
            result = LogicalNode::Filter {
                condition: where_clause.condition.clone(),
                input: Box::new(result),
            };
        }

        // Add ORDER BY if present
        if let Some(order_clause) = &with_clause.order_clause {
            use crate::ast::OrderDirection;
            use crate::plan::logical::SortExpression;

            let expressions = order_clause.items.iter().map(|item| {
                SortExpression {
                    expression: item.expression.clone(),
                    ascending: matches!(item.direction, OrderDirection::Ascending),
                }
            }).collect();

            result = LogicalNode::Sort {
                expressions,
                input: Box::new(result),
            };
        }

        // Add LIMIT if present
        if let Some(limit_clause) = &with_clause.limit_clause {
            result = LogicalNode::Limit {
                count: limit_clause.count,
                offset: limit_clause.offset,
                input: Box::new(result),
            };
        }

        Ok(result)
    }

    /// Check if an expression is an aggregate function
    fn is_aggregate_expression(expr: &Expression) -> bool {
        match expr {
            Expression::FunctionCall(func_call) => {
                Self::get_aggregate_function(&func_call.name).is_some()
            }
            _ => false,
        }
    }

    /// Get aggregate function type from function name
    fn get_aggregate_function(name: &str) -> Option<crate::plan::logical::AggregateFunction> {
        use crate::plan::logical::AggregateFunction;

        match name.to_uppercase().as_str() {
            "COUNT" => Some(AggregateFunction::Count),
            "SUM" => Some(AggregateFunction::Sum),
            "AVG" | "AVERAGE" => Some(AggregateFunction::Avg),
            "MIN" => Some(AggregateFunction::Min),
            "MAX" => Some(AggregateFunction::Max),
            "COLLECT" => Some(AggregateFunction::Collect),
            _ => None,
        }
    }

    // ============================================================================
    // Convenience Methods - Combine multiple planning operations
    // ============================================================================

    /// Plan a complete MATCH...WHERE pipeline
    ///
    /// This is a convenience method that combines MATCH and WHERE planning.
    /// Used for patterns like: MATCH (p:Patient) WHERE p.age > 65
    ///
    /// # Arguments
    /// * `match_clause` - The MATCH clause to plan
    /// * `where_condition` - Optional WHERE condition
    ///
    /// # Returns
    /// LogicalNode tree with MATCH followed by WHERE (if present)
    pub fn plan_match_where(
        match_clause: &MatchClause,
        where_condition: Option<&Expression>,
    ) -> Result<LogicalNode, PlanningError> {
        let mut plan = Self::plan_match(match_clause)?;

        if let Some(condition) = where_condition {
            plan = Self::plan_where(condition, plan)?;
        }

        Ok(plan)
    }

    /// Plan a complete MATCH...WHERE...WITH pipeline
    ///
    /// This is a convenience method that combines MATCH, WHERE, and WITH planning.
    /// Used for patterns like:
    /// MATCH (p:Patient) WHERE p.age > 65 WITH p, COUNT(*) AS patient_count
    ///
    /// # Arguments
    /// * `match_clause` - The MATCH clause to plan
    /// * `where_condition` - Optional WHERE condition
    /// * `with_clause` - Optional WITH clause
    ///
    /// # Returns
    /// LogicalNode tree with MATCH, WHERE (if present), and WITH (if present)
    pub fn plan_match_where_with(
        match_clause: &MatchClause,
        where_condition: Option<&Expression>,
        with_clause: Option<&WithClause>,
    ) -> Result<LogicalNode, PlanningError> {
        let mut plan = Self::plan_match_where(match_clause, where_condition)?;

        if let Some(with) = with_clause {
            plan = Self::plan_with(with, plan)?;
        }

        Ok(plan)
    }

    /// Plan a complete query pipeline for data modification operations
    ///
    /// This is the main entry point for planning MATCH...WHERE...WITH operations
    /// that will feed into INSERT/SET/DELETE/REMOVE operations.
    ///
    /// # Arguments
    /// * `match_clause` - Optional MATCH clause (None for standalone operations)
    /// * `where_condition` - Optional WHERE condition (requires MATCH)
    /// * `with_clause` - Optional WITH clause (requires MATCH)
    ///
    /// # Returns
    /// * `Some(LogicalNode)` - If MATCH clause is present
    /// * `None` - If no MATCH clause (standalone operation)
    ///
    /// # Examples
    /// ```ignore
    /// // Standalone INSERT: None
    /// let input = UnifiedQueryPlanner::plan_query_pipeline(None, None, None)?;
    /// assert!(input.is_none());
    ///
    /// // MATCH...INSERT: Some(NodeScan)
    /// let input = UnifiedQueryPlanner::plan_query_pipeline(
    ///     Some(&match_clause),
    ///     None,
    ///     None
    /// )?;
    /// assert!(input.is_some());
    ///
    /// // MATCH...WHERE...WITH...INSERT: Some(Filter -> Aggregate -> NodeScan)
    /// let input = UnifiedQueryPlanner::plan_query_pipeline(
    ///     Some(&match_clause),
    ///     Some(&where_condition),
    ///     Some(&with_clause)
    /// )?;
    /// ```
    pub fn plan_query_pipeline(
        match_clause: Option<&MatchClause>,
        where_condition: Option<&Expression>,
        with_clause: Option<&WithClause>,
    ) -> Result<Option<LogicalNode>, PlanningError> {
        if let Some(match_clause) = match_clause {
            let plan = Self::plan_match_where_with(match_clause, where_condition, with_clause)?;
            log::debug!("UnifiedQueryPlanner generated logical plan: {:#?}", plan);
            Ok(Some(plan))
        } else {
            // Standalone operation (no MATCH clause)
            // WHERE and WITH require MATCH, so we ignore them
            if where_condition.is_some() || with_clause.is_some() {
                return Err(PlanningError::InvalidPattern(
                    "WHERE and WITH clauses require a MATCH clause".to_string(),
                ));
            }
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        DistinctQualifier, EdgeDirection, Location, Node, OrderClause, OrderDirection, OrderItem,
        PathPattern, PatternElement, PropertyAccess, Variable, WithItem,
    };

    #[test]
    fn test_plan_match_single_node() {
        let match_clause = MatchClause {
            patterns: vec![PathPattern {
                assignment: None,
                path_type: None,
                elements: vec![PatternElement::Node(Node {
                    identifier: Some("p".to_string()),
                    labels: vec!["Person".to_string()],
                    label_expression: None,
                    properties: None,
                    location: Location::default(),
                })],
                location: Location::default(),
            }],
            location: Location::default(),
        };

        let result = UnifiedQueryPlanner::plan_match(&match_clause);
        assert!(result.is_ok());

        match result.unwrap() {
            LogicalNode::NodeScan { variable, labels, .. } => {
                assert_eq!(variable, "p");
                assert_eq!(labels, vec!["Person"]);
            }
            _ => panic!("Expected NodeScan"),
        }
    }

    #[test]
    fn test_plan_match_empty_patterns_error() {
        let match_clause = MatchClause {
            patterns: vec![],
            location: Location::default(),
        };

        let result = UnifiedQueryPlanner::plan_match(&match_clause);
        assert!(result.is_err());
    }

    #[test]
    fn test_plan_where_adds_filter() {
        let condition = Expression::Variable(Variable {
            name: "x".to_string(),
            location: Location::default(),
        });

        let input = LogicalNode::NodeScan {
            variable: "p".to_string(),
            labels: vec![],
            properties: None,
        };

        let result = UnifiedQueryPlanner::plan_where(&condition, input);
        assert!(result.is_ok());

        match result.unwrap() {
            LogicalNode::Filter { .. } => {}
            _ => panic!("Expected Filter node"),
        }
    }

    #[test]
    fn test_plan_with_simple_projection() {
        let with_clause = WithClause {
            distinct: DistinctQualifier::None,
            items: vec![WithItem {
                expression: Expression::Variable(Variable {
                    name: "p".to_string(),
                    location: Location::default(),
                }),
                alias: Some("person".to_string()),
                location: Location::default(),
            }],
            where_clause: None,
            order_clause: None,
            limit_clause: None,
            location: Location::default(),
        };

        let input = LogicalNode::NodeScan {
            variable: "p".to_string(),
            labels: vec![],
            properties: None,
        };

        let result = UnifiedQueryPlanner::plan_with(&with_clause, input);
        assert!(result.is_ok());

        match result.unwrap() {
            LogicalNode::Project { .. } => {}
            _ => panic!("Expected Project node"),
        }
    }

    #[test]
    fn test_plan_with_order_by() {
        let with_clause = WithClause {
            distinct: DistinctQualifier::None,
            items: vec![WithItem {
                expression: Expression::Variable(Variable {
                    name: "p".to_string(),
                    location: Location::default(),
                }),
                alias: None,
                location: Location::default(),
            }],
            where_clause: None,
            order_clause: Some(OrderClause {
                items: vec![OrderItem {
                    expression: Expression::PropertyAccess(PropertyAccess {
                        object: "p".to_string(),
                        property: "name".to_string(),
                        location: Location::default(),
                    }),
                    direction: OrderDirection::Ascending,
                    nulls_ordering: None,
                    location: Location::default(),
                }],
                location: Location::default(),
            }),
            limit_clause: None,
            location: Location::default(),
        };

        let input = LogicalNode::NodeScan {
            variable: "p".to_string(),
            labels: vec![],
            properties: None,
        };

        let result = UnifiedQueryPlanner::plan_with(&with_clause, input);
        assert!(result.is_ok());

        // Should produce Project -> Sort
        match result.unwrap() {
            LogicalNode::Sort { .. } => {}
            _ => panic!("Expected Sort node"),
        }
    }

    #[test]
    fn test_plan_query_pipeline_standalone() {
        let result = UnifiedQueryPlanner::plan_query_pipeline(None, None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_plan_query_pipeline_with_match() {
        let match_clause = MatchClause {
            patterns: vec![PathPattern {
                assignment: None,
                path_type: None,
                elements: vec![PatternElement::Node(Node {
                    identifier: Some("p".to_string()),
                    labels: vec![],
                    label_expression: None,
                    properties: None,
                    location: Location::default(),
                })],
                location: Location::default(),
            }],
            location: Location::default(),
        };

        let result = UnifiedQueryPlanner::plan_query_pipeline(Some(&match_clause), None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_plan_query_pipeline_rejects_where_without_match() {
        let condition = Expression::Variable(Variable {
            name: "x".to_string(),
            location: Location::default(),
        });

        let result = UnifiedQueryPlanner::plan_query_pipeline(None, Some(&condition), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_aggregate_function_count() {
        assert!(UnifiedQueryPlanner::get_aggregate_function("COUNT").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("count").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("CoUnT").is_some());
    }

    #[test]
    fn test_get_aggregate_function_all_types() {
        assert!(UnifiedQueryPlanner::get_aggregate_function("SUM").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("AVG").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("AVERAGE").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("MIN").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("MAX").is_some());
        assert!(UnifiedQueryPlanner::get_aggregate_function("COLLECT").is_some());
    }

    #[test]
    fn test_get_aggregate_function_unknown() {
        assert!(UnifiedQueryPlanner::get_aggregate_function("UNKNOWN").is_none());
        assert!(UnifiedQueryPlanner::get_aggregate_function("CONCAT").is_none());
    }
}
