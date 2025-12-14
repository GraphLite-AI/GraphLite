// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;

use crate::ast::{SetItem, SetStatement};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
#[allow(unused_imports)]
use crate::storage::{GraphCache, Node, Value};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for SET statements
pub struct SetExecutor {
    statement: SetStatement,
}

impl SetExecutor {
    /// Create a new SetExecutor
    pub fn new(statement: SetStatement) -> Self {
        Self { statement }
    }

    /// Phase 3: Auto-index updated nodes on text indexes
    /// When a property is updated, re-index affected text indexes
    fn auto_index_node_update(
        node_id: &str,
        updated_property: &str,
        _context: &ExecutionContext,
        graph: &GraphCache,
    ) {
        use crate::storage::indexes::text::metadata::get_metadata_for_label;
        use crate::storage::indexes::text::registry::get_text_index;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Get the updated node to find its labels
        let node = match graph.get_node(node_id) {
            Some(n) => n,
            None => {
                log::debug!("Node '{}' not found for auto-indexing", node_id);
                return;
            }
        };

        // For each label on the updated node
        for label in &node.labels {
            // Find all text indexes defined on this label
            let metadata_vec = match get_metadata_for_label(label) {
                Ok(vec) => vec,
                Err(e) => {
                    log::debug!("No indexes found for label '{}': {}", label, e);
                    continue;
                }
            };

            // For each matching index, update the node if the indexed field was changed
            for metadata in metadata_vec {
                // Only reindex if the updated property matches the indexed field
                if metadata.field != updated_property {
                    log::debug!(
                        "Property '{}' doesn't match indexed field '{}', skipping auto-index",
                        updated_property,
                        metadata.field
                    );
                    continue;
                }

                // Get the index from registry
                let index = match get_text_index(&metadata.name) {
                    Ok(Some(idx)) => idx,
                    Ok(None) => {
                        log::warn!("Index '{}' not found in registry", metadata.name);
                        continue;
                    }
                    Err(e) => {
                        log::warn!("Failed to get index '{}': {}", metadata.name, e);
                        continue;
                    }
                };

                // Extract the updated field value
                let field_value = match node.properties.get(&metadata.field) {
                    Some(value) => value,
                    None => {
                        log::debug!(
                            "Node '{}' doesn't have field '{}', skipping auto-index",
                            node_id,
                            metadata.field
                        );
                        continue;
                    }
                };

                // Convert value to string for indexing
                let text_value = match field_value {
                    crate::storage::Value::String(s) => s.clone(),
                    crate::storage::Value::Number(n) => n.to_string(),
                    crate::storage::Value::Boolean(b) => b.to_string(),
                    _ => {
                        log::debug!(
                            "Field '{}' has non-textual type, skipping auto-index",
                            metadata.field
                        );
                        continue;
                    }
                };

                // Convert node ID to u64 by hashing
                let mut hasher = DefaultHasher::new();
                node_id.hash(&mut hasher);
                let doc_id = hasher.finish();

                // Add/update the document in the index
                if let Err(e) = index.add_document(doc_id, &text_value) {
                    log::warn!(
                        "Failed to auto-index update for node '{}' in index '{}': {}",
                        node_id,
                        metadata.name,
                        e
                    );
                } else {
                    log::debug!(
                        "Auto-indexed update for node '{}' in index '{}' (field: {})",
                        node_id,
                        metadata.name,
                        metadata.field
                    );

                    // Commit after update (Phase 3.3 will optimize this)
                    if let Err(e) = index.commit() {
                        log::warn!(
                            "Failed to commit index '{}' after auto-indexing: {}",
                            metadata.name,
                            e
                        );
                    }
                }
            }
        }
    }
}

impl StatementExecutor for SetExecutor {
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

impl DataStatementExecutor for SetExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut updated_count = 0;

        // TRANSACTIONAL GUARANTEE: Pre-evaluate ALL property expressions before making ANY changes
        // This ensures that if any expression fails, we fail the entire SET operation atomically
        let mut evaluated_properties = Vec::new();
        for item in &self.statement.items {
            if let SetItem::Property { property, value } = item {
                // Evaluate the value - fail immediately if invalid (no partial updates!)
                let new_value = context.evaluate_simple_expression(value).map_err(|e| {
                    ExecutionError::ExpressionError(format!(
                        "Failed to evaluate SET property '{}': {}. Transaction aborted.",
                        property.property, e
                    ))
                })?;
                evaluated_properties.push((property.clone(), new_value));
            }
        }

        // Now that ALL expressions are valid, apply the changes
        for (property, new_value) in evaluated_properties {
            let var_name = &property.object;

            // Find and update nodes with this variable identifier
            // This is a simplified approach - in reality, would use execution context
            let node_ids_to_update: Vec<String> = graph
                .get_all_nodes()
                .iter()
                .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                .map(|node| node.id.clone())
                .collect();

            for node_id in node_ids_to_update {
                // Get ALL old properties and labels for undo (need full state for rollback)
                let (old_properties, old_labels) = if let Some(node) = graph.get_node(&node_id) {
                    (node.properties.clone(), node.labels.clone())
                } else {
                    (HashMap::new(), Vec::new())
                };

                // Update the node
                if let Some(node_mut) = graph.get_node_mut(&node_id) {
                    node_mut.set_property(property.property.clone(), new_value.clone());
                    log::debug!(
                        "Set property {} on node {} to {:?}",
                        property.property,
                        node_id,
                        new_value
                    );
                    updated_count += 1;

                    // Phase 3: Auto-indexing on UPDATE
                    // Re-index this node if the updated property is indexed
                    Self::auto_index_node_update(&node_id, &property.property, context, graph);

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

        // Handle other SET item types (TODO: these should also be transactional)
        for item in &self.statement.items {
            match item {
                SetItem::Property { .. } => {
                    // Already handled above
                }
                SetItem::Variable { variable, value } => {
                    log::warn!(
                        "Variable assignment in SET not yet fully supported: {} = {:?}",
                        variable,
                        value
                    );
                }
                SetItem::Label { variable, labels } => {
                    log::warn!(
                        "Label assignment in SET not yet fully supported: {} {:?}",
                        variable,
                        labels
                    );
                }
            }
        }

        // Return all undo operations as a batch for transactional rollback
        let undo_op = if undo_operations.is_empty() {
            UndoOperation::UpdateNode {
                graph_path: graph_name,
                node_id: "no_operations".to_string(),
                old_properties: HashMap::new(),
                old_labels: vec![],
            }
        } else if undo_operations.len() == 1 {
            // Single operation - return it directly
            undo_operations.into_iter().next().unwrap()
        } else {
            // Multiple operations - return as batch for atomic undo
            UndoOperation::Batch {
                operations: undo_operations,
            }
        };

        Ok((undo_op, updated_count))
    }
}

/// Phase 3: Unit tests for auto-indexing on UPDATE
#[cfg(test)]
mod auto_index_update_tests {
    use super::*;
    use crate::ast::TextIndexTypeSpecifier;
    use crate::storage::indexes::text::inverted_tantivy_clean::InvertedIndex;
    use crate::storage::indexes::text::metadata::{register_metadata, TextIndexMetadata};
    use crate::storage::indexes::text::registry::{get_text_index, register_text_index};
    use crate::storage::StorageManager;
    use tempfile::tempdir;

    fn create_test_context() -> ExecutionContext {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        use crate::storage::StorageMethod;
        use crate::storage::StorageType;
        let storage = StorageManager::new(
            temp_dir.path().to_str().unwrap(),
            StorageMethod::DiskOnly,
            StorageType::Sled,
        )
        .expect("Failed to create storage manager");
        ExecutionContext::new("test_session".to_string(), Arc::new(storage))
    }

    #[test]
    fn test_auto_index_update_string_field() {
        // Setup: Create and register an inverted index
        let index = InvertedIndex::new("test_idx_update_str").expect("Failed to create index");
        register_text_index("test_idx_update_str".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_update_str".to_string(),
            label: "Document".to_string(),
            field: "content".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        // Create graph with a node
        let mut graph = GraphCache::new();
        let mut node = Node::new("doc_1".to_string());
        node.add_label("Document".to_string());
        node.set_property(
            "content".to_string(),
            Value::String("initial content".to_string()),
        );
        graph.add_node(node).expect("Failed to add node");

        // Update the indexed field
        let execution_context = create_test_context();
        if let Some(node_mut) = graph.get_node_mut("doc_1") {
            node_mut.set_property(
                "content".to_string(),
                Value::String("updated content".to_string()),
            );
            SetExecutor::auto_index_node_update("doc_1", "content", &execution_context, &graph);
        }

        // Verify the document was re-indexed
        let index = get_text_index("test_idx_update_str")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert!(count > 0, "Updated document should be indexed");
    }

    #[test]
    fn test_auto_index_update_different_label() {
        let index = InvertedIndex::new("test_idx_diff_label").expect("Failed to create index");
        register_text_index("test_idx_diff_label".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_diff_label".to_string(),
            label: "Article".to_string(),
            field: "title".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut graph = GraphCache::new();
        let mut node = Node::new("node_1".to_string());
        node.add_label("DifferentLabel".to_string());
        node.set_property("title".to_string(), Value::String("old".to_string()));
        graph.add_node(node).expect("Failed to add node");

        let execution_context = create_test_context();
        if let Some(node_mut) = graph.get_node_mut("node_1") {
            node_mut.set_property("title".to_string(), Value::String("new".to_string()));
            SetExecutor::auto_index_node_update("node_1", "title", &execution_context, &graph);
        }

        // Should not be indexed (different label)
        let index = get_text_index("test_idx_diff_label")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 0, "Different label should not be indexed");
    }

    #[test]
    fn test_auto_index_update_non_indexed_property() {
        let index = InvertedIndex::new("test_idx_non_indexed").expect("Failed to create index");
        register_text_index("test_idx_non_indexed".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_non_indexed".to_string(),
            label: "Entity".to_string(),
            field: "indexed_field".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut graph = GraphCache::new();
        let mut node = Node::new("entity_1".to_string());
        node.add_label("Entity".to_string());
        node.set_property(
            "indexed_field".to_string(),
            Value::String("indexed".to_string()),
        );
        node.set_property(
            "non_indexed_field".to_string(),
            Value::String("not indexed".to_string()),
        );
        graph.add_node(node).expect("Failed to add node");

        let execution_context = create_test_context();
        // Update non-indexed field
        if let Some(node_mut) = graph.get_node_mut("entity_1") {
            node_mut.set_property(
                "non_indexed_field".to_string(),
                Value::String("updated".to_string()),
            );
            SetExecutor::auto_index_node_update(
                "entity_1",
                "non_indexed_field",
                &execution_context,
                &graph,
            );
        }

        // Should not trigger re-index (different field)
        let index = get_text_index("test_idx_non_indexed")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(
            count, 0,
            "Non-indexed field update should not trigger indexing"
        );
    }

    #[test]
    fn test_auto_index_update_number_to_string() {
        let index = InvertedIndex::new("test_idx_num_str").expect("Failed to create index");
        register_text_index("test_idx_num_str".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_num_str".to_string(),
            label: "Item".to_string(),
            field: "value".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut graph = GraphCache::new();
        let mut node = Node::new("item_1".to_string());
        node.add_label("Item".to_string());
        node.set_property("value".to_string(), Value::Number(42.5));
        graph.add_node(node).expect("Failed to add node");

        let execution_context = create_test_context();
        // Update to string
        if let Some(node_mut) = graph.get_node_mut("item_1") {
            node_mut.set_property(
                "value".to_string(),
                Value::String("new string value".to_string()),
            );
            SetExecutor::auto_index_node_update("item_1", "value", &execution_context, &graph);
        }

        let index = get_text_index("test_idx_num_str")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert!(count > 0, "Value type change should still be indexed");
    }

    #[test]
    fn test_auto_index_update_nonexistent_node() {
        let index = InvertedIndex::new("test_idx_no_node").expect("Failed to create index");
        register_text_index("test_idx_no_node".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_no_node".to_string(),
            label: "Thing".to_string(),
            field: "field".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let graph = GraphCache::new();
        let execution_context = create_test_context();

        // Should not crash when node doesn't exist
        SetExecutor::auto_index_node_update("nonexistent", "field", &execution_context, &graph);

        let index = get_text_index("test_idx_no_node")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 0, "Nonexistent node should not be indexed");
    }

    #[test]
    fn test_auto_index_update_boolean_conversion() {
        let index = InvertedIndex::new("test_idx_bool_update").expect("Failed to create index");
        register_text_index("test_idx_bool_update".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_bool_update".to_string(),
            label: "Flag".to_string(),
            field: "active".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut graph = GraphCache::new();
        let mut node = Node::new("flag_1".to_string());
        node.add_label("Flag".to_string());
        node.set_property("active".to_string(), Value::Boolean(false));
        graph.add_node(node).expect("Failed to add node");

        let execution_context = create_test_context();
        if let Some(node_mut) = graph.get_node_mut("flag_1") {
            node_mut.set_property("active".to_string(), Value::Boolean(true));
            SetExecutor::auto_index_node_update("flag_1", "active", &execution_context, &graph);
        }

        let index = get_text_index("test_idx_bool_update")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert!(count > 0, "Boolean value should be converted and indexed");
    }

    #[test]
    fn test_auto_index_update_missing_field_after_update() {
        let index = InvertedIndex::new("test_idx_missing_after").expect("Failed to create index");
        register_text_index("test_idx_missing_after".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_missing_after".to_string(),
            label: "Record".to_string(),
            field: "data".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut graph = GraphCache::new();
        let mut node = Node::new("record_1".to_string());
        node.add_label("Record".to_string());
        node.set_property("data".to_string(), Value::String("some data".to_string()));
        graph.add_node(node).expect("Failed to add node");

        let execution_context = create_test_context();
        // Update different field (data field will be missing after)
        if let Some(node_mut) = graph.get_node_mut("record_1") {
            node_mut.set_property(
                "other".to_string(),
                Value::String("other value".to_string()),
            );
            SetExecutor::auto_index_node_update("record_1", "other", &execution_context, &graph);
        }

        // Should not fail, index remains unchanged
        let index = get_text_index("test_idx_missing_after")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(
            count, 0,
            "Non-indexed field update should not trigger indexing"
        );
    }

    #[test]
    fn test_auto_index_update_multiple_sequential_updates() {
        let index = InvertedIndex::new("test_idx_sequential").expect("Failed to create index");
        register_text_index("test_idx_sequential".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_sequential".to_string(),
            label: "Document".to_string(),
            field: "text".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut graph = GraphCache::new();
        let mut node = Node::new("doc_1".to_string());
        node.add_label("Document".to_string());
        node.set_property("text".to_string(), Value::String("initial".to_string()));
        graph.add_node(node).expect("Failed to add node");

        let execution_context = create_test_context();

        // First update
        if let Some(node_mut) = graph.get_node_mut("doc_1") {
            node_mut.set_property(
                "text".to_string(),
                Value::String("first update".to_string()),
            );
            SetExecutor::auto_index_node_update("doc_1", "text", &execution_context, &graph);
        }

        // Second update
        if let Some(node_mut) = graph.get_node_mut("doc_1") {
            node_mut.set_property(
                "text".to_string(),
                Value::String("second update".to_string()),
            );
            SetExecutor::auto_index_node_update("doc_1", "text", &execution_context, &graph);
        }

        let index = get_text_index("test_idx_sequential")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert!(
            count > 0,
            "Multiple updates should result in indexed document"
        );
    }
}
