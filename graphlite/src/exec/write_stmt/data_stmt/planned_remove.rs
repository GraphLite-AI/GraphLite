// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Planned REMOVE executor using logical and physical planning
//!
//! This executor handles REMOVE statements with optional MATCH/WHERE/WITH clauses
//! using the unified planning infrastructure.

use std::collections::HashMap;

use crate::ast::{LabelFactor, RemoveItem, RemoveStatement};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::physical::PhysicalPlan;
use crate::storage::GraphCache;
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for REMOVE statements using planned execution
pub struct PlannedRemoveExecutor {
    statement: RemoveStatement,
}

impl PlannedRemoveExecutor {
    /// Create a new PlannedRemoveExecutor
    pub fn new(statement: RemoveStatement) -> Self {
        Self { statement }
    }

    /// Execute a physical plan for REMOVE operation
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
        let mut removed_count = 0;

        // Process each REMOVE item
        for item in &self.statement.items {
            match item {
                RemoveItem::Variable(_var_name) => {
                    // Variable removal is not supported in this executor
                    // This would be handled by DELETE statement instead
                    return Err(ExecutionError::InvalidQuery(
                        "REMOVE variable is not supported. Use DELETE instead.".to_string(),
                    ));
                }
                RemoveItem::Property(property_access) => {
                    // Handle property removal (e.g., REMOVE n.name)
                    let var_name = &property_access.object;

                    // Find nodes to update
                    let node_ids_to_update: Vec<String> = graph
                        .get_all_nodes()
                        .iter()
                        .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                        .map(|node| node.id.clone())
                        .collect();

                    for node_id in node_ids_to_update {
                        // Get old property value for undo
                        let (old_properties, old_labels, has_property) = if let Some(node) =
                            graph.get_node(&node_id)
                        {
                            let mut old_props = HashMap::new();
                            let has_prop = if let Some(old_val) =
                                node.properties.get(&property_access.property)
                            {
                                old_props
                                    .insert(property_access.property.clone(), old_val.clone());
                                true
                            } else {
                                false
                            };
                            (old_props, node.labels.clone(), has_prop)
                        } else {
                            (HashMap::new(), Vec::new(), false)
                        };

                        if has_property {
                            // Remove the property
                            if let Some(node_mut) = graph.get_node_mut(&node_id) {
                                node_mut.remove_property(&property_access.property);
                                removed_count += 1;

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
                }
                RemoveItem::Label { variable: var_name, labels: label_expr } => {
                    // Handle label removal (e.g., REMOVE n:Person)
                    let labels_to_remove = Self::extract_labels_from_expression(label_expr);

                    // Find nodes to update
                    let node_ids_to_update: Vec<String> = graph
                        .get_all_nodes()
                        .iter()
                        .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                        .map(|node| node.id.clone())
                        .collect();

                    for node_id in node_ids_to_update {
                        // Get old labels for undo
                        let (old_properties, old_labels) = if let Some(node) =
                            graph.get_node(&node_id)
                        {
                            (node.properties.clone(), node.labels.clone())
                        } else {
                            (HashMap::new(), Vec::new())
                        };

                        // Remove labels
                        if let Some(node_mut) = graph.get_node_mut(&node_id) {
                            for label in &labels_to_remove {
                                if node_mut.labels.contains(label) {
                                    node_mut.labels.retain(|l| l != label);
                                    removed_count += 1;
                                }
                            }

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
            removed_count,
        ))
    }

    /// Extract label names from a label expression
    fn extract_labels_from_expression(expr: &crate::ast::LabelExpression) -> Vec<String> {
        let mut labels = Vec::new();
        for term in &expr.terms {
            for factor in &term.factors {
                if let LabelFactor::Identifier(label) = factor {
                    labels.push(label.clone());
                }
            }
        }
        labels
    }
}

impl StatementExecutor for PlannedRemoveExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Remove
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("REMOVE properties/labels in graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for PlannedRemoveExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        // Check if statement has MATCH/WHERE/WITH clauses
        // For now, we'll use direct execution
        // TODO: In Phase 4, add planning support

        self.execute_direct(graph, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{LabelExpression, LabelTerm, Location, PropertyAccess, RemoveStatement};
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

        // Add test node with properties
        let mut props = HashMap::new();
        props.insert("name".to_string(), Value::String("Alice".to_string()));
        props.insert("age".to_string(), Value::Number(30.0));
        props.insert("city".to_string(), Value::String("NYC".to_string()));

        let node = Node {
            id: "n1".to_string(),
            labels: vec!["Person".to_string(), "Employee".to_string()],
            properties: props,
        };

        graph.add_node(node).unwrap();
        graph
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_remove_property() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create REMOVE statement: REMOVE n1.city
        let statement = RemoveStatement {
            items: vec![crate::ast::RemoveItem::Property(PropertyAccess {
                object: "n1".to_string(),
                property: "city".to_string(),
                location: Location::default(),
            })],
            location: Location::default(),
        };

        let executor = PlannedRemoveExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 1);

        // Verify property was removed
        let node = graph.get_node("n1").unwrap();
        assert!(!node.properties.contains_key("city"));
        assert!(node.properties.contains_key("name")); // Other properties still exist
        assert!(node.properties.contains_key("age"));
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_remove_label() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create REMOVE statement: REMOVE n1:Employee
        let statement = RemoveStatement {
            items: vec![crate::ast::RemoveItem::Label {
                variable: "n1".to_string(),
                labels: LabelExpression {
                    terms: vec![LabelTerm {
                        factors: vec![crate::ast::LabelFactor::Identifier("Employee".to_string())],
                        location: Location::default(),
                    }],
                    location: Location::default(),
                },
            }],
            location: Location::default(),
        };

        let executor = PlannedRemoveExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 1);

        // Verify label was removed
        let node = graph.get_node("n1").unwrap();
        assert!(!node.labels.contains(&"Employee".to_string()));
        assert!(node.labels.contains(&"Person".to_string())); // Other label still exists
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_remove_nonexistent_property() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create REMOVE statement for non-existent property
        let statement = RemoveStatement {
            items: vec![crate::ast::RemoveItem::Property(PropertyAccess {
                object: "n1".to_string(),
                property: "nonexistent".to_string(),
                location: Location::default(),
            })],
            location: Location::default(),
        };

        let executor = PlannedRemoveExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        // Should still process the node even if property doesn't exist
        assert!(count <= 1);
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_remove_nonexistent_node() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create REMOVE statement for non-existent node
        let statement = RemoveStatement {
            items: vec![crate::ast::RemoveItem::Property(PropertyAccess {
                object: "nonexistent".to_string(),
                property: "name".to_string(),
                location: Location::default(),
            })],
            location: Location::default(),
        };

        let executor = PlannedRemoveExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 0); // Nothing removed
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_remove_variable_unsupported() {
        let mut graph = create_test_graph();
        let mut context = create_test_context();

        // Create REMOVE statement with variable (not supported)
        let statement = RemoveStatement {
            items: vec![crate::ast::RemoveItem::Variable("x".to_string())],
            location: Location::default(),
        };

        let executor = PlannedRemoveExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_err());
        match result {
            Err(ExecutionError::InvalidQuery(msg)) => {
                assert!(msg.contains("variable"));
            }
            _ => panic!("Expected InvalidQuery error"),
        }
    }
}
