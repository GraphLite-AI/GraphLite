// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use parking_lot::RwLock;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::ast::{InsertStatement, PatternElement};
use crate::catalog::manager::CatalogManager;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::schema::integration::runtime_validator::RuntimeValidator;
use crate::storage::{Edge, GraphCache, Node, Value};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for INSERT statements
pub struct InsertExecutor {
    statement: InsertStatement,
    runtime_validator: Option<Arc<RuntimeValidator>>,
}

impl InsertExecutor {
    /// Create a new InsertExecutor
    #[allow(dead_code)] // ROADMAP v0.5.0 - Direct INSERT executor construction without validation
    pub fn new(statement: InsertStatement) -> Self {
        Self {
            statement,
            runtime_validator: None,
        }
    }

    /// Create a new InsertExecutor with schema validation
    #[allow(dead_code)] // ROADMAP v0.5.0 - Schema-validated INSERT for type safety
    pub fn with_validation(
        statement: InsertStatement,
        catalog_manager: Arc<RwLock<CatalogManager>>,
    ) -> Self {
        Self {
            statement,
            runtime_validator: Some(Arc::new(RuntimeValidator::new(catalog_manager))),
        }
    }

    /// Convert AST literal to storage value
    fn literal_to_value(literal: &crate::ast::Literal) -> Value {
        match literal {
            crate::ast::Literal::String(s) => Value::String(s.clone()),
            crate::ast::Literal::Integer(i) => Value::Number(*i as f64),
            crate::ast::Literal::Float(f) => Value::Number(*f),
            crate::ast::Literal::Boolean(b) => Value::Boolean(*b),
            crate::ast::Literal::Null => Value::Null,
            crate::ast::Literal::DateTime(dt) => Value::String(dt.clone()),
            crate::ast::Literal::Duration(dur) => Value::String(dur.clone()),
            crate::ast::Literal::TimeWindow(tw) => Value::String(tw.clone()),
            crate::ast::Literal::Vector(vec) => {
                Value::Vector(vec.iter().map(|&f| f as f32).collect())
            }
            crate::ast::Literal::List(list) => {
                let converted: Vec<Value> = list.iter().map(Self::literal_to_value).collect();
                Value::List(converted)
            }
        }
    }

    /// Extract properties from a property map
    fn extract_properties(prop_map: &crate::ast::PropertyMap) -> HashMap<String, Value> {
        let mut properties = HashMap::new();

        for property in &prop_map.properties {
            if let crate::ast::Expression::Literal(literal) = &property.value {
                properties.insert(property.key.clone(), Self::literal_to_value(literal));
            } else {
                log::warn!(
                    "Complex property expressions not supported in INSERT, skipping property: {}",
                    property.key
                );
            }
        }

        properties
    }

    /// Generate a content-based hash ID for a node based on labels and properties
    fn generate_node_content_id(labels: &[String], properties: &HashMap<String, Value>) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash labels (sorted for consistency)
        let mut sorted_labels = labels.to_vec();
        sorted_labels.sort();
        for label in &sorted_labels {
            label.hash(&mut hasher);
        }

        // Hash properties (sorted by key for consistency)
        let mut sorted_properties: Vec<_> = properties.iter().collect();
        sorted_properties.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_properties {
            key.hash(&mut hasher);
            // Hash the value in a consistent way
            match value {
                Value::String(s) => s.hash(&mut hasher),
                Value::Number(n) => n.to_bits().hash(&mut hasher),
                Value::Boolean(b) => b.hash(&mut hasher),
                Value::Null => "null".hash(&mut hasher),
                Value::Vector(v) => {
                    for f in v {
                        f.to_bits().hash(&mut hasher);
                    }
                }
                Value::List(list) => {
                    // Simplified hash for lists - could be enhanced
                    list.len().hash(&mut hasher);
                    for item in list {
                        // Recursively hash list items (simplified)
                        match item {
                            Value::String(s) => s.hash(&mut hasher),
                            Value::Number(n) => n.to_bits().hash(&mut hasher),
                            Value::Boolean(b) => b.hash(&mut hasher),
                            _ => "complex".hash(&mut hasher),
                        }
                    }
                }
                // Handle additional Value types
                Value::DateTime(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithFixedOffset(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithNamedTz(tz, dt) => {
                    tz.hash(&mut hasher);
                    dt.timestamp().hash(&mut hasher);
                }
                Value::TimeWindow(tw) => format!("{:?}", tw).hash(&mut hasher),
                Value::Array(arr) => {
                    arr.len().hash(&mut hasher);
                    for item in arr {
                        // Simplified recursive hashing for arrays
                        format!("{:?}", item).hash(&mut hasher);
                    }
                }
                // Catch-all for any other Value types
                _ => format!("{:?}", value).hash(&mut hasher),
            }
        }

        let hash = hasher.finish();
        format!("node_{:x}", hash)
    }

    /// Generate a content-based hash ID for an edge
    fn generate_edge_content_id(
        from_node_id: &str,
        to_node_id: &str,
        label: &str,
        properties: &HashMap<String, Value>,
    ) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash the connection (from_node -> to_node -> label)
        from_node_id.hash(&mut hasher);
        to_node_id.hash(&mut hasher);
        label.hash(&mut hasher);

        // Hash properties (sorted by key for consistency)
        let mut sorted_properties: Vec<_> = properties.iter().collect();
        sorted_properties.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_properties {
            key.hash(&mut hasher);
            // Hash the value (same logic as node properties)
            match value {
                Value::String(s) => s.hash(&mut hasher),
                Value::Number(n) => n.to_bits().hash(&mut hasher),
                Value::Boolean(b) => b.hash(&mut hasher),
                Value::Null => "null".hash(&mut hasher),
                Value::Vector(v) => {
                    for f in v {
                        f.to_bits().hash(&mut hasher);
                    }
                }
                Value::List(list) => {
                    list.len().hash(&mut hasher);
                    for item in list {
                        match item {
                            Value::String(s) => s.hash(&mut hasher),
                            Value::Number(n) => n.to_bits().hash(&mut hasher),
                            Value::Boolean(b) => b.hash(&mut hasher),
                            _ => "complex".hash(&mut hasher),
                        }
                    }
                }
                // Handle additional Value types
                Value::DateTime(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithFixedOffset(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithNamedTz(tz, dt) => {
                    tz.hash(&mut hasher);
                    dt.timestamp().hash(&mut hasher);
                }
                Value::TimeWindow(tw) => format!("{:?}", tw).hash(&mut hasher),
                Value::Array(arr) => {
                    arr.len().hash(&mut hasher);
                    for item in arr {
                        // Simplified recursive hashing for arrays
                        format!("{:?}", item).hash(&mut hasher);
                    }
                }
                // Catch-all for any other Value types
                _ => format!("{:?}", value).hash(&mut hasher),
            }
        }

        let hash = hasher.finish();
        format!("edge_{:x}", hash)
    }
}

impl StatementExecutor for InsertExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Insert
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("INSERT into graph '{}'", graph_name)
    }
}

/// Helper functions for InsertExecutor
impl InsertExecutor {
    /// Phase 3: Auto-index inserted nodes on text indexes
    /// For each label on the inserted node, find matching text indexes and add the node
    fn auto_index_node_insert(node: &Node, context: &ExecutionContext, labels: &[String]) {
        use crate::storage::indexes::text::metadata::get_metadata_for_label;
        use crate::storage::indexes::text::registry::get_text_index;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Verify context is available (just for safety)
        let _storage_manager = match &context.storage_manager {
            Some(sm) => sm,
            None => {
                log::debug!("Auto-indexing skipped: no storage manager in context");
                return;
            }
        };

        // For each label on the inserted node
        for label in labels {
            // Find all text indexes defined on this label
            let metadata_vec = match get_metadata_for_label(label) {
                Ok(vec) => vec,
                Err(e) => {
                    log::debug!("No indexes found for label '{}': {}", label, e);
                    continue;
                }
            };

            // For each matching index, add the node
            for metadata in metadata_vec {
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

                // Extract the indexed field value from the node
                let field_name = &metadata.field;
                let field_value = match node.properties.get(field_name) {
                    Some(value) => value,
                    None => {
                        log::debug!(
                            "Node '{}' doesn't have field '{}', skipping auto-index",
                            node.id,
                            field_name
                        );
                        continue;
                    }
                };

                // Convert value to string for indexing
                let text_value = match field_value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    _ => {
                        log::debug!(
                            "Field '{}' has non-textual type, skipping auto-index",
                            field_name
                        );
                        continue;
                    }
                };

                // Convert node ID (string) to u64 by hashing
                let mut hasher = DefaultHasher::new();
                node.id.hash(&mut hasher);
                let doc_id = hasher.finish();

                // Add the document to the index
                if let Err(e) = index.add_document(doc_id, &text_value) {
                    log::warn!(
                        "Failed to auto-index node '{}' in index '{}': {}",
                        node.id,
                        metadata.name,
                        e
                    );
                } else {
                    log::debug!(
                        "Auto-indexed node '{}' in index '{}' (field: {})",
                        node.id,
                        metadata.name,
                        field_name
                    );

                    // Commit after each document (Phase 3.3 will optimize this)
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

impl DataStatementExecutor for InsertExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        log::debug!("DEBUG INSERT: execute_modification called");

        let graph_name = match context.get_graph_name() {
            Ok(name) => {
                log::debug!("DEBUG INSERT: Successfully got graph_name: '{}'", name);
                name
            }
            Err(e) => {
                log::debug!("DEBUG INSERT: Failed to get graph_name: {}", e);
                return Err(e);
            }
        };
        let patterns = &self.statement.graph_patterns;

        let mut undo_operations = Vec::new();
        let mut inserted_nodes = 0;
        let mut inserted_edges = 0;
        let mut created_node_ids: Vec<String> = Vec::new();
        // Map from user identifiers to actual storage node IDs
        let mut identifier_to_node_id: HashMap<String, String> = HashMap::new();

        // Process each graph pattern to extract nodes and edges to insert
        log::debug!("INSERT processing {} graph patterns", patterns.len());

        // First pass: process all nodes
        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            log::debug!(
                "INSERT processing pattern {} for nodes (pass 1)",
                pattern_idx
            );
            for element in pattern.elements.iter() {
                if let PatternElement::Node(node_pattern) = element {
                    // Check if this is a reference to an existing node
                    if let Some(ref user_identifier) = node_pattern.identifier {
                        // If we already have a mapping for this identifier, it's a reference
                        if identifier_to_node_id.contains_key(user_identifier) {
                            log::debug!(
                                "Skipping node creation for identifier '{}' - already exists",
                                user_identifier
                            );
                            continue;
                        }
                    }

                    // Extract properties first
                    let properties = if let Some(ref prop_map) = node_pattern.properties {
                        Self::extract_properties(prop_map)
                    } else {
                        HashMap::new()
                    };

                    // Validate against schema if validator is available
                    if let Some(ref validator) = self.runtime_validator {
                        // Use the first label as the primary type (GQL allows multiple labels)
                        if let Some(primary_label) = node_pattern.labels.first() {
                            // Convert storage::Value to serde_json::Value for validation
                            let json_properties: HashMap<String, serde_json::Value> = properties
                                .iter()
                                .map(|(k, v)| {
                                    let json_val = match v {
                                        Value::String(s) => serde_json::Value::String(s.clone()),
                                        Value::Number(n) => serde_json::json!(n),
                                        Value::Boolean(b) => serde_json::Value::Bool(*b),
                                        Value::Null => serde_json::Value::Null,
                                        Value::Vector(vec) => serde_json::json!(vec),
                                        Value::List(list) => {
                                            // Convert list recursively (simplified for now)
                                            serde_json::json!(list)
                                        }
                                        _ => serde_json::Value::Null, // Handle other types as needed
                                    };
                                    (k.clone(), json_val)
                                })
                                .collect();

                            // Run validation synchronously
                            if let Err(e) = validator.validate_insert(
                                &graph_name,
                                primary_label,
                                &json_properties,
                            ) {
                                log::error!("Schema validation failed for node: {}", e);
                                return Err(e);
                            }
                        }
                    }

                    // Generate content-based storage ID from labels and properties
                    let storage_node_id =
                        Self::generate_node_content_id(&node_pattern.labels, &properties);

                    // If there's a user identifier, map it to the storage ID
                    if let Some(ref user_identifier) = node_pattern.identifier {
                        log::debug!(
                            "INSERT mapping user identifier '{}' to storage ID '{}'",
                            user_identifier,
                            storage_node_id
                        );
                        identifier_to_node_id
                            .insert(user_identifier.clone(), storage_node_id.clone());
                    }

                    log::debug!(
                        "INSERT creating node with content-based storage ID: {}",
                        storage_node_id
                    );

                    // Create the node
                    let node = Node {
                        id: storage_node_id.clone(),
                        labels: node_pattern.labels.clone(),
                        properties,
                    };

                    // Try to add to graph - this will detect duplicates automatically
                    match graph.add_node(node.clone()) {
                        Ok(_) => {
                            log::info!("Successfully added node '{}' to graph", storage_node_id);
                            inserted_nodes += 1;

                            // Phase 3: Auto-indexing on INSERT
                            // Add inserted node to any text indexes defined on its labels
                            Self::auto_index_node_insert(&node, context, &node_pattern.labels);

                            // Add undo operation only for newly inserted nodes
                            let undo_op = UndoOperation::InsertNode {
                                graph_path: graph_name.clone(),
                                node_id: storage_node_id.clone(),
                            };
                            log::debug!("DEBUG INSERT: Created undo_op with graph_path: '{}', node_id: '{}'", graph_name, storage_node_id);
                            undo_operations.push(undo_op);
                        }
                        Err(crate::storage::types::GraphError::NodeAlreadyExists(_)) => {
                            log::info!(
                                "Node with content '{}' already exists, skipping duplicate",
                                storage_node_id
                            );
                            // Add warning about duplicate insertion
                            let warning_msg = format!("Duplicate node detected: Node with identical properties already exists (node_id: {})", storage_node_id);
                            context.add_warning(warning_msg);
                            // Don't count this as an insertion or error - it's a duplicate
                        }
                        Err(e) => {
                            log::error!("Failed to insert node '{}': {}", storage_node_id, e);
                            return Err(ExecutionError::RuntimeError(format!(
                                "Failed to insert node '{}': {}",
                                storage_node_id, e
                            )));
                        }
                    }

                    log::debug!("Processed node with storage ID: {}", storage_node_id);
                    created_node_ids.push(storage_node_id.clone());
                }
            }
        }

        // Second pass: process all edges
        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            log::debug!(
                "INSERT processing pattern {} for edges (pass 2)",
                pattern_idx
            );
            for (i, element) in pattern.elements.iter().enumerate() {
                match element {
                    PatternElement::Node(_node_pattern) => {
                        // In pass 2, we skip node processing - all nodes were already created in pass 1
                        // The identifier mappings are already established from pass 1
                    }
                    PatternElement::Edge(edge_pattern) => {
                        // For edges, we need to connect the previous and next nodes
                        if i == 0 || i >= pattern.elements.len() - 1 {
                            return Err(ExecutionError::RuntimeError(
                                "Edge patterns in INSERT must be between two nodes".to_string(),
                            ));
                        }

                        // Get the source node from the previous element
                        let source_node_id = match pattern.elements.get(i - 1) {
                            Some(PatternElement::Node(source_node)) => {
                                if let Some(ref identifier) = source_node.identifier {
                                    // Use the mapping to get the actual storage ID
                                    match identifier_to_node_id.get(identifier) {
                                        Some(storage_id) => {
                                            log::debug!(
                                                "Edge source: found mapping '{}' -> '{}'",
                                                identifier,
                                                storage_id
                                            );
                                            storage_id.clone()
                                        }
                                        None => {
                                            log::error!("Edge source: identifier '{}' not found in mapping. Available mappings: {:?}", identifier, identifier_to_node_id);
                                            return Err(ExecutionError::RuntimeError(format!("Source node identifier '{}' not found in current statement", identifier)));
                                        }
                                    }
                                } else {
                                    // Anonymous nodes in edge patterns need special handling
                                    // Check if this is an empty node reference that shouldn't create a new node
                                    if source_node.labels.is_empty()
                                        && source_node.properties.is_none()
                                    {
                                        // This is likely a reference like (n) with no content - error out
                                        return Err(ExecutionError::RuntimeError(
                                        "Cannot create edge from anonymous empty node - use an identifier instead".to_string()
                                    ));
                                    }
                                    // Generate storage ID from node content for truly anonymous nodes with content
                                    let properties =
                                        if let Some(ref prop_map) = source_node.properties {
                                            Self::extract_properties(prop_map)
                                        } else {
                                            HashMap::new()
                                        };
                                    let storage_id = Self::generate_node_content_id(
                                        &source_node.labels,
                                        &properties,
                                    );
                                    log::debug!("Edge source: anonymous node with labels={:?}, properties={:?} generated ID '{}'", source_node.labels, properties, storage_id);
                                    storage_id
                                }
                            }
                            _ => {
                                return Err(ExecutionError::RuntimeError(
                                    "Edge pattern must be preceded by a source node".to_string(),
                                ))
                            }
                        };

                        // Get the target node from the next element
                        let target_node_id = match pattern.elements.get(i + 1) {
                            Some(PatternElement::Node(target_node)) => {
                                if let Some(ref identifier) = target_node.identifier {
                                    // Use the mapping to get the actual storage ID
                                    match identifier_to_node_id.get(identifier) {
                                        Some(storage_id) => {
                                            log::debug!(
                                                "Edge target: found mapping '{}' -> '{}'",
                                                identifier,
                                                storage_id
                                            );
                                            storage_id.clone()
                                        }
                                        None => {
                                            log::error!("Edge target: identifier '{}' not found in mapping. Available mappings: {:?}", identifier, identifier_to_node_id);
                                            return Err(ExecutionError::RuntimeError(format!("Target node identifier '{}' not found in current statement", identifier)));
                                        }
                                    }
                                } else {
                                    // Anonymous nodes in edge patterns need special handling
                                    // Check if this is an empty node reference that shouldn't create a new node
                                    if target_node.labels.is_empty()
                                        && target_node.properties.is_none()
                                    {
                                        // This is likely a reference like (m) with no content - error out
                                        return Err(ExecutionError::RuntimeError(
                                        "Cannot create edge to anonymous empty node - use an identifier instead".to_string()
                                    ));
                                    }
                                    // Generate storage ID from node content for truly anonymous nodes with content
                                    let properties =
                                        if let Some(ref prop_map) = target_node.properties {
                                            Self::extract_properties(prop_map)
                                        } else {
                                            HashMap::new()
                                        };
                                    let storage_id = Self::generate_node_content_id(
                                        &target_node.labels,
                                        &properties,
                                    );
                                    log::debug!("Edge target: anonymous node with labels={:?}, properties={:?} generated ID '{}'", target_node.labels, properties, storage_id);
                                    storage_id
                                }
                            }
                            _ => {
                                return Err(ExecutionError::RuntimeError(
                                    "Edge pattern must be followed by a target node".to_string(),
                                ))
                            }
                        };

                        // Extract edge properties if present
                        let edge_properties = if let Some(ref prop_map) = edge_pattern.properties {
                            Self::extract_properties(prop_map)
                        } else {
                            HashMap::new()
                        };

                        let edge_label = edge_pattern
                            .labels
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "CONNECTED".to_string());

                        // Generate content-based storage ID for the edge
                        let edge_storage_id = Self::generate_edge_content_id(
                            &source_node_id,
                            &target_node_id,
                            &edge_label,
                            &edge_properties,
                        );

                        log::debug!(
                            "Creating edge with content-based storage ID: {}",
                            edge_storage_id
                        );

                        // Create the edge
                        let edge = Edge {
                            id: edge_storage_id.clone(),
                            from_node: source_node_id,
                            to_node: target_node_id,
                            label: edge_label,
                            properties: edge_properties,
                        };

                        // Try to add to graph - this will detect duplicate edges automatically
                        match graph.add_edge(edge) {
                            Ok(_) => {
                                log::info!(
                                    "Successfully added edge '{}' to graph",
                                    edge_storage_id
                                );
                                inserted_edges += 1;

                                // Add undo operation only for newly inserted edges
                                undo_operations.push(UndoOperation::InsertEdge {
                                    graph_path: graph_name.clone(),
                                    edge_id: edge_storage_id,
                                });
                            }
                            Err(crate::storage::types::GraphError::EdgeAlreadyExists(_)) => {
                                log::info!(
                                    "Edge with content '{}' already exists, skipping duplicate",
                                    edge_storage_id
                                );
                                // Add warning about duplicate insertion
                                let warning_msg = format!("Duplicate edge detected: Edge with identical properties already exists (edge_id: {})", edge_storage_id);
                                context.add_warning(warning_msg);
                                // Don't count this as an insertion or error - it's a duplicate
                            }
                            Err(e) => {
                                return Err(ExecutionError::RuntimeError(format!(
                                    "Failed to insert edge: {}",
                                    e
                                )));
                            }
                        }
                    }
                }
            }
        }

        let total_inserted = inserted_nodes + inserted_edges;

        // Return the first undo operation (unified system handles multiple operations internally)
        let undo_op =
            undo_operations
                .into_iter()
                .next()
                .unwrap_or_else(|| UndoOperation::InsertNode {
                    graph_path: graph_name.clone(),
                    node_id: "no_operations".to_string(),
                });

        Ok((undo_op, total_inserted))
    }
}

/// Phase 3: Unit tests for auto-indexing on INSERT
#[cfg(test)]
mod auto_index_insert_tests {
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
    fn test_auto_index_node_insert_basic() {
        // Setup: Create and register an inverted index
        let index = InvertedIndex::new("test_idx_basic").expect("Failed to create index");
        register_text_index("test_idx_basic".to_string(), Arc::new(index))
            .expect("Failed to register index");

        // Setup: Register metadata
        let metadata = TextIndexMetadata {
            name: "test_idx_basic".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        // Create a test node
        let mut node = Node::new("person_1".to_string());
        node.add_label("Person".to_string());
        node.set_property(
            "bio".to_string(),
            Value::String("Software Engineer".to_string()),
        );

        // Call auto_index_node_insert
        let execution_context = create_test_context();
        InsertExecutor::auto_index_node_insert(&node, &execution_context, &["Person".to_string()]);

        // Verify the index contains the document
        let retrieved_index = get_text_index("test_idx_basic")
            .expect("Failed to get index")
            .expect("Index should exist");
        let doc_count = retrieved_index
            .doc_count()
            .expect("Failed to get doc count");
        assert!(
            doc_count > 0,
            "Index should have at least 1 document after auto-index"
        );
    }

    #[test]
    fn test_auto_index_multiple_labels() {
        // Setup: Create indexes for two labels
        let index1 = InvertedIndex::new("test_idx_person").expect("Failed to create index");
        register_text_index("test_idx_person".to_string(), Arc::new(index1))
            .expect("Failed to register");

        let metadata1 = TextIndexMetadata {
            name: "test_idx_person".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata1).expect("Failed to register metadata");

        // Create a node with Person label
        let mut node = Node::new("p1".to_string());
        node.add_label("Person".to_string());
        node.set_property("bio".to_string(), Value::String("Developer".to_string()));

        // Auto-index
        let execution_context = create_test_context();
        InsertExecutor::auto_index_node_insert(&node, &execution_context, &["Person".to_string()]);

        // Verify
        let index = get_text_index("test_idx_person")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert!(count > 0, "Document should be indexed");
    }

    #[test]
    fn test_auto_index_string_value() {
        let index = InvertedIndex::new("test_idx_string").expect("Failed to create index");
        register_text_index("test_idx_string".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_string".to_string(),
            label: "Document".to_string(),
            field: "title".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut node = Node::new("doc_1".to_string());
        node.add_label("Document".to_string());
        node.set_property(
            "title".to_string(),
            Value::String("Introduction to GraphLite".to_string()),
        );

        let execution_context = create_test_context();
        InsertExecutor::auto_index_node_insert(
            &node,
            &execution_context,
            &["Document".to_string()],
        );

        let index = get_text_index("test_idx_string")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 1, "String value should be indexed");
    }

    #[test]
    fn test_auto_index_number_value_conversion() {
        let index = InvertedIndex::new("test_idx_number").expect("Failed to create index");
        register_text_index("test_idx_number".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_number".to_string(),
            label: "Metric".to_string(),
            field: "value".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut node = Node::new("metric_1".to_string());
        node.add_label("Metric".to_string());
        node.set_property("value".to_string(), Value::Number(42.5));

        let execution_context = create_test_context();
        InsertExecutor::auto_index_node_insert(&node, &execution_context, &["Metric".to_string()]);

        let index = get_text_index("test_idx_number")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 1, "Number value should be converted and indexed");
    }

    #[test]
    fn test_auto_index_boolean_value_conversion() {
        let index = InvertedIndex::new("test_idx_bool").expect("Failed to create index");
        register_text_index("test_idx_bool".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_bool".to_string(),
            label: "Flag".to_string(),
            field: "active".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let mut node = Node::new("flag_1".to_string());
        node.add_label("Flag".to_string());
        node.set_property("active".to_string(), Value::Boolean(true));

        let execution_context = create_test_context();
        InsertExecutor::auto_index_node_insert(&node, &execution_context, &["Flag".to_string()]);

        let index = get_text_index("test_idx_bool")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 1, "Boolean value should be converted and indexed");
    }

    #[test]
    fn test_auto_index_missing_field() {
        let index = InvertedIndex::new("test_idx_missing").expect("Failed to create index");
        register_text_index("test_idx_missing".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_missing".to_string(),
            label: "Item".to_string(),
            field: "description".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        // Node without the indexed field
        let mut node = Node::new("item_1".to_string());
        node.add_label("Item".to_string());
        node.set_property("name".to_string(), Value::String("Widget".to_string()));
        // Note: no "description" field

        let execution_context = create_test_context();
        InsertExecutor::auto_index_node_insert(&node, &execution_context, &["Item".to_string()]);

        // Should not crash, index remains empty
        let index = get_text_index("test_idx_missing")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 0, "Index should be empty when field is missing");
    }

    #[test]
    fn test_auto_index_no_matching_metadata() {
        let index = InvertedIndex::new("test_idx_nomatch").expect("Failed to create index");
        register_text_index("test_idx_nomatch".to_string(), Arc::new(index))
            .expect("Failed to register");

        // Register metadata for different label
        let metadata = TextIndexMetadata {
            name: "test_idx_nomatch".to_string(),
            label: "OtherLabel".to_string(),
            field: "field".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        // Create node with different label
        let mut node = Node::new("node_1".to_string());
        node.add_label("SomeLabel".to_string());
        node.set_property("field".to_string(), Value::String("value".to_string()));

        let execution_context = create_test_context();
        // Should not crash, just not index anything
        InsertExecutor::auto_index_node_insert(
            &node,
            &execution_context,
            &["SomeLabel".to_string()],
        );

        let index = get_text_index("test_idx_nomatch")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 0, "Index should be empty when label doesn't match");
    }

    #[test]
    fn test_auto_index_empty_labels() {
        let index = InvertedIndex::new("test_idx_empty").expect("Failed to create index");
        register_text_index("test_idx_empty".to_string(), Arc::new(index))
            .expect("Failed to register");

        let mut node = Node::new("node_1".to_string());
        node.set_property("field".to_string(), Value::String("value".to_string()));

        let execution_context = create_test_context();
        // Should not crash with empty labels
        InsertExecutor::auto_index_node_insert(&node, &execution_context, &[]);

        let index = get_text_index("test_idx_empty")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 0, "Index should be empty when no labels provided");
    }

    #[test]
    fn test_auto_index_multiple_values_different_nodes() {
        let index = InvertedIndex::new("test_idx_multi").expect("Failed to create index");
        register_text_index("test_idx_multi".to_string(), Arc::new(index))
            .expect("Failed to register");

        let metadata = TextIndexMetadata {
            name: "test_idx_multi".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).expect("Failed to register metadata");

        let execution_context = create_test_context();

        // Insert first node
        let mut node1 = Node::new("person_1".to_string());
        node1.add_label("Person".to_string());
        node1.set_property("bio".to_string(), Value::String("Engineer".to_string()));
        InsertExecutor::auto_index_node_insert(&node1, &execution_context, &["Person".to_string()]);

        // Insert second node
        let mut node2 = Node::new("person_2".to_string());
        node2.add_label("Person".to_string());
        node2.set_property("bio".to_string(), Value::String("Designer".to_string()));
        InsertExecutor::auto_index_node_insert(&node2, &execution_context, &["Person".to_string()]);

        let index = get_text_index("test_idx_multi")
            .expect("Failed to get index")
            .expect("Index should exist");
        let count = index.doc_count().expect("Failed to get doc count");
        assert_eq!(count, 2, "Both nodes should be indexed separately");
    }
}
