// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;

use crate::ast::ast::{RemoveStatement, RemoveItem, LabelFactor};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::storage::GraphCache;
use crate::txn::{UndoOperation, state::OperationType};

/// Executor for REMOVE statements
pub struct RemoveExecutor {
    statement: RemoveStatement,
}

impl RemoveExecutor {
    /// Create a new RemoveExecutor
    pub fn new(statement: RemoveStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for RemoveExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Remove
    }
    
    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context.get_graph_name().unwrap_or_else(|_| "unknown".to_string());
        format!("REMOVE properties/labels in graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for RemoveExecutor {
    
    fn execute_modification(
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
                RemoveItem::Property(property_access) => {
                    // Handle property removal (e.g., REMOVE n.name)
                    let var_name = &property_access.object;
                    
                    // Find nodes with this variable identifier
                    let node_ids_to_update: Vec<String> = graph.get_all_nodes()
                        .iter()
                        .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                        .map(|node| node.id.clone())
                        .collect();
                    
                    for node_id in node_ids_to_update {
                        // Get old property value and labels for undo
                        let (old_properties, old_labels, has_property) = if let Some(node) = graph.get_node(&node_id) {
                            let mut old_props = HashMap::new();
                            let has_prop = if let Some(old_val) = node.properties.get(&property_access.property) {
                                old_props.insert(property_access.property.clone(), old_val.clone());
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
                                log::debug!("Removed property {} from node {}", property_access.property, node_id);
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
                },
                RemoveItem::Label { variable, labels } => {
                    // Find nodes with this variable identifier
                    let node_ids_to_update: Vec<String> = graph.get_all_nodes()
                        .iter()
                        .filter(|node| node.id == *variable || node.labels.contains(variable))
                        .map(|node| node.id.clone())
                        .collect();
                    
                    for node_id in node_ids_to_update {
                        // Get original labels for undo
                        let old_labels = if let Some(node) = graph.get_node(&node_id) {
                            node.labels.clone()
                        } else {
                            continue;
                        };
                        
                        let mut removed_any = false;
                        
                        // Extract labels to remove
                        for term in &labels.terms {
                            for factor in &term.factors {
                                if let LabelFactor::Identifier(label_name) = factor {
                                    if let Some(node_mut) = graph.get_node_mut(&node_id) {
                                        let original_len = node_mut.labels.len();
                                        node_mut.labels.retain(|l| l != label_name);
                                        if node_mut.labels.len() < original_len {
                                            log::debug!("Removed label {} from node {}", label_name, node_id);
                                            removed_any = true;
                                        }
                                    }
                                }
                            }
                        }
                        
                        if removed_any {
                            removed_count += 1;
                            
                            // Add undo operation
                            undo_operations.push(UndoOperation::UpdateNode {
                                graph_path: graph_name.clone(),
                                node_id: node_id.clone(),
                                old_properties: HashMap::new(),
                                old_labels,
                            });
                        }
                    }
                },
                RemoveItem::Variable(variable) => {
                    log::warn!("Variable removal in REMOVE not yet supported: {}", variable);
                    // This would typically remove variable bindings from execution context
                }
            }
        }
        
        // Return the first undo operation if any
        let undo_op = undo_operations.into_iter().next().unwrap_or_else(|| {
            UndoOperation::UpdateNode {
                graph_path: graph_name,
                node_id: "no_operations".to_string(),
                old_properties: HashMap::new(),
                old_labels: vec![],
            }
        });
        
        Ok((undo_op, removed_count))
    }
}
