// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Planned SET executor using logical and physical planning
//!
//! This executor handles SET statements with optional MATCH/WHERE/WITH clauses
//! using the unified planning infrastructure.

use std::collections::HashMap;

use crate::ast::SetStatement;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::physical::PhysicalPlan;
use crate::plan::unified_query_planner::UnifiedQueryPlanner;
use crate::storage::GraphCache;
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for SET statements using planned execution
pub struct PlannedSetExecutor {
    statement: SetStatement,
}

impl PlannedSetExecutor {
    /// Create a new PlannedSetExecutor
    pub fn new(statement: SetStatement) -> Self {
        Self { statement }
    }

    /// Execute a physical plan for SET operation
    fn execute_physical_plan(
        &self,
        _plan: &PhysicalPlan,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        // For now, fall back to direct execution
        // TODO: Implement full physical plan execution in Phase 4
        self.execute_direct(graph, context)
    }

    /// Direct execution without planning (backward compatibility)
    fn execute_direct(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut updated_count = 0;

        // Process each SET item
        for item in &self.statement.items {
            match item {
                crate::ast::SetItem::Property { property, value } => {
                    let new_value = context.evaluate_simple_expression(value).map_err(|e| {
                        ExecutionError::ExpressionError(format!(
                            "Failed to evaluate SET property '{}': {}",
                            property.property, e
                        ))
                    })?;

                    let var_name = &property.object;

                    // Find nodes to update
                    let node_ids_to_update: Vec<String> = graph
                        .get_all_nodes()
                        .iter()
                        .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                        .map(|node| node.id.clone())
                        .collect();

                    for node_id in node_ids_to_update {
                        // Get old state for undo
                        let (old_properties, old_labels) = if let Some(node) = graph.get_node(&node_id) {
                            (node.properties.clone(), node.labels.clone())
                        } else {
                            (HashMap::new(), Vec::new())
                        };

                        // Update the node
                        if let Some(node_mut) = graph.get_node_mut(&node_id) {
                            node_mut.set_property(property.property.clone(), new_value.clone());
                            updated_count += 1;

                            // Add undo operation
                            undo_operations.push(UndoOperation::UpdateNode {
                                graph_path: graph_name.clone(),
                                node_id: node_id.clone(),
                                old_properties,
                                old_labels,
                            });
                        }
                    }
                }
                crate::ast::SetItem::Variable { variable: _var_name, value: _value } => {
                    // Variable assignment is not supported in this executor
                    return Err(ExecutionError::InvalidQuery(
                        "SET variable assignment is not yet supported.".to_string(),
                    ));
                }
                crate::ast::SetItem::Label { variable: _var_name, labels: _label_expr } => {
                    // Label assignment is not supported in this executor
                    return Err(ExecutionError::InvalidQuery(
                        "SET label assignment is not yet supported.".to_string(),
                    ));
                }
            }
        }

        // Return batch undo operation
        Ok((
            if undo_operations.len() == 1 {
                undo_operations.into_iter().next().unwrap()
            } else {
                UndoOperation::Batch {
                    operations: undo_operations,
                }
            },
            updated_count,
        ))
    }
}

impl StatementExecutor for PlannedSetExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Set
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("SET properties in graph '{}'", graph_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Location, PropertyAccess, SetStatement};
    use crate::storage::{GraphCache, Node, StorageManager, StorageMethod, StorageType, Value};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::env;

    fn create_test_context() -> ExecutionContext {
        let temp_dir = env::temp_dir().join(format!("graphlite_test_{}", uuid::Uuid::new_v4()));
        let storage_manager = Arc::new(
            StorageManager::new(&temp_dir, StorageMethod::DiskOnly, StorageType::Sled).unwrap()
        );
        let session_id = format!("test_session_{}", uuid::Uuid::new_v4());
        let mut context = ExecutionContext::new(session_id.clone(), storage_manager.clone());
        context.current_graph = Some(Arc::new(GraphCache::new()));

        // Create a minimal session with graph name
        use crate::session::{manager::create_session, models::SessionPermissionCache};
        let permission_cache = SessionPermissionCache::new();
        let _ = create_session("test_user".to_string(), vec![], permission_cache);

        // Set the current graph name in the session
        if let Some(session_arc) = context.get_session() {
            if let Ok(mut session) = session_arc.write() {
                session.current_graph = Some("/test_schema/test_graph".to_string());
            }
        }

        context
    }

    fn create_test_graph() -> GraphCache {
        let mut graph = GraphCache::new();

        // Add test nodes
        let mut props1 = HashMap::new();
        props1.insert("name".to_string(), Value::String("Alice".to_string()));
        props1.insert("age".to_string(), Value::Number(30.0));

        let node1 = Node {
            id: "n1".to_string(),
            labels: vec!["Person".to_string()],
            properties: props1,
        };

        graph.add_node(node1).unwrap();
        graph
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_set_property() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create SET statement: SET n1.age = 31
        let statement = SetStatement {
            items: vec![crate::ast::SetItem::Property {
                property: PropertyAccess {
                    object: "n1".to_string(),
                    property: "age".to_string(),
                    location: Location::default(),
                },
                value: crate::ast::Expression::Literal(crate::ast::Literal::Integer(31)),
            }],
            location: Location::default(),
        };

        let executor = PlannedSetExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 1);

        // Verify the property was updated
        let node = graph.get_node("n1").unwrap();
        assert_eq!(node.properties.get("age"), Some(&Value::Number(31.0)));
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_set_new_property() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create SET statement: SET n1.city = "NYC"
        let statement = SetStatement {
            items: vec![crate::ast::SetItem::Property {
                property: PropertyAccess {
                    object: "n1".to_string(),
                    property: "city".to_string(),
                    location: Location::default(),
                },
                value: crate::ast::Expression::Literal(crate::ast::Literal::String("NYC".to_string())),
            }],
            location: Location::default(),
        };

        let executor = PlannedSetExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 1);

        // Verify the new property was added
        let node = graph.get_node("n1").unwrap();
        assert_eq!(node.properties.get("city"), Some(&Value::String("NYC".to_string())));
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_set_nonexistent_node() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create SET statement for non-existent node
        let statement = SetStatement {
            items: vec![crate::ast::SetItem::Property {
                property: PropertyAccess {
                    object: "nonexistent".to_string(),
                    property: "name".to_string(),
                    location: Location::default(),
                },
                value: crate::ast::Expression::Literal(crate::ast::Literal::String("Test".to_string())),
            }],
            location: Location::default(),
        };

        let executor = PlannedSetExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 0); // No nodes updated
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_set_variable_unsupported() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create SET statement with variable assignment (not supported yet)
        let statement = SetStatement {
            items: vec![crate::ast::SetItem::Variable {
                variable: "x".to_string(),
                value: crate::ast::Expression::Literal(crate::ast::Literal::Integer(42)),
            }],
            location: Location::default(),
        };

        let executor = PlannedSetExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_err());
        match result {
            Err(ExecutionError::InvalidQuery(msg)) => {
                assert!(msg.contains("variable assignment"));
            }
            _ => panic!("Expected InvalidQuery error"),
        }
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_set_label_unsupported() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create SET statement with label assignment (not supported yet)
        let statement = SetStatement {
            items: vec![crate::ast::SetItem::Label {
                variable: "n1".to_string(),
                labels: crate::ast::LabelExpression {
                    terms: vec![],
                    location: Location::default(),
                },
            }],
            location: Location::default(),
        };

        let executor = PlannedSetExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_err());
        match result {
            Err(ExecutionError::InvalidQuery(msg)) => {
                assert!(msg.contains("label assignment"));
            }
            _ => panic!("Expected InvalidQuery error"),
        }
    }
}

impl DataStatementExecutor for PlannedSetExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        // Check if statement has MATCH/WHERE/WITH clauses
        // For now, we'll use direct execution
        // TODO: In Phase 4, add planning support:
        // if self.statement.has_match_clause() {
        //     let logical_plan = UnifiedQueryPlanner::plan_query_pipeline(...)?;
        //     let physical_plan = PhysicalPlan::from_logical(&logical_plan);
        //     return self.execute_physical_plan(&physical_plan, graph, context);
        // }

        self.execute_direct(graph, context)
    }
}
