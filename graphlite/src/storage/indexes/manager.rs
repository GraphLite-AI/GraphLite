// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Index manager for GraphLite
//!
//! Simplified index manager that supports only graph indexes.

use log::{debug, info, warn};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use super::IndexError;
use crate::storage::GraphCache;

/// Manager for all indexes in the system
pub struct IndexManager {
    /// Index names storage
    index_names: Arc<RwLock<HashSet<String>>>,
}

impl Default for IndexManager {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexManager {
    /// Create a new index manager
    pub fn new() -> Self {
        Self {
            index_names: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create a new index
    pub async fn create_index(
        &self,
        name: String,
        _index_type: super::IndexType,
        _config: super::IndexConfig,
    ) -> Result<(), IndexError> {
        info!("Creating index '{}'", name);

        let mut index_names = self
            .index_names
            .write()
            .map_err(|e| IndexError::creation(format!("Failed to acquire lock: {}", e)))?;

        if index_names.contains(&name) {
            return Err(IndexError::AlreadyExists(name));
        }

        // Store index name
        index_names.insert(name.clone());

        debug!("Index '{}' created successfully", name);
        Ok(())
    }

    /// Delete an index
    pub async fn delete_index(&self, name: &str) -> Result<(), IndexError> {
        info!("Deleting index '{}'", name);

        let mut index_names = self
            .index_names
            .write()
            .map_err(|e| IndexError::creation(format!("Failed to acquire lock: {}", e)))?;

        if !index_names.remove(name) {
            return Err(IndexError::NotFound(name.to_string()));
        }

        debug!("Index '{}' deleted successfully", name);
        Ok(())
    }

    /// Check if an index exists
    pub fn index_exists(&self, name: &str) -> bool {
        self.index_names
            .read()
            .map(|names| names.contains(name))
            .unwrap_or(false)
    }

    /// List all index names
    pub fn list_indexes(&self) -> Vec<String> {
        self.index_names
            .read()
            .map(|names| names.iter().cloned().collect())
            .unwrap_or_else(|_| Vec::new())
    }

    /// Reindex a text index (stub for compatibility)
    /// Reindex a text index (rebuild from existing graph data)
    pub fn reindex_text_index(
        &self,
        name: &str,
        graph: &Arc<GraphCache>,
    ) -> Result<usize, IndexError> {
        use crate::storage::indexes::text::registry::get_text_index;
        use crate::storage::indexes::text::metadata::get_metadata;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // Get the index from global registry
        let index = get_text_index(name)
            .map_err(|e| IndexError::NotFound(format!("Registry error: {}", e)))?
            .ok_or_else(|| IndexError::NotFound(format!("Text index '{}' not found", name)))?;
        
        // Get index metadata to know which label and field to index
        let metadata = get_metadata(name)
            .map_err(|e| IndexError::NotFound(format!("Metadata error: {}", e)))?
            .ok_or_else(|| IndexError::NotFound(format!("Metadata not found for index '{}'", name)))?;
        
        let label = &metadata.label;
        let field = &metadata.field;
        
        if field.is_empty() {
            return Err(IndexError::NotFound("Index field is empty".to_string()));
        }
        
        // Get all nodes with the target label from the graph
        let target_nodes = graph.get_nodes_by_label(label);
        let mut indexed_count = 0;
        
        // Batch add documents to the index
        for node in target_nodes {
            // Extract the text field value from node properties
            if let Some(value) = node.properties.get(field) {
                // Convert property value to string for indexing
                let text_value = match value {
                    crate::storage::Value::String(s) => s.clone(),
                    crate::storage::Value::Number(n) => n.to_string(),
                    crate::storage::Value::Boolean(b) => b.to_string(),
                    _ => continue, // Skip non-textual values (arrays, paths, nodes, etc.)
                };
                
                // Convert node ID (string) to u64 by hashing
                let mut hasher = DefaultHasher::new();
                node.id.hash(&mut hasher);
                let doc_id = hasher.finish();
                
                // Add document to index
                if let Ok(_) = index.add_document(doc_id, &text_value) {
                    indexed_count += 1;
                }
            }
        }
        
        // Commit the batch of documents
        if indexed_count > 0 {
            index.commit()
                .map_err(|e| IndexError::maintenance(format!("Commit failed: {}", e)))?;
            
            info!("Reindexed text index '{}': {} documents indexed", name, indexed_count);
        }
        
        Ok(indexed_count)
    }

    /// Search an index synchronously (stub for compatibility)
    pub fn search_index_sync(
        &self,
        _index_name: &str,
        _query_text: &str,
        _limit: usize,
    ) -> Result<Vec<(String, f32)>, IndexError> {
        warn!("Text search not supported in GraphLite");
        Ok(Vec::new())
    }

    /// Find indexes for a label (stub for compatibility)
    pub fn find_indexes_for_label(&self, _label: &str) -> Vec<String> {
        Vec::new()
    }

    /// Find index by label and property (stub for compatibility)
    pub fn find_index_by_label_and_property(
        &self,
        _label: &str,
        _property: &str,
    ) -> Option<String> {
        None
    }
}

/// Phase 3 Tests: Text Index Reindexing Functionality
#[cfg(test)]
mod reindex_tests {
    use super::*;
    use crate::ast::TextIndexTypeSpecifier;
    use crate::storage::graph_cache::GraphCache;
    use crate::storage::indexes::text::inverted_tantivy_clean::InvertedIndex;
    use crate::storage::indexes::text::metadata::{register_metadata, TextIndexMetadata};
    use crate::storage::indexes::text::registry::register_text_index;
    use crate::storage::types::Node;
    use crate::storage::Value;
    use std::sync::Arc;

    #[test]
    fn test_reindex_basic_functionality() {
        let mut graph = GraphCache::new();
        
        // Add test nodes
        let mut node1 = Node::new("n1".to_string());
        node1.add_label("Person".to_string());
        node1.set_property("name".to_string(), Value::String("Alice".to_string()));
        node1.set_property("bio".to_string(), Value::String("Software Engineer".to_string()));
        graph.add_node(node1).unwrap();

        let mut node2 = Node::new("n2".to_string());
        node2.add_label("Person".to_string());
        node2.set_property("name".to_string(), Value::String("Bob".to_string()));
        node2.set_property("bio".to_string(), Value::String("Product Manager".to_string()));
        graph.add_node(node2).unwrap();

        let graph_arc = Arc::new(graph);
        
        // Create and register text index directly
        let index = InvertedIndex::new("idx_bio_basic").unwrap();
        register_text_index("idx_bio_basic".to_string(), Arc::new(index)).unwrap();
        
        // Register metadata
        let metadata = TextIndexMetadata {
            name: "idx_bio_basic".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).unwrap();

        let index_manager = IndexManager::default();

        // Reindex
        let reindex_result = index_manager.reindex_text_index("idx_bio_basic", &graph_arc);
        assert!(reindex_result.is_ok());

        // Should have indexed 2 documents
        let count = reindex_result.unwrap();
        assert_eq!(count, 2, "Expected 2 documents indexed, got {}", count);
    }

    #[test]
    fn test_reindex_empty_graph() {
        let graph = Arc::new(GraphCache::new());
        
        // Create and register index
        let index = InvertedIndex::new("idx_empty").unwrap();
        register_text_index("idx_empty".to_string(), Arc::new(index)).unwrap();
        
        // Register metadata
        let metadata = TextIndexMetadata {
            name: "idx_empty".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).unwrap();

        let index_manager = IndexManager::default();

        // Reindex on empty graph
        let reindex_result = index_manager.reindex_text_index("idx_empty", &graph);
        assert!(reindex_result.is_ok());

        // Should have indexed 0 documents
        let count = reindex_result.unwrap();
        assert_eq!(count, 0, "Expected 0 documents in empty graph");
    }

    #[test]
    fn test_reindex_missing_field() {
        let mut graph = GraphCache::new();
        
        // Add node without the indexed field
        let mut node1 = Node::new("n1".to_string());
        node1.add_label("Person".to_string());
        node1.set_property("name".to_string(), Value::String("Alice".to_string()));
        // Note: no "bio" field
        graph.add_node(node1).unwrap();

        let graph_arc = Arc::new(graph);
        
        // Create and register index with unique name
        let index = InvertedIndex::new("idx_bio_missing_field").unwrap();
        register_text_index("idx_bio_missing_field".to_string(), Arc::new(index)).unwrap();
        
        // Register metadata
        let metadata = TextIndexMetadata {
            name: "idx_bio_missing_field".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).unwrap();

        let index_manager = IndexManager::default();

        // Reindex - should handle missing fields gracefully
        let reindex_result = index_manager.reindex_text_index("idx_bio_missing_field", &graph_arc);
        assert!(reindex_result.is_ok());

        // Should have indexed 0 documents (field not present)
        let count = reindex_result.unwrap();
        assert_eq!(count, 0, "Expected 0 documents when field is missing");
    }

    #[test]
    fn test_reindex_mixed_types() {
        let mut graph = GraphCache::new();
        
        // Add nodes with different value types
        let mut node1 = Node::new("n1".to_string());
        node1.add_label("Item".to_string());
        node1.set_property("desc".to_string(), Value::String("String desc".to_string()));
        graph.add_node(node1).unwrap();

        let mut node2 = Node::new("n2".to_string());
        node2.add_label("Item".to_string());
        node2.set_property("desc".to_string(), Value::Number(42.5));
        graph.add_node(node2).unwrap();

        let mut node3 = Node::new("n3".to_string());
        node3.add_label("Item".to_string());
        node3.set_property("desc".to_string(), Value::Boolean(true));
        graph.add_node(node3).unwrap();

        let graph_arc = Arc::new(graph);
        
        // Create and register index
        let index = InvertedIndex::new("idx_desc_mixed").unwrap();
        register_text_index("idx_desc_mixed".to_string(), Arc::new(index)).unwrap();
        
        // Register metadata
        let metadata = TextIndexMetadata {
            name: "idx_desc_mixed".to_string(),
            label: "Item".to_string(),
            field: "desc".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).unwrap();

        let index_manager = IndexManager::default();

        // Reindex
        let reindex_result = index_manager.reindex_text_index("idx_desc_mixed", &graph_arc);
        assert!(reindex_result.is_ok());

        // Should have indexed all 3 documents
        let count = reindex_result.unwrap();
        assert_eq!(count, 3, "Expected 3 documents with mixed types");
    }

    #[test]
    fn test_reindex_filters_by_label() {
        let mut graph = GraphCache::new();
        
        // Add nodes with different labels
        let mut person = Node::new("n1".to_string());
        person.add_label("Person".to_string());
        person.set_property("bio".to_string(), Value::String("Person bio".to_string()));
        graph.add_node(person).unwrap();

        let mut org = Node::new("n2".to_string());
        org.add_label("Organization".to_string());
        org.set_property("bio".to_string(), Value::String("Org bio".to_string()));
        graph.add_node(org).unwrap();

        let graph_arc = Arc::new(graph);
        
        // Create and register index for Person only
        let index = InvertedIndex::new("idx_person_bio_label").unwrap();
        register_text_index("idx_person_bio_label".to_string(), Arc::new(index)).unwrap();
        
        // Register metadata
        let metadata = TextIndexMetadata {
            name: "idx_person_bio_label".to_string(),
            label: "Person".to_string(),
            field: "bio".to_string(),
            index_type: TextIndexTypeSpecifier::FullText,
            doc_count: 0,
            size_bytes: 0,
        };
        register_metadata(metadata).unwrap();

        let index_manager = IndexManager::default();

        // Reindex
        let reindex_result = index_manager.reindex_text_index("idx_person_bio_label", &graph_arc);
        assert!(reindex_result.is_ok());

        // Should have indexed only 1 document (only Person nodes)
        let count = reindex_result.unwrap();
        assert_eq!(count, 1, "Expected only 1 Person node to be indexed");
    }

    #[test]
    fn test_reindex_nonexistent_index() {
        let graph = Arc::new(GraphCache::new());
        let index_manager = IndexManager::default();

        // Try to reindex non-existent index
        let reindex_result = index_manager.reindex_text_index("non_existent", &graph);
        
        // Should fail
        assert!(reindex_result.is_err(), "Should fail when index doesn't exist");
    }
}
