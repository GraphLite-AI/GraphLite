// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! System procedures for catalog information access
//!
//! Implements vendor-specific system procedures for accessing catalog metadata via ISO GQL CALL syntax:
//! - CALL gql.list_schemas() YIELD schema_name
//! - CALL gql.list_graphs() YIELD graph_name, schema_name
//! - CALL gql.list_graph_types() YIELD graph_type_name, schema_name
//! - CALL gql.list_functions() YIELD name
//! - CALL gql.list_roles() YIELD role_name
//! - CALL gql.list_users() YIELD user_name
//! - CALL gql.show_session() YIELD session_id, user_name, schema_name, graph_name
//! - CALL gql.cache_stats() YIELD cache_type, entries, hit_rate, memory_bytes
//! - CALL gql.clear_cache([cache_type]) YIELD status, cleared_caches

use std::collections::HashMap;
use std::sync::Arc;
use crate::storage::Value;
use crate::exec::result::{QueryResult, Row};
use crate::exec::error::ExecutionError;
use super::manager::CatalogManager;
use super::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use serde_json::json;

/// System catalog procedures registry (vendor-specific system procedures)
pub struct SystemProcedures {
    catalog_manager: Arc<std::sync::RwLock<CatalogManager>>,
    storage: Arc<crate::storage::StorageManager>,
    cache_manager: Option<Arc<crate::cache::CacheManager>>,
}

impl SystemProcedures {
    pub fn new(
        catalog_manager: Arc<std::sync::RwLock<CatalogManager>>,
        storage: Arc<crate::storage::StorageManager>,
        cache_manager: Option<Arc<crate::cache::CacheManager>>,
    ) -> Self {
        Self {
            catalog_manager,
            storage,
            cache_manager,
        }
    }

    /// Execute a system procedure by name
    ///
    /// Only gql.* namespace procedures are supported.
    /// The gql.* namespace is reserved for vendor-provided system procedures.
    /// User-defined procedures (future feature) will use schema-based namespaces.
    pub fn execute_procedure(
        &self,
        procedure_name: &str,
        args: Vec<Value>,
        session_id: Option<&str>,
    ) -> Result<QueryResult, ExecutionError> {
        // Validate that only gql.* namespace is used
        if !procedure_name.starts_with("gql.") {
            return Err(ExecutionError::UnsupportedOperator(format!(
                "Invalid procedure namespace: '{}'. System procedures must use 'gql.' prefix. Example: CALL gql.list_graphs()",
                procedure_name
            )));
        }

        let normalized_name = procedure_name.to_string();

        match normalized_name.as_str() {
            "gql.list_schemas" => self.list_schemas(args),
            "gql.list_graphs" => self.list_graphs(args),
            "gql.list_graph_types" => self.list_graph_types(args),
            "gql.list_functions" => self.list_functions(args),
            "gql.list_roles" => self.list_roles(args),
            "gql.list_users" => self.list_users(args),
            "gql.authenticate_user" => self.authenticate_user(args),
            "gql.show_session" => {
                match session_id {
                    Some(id) => self.show_session(args, id),
                    None => Err(ExecutionError::RuntimeError("show_session requires an active session".to_string()))
                }
            },
            "gql.cache_stats" => self.cache_stats(args),
            "gql.clear_cache" => self.clear_cache(args),
            _ => Err(ExecutionError::UnsupportedOperator(format!(
                "System procedure not found or not supported: {}. Available system procedures: list_schemas, list_graphs, list_graph_types, list_functions, list_roles, list_users, authenticate_user, show_session, cache_stats, clear_cache",
                procedure_name
            ))),
        }
    }

    /// Check if a procedure name is a valid system procedure
    #[allow(dead_code)] // ROADMAP v0.4.0 - System procedure validation (see ROADMAP.md §4)
    pub fn is_valid_procedure(&self, procedure_name: &str) -> bool {
        let normalized_name = if procedure_name.starts_with("system.") {
            procedure_name.replace("system.", "gql.")
        } else {
            procedure_name.to_string()
        };

        matches!(
            normalized_name.as_str(),
            "gql.list_schemas"
            | "gql.list_graphs"
            | "gql.list_graph_types"
            | "gql.list_functions"
            | "gql.list_roles"
            | "gql.list_users"
            | "gql.authenticate_user"
            | "gql.show_session"
            | "gql.cache_stats"
            | "gql.clear_cache"
        )
    }

    /// CALL gql.list_schemas() YIELD schema_name, schema_path, created_at, modified_at, description
    fn list_schemas(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let mut catalog_manager = self.catalog_manager.write()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire catalog manager lock".to_string()))?;

        let response = catalog_manager.execute("schema", CatalogOperation::List {
            entity_type: EntityType::Schema,
            filters: None,
        }).map_err(|e| ExecutionError::CatalogError(format!("Failed to list schemas: {}", e)))?;

        let mut rows = Vec::new();
        let columns = vec![
            "schema_name".to_string(),
            "schema_path".to_string(),
            "created_at".to_string(),
            "modified_at".to_string(),
            "description".to_string(),
        ];

        if let CatalogResponse::List { items } = response {
            for item in items {
                if let Some(schema) = item.as_object() {
                    let mut row_values = HashMap::new();
                    row_values.insert("schema_name".to_string(),
                        schema.get("id")
                            .and_then(|id| id.get("name"))
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null));
                    row_values.insert("schema_path".to_string(),
                        schema.get("id")
                            .and_then(|id| id.get("path"))
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null));
                    row_values.insert("created_at".to_string(),
                        schema.get("created_at")
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null));
                    row_values.insert("modified_at".to_string(),
                        schema.get("modified_at")
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null));
                    row_values.insert("description".to_string(),
                        schema.get("description")
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null));

                    rows.push(Row::from_values(row_values));
                }
            }
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.list_graphs() YIELD graph_name, schema_name
    fn list_graphs(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let mut catalog_manager = self.catalog_manager.write()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire catalog manager lock".to_string()))?;

        let response = catalog_manager.execute("graph_metadata", CatalogOperation::List {
            entity_type: EntityType::Graph,
            filters: None,
        }).map_err(|e| ExecutionError::CatalogError(format!("Failed to list graphs: {}", e)))?;

        let mut rows = Vec::new();
        let columns = vec!["graph_name".to_string(), "schema_name".to_string()];

        if let CatalogResponse::List { items } = response {
            for item in items {
                if let Some(graph) = item.as_object() {
                    let mut row_values = HashMap::new();
                    row_values.insert(
                        "graph_name".to_string(),
                        graph.get("id")
                            .and_then(|id| id.get("name"))
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null),
                    );
                    row_values.insert(
                        "schema_name".to_string(),
                        graph.get("id")
                            .and_then(|id| id.get("schema_name"))
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null),
                    );
                    rows.push(Row::from_values(row_values));
                }
            }
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.list_graph_types() YIELD graph_type_name, schema_name
    fn list_graph_types(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let mut catalog_manager = self.catalog_manager.write()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire catalog manager lock".to_string()))?;

        let response = catalog_manager.execute("graph_type", CatalogOperation::List {
            entity_type: EntityType::GraphType,
            filters: None,
        }).map_err(|e| ExecutionError::CatalogError(format!("Failed to list graph types: {}", e)))?;

        let mut rows = Vec::new();
        let columns = vec!["graph_type_name".to_string(), "schema_name".to_string()];

        if let CatalogResponse::List { items } = response {
            for item in items {
                if let Some(graph_type) = item.as_object() {
                    let mut row_values = HashMap::new();
                    row_values.insert(
                        "graph_type_name".to_string(),
                        graph_type.get("id")
                            .and_then(|id| id.get("name"))
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null),
                    );
                    row_values.insert(
                        "schema_name".to_string(),
                        graph_type.get("id")
                            .and_then(|id| id.get("schema_name"))
                            .and_then(|v| v.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .unwrap_or(Value::Null),
                    );
                    rows.push(Row::from_values(row_values));
                }
            }
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.list_functions() YIELD name
    fn list_functions(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        use crate::functions::FunctionRegistry;

        let registry = FunctionRegistry::new();
        let function_names = registry.function_names();

        let mut rows = Vec::new();
        let columns = vec!["name".to_string()];

        for name in function_names {
            let mut row_values = HashMap::new();
            row_values.insert("name".to_string(), Value::String(name));
            rows.push(Row::from_values(row_values));
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.show_session() YIELD property_name, property_value, property_type
    fn show_session(&self, _args: Vec<Value>, session_id: &str) -> Result<QueryResult, ExecutionError> {
        use crate::session::manager::get_session;

        let mut rows = Vec::new();
        let columns = vec![
            "property_name".to_string(),
            "property_value".to_string(),
            "property_type".to_string(),
        ];

        if let Some(session_arc) = get_session(session_id) {
            let session_state = session_arc.read()
                .map_err(|_| ExecutionError::RuntimeError("Failed to acquire session lock".to_string()))?;

            // Show current schema
            if let Some(ref schema) = session_state.current_schema {
                let mut row_values = HashMap::new();
                row_values.insert("property_name".to_string(), Value::String("current_schema".to_string()));
                row_values.insert("property_value".to_string(), Value::String(schema.clone()));
                row_values.insert("property_type".to_string(), Value::String("schema_reference".to_string()));
                rows.push(Row::from_values(row_values));
            }

            // Show current graph
            if let Some(ref graph_expr) = session_state.current_graph {
                let mut row_values = HashMap::new();
                row_values.insert("property_name".to_string(), Value::String("current_graph".to_string()));
                row_values.insert("property_value".to_string(), Value::String(graph_expr.clone()));
                row_values.insert("property_type".to_string(), Value::String("graph_expression".to_string()));
                rows.push(Row::from_values(row_values));
            }

            // Show current time zone
            if let Some(ref time_zone) = session_state.current_timezone {
                let mut row_values = HashMap::new();
                row_values.insert("property_name".to_string(), Value::String("current_time_zone".to_string()));
                row_values.insert("property_value".to_string(), Value::String(time_zone.clone()));
                row_values.insert("property_type".to_string(), Value::String("time_zone".to_string()));
                rows.push(Row::from_values(row_values));
            }

            // Show session parameters
            for (param_name, param_value) in &session_state.parameters {
                let mut row_values = HashMap::new();
                row_values.insert("property_name".to_string(), Value::String(format!("parameter.{}", param_name)));
                row_values.insert("property_value".to_string(), param_value.clone());
                row_values.insert("property_type".to_string(), Value::String("session_parameter".to_string()));
                rows.push(Row::from_values(row_values));
            }

            // Show graph parameters from general parameters if they exist
            for (param_name, param_value) in &session_state.parameters {
                if param_name.starts_with("graph_") {
                    let mut row_values = HashMap::new();
                    row_values.insert("property_name".to_string(), Value::String(format!("graph_parameter.{}", param_name)));
                    row_values.insert("property_value".to_string(), Value::String(format!("{:?}", param_value)));
                    row_values.insert("property_type".to_string(), Value::String("graph_parameter".to_string()));
                    rows.push(Row::from_values(row_values));
                }
            }

            // Show binding table parameters from general parameters if they exist
            for (param_name, param_value) in &session_state.parameters {
                if param_name.starts_with("binding_") {
                    let mut row_values = HashMap::new();
                    row_values.insert("property_name".to_string(), Value::String(format!("binding_table.{}", param_name)));
                    row_values.insert("property_value".to_string(), Value::String(format!("{:?}", param_value)));
                    row_values.insert("property_type".to_string(), Value::String("binding_table_parameter".to_string()));
                    rows.push(Row::from_values(row_values));
                }
            }
        } else {
            // Session not found
            let mut row_values = HashMap::new();
            row_values.insert("property_name".to_string(), Value::String("error".to_string()));
            row_values.insert("property_value".to_string(), Value::String("Session not found".to_string()));
            row_values.insert("property_type".to_string(), Value::String("error".to_string()));
            rows.push(Row::from_values(row_values));
        }

        // If no session state or no parameters, show a message
        if rows.is_empty() {
            let mut row_values = HashMap::new();
            row_values.insert("property_name".to_string(), Value::String("status".to_string()));
            row_values.insert("property_value".to_string(), Value::String("No active session parameters".to_string()));
            row_values.insert("property_type".to_string(), Value::String("info".to_string()));
            rows.push(Row::from_values(row_values));
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.list_roles() YIELD role_name, description, created_at
    /// Lists all roles in the security catalog
    fn list_roles(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let mut catalog_manager = self.catalog_manager.write()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire catalog manager lock".to_string()))?;

        let response = catalog_manager.execute("security", CatalogOperation::Query {
            query_type: QueryType::ListRoles,
            params: json!({}),
        }).map_err(|e| ExecutionError::CatalogError(format!("Failed to list roles: {}", e)))?;

        let mut rows = Vec::new();
        let columns = vec![
            "role_name".to_string(),
            "description".to_string(),
            "created_at".to_string(),
        ];

        if let CatalogResponse::Query { results } = response {
            if let Some(roles) = results.as_array() {
                for role_value in roles {
                    if let Some(role) = role_value.as_object() {
                        let mut row_values = HashMap::new();
                        row_values.insert("role_name".to_string(),
                            role.get("id")
                                .and_then(|id| id.as_object())
                                .and_then(|id_obj| id_obj.get("name"))
                                .and_then(|v| v.as_str())
                                .map(|s| Value::String(s.to_string()))
                                .unwrap_or(Value::Null));
                        row_values.insert("description".to_string(),
                            role.get("description")
                                .and_then(|v| {
                                    if v.is_null() {
                                        None
                                    } else {
                                        v.as_str()
                                    }
                                })
                                .map(|s| Value::String(s.to_string()))
                                .unwrap_or(Value::String("".to_string())));
                        row_values.insert("created_at".to_string(),
                            role.get("created_at")
                                .and_then(|v| v.as_str())
                                .map(|s| Value::String(s.to_string()))
                                .unwrap_or(Value::Null));

                        rows.push(Row::from_values(row_values));
                    }
                }
            }
        } else if let CatalogResponse::Error { message } = response {
            return Err(ExecutionError::RuntimeError(
                format!("Failed to list roles: {}", message)
            ));
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.list_users() YIELD username, email, active, created_at, roles
    /// Lists all users in the security catalog
    fn list_users(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let mut catalog_manager = self.catalog_manager.write()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire catalog manager lock".to_string()))?;

        let response = catalog_manager.execute("security", CatalogOperation::Query {
            query_type: QueryType::ListUsers,
            params: json!({}),
        }).map_err(|e| ExecutionError::CatalogError(format!("Failed to list users: {}", e)))?;

        let mut rows = Vec::new();
        let columns = vec![
            "username".to_string(),
            "email".to_string(),
            "active".to_string(),
            "created_at".to_string(),
            "roles".to_string(),
        ];

        if let CatalogResponse::Query { results } = response {
            if let Some(users) = results.as_array() {
                for user_value in users {
                    if let Some(user) = user_value.as_object() {
                        let mut row_values = HashMap::new();
                        row_values.insert("username".to_string(),
                            user.get("id")
                                .and_then(|id| id.as_object())
                                .and_then(|id_obj| id_obj.get("name"))
                                .and_then(|v| v.as_str())
                                .map(|s| Value::String(s.to_string()))
                                .unwrap_or(Value::Null));
                        row_values.insert("email".to_string(),
                            user.get("properties")
                                .and_then(|props| props.as_object())
                                .and_then(|props_obj| props_obj.get("email"))
                                .and_then(|v| v.as_str())
                                .map(|s| Value::String(s.to_string()))
                                .unwrap_or(Value::Null));
                        row_values.insert("active".to_string(),
                            user.get("enabled")
                                .and_then(|v| v.as_bool())
                                .map(|b| Value::Boolean(b))
                                .unwrap_or(Value::Boolean(true)));
                        row_values.insert("created_at".to_string(),
                            user.get("created_at")
                                .and_then(|v| v.as_str())
                                .map(|s| Value::String(s.to_string()))
                                .unwrap_or(Value::Null));
                        row_values.insert("roles".to_string(),
                            user.get("roles")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    let role_names: Vec<String> = arr.iter()
                                        .filter_map(|r| r.as_str())
                                        .map(|s| s.to_string())
                                        .collect();
                                    Value::String(role_names.join(", "))
                                })
                                .unwrap_or(Value::String("".to_string())));

                        rows.push(Row::from_values(row_values));
                    }
                }
            }
        } else if let CatalogResponse::Error { message } = response {
            return Err(ExecutionError::RuntimeError(
                format!("Failed to list users: {}", message)
            ));
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.cache_stats() YIELD cache_type, entries, hit_rate, memory_bytes
    fn cache_stats(&self, _args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let mut rows = Vec::new();
        let columns = vec![
            "cache_type".to_string(),
            "entries".to_string(),
            "hit_rate".to_string(),
            "memory_bytes".to_string(),
        ];

        // Check if cache manager is available
        if let Some(cache_manager) = &self.cache_manager {
            let stats = cache_manager.get_stats();

            // Storage cache (from StorageManager)
            let storage_stats = self.storage.get_cache_stats();
            let mut row_values = HashMap::new();
            row_values.insert("cache_type".to_string(), Value::String("storage_cache".to_string()));
            row_values.insert("entries".to_string(), Value::Number(storage_stats.0 as f64));
            row_values.insert("hit_rate".to_string(), Value::String("N/A".to_string()));
            row_values.insert("memory_bytes".to_string(), Value::Number(storage_stats.1 as f64));
            rows.push(Row::from_values(row_values));

            // Result cache
            let result_stats = &stats.result_cache;
            let mut row_values = HashMap::new();
            row_values.insert("cache_type".to_string(), Value::String("result_cache".to_string()));
            row_values.insert("entries".to_string(), Value::Number(0.0)); // ResultCacheStats doesn't track entries directly
            row_values.insert("hit_rate".to_string(), Value::Number(result_stats.hit_rate()));
            row_values.insert("memory_bytes".to_string(), Value::Number(result_stats.memory_savings_bytes as f64));
            rows.push(Row::from_values(row_values));

            // Plan cache
            let plan_stats = &stats.plan_cache;
            let mut row_values = HashMap::new();
            row_values.insert("cache_type".to_string(), Value::String("plan_cache".to_string()));
            row_values.insert("entries".to_string(), Value::Number(plan_stats.current_entries as f64));
            row_values.insert("hit_rate".to_string(), Value::Number(plan_stats.hit_rate()));
            row_values.insert("memory_bytes".to_string(), Value::Number(plan_stats.current_memory_bytes as f64));
            rows.push(Row::from_values(row_values));

            // Subquery cache (if available)
            if let Some(subquery_stats) = &stats.subquery_cache {
                let mut row_values = HashMap::new();
                row_values.insert("cache_type".to_string(), Value::String("subquery_cache".to_string()));
                row_values.insert("entries".to_string(), Value::Number(subquery_stats.current_entries as f64));
                row_values.insert("hit_rate".to_string(), Value::Number(subquery_stats.hit_rate()));
                row_values.insert("memory_bytes".to_string(), Value::Number(subquery_stats.memory_bytes as f64));
                rows.push(Row::from_values(row_values));
            }
        } else {
            // No cache manager - just return storage cache stats
            let storage_stats = self.storage.get_cache_stats();
            let mut row_values = HashMap::new();
            row_values.insert("cache_type".to_string(), Value::String("storage_cache".to_string()));
            row_values.insert("entries".to_string(), Value::Number(storage_stats.0 as f64));
            row_values.insert("hit_rate".to_string(), Value::String("N/A".to_string()));
            row_values.insert("memory_bytes".to_string(), Value::Number(storage_stats.1 as f64));
            rows.push(Row::from_values(row_values));
        }

        Ok(QueryResult {
            rows_affected: rows.len(),
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.clear_cache([cache_type]) YIELD status, cleared_caches
    fn clear_cache(&self, args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        let cache_type = if !args.is_empty() {
            match &args[0] {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            }
        } else {
            None
        };

        let mut cleared_caches = Vec::new();
        let status = "success";

        match cache_type {
            Some("storage_cache") => {
                let _ = self.storage.clear_cache();
                cleared_caches.push("storage_cache");
            }
            Some("result_cache") | Some("plan_cache") | Some("subquery_cache") => {
                if let Some(cache_manager) = &self.cache_manager {
                    cache_manager.clear_all();
                    cleared_caches.push("result_cache");
                    cleared_caches.push("plan_cache");
                    cleared_caches.push("subquery_cache");
                } else {
                    return Err(ExecutionError::RuntimeError(
                        "Cache manager not available".to_string()
                    ));
                }
            }
            Some(unknown) => {
                return Err(ExecutionError::RuntimeError(
                    format!("Unknown cache type: {}. Valid types: storage_cache, result_cache, plan_cache, subquery_cache", unknown)
                ));
            }
            None => {
                // Clear all caches
                let _ = self.storage.clear_cache();
                cleared_caches.push("storage_cache");

                if let Some(cache_manager) = &self.cache_manager {
                    cache_manager.clear_all();
                    cleared_caches.push("result_cache");
                    cleared_caches.push("plan_cache");
                    cleared_caches.push("subquery_cache");
                }
            }
        }

        let columns = vec!["status".to_string(), "cleared_caches".to_string()];
        let mut row_values = HashMap::new();
        row_values.insert("status".to_string(), Value::String(status.to_string()));
        row_values.insert("cleared_caches".to_string(), Value::String(cleared_caches.join(", ")));

        let rows = vec![Row::from_values(row_values)];

        Ok(QueryResult {
            rows_affected: 1,
            session_result: None,
            warnings: Vec::new(),

            rows,
            variables: columns,
            execution_time_ms: 0,
        })
    }

    /// CALL gql.authenticate_user(username, password) YIELD authenticated, user_id, username, roles
    /// Authenticates a user with username and password
    fn authenticate_user(&self, args: Vec<Value>) -> Result<QueryResult, ExecutionError> {
        // Validate exactly 2 arguments
        if args.len() != 2 {
            return Err(ExecutionError::RuntimeError(
                format!("authenticate_user expects exactly 2 arguments, got {}", args.len())
            ));
        }

        // Validate argument types
        let username = match &args[0] {
            Value::String(s) => s.clone(),
            _ => return Err(ExecutionError::RuntimeError(
                "First argument (username) must be a string".to_string()
            )),
        };

        let password = match &args[1] {
            Value::String(s) => s.clone(),
            _ => return Err(ExecutionError::RuntimeError(
                "Second argument (password) must be a string".to_string()
            )),
        };

        // Query the security catalog for authentication
        let mut catalog_manager = self.catalog_manager.write()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire catalog manager lock".to_string()))?;

        let auth_params = json!({
            "username": username,
            "password": password
        });

        let result = catalog_manager.execute(
            "security",
            CatalogOperation::Query {
                query_type: QueryType::Authenticate,
                params: auth_params,
            }
        ).map_err(|e| ExecutionError::RuntimeError(format!("Authentication failed: {}", e)))?;

        // Parse authentication response
        if let CatalogResponse::Query { results } = result {
            if results.get("authenticated").and_then(|v| v.as_bool()).unwrap_or(false) {
                // Authentication successful
                let user_id = results.get("user_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let roles_json = results.get("roles")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_else(|| "".to_string());

                let columns = vec![
                    "authenticated".to_string(),
                    "user_id".to_string(),
                    "username".to_string(),
                    "roles".to_string()
                ];

                let mut row_values = HashMap::new();
                row_values.insert("authenticated".to_string(), Value::Boolean(true));
                row_values.insert("user_id".to_string(), Value::String(user_id));
                row_values.insert("username".to_string(), Value::String(username));
                row_values.insert("roles".to_string(), Value::String(roles_json));

                let rows = vec![Row::from_values(row_values)];

                Ok(QueryResult {
                    rows_affected: 1,
                    session_result: None,
            warnings: Vec::new(),

                    rows,
                    variables: columns,
                    execution_time_ms: 0,
                })
            } else {
                // Authentication failed
                Err(ExecutionError::RuntimeError("Authentication failed: Invalid username or password".to_string()))
            }
        } else {
            Err(ExecutionError::RuntimeError("Unexpected response from security catalog".to_string()))
        }
    }
}

/// Check if a procedure name is a valid system procedure
pub fn is_system_procedure(procedure_name: &str) -> bool {
    let normalized = if procedure_name.starts_with("system.") {
        procedure_name.replace("system.", "gql.")
    } else {
        procedure_name.to_string()
    };

    matches!(
        normalized.as_str(),
        "gql.list_schemas" | "gql.list_graphs" | "gql.list_graph_types" | "gql.list_functions" | "gql.list_roles" | "gql.list_users" | "gql.authenticate_user" | "gql.show_session" | "gql.cache_stats" | "gql.clear_cache"
    )
}
