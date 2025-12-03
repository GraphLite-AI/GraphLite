// Text index metadata for tracking index configuration
// Stores which label, field, and index type each text index uses

use serde::{Deserialize, Serialize};
use crate::ast::TextIndexTypeSpecifier;

/// Metadata for a text index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextIndexMetadata {
    /// Index name
    pub name: String,
    /// Label (node type) being indexed
    pub label: String,
    /// Property field being indexed
    pub field: String,
    /// Index type (FULLTEXT, FUZZY, BOOLEAN)
    pub index_type: TextIndexTypeSpecifier,
    /// Number of documents indexed
    pub doc_count: u64,
    /// Total bytes indexed (approximate)
    pub size_bytes: u64,
}

impl TextIndexMetadata {
    /// Create new text index metadata
    pub fn new(name: String, label: String, field: String, index_type: TextIndexTypeSpecifier) -> Self {
        Self {
            name,
            label,
            field,
            index_type,
            doc_count: 0,
            size_bytes: 0,
        }
    }
    
    /// Update document count and size
    pub fn update_stats(&mut self, doc_count: u64, size_bytes: u64) {
        self.doc_count = doc_count;
        self.size_bytes = size_bytes;
    }
}

/// Global registry for text index metadata
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;

lazy_static! {
    /// Maps index name -> metadata
    static ref TEXT_INDEX_METADATA: RwLock<HashMap<String, TextIndexMetadata>> =
        RwLock::new(HashMap::new());
}

/// Register text index metadata
pub fn register_metadata(metadata: TextIndexMetadata) -> Result<(), String> {
    let mut registry = TEXT_INDEX_METADATA
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
    
    if registry.contains_key(&metadata.name) {
        return Err(format!("Metadata for index '{}' already registered", metadata.name));
    }
    
    registry.insert(metadata.name.clone(), metadata);
    Ok(())
}

/// Get text index metadata
pub fn get_metadata(name: &str) -> Result<Option<TextIndexMetadata>, String> {
    let registry = TEXT_INDEX_METADATA
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
    
    Ok(registry.get(name).cloned())
}

/// Update text index metadata
pub fn update_metadata(name: &str, doc_count: u64, size_bytes: u64) -> Result<(), String> {
    let mut registry = TEXT_INDEX_METADATA
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
    
    if let Some(metadata) = registry.get_mut(name) {
        metadata.update_stats(doc_count, size_bytes);
        Ok(())
    } else {
        Err(format!("Metadata for index '{}' not found", name))
    }
}

/// Get all metadata for a label
pub fn get_metadata_for_label(label: &str) -> Result<Vec<TextIndexMetadata>, String> {
    let registry = TEXT_INDEX_METADATA
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
    
    Ok(registry
        .values()
        .filter(|m| m.label == label)
        .cloned()
        .collect())
}

/// Unregister text index metadata
pub fn unregister_metadata(name: &str) -> Result<bool, String> {
    let mut registry = TEXT_INDEX_METADATA
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
    
    Ok(registry.remove(name).is_some())
}

/// Clear all metadata (for testing)
#[allow(dead_code)]
pub fn clear_all_metadata() -> Result<(), String> {
    let mut registry = TEXT_INDEX_METADATA
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
    
    registry.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_retrieve_metadata() {
        let _ = clear_all_metadata();
        
        let metadata = TextIndexMetadata::new(
            "test_idx".to_string(),
            "User".to_string(),
            "bio".to_string(),
            TextIndexTypeSpecifier::FullText,
        );
        
        register_metadata(metadata.clone()).unwrap();
        
        let retrieved = get_metadata("test_idx").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test_idx");
    }

    #[test]
    fn test_update_metadata_stats() {
        let _ = clear_all_metadata();
        
        let metadata = TextIndexMetadata::new(
            "test_idx".to_string(),
            "User".to_string(),
            "bio".to_string(),
            TextIndexTypeSpecifier::FullText,
        );
        
        register_metadata(metadata).unwrap();
        update_metadata("test_idx", 100, 50000).unwrap();
        
        let retrieved = get_metadata("test_idx").unwrap().unwrap();
        assert_eq!(retrieved.doc_count, 100);
        assert_eq!(retrieved.size_bytes, 50000);
    }

    #[test]
    fn test_get_metadata_for_label() {
        let _ = clear_all_metadata();
        
        let meta1 = TextIndexMetadata::new(
            "bio_idx".to_string(),
            "User".to_string(),
            "bio".to_string(),
            TextIndexTypeSpecifier::FullText,
        );
        let meta2 = TextIndexMetadata::new(
            "name_idx".to_string(),
            "User".to_string(),
            "name".to_string(),
            TextIndexTypeSpecifier::Fuzzy,
        );
        
        register_metadata(meta1).unwrap();
        register_metadata(meta2).unwrap();
        
        let user_indexes = get_metadata_for_label("User").unwrap();
        assert_eq!(user_indexes.len(), 2);
    }
}
