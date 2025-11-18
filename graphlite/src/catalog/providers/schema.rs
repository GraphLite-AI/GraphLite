// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Schema catalog provider implementation
//!
//! This module provides the schema catalog implementation that follows the
//! pluggable catalog architecture. It manages database schemas with
//! properties, configuration, and metadata.

use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::storage::StorageManager;
use crate::catalog::traits::{CatalogProvider, CatalogSchema as CatalogSchemaInfo};
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::catalog::error::{CatalogError, CatalogResult};

/// Schema identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaId {
    pub id: Uuid,
    pub name: String,
    pub path: String,
}

impl SchemaId {
    pub fn new(name: String, path: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            path,
        }
    }
}

/// Graph identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphId {
    pub id: Uuid,
    pub name: String,
    pub schema_name: String,
}

impl GraphId {
    #[allow(dead_code)] // ROADMAP v0.4.0 - GraphId builder for catalog operations
    pub fn new(name: String, schema_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            schema_name,
        }
    }
}

/// Schema definition containing metadata and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub id: SchemaId,
    
    /// Schema-level properties and configuration
    pub properties: HashMap<String, SchemaProperty>,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last modification timestamp
    pub modified_at: chrono::DateTime<chrono::Utc>,
    
    /// Schema description/documentation
    pub description: Option<String>,
    
    /// Schema version
    pub version: String,
    
    /// Whether this schema is the default schema
    pub is_default: bool,
}

impl Schema {
    pub fn new(id: SchemaId) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            properties: HashMap::new(),
            created_at: now,
            modified_at: now,
            description: None,
            version: "1.0.0".to_string(),
            is_default: false,
        }
    }
    
    /// Create from parameters
    pub fn from_params(name: String, params: &Value) -> Self {
        let path = params.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("/{}", name))
            .to_string();
        
        let id = SchemaId::new(name, path);
        let mut schema = Self::new(id);
        
        if let Some(desc) = params.get("description").and_then(|v| v.as_str()) {
            schema.description = Some(desc.to_string());
        }
        
        if let Some(version) = params.get("version").and_then(|v| v.as_str()) {
            schema.version = version.to_string();
        }
        
        if let Some(is_default) = params.get("is_default").and_then(|v| v.as_bool()) {
            schema.is_default = is_default;
        }
        
        if let Some(props) = params.get("properties").and_then(|v| v.as_object()) {
            for (key, value) in props {
                if let Ok(prop) = serde_json::from_value::<SchemaProperty>(value.clone()) {
                    schema.properties.insert(key.clone(), prop);
                }
            }
        }
        
        schema
    }
    
    /// Set a schema property
    pub fn set_property(&mut self, key: String, value: SchemaProperty) {
        self.properties.insert(key, value);
        self.modified_at = chrono::Utc::now();
    }
    
    /// Get a schema property
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema property accessor for configuration queries
    pub fn get_property(&self, key: &str) -> Option<&SchemaProperty> {
        self.properties.get(key)
    }
    
    /// Remove a schema property
    pub fn remove_property(&mut self, key: &str) -> Option<SchemaProperty> {
        self.modified_at = chrono::Utc::now();
        self.properties.remove(key)
    }
    
    /// Update description
    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description;
        self.modified_at = chrono::Utc::now();
    }
    
    /// Update version
    pub fn set_version(&mut self, version: String) {
        self.version = version;
        self.modified_at = chrono::Utc::now();
    }
}

/// Schema-level properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaProperty {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<SchemaProperty>),
    Map(HashMap<String, SchemaProperty>),
}

impl SchemaProperty {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Type conversion helpers for schema properties
    pub fn as_string(&self) -> Option<&String> {
        match self {
            SchemaProperty::String(s) => Some(s),
            _ => None,
        }
    }
    
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema property type conversion (see ROADMAP.md §4)
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            SchemaProperty::Integer(i) => Some(*i),
            _ => None,
        }
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema property type conversion (see ROADMAP.md §4)
    pub fn as_float(&self) -> Option<f64> {
        match self {
            SchemaProperty::Float(f) => Some(*f),
            _ => None,
        }
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema property type conversion (see ROADMAP.md §4)
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            SchemaProperty::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

/// Schema catalog state
#[derive(Clone, Serialize, Deserialize)]
struct SchemaCatalogState {
    schemas: HashMap<String, Schema>,
    default_schema: Option<String>,
}

/// Schema catalog provider
#[derive(Clone)]
pub struct SchemaCatalog {
    /// Map of schema name to schema
    schemas: HashMap<String, Schema>,
    
    /// Current default schema name
    default_schema: Option<String>,
    
    /// Storage manager reference
    storage: Option<Arc<StorageManager>>,
}

impl SchemaCatalog {
    /// Create a new schema catalog provider
    pub fn new() -> Box<dyn CatalogProvider> {
        Box::new(Self {
            schemas: HashMap::new(),
            default_schema: None,
            storage: None,
        })
    }
    
    /// Add a schema to the catalog
    fn add_schema(&mut self, mut schema: Schema) -> CatalogResult<()> {
        if self.schemas.contains_key(&schema.id.name) {
            return Err(CatalogError::DuplicateEntry(
                format!("Schema '{}' already exists", schema.id.name)
            ));
        }
        
        // If this is marked as default, update the default schema
        if schema.is_default {
            // Remove default flag from existing default schema
            if let Some(default_name) = &self.default_schema {
                if let Some(existing_default) = self.schemas.get_mut(default_name) {
                    existing_default.is_default = false;
                    existing_default.modified_at = chrono::Utc::now();
                }
            }
            self.default_schema = Some(schema.id.name.clone());
        }
        
        schema.modified_at = chrono::Utc::now();
        self.schemas.insert(schema.id.name.clone(), schema);
        Ok(())
    }
    
    /// Get a schema by name
    fn get_schema(&self, name: &str) -> Option<&Schema> {
        self.schemas.get(name)
    }
    
    /// Remove a schema
    fn remove_schema(&mut self, name: &str, cascade: bool) -> CatalogResult<Schema> {
        // Check if this is the default schema
        if self.default_schema.as_ref() == Some(&name.to_string()) && !cascade {
            return Err(CatalogError::InvalidOperation(
                format!("Cannot drop default schema '{}' without cascade", name)
            ));
        }
        
        // Check if schema has dependent objects (graphs) and cascade is not specified
        if !cascade {
            // TODO: Check for dependent graphs - this would require access to graph catalog
            // For now, we'll implement basic schema removal
            log::warn!("CASCADE drop dependencies not fully implemented - only removing schema");
        }
        
        let schema = self.schemas.remove(name)
            .ok_or_else(|| CatalogError::NotFound(format!("Schema '{}' not found", name)))?;
        
        // If this was the default schema, clear the default
        if self.default_schema.as_ref() == Some(&name.to_string()) {
            self.default_schema = None;
        }
        
        // CASCADE behavior: If cascade is true, dependent objects should be removed
        // This requires coordination with other catalogs (graphs, etc.)
        if cascade {
            log::info!("CASCADE drop of schema '{}' - dependent objects should be removed by coordinator", name);
            // The actual CASCADE behavior should be implemented at the catalog manager level
            // to coordinate removal across multiple catalog providers
        }
        
        Ok(schema)
    }
    
    /// Update schema properties
    fn update_schema(&mut self, name: &str, updates: &Value) -> CatalogResult<()> {
        let schema = self.schemas.get_mut(name)
            .ok_or_else(|| CatalogError::NotFound(format!("Schema '{}' not found", name)))?;
        
        if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
            schema.set_description(Some(desc.to_string()));
        }
        
        if let Some(version) = updates.get("version").and_then(|v| v.as_str()) {
            schema.set_version(version.to_string());
        }
        
        // Handle is_default flag separately to avoid double mutable borrow
        let is_default_update = updates.get("is_default").and_then(|v| v.as_bool());
        let schema_was_default = schema.is_default;
        let schema_name = name.to_string();
        let current_default = self.default_schema.clone();
        
        // Drop the schema reference before potentially taking another mutable reference
        let _ = schema;
        
        if let Some(is_default) = is_default_update {
            if is_default && !schema_was_default {
                // Remove default flag from current default schema first
                if let Some(default_name) = &current_default {
                    if default_name != &schema_name {
                        if let Some(existing_default) = self.schemas.get_mut(default_name) {
                            existing_default.is_default = false;
                            existing_default.modified_at = chrono::Utc::now();
                        }
                    }
                }
                self.default_schema = Some(schema_name.clone());
                
                // Now update the target schema
                if let Some(target_schema) = self.schemas.get_mut(&schema_name) {
                    target_schema.is_default = true;
                }
            } else if !is_default && schema_was_default {
                if let Some(target_schema) = self.schemas.get_mut(&schema_name) {
                    target_schema.is_default = false;
                }
                if current_default.as_ref() == Some(&schema_name) {
                    self.default_schema = None;
                }
            }
        }
        
        // Re-get the schema reference for the remaining updates
        let schema = self.schemas.get_mut(name)
            .ok_or_else(|| CatalogError::NotFound(format!("Schema '{}' not found", name)))?;
        
        if let Some(props) = updates.get("properties").and_then(|v| v.as_object()) {
            for (key, value) in props {
                if let Ok(prop) = serde_json::from_value::<SchemaProperty>(value.clone()) {
                    schema.set_property(key.clone(), prop);
                }
            }
        }
        
        if let Some(remove_props) = updates.get("remove_properties").and_then(|v| v.as_array()) {
            for prop_name in remove_props {
                if let Some(name_str) = prop_name.as_str() {
                    schema.remove_property(name_str);
                }
            }
        }
        
        schema.modified_at = chrono::Utc::now();
        Ok(())
    }
    
    /// Set default schema
    fn set_default_schema(&mut self, name: &str) -> CatalogResult<()> {
        if !self.schemas.contains_key(name) {
            return Err(CatalogError::NotFound(format!("Schema '{}' not found", name)));
        }
        
        // Remove default flag from current default
        if let Some(current_default) = &self.default_schema {
            if let Some(schema) = self.schemas.get_mut(current_default) {
                schema.is_default = false;
                schema.modified_at = chrono::Utc::now();
            }
        }
        
        // Set new default
        if let Some(schema) = self.schemas.get_mut(name) {
            schema.is_default = true;
            schema.modified_at = chrono::Utc::now();
        }
        
        self.default_schema = Some(name.to_string());
        Ok(())
    }
    
    /// Get default schema
    fn get_default_schema(&self) -> Option<&Schema> {
        self.default_schema.as_ref()
            .and_then(|name| self.schemas.get(name))
    }
    
    /// List schemas with optional filters
    fn list_schemas_filtered(&self, filters: Option<&Value>) -> Vec<&Schema> {
        if let Some(filters) = filters {
            self.schemas.values()
                .filter(|schema| {
                    // Filter by is_default
                    if let Some(is_default) = filters.get("is_default").and_then(|v| v.as_bool()) {
                        if schema.is_default != is_default {
                            return false;
                        }
                    }
                    
                    // Filter by version
                    if let Some(version) = filters.get("version").and_then(|v| v.as_str()) {
                        if schema.version != version {
                            return false;
                        }
                    }
                    
                    // Filter by property existence
                    if let Some(has_property) = filters.get("has_property").and_then(|v| v.as_str()) {
                        if !schema.properties.contains_key(has_property) {
                            return false;
                        }
                    }
                    
                    true
                })
                .collect()
        } else {
            self.schemas.values().collect()
        }
    }
}

impl CatalogProvider for SchemaCatalog {
    fn init(&mut self, storage: Arc<StorageManager>) -> CatalogResult<()> {
        self.storage = Some(storage.clone());

        // Try to load persisted state from storage (same pattern as SecurityCatalog)
        match storage.load_catalog_provider("schema") {
            Ok(Some(data)) => {
                // Load persisted catalog state
                match self.load(&data) {
                    Ok(_) => {
                        log::info!("Schema catalog loaded from storage with {} schemas",
                            self.schemas.len());
                    }
                    Err(e) => {
                        log::warn!("Failed to deserialize schema catalog from storage: {}. Using defaults.", e);
                    }
                }
            }
            Ok(None) => {
                // This is expected on first run - no catalog data exists yet
                log::debug!("No persisted schema catalog found. Using default initialization.");
            }
            Err(e) => {
                log::warn!("Error loading schema catalog: {}. Using default initialization.", e);
            }
        }

        Ok(())
    }

    fn execute(&mut self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        match op {
            CatalogOperation::Create { entity_type, name, params } => {
                match entity_type {
                    EntityType::Schema => {
                        // Check if schema already exists and handle IF NOT EXISTS
                        let if_not_exists = params.get("if_not_exists")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        
                        if self.schemas.contains_key(&name) {
                            if if_not_exists {
                                // Schema exists but IF NOT EXISTS was specified, so succeed silently
                                Ok(CatalogResponse::Success { 
                                    data: Some(json!({ "message": format!("Schema '{}' already exists", name) }))
                                })
                            } else {
                                // Schema exists and no IF NOT EXISTS, return error
                                Err(CatalogError::DuplicateEntry(
                                    format!("Schema '{}' already exists", name)
                                ))
                            }
                        } else {
                            // Schema doesn't exist, create it
                            let schema = Schema::from_params(name.clone(), &params);
                            self.add_schema(schema)?;
                            Ok(CatalogResponse::Success { 
                                data: Some(json!({ "message": format!("Schema '{}' created", name) }))
                            })
                        }
                    }
                    _ => Ok(CatalogResponse::NotSupported)
                }
            }
            
            CatalogOperation::Drop { entity_type, name, cascade } => {
                match entity_type {
                    EntityType::Schema => {
                        let removed = self.remove_schema(&name, cascade)?;
                        Ok(CatalogResponse::Success { 
                            data: Some(serde_json::to_value(removed)?)
                        })
                    }
                    _ => Ok(CatalogResponse::NotSupported)
                }
            }
            
            CatalogOperation::Query { query_type, params } => {
                match query_type {
                    QueryType::Get => {
                        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                            if let Some(schema) = self.get_schema(name) {
                                Ok(CatalogResponse::Query { 
                                    results: serde_json::to_value(schema)?
                                })
                            } else {
                                Err(CatalogError::NotFound(format!("Schema '{}' not found", name)))
                            }
                        } else {
                            Err(CatalogError::InvalidParameters("Missing 'name' parameter".to_string()))
                        }
                    }
                    QueryType::GetDefault => {
                        if let Some(schema) = self.get_default_schema() {
                            Ok(CatalogResponse::Query { 
                                results: serde_json::to_value(schema)?
                            })
                        } else {
                            Ok(CatalogResponse::Query { 
                                results: json!(null)
                            })
                        }
                    }
                    QueryType::Exists => {
                        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                            let exists = self.schemas.contains_key(name);
                            Ok(CatalogResponse::Query { 
                                results: json!({ "exists": exists })
                            })
                        } else {
                            Err(CatalogError::InvalidParameters("Missing 'name' parameter".to_string()))
                        }
                    }
                    _ => Ok(CatalogResponse::NotSupported)
                }
            }
            
            CatalogOperation::Update { entity_type, name, updates } => {
                match entity_type {
                    EntityType::Schema => {
                        self.update_schema(&name, &updates)?;
                        Ok(CatalogResponse::Success { 
                            data: Some(json!({ "message": format!("Schema '{}' updated", name) }))
                        })
                    }
                    EntityType::DefaultSchema => {
                        self.set_default_schema(&name)?;
                        Ok(CatalogResponse::Success { 
                            data: Some(json!({ "message": format!("Default schema set to '{}'", name) }))
                        })
                    }
                    _ => Ok(CatalogResponse::NotSupported)
                }
            }
            
            CatalogOperation::List { entity_type, filters } => {
                match entity_type {
                    EntityType::Schema => {
                        let schemas = self.list_schemas_filtered(filters.as_ref());
                        Ok(CatalogResponse::List { 
                            items: schemas.iter()
                                .map(|s| serde_json::to_value(s))
                                .collect::<Result<Vec<_>, _>>()?
                        })
                    }
                    _ => Ok(CatalogResponse::NotSupported)
                }
            }
            
            _ => Ok(CatalogResponse::NotSupported)
        }
    }
    
    fn save(&self) -> CatalogResult<Vec<u8>> {
        let state = SchemaCatalogState {
            schemas: self.schemas.clone(),
            default_schema: self.default_schema.clone(),
        };
        
        let data = bincode::serialize(&state)
            .map_err(|e| CatalogError::SerializationError(e.to_string()))?;
        Ok(data)
    }
    
    fn load(&mut self, data: &[u8]) -> CatalogResult<()> {
        let state: SchemaCatalogState = bincode::deserialize(data)
            .map_err(|e| CatalogError::DeserializationError(e.to_string()))?;

        // If the loaded state has schemas, use them
        // But ensure we always have at least a default schema
        if !state.schemas.is_empty() {
            self.schemas = state.schemas;
            self.default_schema = state.default_schema;
        } else {
            // Loaded state is empty - keep the defaults created in new()
            log::debug!("Loaded empty schema catalog, keeping defaults");
        }
        Ok(())
    }
    
    fn schema(&self) -> CatalogSchemaInfo {
        CatalogSchemaInfo {
            name: "schema".to_string(),
            version: "1.0.0".to_string(),
            entities: vec![EntityType::Schema.to_string()],
            operations: self.supported_operations(),
        }
    }
    
    fn supported_operations(&self) -> Vec<String> {
        vec![
            "create_schema".to_string(),
            "drop_schema".to_string(),
            "get_schema".to_string(),
            "get_default_schema".to_string(),
            "list_schemas".to_string(),
            "update_schema".to_string(),
            "set_default_schema".to_string(),
            "schema_exists".to_string(),
        ]
    }

    fn execute_read_only(&self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        // Handle read-only operations by delegating to the main execute method
        // Since SchemaCatalog operations don't mutate during queries, this is safe
        match op {
            CatalogOperation::Query { query_type, params } => {
                match query_type {
                    QueryType::List => {
                        let schemas: Vec<_> = self.schemas.values().collect();
                        let results = serde_json::to_value(schemas)?;
                        Ok(CatalogResponse::Query { results })
                    }
                    QueryType::Get => {
                        let schema_name = params.get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| CatalogError::InvalidParameters("Schema name required".to_string()))?;

                        if let Some(schema) = self.schemas.get(schema_name) {
                            let results = serde_json::to_value(schema)?;
                            Ok(CatalogResponse::Query { results })
                        } else {
                            Err(CatalogError::EntityNotFound(format!("Schema '{}' not found", schema_name)))
                        }
                    }
                    QueryType::GetDefault => {
                        if let Some(default_name) = &self.default_schema {
                            if let Some(schema) = self.schemas.get(default_name) {
                                let results = serde_json::to_value(schema)?;
                                Ok(CatalogResponse::Query { results })
                            } else {
                                Err(CatalogError::EntityNotFound("Default schema not found".to_string()))
                            }
                        } else {
                            Err(CatalogError::EntityNotFound("No default schema set".to_string()))
                        }
                    }
                    QueryType::Exists => {
                        let schema_name = params.get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| CatalogError::InvalidParameters("Schema name required".to_string()))?;

                        let exists = self.schemas.contains_key(schema_name);
                        Ok(CatalogResponse::Query {
                            results: serde_json::to_value(exists)?
                        })
                    }
                    // Handle all other QueryType variants with NotSupported
                    _ => Ok(CatalogResponse::NotSupported)
                }
            }
            _ => Err(CatalogError::NotSupported(
                "Only query operations are supported in read-only mode".to_string()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_schema_catalog_creation() {
        let catalog = SchemaCatalog::new();
        assert_eq!(catalog.schema().name, "schema");
    }
    
    #[test]
    fn test_create_and_get_schema() {
        let mut catalog = SchemaCatalog {
            schemas: HashMap::new(),
            default_schema: None,
            storage: None,
        };
        
        let params = json!({
            "description": "Test schema",
            "version": "1.0.0",
            "is_default": true,
            "properties": {
                "retention_days": {
                    "Integer": 30
                }
            }
        });
        
        let result = catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Schema,
            name: "test_schema".to_string(),
            params,
        });
        
        assert!(result.is_ok());
        
        let query_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::Get,
            params: json!({ "name": "test_schema" }),
        });
        
        assert!(query_result.is_ok());
    }
    
    #[test]
    fn test_default_schema_management() {
        let mut catalog = SchemaCatalog {
            schemas: HashMap::new(),
            default_schema: None,
            storage: None,
        };
        
        // Create first schema as default
        catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Schema,
            name: "schema1".to_string(),
            params: json!({ "is_default": true }),
        }).unwrap();
        
        // Create second schema
        catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Schema,
            name: "schema2".to_string(),
            params: json!({ "is_default": false }),
        }).unwrap();
        
        // Verify first schema is default
        let default_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::GetDefault,
            params: json!({}),
        });
        
        assert!(default_result.is_ok());
        if let Ok(CatalogResponse::Query { results }) = default_result {
            let schema: Schema = serde_json::from_value(results).unwrap();
            assert_eq!(schema.id.name, "schema1");
        }
        
        // Change default to schema2
        catalog.execute(CatalogOperation::Update {
            entity_type: EntityType::DefaultSchema,
            name: "schema2".to_string(),
            updates: json!({}),
        }).unwrap();
        
        // Verify schema2 is now default
        let default_result2 = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::GetDefault,
            params: json!({}),
        });
        
        assert!(default_result2.is_ok());
        if let Ok(CatalogResponse::Query { results }) = default_result2 {
            let schema: Schema = serde_json::from_value(results).unwrap();
            assert_eq!(schema.id.name, "schema2");
        }
    }
    
    #[test]
    fn test_schema_properties() {
        let mut catalog = SchemaCatalog {
            schemas: HashMap::new(),
            default_schema: None,
            storage: None,
        };
        
        // Create schema with properties
        catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Schema,
            name: "prop_schema".to_string(),
            params: json!({
                "properties": {
                    "max_connections": { "Integer": 100 },
                    "auto_backup": { "Boolean": true },
                    "backup_location": { "String": "/backups" }
                }
            }),
        }).unwrap();
        
        // Update properties
        catalog.execute(CatalogOperation::Update {
            entity_type: EntityType::Schema,
            name: "prop_schema".to_string(),
            updates: json!({
                "properties": {
                    "max_connections": { "Integer": 200 },
                    "compression": { "String": "gzip" }
                },
                "remove_properties": ["auto_backup"]
            }),
        }).unwrap();
        
        // Verify properties
        let query_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::Get,
            params: json!({ "name": "prop_schema" }),
        });
        
        assert!(query_result.is_ok());
        if let Ok(CatalogResponse::Query { results }) = query_result {
            let schema: Schema = serde_json::from_value(results).unwrap();
            assert!(schema.properties.contains_key("max_connections"));
            assert!(schema.properties.contains_key("compression"));
            assert!(!schema.properties.contains_key("auto_backup"));
        }
    }
}
