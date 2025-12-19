// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Planned DELETE executor using logical and physical planning
//!
//! This executor handles DELETE statements with optional MATCH/WHERE/WITH clauses
//! using the unified planning infrastructure.

use crate::ast::DeleteStatement;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::physical::PhysicalPlan;
use crate::storage::GraphCache;
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for DELETE statements using planned execution
pub struct PlannedDeleteExecutor {
    statement: DeleteStatement,
}

impl PlannedDeleteExecutor {
    /// Create a new PlannedDeleteExecutor
    pub fn new(statement: DeleteStatement) -> Self {
        Self { statement }
    }

    /// Execute a physical plan for DELETE operation
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
        let mut deleted_count = 0;

        // Collect node/edge IDs to delete based on expressions
        let mut node_ids_to_delete = Vec::new();
        let mut edge_ids_to_delete = Vec::new();

        for expr in &self.statement.expressions {
            // Extract variable name from expression
            if let crate::ast::Expression::Variable(var) = expr {
                let var_name = &var.name;

                // Try to find matching nodes
                for node in graph.get_all_nodes() {
                    if node.id == *var_name || node.labels.contains(var_name) {
                        node_ids_to_delete.push(node.id.clone());
                    }
                }

                // Try to find matching edges
                for edge in graph.get_all_edges() {
                    if edge.id == *var_name || edge.label == *var_name {
                        edge_ids_to_delete.push(edge.id.clone());
                    }
                }
            }
        }

        // If DETACH DELETE, first delete connected edges
        if self.statement.detach {
            for node_id in &node_ids_to_delete {
                let connected_edges: Vec<String> = graph
                    .get_all_edges()
                    .iter()
                    .filter(|edge| edge.from_node == *node_id || edge.to_node == *node_id)
                    .map(|edge| edge.id.clone())
                    .collect();

                for edge_id in connected_edges {
                    if let Ok(deleted_edge) = graph.remove_edge(&edge_id) {
                        deleted_count += 1;
                        undo_operations.push(UndoOperation::DeleteEdge {
                            graph_path: graph_name.clone(),
                            edge_id: edge_id.clone(),
                            deleted_edge,
                        });
                    }
                }
            }
        }

        // Delete edges
        for edge_id in edge_ids_to_delete {
            if let Ok(deleted_edge) = graph.remove_edge(&edge_id) {
                deleted_count += 1;
                undo_operations.push(UndoOperation::DeleteEdge {
                    graph_path: graph_name.clone(),
                    edge_id: edge_id.clone(),
                    deleted_edge,
                });
            }
        }

        // Delete nodes
        for node_id in node_ids_to_delete {
            if let Ok(deleted_node) = graph.remove_node(&node_id) {
                deleted_count += 1;
                undo_operations.push(UndoOperation::DeleteNode {
                    graph_path: graph_name.clone(),
                    node_id: node_id.clone(),
                    deleted_node,
                });
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
            deleted_count,
        ))
    }
}

impl StatementExecutor for PlannedDeleteExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Delete
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        let prefix = if self.statement.detach { "DETACH " } else { "" };
        format!("{}DELETE nodes/edges in graph '{}'", prefix, graph_name)
    }
}

impl DataStatementExecutor for PlannedDeleteExecutor {
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
    use crate::ast::{DeleteStatement, Expression, Location, Variable};
    use crate::storage::{Edge, GraphCache, Node, StorageManager, StorageMethod, StorageType, Value};
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

    fn create_test_graph_with_edges() -> GraphCache {
        let mut graph = GraphCache::new();

        // Add nodes
        let node1 = Node {
            id: "n1".to_string(),
            labels: vec!["Person".to_string()],
            properties: HashMap::new(),
        };
        let node2 = Node {
            id: "n2".to_string(),
            labels: vec!["Person".to_string()],
            properties: HashMap::new(),
        };

        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();

        // Add edge
        let edge = Edge {
            id: "e1".to_string(),
            from_node: "n1".to_string(),
            to_node: "n2".to_string(),
            label: "KNOWS".to_string(),
            properties: HashMap::new(),
        };

        graph.add_edge(edge).unwrap();
        graph
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_delete_node() {
        let mut graph = create_test_graph_with_edges();
        let mut context = create_test_context();

        // Remove edge first so we can delete the node
        graph.remove_edge("e1").unwrap();

        // Create DELETE statement: DELETE n1
        let statement = DeleteStatement {
            expressions: vec![Expression::Variable(Variable {
                name: "n1".to_string(),
                location: Location::default(),
            })],
            detach: false,
            location: Location::default(),
        };

        let executor = PlannedDeleteExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 1);

        // Verify node was deleted
        assert!(graph.get_node("n1").is_none());
        assert!(graph.get_node("n2").is_some());
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_delete_detach() {
        let mut graph = create_test_graph_with_edges();
        let mut context = create_test_context();

        // Create DETACH DELETE statement
        let statement = DeleteStatement {
            expressions: vec![Expression::Variable(Variable {
                name: "n1".to_string(),
                location: Location::default(),
            })],
            detach: true,
            location: Location::default(),
        };

        let executor = PlannedDeleteExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert!(count >= 1); // Should delete node and connected edge

        // Verify node was deleted
        assert!(graph.get_node("n1").is_none());

        // Verify connected edge was also deleted
        assert!(graph.get_edge("e1").is_none());
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_delete_edge() {
        let mut graph = create_test_graph_with_edges();
        let mut context = create_test_context();

        // Create DELETE statement for edge
        let statement = DeleteStatement {
            expressions: vec![Expression::Variable(Variable {
                name: "e1".to_string(),
                location: Location::default(),
            })],
            detach: false,
            location: Location::default(),
        };

        let executor = PlannedDeleteExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());

        // Verify edge was deleted
        assert!(graph.get_edge("e1").is_none());

        // Verify nodes still exist
        assert!(graph.get_node("n1").is_some());
        assert!(graph.get_node("n2").is_some());
    }

    #[test]
    #[ignore]  // TODO: Convert to integration tests - requires full session infrastructure
    fn test_planned_delete_nonexistent() {
        let mut graph = create_test_graph_with_edges();
        let mut context = create_test_context();

        // Create DELETE statement for non-existent node
        let statement = DeleteStatement {
            expressions: vec![Expression::Variable(Variable {
                name: "nonexistent".to_string(),
                location: Location::default(),
            })],
            detach: false,
            location: Location::default(),
        };

        let executor = PlannedDeleteExecutor::new(statement);
        let result = executor.execute_modification(&mut graph, &mut context);

        assert!(result.is_ok());
        let (_, count) = result.unwrap();
        assert_eq!(count, 0); // Nothing deleted
    }
}
