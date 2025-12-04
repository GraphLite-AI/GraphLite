// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Global registry for text indexes

use super::inverted_tantivy_clean::InvertedIndex;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

lazy_static! {
    /// Global registry for active text indexes
    /// Maps index name -> InvertedIndex instance
    static ref TEXT_INDEX_REGISTRY: RwLock<HashMap<String, Arc<InvertedIndex>>> =
        RwLock::new(HashMap::new());
}

/// Register a text index in the global registry
pub fn register_text_index(name: String, index: Arc<InvertedIndex>) -> Result<(), String> {
    let mut registry = TEXT_INDEX_REGISTRY
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

    if registry.contains_key(&name) {
        return Err(format!("Text index '{}' already registered", name));
    }

    registry.insert(name, index);
    Ok(())
}

/// Get a text index from the global registry
pub fn get_text_index(name: &str) -> Result<Option<Arc<InvertedIndex>>, String> {
    let registry = TEXT_INDEX_REGISTRY
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

    Ok(registry.get(name).cloned())
}

/// Remove a text index from the global registry
pub fn unregister_text_index(name: &str) -> Result<bool, String> {
    let mut registry = TEXT_INDEX_REGISTRY
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

    Ok(registry.remove(name).is_some())
}

/// List all registered text indexes
pub fn list_text_indexes() -> Result<Vec<String>, String> {
    let registry = TEXT_INDEX_REGISTRY
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

    Ok(registry.keys().cloned().collect())
}

/// Check if a text index is registered
pub fn text_index_exists(name: &str) -> Result<bool, String> {
    let registry = TEXT_INDEX_REGISTRY
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

    Ok(registry.contains_key(name))
}

/// Clear all text indexes (useful for testing or cleanup)
#[allow(dead_code)]
pub fn clear_all_text_indexes() -> Result<(), String> {
    let mut registry = TEXT_INDEX_REGISTRY
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

    registry.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_retrieve() {
        let _ = clear_all_text_indexes();

        let index = Arc::new(InvertedIndex::new("test_idx").unwrap());
        register_text_index("test_idx".to_string(), index.clone()).unwrap();

        let retrieved = get_text_index("test_idx").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "test_idx");
    }

    #[test]
    fn test_duplicate_registration() {
        let _ = clear_all_text_indexes();

        let index = Arc::new(InvertedIndex::new("test_idx").unwrap());
        register_text_index("test_idx".to_string(), index.clone()).unwrap();

        let result = register_text_index("test_idx".to_string(), index.clone());
        assert!(result.is_err());
    }

    #[test]
    fn test_unregister() {
        let _ = clear_all_text_indexes();

        let index = Arc::new(InvertedIndex::new("test_idx").unwrap());
        register_text_index("test_idx".to_string(), index).unwrap();

        let existed = unregister_text_index("test_idx").unwrap();
        assert!(existed);

        let retrieved = get_text_index("test_idx").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_list_indexes() {
        let _ = clear_all_text_indexes();

        let idx1 = Arc::new(InvertedIndex::new("idx1").unwrap());
        let idx2 = Arc::new(InvertedIndex::new("idx2").unwrap());

        register_text_index("idx1".to_string(), idx1).unwrap();
        register_text_index("idx2".to_string(), idx2).unwrap();

        let indexes = list_text_indexes().unwrap();
        assert_eq!(indexes.len(), 2);
        assert!(indexes.contains(&"idx1".to_string()));
        assert!(indexes.contains(&"idx2".to_string()));
    }
}
