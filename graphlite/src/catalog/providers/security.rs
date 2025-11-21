// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Security catalog provider implementation
//!
//! This module provides the security catalog implementation that follows the
//! pluggable catalog architecture. It manages users, roles, permissions,
//! and access control for catalog resources.

use crate::catalog::error::{CatalogError, CatalogResult};
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::catalog::traits::{CatalogProvider, CatalogSchema};
use crate::storage::StorageManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Principal identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrincipalId {
    pub id: Uuid,
    pub name: String,
    pub principal_type: PrincipalType,
}

impl PrincipalId {
    pub fn user(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            principal_type: PrincipalType::User,
        }
    }

    pub fn role(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            principal_type: PrincipalType::Role,
        }
    }
}

/// Type of principal
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrincipalType {
    User,
    Role,
}

/// User definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: PrincipalId,
    pub password_hash: Option<String>,
    pub enabled: bool,
    pub roles: HashSet<String>,
    pub properties: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

impl User {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: PrincipalId::user(name),
            password_hash: None,
            enabled: true,
            roles: HashSet::new(),
            properties: HashMap::new(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Create from parameters
    pub fn from_params(name: String, params: &Value) -> Self {
        let mut user = Self::new(name);

        // All users get the 'user' role by default
        user.roles.insert("user".to_string());

        if let Some(enabled) = params.get("enabled").and_then(|v| v.as_bool()) {
            user.enabled = enabled;
        }

        if let Some(password) = params.get("password").and_then(|v| v.as_str()) {
            user.set_password(password);
        }

        if let Some(roles) = params.get("roles").and_then(|v| v.as_array()) {
            for role in roles {
                if let Some(role_name) = role.as_str() {
                    user.roles.insert(role_name.to_string());
                }
            }
        }

        if let Some(props) = params.get("properties").and_then(|v| v.as_object()) {
            for (key, value) in props {
                if let Some(val_str) = value.as_str() {
                    user.properties.insert(key.clone(), val_str.to_string());
                }
            }
        }

        user
    }

    pub fn set_password(&mut self, password: &str) {
        // Simple hash for demo - in production use proper password hashing
        self.password_hash = Some(format!("hash_{}", password));
        self.modified_at = chrono::Utc::now();
    }

    pub fn verify_password(&self, password: &str) -> bool {
        if let Some(ref hash) = self.password_hash {
            hash == &format!("hash_{}", password)
        } else {
            false
        }
    }

    pub fn add_role(&mut self, role_name: String) {
        self.roles.insert(role_name);
        self.modified_at = chrono::Utc::now();
    }

    pub fn remove_role(&mut self, role_name: &str) -> Result<(), String> {
        // Prevent removal of system 'user' role
        if role_name == "user" {
            return Err("Cannot revoke system role 'user'".to_string());
        }
        self.roles.remove(role_name);
        self.modified_at = chrono::Utc::now();
        Ok(())
    }
}

/// Role definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: PrincipalId,
    pub description: Option<String>,
    pub parent_roles: HashSet<String>,
    pub permissions: HashSet<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

impl Role {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: PrincipalId::role(name),
            description: None,
            parent_roles: HashSet::new(),
            permissions: HashSet::new(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Create from parameters
    pub fn from_params(name: String, params: &Value) -> Self {
        let mut role = Self::new(name);

        if let Some(desc) = params.get("description").and_then(|v| v.as_str()) {
            role.description = Some(desc.to_string());
        }

        if let Some(parent_roles) = params.get("parent_roles").and_then(|v| v.as_array()) {
            for parent in parent_roles {
                if let Some(parent_name) = parent.as_str() {
                    role.parent_roles.insert(parent_name.to_string());
                }
            }
        }

        if let Some(permissions) = params.get("permissions").and_then(|v| v.as_array()) {
            for permission in permissions {
                if let Some(perm_name) = permission.as_str() {
                    role.permissions.insert(perm_name.to_string());
                }
            }
        }

        role
    }

    pub fn add_permission(&mut self, permission: String) {
        self.permissions.insert(permission);
        self.modified_at = chrono::Utc::now();
    }

    pub fn remove_permission(&mut self, permission: &str) {
        self.permissions.remove(permission);
        self.modified_at = chrono::Utc::now();
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Role hierarchy management (see ROADMAP.md §4)
    pub fn add_parent_role(&mut self, parent_role: String) {
        self.parent_roles.insert(parent_role);
        self.modified_at = chrono::Utc::now();
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Role hierarchy management (see ROADMAP.md §4)
    pub fn remove_parent_role(&mut self, parent_role: &str) {
        self.parent_roles.remove(parent_role);
        self.modified_at = chrono::Utc::now();
    }
}

/// Permission types for catalog objects
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Schema permissions
    CreateSchema,
    DropSchema,
    AlterSchema,

    // Graph permissions
    CreateGraph,
    DropGraph,
    AlterGraph,
    SelectGraph,
    InsertGraph,
    UpdateGraph,
    DeleteGraph,

    // Graph type permissions
    CreateGraphType,
    DropGraphType,
    AlterGraphType,

    // Administrative permissions
    GrantPermission,
    RevokePermission,

    // Special permissions
    All, // Grants all permissions
}

/// Access Control Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ACE {
    pub id: Uuid,
    pub principal_name: String,
    pub principal_type: PrincipalType,
    pub resource_path: String,
    pub resource_type: String,
    pub permissions: HashSet<String>,
    pub granted: bool, // true for GRANT, false for DENY
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ACE {
    pub fn new(
        principal_name: String,
        principal_type: PrincipalType,
        resource_path: String,
        resource_type: String,
        permissions: HashSet<String>,
        granted: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            principal_name,
            principal_type,
            resource_path,
            resource_type,
            permissions,
            granted,
            created_at: chrono::Utc::now(),
        }
    }

    /// Create from parameters
    pub fn from_params(params: &Value) -> CatalogResult<Self> {
        let principal_name = params
            .get("principal_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CatalogError::InvalidParameters("Missing 'principal_name'".to_string()))?
            .to_string();

        let principal_type = params
            .get("principal_type")
            .and_then(|v| v.as_str())
            .map(|t| match t.to_lowercase().as_str() {
                "user" => PrincipalType::User,
                "role" => PrincipalType::Role,
                _ => PrincipalType::User,
            })
            .unwrap_or(PrincipalType::User);

        let resource_path = params
            .get("resource_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CatalogError::InvalidParameters("Missing 'resource_path'".to_string()))?
            .to_string();

        let resource_type = params
            .get("resource_type")
            .and_then(|v| v.as_str())
            .unwrap_or("catalog")
            .to_string();

        let permissions = params
            .get("permissions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let granted = params
            .get("granted")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(Self::new(
            principal_name,
            principal_type,
            resource_path,
            resource_type,
            permissions,
            granted,
        ))
    }
}

/// Security catalog state
#[derive(Clone, Serialize, Deserialize)]
struct SecurityCatalogState {
    users: HashMap<String, User>,
    roles: HashMap<String, Role>,
    aces: HashMap<Uuid, ACE>,
}

/// Security catalog provider
#[derive(Clone)]
pub struct SecurityCatalog {
    /// Map of user name to user
    users: HashMap<String, User>,

    /// Map of role name to role
    roles: HashMap<String, Role>,

    /// Map of ACE ID to ACE
    aces: HashMap<Uuid, ACE>,

    /// Storage manager reference
    storage: Option<Arc<StorageManager>>,
}

impl SecurityCatalog {
    /// Create a new security catalog provider
    pub fn new() -> Box<dyn CatalogProvider> {
        let mut catalog = Self {
            users: HashMap::new(),
            roles: HashMap::new(),
            aces: HashMap::new(),
            storage: None,
        };

        // Initialize default user role (all users get this)
        let mut user_role = Role::new("user".to_string());
        user_role.description = Some("Default user role for basic system access".to_string());
        catalog.roles.insert("user".to_string(), user_role);

        // Initialize default admin role
        let mut admin_role = Role::new("admin".to_string());
        admin_role.description =
            Some("Default administrator role with full system access".to_string());
        catalog.roles.insert("admin".to_string(), admin_role);

        // Initialize default admin user with both admin and user roles
        let mut admin_user = User::new("admin".to_string());
        admin_user.roles.insert("user".to_string());
        admin_user.roles.insert("admin".to_string());
        catalog.users.insert("admin".to_string(), admin_user);

        log::info!("Security catalog initialized with default admin user and roles (user, admin)");

        Box::new(catalog)
    }

    /// Add a user to the catalog
    fn add_user(&mut self, user: User) -> CatalogResult<()> {
        if self.users.contains_key(&user.id.name) {
            return Err(CatalogError::DuplicateEntry(format!(
                "User '{}' already exists",
                user.id.name
            )));
        }

        self.users.insert(user.id.name.clone(), user);
        Ok(())
    }

    /// Add a role to the catalog
    fn add_role(&mut self, role: Role) -> CatalogResult<()> {
        if self.roles.contains_key(&role.id.name) {
            return Err(CatalogError::DuplicateEntry(format!(
                "Role '{}' already exists",
                role.id.name
            )));
        }

        self.roles.insert(role.id.name.clone(), role);
        Ok(())
    }

    /// Add an ACE to the catalog
    fn add_ace(&mut self, ace: ACE) -> CatalogResult<()> {
        self.aces.insert(ace.id, ace);
        Ok(())
    }

    /// Get a user by name
    fn get_user(&self, name: &str) -> Option<&User> {
        self.users.get(name)
    }

    /// Get a role by name
    fn get_role(&self, name: &str) -> Option<&Role> {
        self.roles.get(name)
    }

    /// Remove a user
    fn remove_user(&mut self, name: &str) -> CatalogResult<User> {
        self.users
            .remove(name)
            .ok_or_else(|| CatalogError::NotFound(format!("User '{}' not found", name)))
    }

    /// Remove a role
    fn remove_role(&mut self, name: &str, cascade: bool) -> CatalogResult<Role> {
        // Check if any users have this role
        if !cascade {
            let users_with_role: Vec<_> = self
                .users
                .values()
                .filter(|u| u.roles.contains(name))
                .map(|u| u.id.name.clone())
                .collect();

            if !users_with_role.is_empty() {
                return Err(CatalogError::InvalidOperation(format!(
                    "Cannot drop role '{}': assigned to users: {:?}",
                    name, users_with_role
                )));
            }
        }

        self.roles
            .remove(name)
            .ok_or_else(|| CatalogError::NotFound(format!("Role '{}' not found", name)))
    }

    /// Update user properties
    fn update_user(&mut self, name: &str, updates: &Value) -> CatalogResult<()> {
        let user = self
            .users
            .get_mut(name)
            .ok_or_else(|| CatalogError::NotFound(format!("User '{}' not found", name)))?;

        if let Some(enabled) = updates.get("enabled").and_then(|v| v.as_bool()) {
            user.enabled = enabled;
        }

        if let Some(password) = updates.get("password").and_then(|v| v.as_str()) {
            user.set_password(password);
        }

        if let Some(roles) = updates.get("add_roles").and_then(|v| v.as_array()) {
            for role in roles {
                if let Some(role_name) = role.as_str() {
                    user.add_role(role_name.to_string());
                }
            }
        }

        if let Some(roles) = updates.get("remove_roles").and_then(|v| v.as_array()) {
            for role in roles {
                if let Some(role_name) = role.as_str() {
                    // Special protection: cannot revoke 'admin' role from 'admin' user
                    if name == "admin" && role_name == "admin" {
                        return Err(CatalogError::InvalidOperation(
                            "Cannot revoke 'admin' role from 'admin' user".to_string(),
                        ));
                    }
                    user.remove_role(role_name)
                        .map_err(CatalogError::InvalidOperation)?;
                }
            }
        }

        user.modified_at = chrono::Utc::now();
        Ok(())
    }

    /// Update role properties
    fn update_role(&mut self, name: &str, updates: &Value) -> CatalogResult<()> {
        let role = self
            .roles
            .get_mut(name)
            .ok_or_else(|| CatalogError::NotFound(format!("Role '{}' not found", name)))?;

        if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
            role.description = Some(desc.to_string());
        }

        if let Some(permissions) = updates.get("add_permissions").and_then(|v| v.as_array()) {
            for permission in permissions {
                if let Some(perm_name) = permission.as_str() {
                    role.add_permission(perm_name.to_string());
                }
            }
        }

        if let Some(permissions) = updates.get("remove_permissions").and_then(|v| v.as_array()) {
            for permission in permissions {
                if let Some(perm_name) = permission.as_str() {
                    role.remove_permission(perm_name);
                }
            }
        }

        role.modified_at = chrono::Utc::now();
        Ok(())
    }

    /// Get ACEs for a resource
    fn get_aces_for_resource(&self, resource_path: &str) -> Vec<&ACE> {
        self.aces
            .values()
            .filter(|ace| ace.resource_path == resource_path)
            .collect()
    }

    /// Get ACEs for a principal
    fn get_aces_for_principal(&self, principal_name: &str) -> Vec<&ACE> {
        self.aces
            .values()
            .filter(|ace| ace.principal_name == principal_name)
            .collect()
    }
}

impl CatalogProvider for SecurityCatalog {
    fn init(&mut self, storage: Arc<StorageManager>) -> CatalogResult<()> {
        self.storage = Some(storage.clone());

        // Try to load persisted state from storage
        match storage.load_catalog_provider("security") {
            Ok(Some(data)) => {
                // Load persisted catalog state
                match self.load(&data) {
                    Ok(_) => {
                        log::info!(
                            "Security catalog loaded from storage with {} users, {} roles",
                            self.users.len(),
                            self.roles.len()
                        );
                    }
                    Err(e) => {
                        log::warn!("Failed to deserialize security catalog from storage: {}. Using defaults.", e);
                    }
                }
            }
            Ok(None) => {
                // This is expected on first run - no catalog data exists yet
                log::debug!("No persisted security catalog found. Using default initialization.");
            }
            Err(e) => {
                log::warn!(
                    "Error loading security catalog: {}. Using default initialization.",
                    e
                );
            }
        }

        Ok(())
    }

    fn execute(&mut self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        match op {
            CatalogOperation::Create {
                entity_type,
                name,
                params,
            } => match entity_type {
                EntityType::User => {
                    let user = User::from_params(name.clone(), &params);
                    self.add_user(user)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("User '{}' created", name) })),
                    })
                }
                EntityType::Role => {
                    let role = Role::from_params(name.clone(), &params);
                    self.add_role(role)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("Role '{}' created", name) })),
                    })
                }
                EntityType::Ace => {
                    let ace = ACE::from_params(&params)?;
                    self.add_ace(ace)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": "Access control entry created" })),
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::Drop {
                entity_type,
                name,
                cascade,
            } => match entity_type {
                EntityType::User => {
                    let removed = self.remove_user(&name)?;
                    Ok(CatalogResponse::Success {
                        data: Some(serde_json::to_value(removed)?),
                    })
                }
                EntityType::Role => {
                    let removed = self.remove_role(&name, cascade)?;
                    Ok(CatalogResponse::Success {
                        data: Some(serde_json::to_value(removed)?),
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::Query { query_type, params } => {
                match query_type {
                    QueryType::GetUser => {
                        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                            if let Some(user) = self.get_user(name) {
                                Ok(CatalogResponse::Query {
                                    results: serde_json::to_value(user)?,
                                })
                            } else {
                                Err(CatalogError::NotFound(format!("User '{}' not found", name)))
                            }
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'name' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::GetRole => {
                        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                            if let Some(role) = self.get_role(name) {
                                Ok(CatalogResponse::Query {
                                    results: serde_json::to_value(role)?,
                                })
                            } else {
                                Err(CatalogError::NotFound(format!("Role '{}' not found", name)))
                            }
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'name' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::GetAcesForResource => {
                        if let Some(resource_path) =
                            params.get("resource_path").and_then(|v| v.as_str())
                        {
                            let aces = self.get_aces_for_resource(resource_path);
                            Ok(CatalogResponse::Query {
                                results: serde_json::to_value(aces)?,
                            })
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'resource_path' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::GetAcesForPrincipal => {
                        if let Some(principal_name) =
                            params.get("principal_name").and_then(|v| v.as_str())
                        {
                            let aces = self.get_aces_for_principal(principal_name);
                            Ok(CatalogResponse::Query {
                                results: serde_json::to_value(aces)?,
                            })
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'principal_name' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::Authenticate => {
                        let username =
                            params
                                .get("username")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    CatalogError::InvalidParameters(
                                        "Missing 'username'".to_string(),
                                    )
                                })?;
                        let password =
                            params
                                .get("password")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    CatalogError::InvalidParameters(
                                        "Missing 'password'".to_string(),
                                    )
                                })?;

                        if let Some(user) = self.get_user(username) {
                            if user.verify_password(password) && user.enabled {
                                Ok(CatalogResponse::Query {
                                    results: json!({
                                        "authenticated": true,
                                        "username": username,
                                        "user_id": user.id.id.to_string(),
                                        "roles": user.roles
                                    }),
                                })
                            } else {
                                Ok(CatalogResponse::Query {
                                    results: json!({ "authenticated": false }),
                                })
                            }
                        } else {
                            Ok(CatalogResponse::Query {
                                results: json!({ "authenticated": false }),
                            })
                        }
                    }
                    QueryType::ListRoles => {
                        // Return all roles
                        let roles: Vec<&Role> = self.roles.values().collect();
                        Ok(CatalogResponse::Query {
                            results: serde_json::to_value(roles)?,
                        })
                    }
                    QueryType::ListUsers => {
                        // Return all users
                        let users: Vec<&User> = self.users.values().collect();
                        Ok(CatalogResponse::Query {
                            results: serde_json::to_value(users)?,
                        })
                    }
                    _ => Ok(CatalogResponse::NotSupported),
                }
            }

            CatalogOperation::Update {
                entity_type,
                name,
                updates,
            } => match entity_type {
                EntityType::User => {
                    self.update_user(&name, &updates)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("User '{}' updated", name) })),
                    })
                }
                EntityType::Role => {
                    self.update_role(&name, &updates)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("Role '{}' updated", name) })),
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::List {
                entity_type,
                filters: _,
            } => match entity_type {
                EntityType::User => {
                    let users: Vec<&User> = self.users.values().collect();
                    Ok(CatalogResponse::List {
                        items: users
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                EntityType::Role => {
                    let roles: Vec<&Role> = self.roles.values().collect();
                    Ok(CatalogResponse::List {
                        items: roles
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                EntityType::Ace => {
                    let aces: Vec<&ACE> = self.aces.values().collect();
                    Ok(CatalogResponse::List {
                        items: aces
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            _ => Ok(CatalogResponse::NotSupported),
        }
    }

    fn save(&self) -> CatalogResult<Vec<u8>> {
        let state = SecurityCatalogState {
            users: self.users.clone(),
            roles: self.roles.clone(),
            aces: self.aces.clone(),
        };

        let data = bincode::serialize(&state)
            .map_err(|e| CatalogError::SerializationError(e.to_string()))?;
        Ok(data)
    }

    fn load(&mut self, data: &[u8]) -> CatalogResult<()> {
        let state: SecurityCatalogState = bincode::deserialize(data)
            .map_err(|e| CatalogError::DeserializationError(e.to_string()))?;

        self.users = state.users;
        self.roles = state.roles;
        self.aces = state.aces;
        Ok(())
    }

    fn schema(&self) -> CatalogSchema {
        CatalogSchema {
            name: "security".to_string(),
            version: "1.0.0".to_string(),
            entities: vec![
                EntityType::User.to_string(),
                EntityType::Role.to_string(),
                "ace".to_string(),
            ],
            operations: self.supported_operations(),
        }
    }

    fn supported_operations(&self) -> Vec<String> {
        vec![
            "create_user".to_string(),
            "create_role".to_string(),
            "create_ace".to_string(),
            "drop_user".to_string(),
            "drop_role".to_string(),
            "get_user".to_string(),
            "get_role".to_string(),
            "list_users".to_string(),
            "list_roles".to_string(),
            "list_aces".to_string(),
            "update_user".to_string(),
            "update_role".to_string(),
            "authenticate".to_string(),
            "get_aces_for_resource".to_string(),
            "get_aces_for_principal".to_string(),
        ]
    }

    fn execute_read_only(&self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        // Handle read-only operations for the security catalog
        // These operations don't modify state, so they're safe for concurrent access
        match op {
            CatalogOperation::Query { query_type, params } => {
                match query_type {
                    QueryType::Authenticate => {
                        let username =
                            params
                                .get("username")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    CatalogError::InvalidParameters(
                                        "Missing 'username'".to_string(),
                                    )
                                })?;
                        let password =
                            params
                                .get("password")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    CatalogError::InvalidParameters(
                                        "Missing 'password'".to_string(),
                                    )
                                })?;

                        if let Some(user) = self.get_user(username) {
                            if user.verify_password(password) && user.enabled {
                                Ok(CatalogResponse::Query {
                                    results: json!({
                                        "authenticated": true,
                                        "username": username,
                                        "user_id": user.id.id.to_string(),
                                        "roles": user.roles
                                    }),
                                })
                            } else {
                                Ok(CatalogResponse::Query {
                                    results: json!({ "authenticated": false }),
                                })
                            }
                        } else {
                            Ok(CatalogResponse::Query {
                                results: json!({ "authenticated": false }),
                            })
                        }
                    }
                    QueryType::GetUser => {
                        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                            if let Some(user) = self.get_user(name) {
                                Ok(CatalogResponse::Query {
                                    results: serde_json::to_value(user)?,
                                })
                            } else {
                                Err(CatalogError::NotFound(format!("User '{}' not found", name)))
                            }
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'name' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::GetRole => {
                        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                            if let Some(role) = self.get_role(name) {
                                Ok(CatalogResponse::Query {
                                    results: serde_json::to_value(role)?,
                                })
                            } else {
                                Err(CatalogError::NotFound(format!("Role '{}' not found", name)))
                            }
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'name' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::ListUsers => {
                        let users: Vec<&User> = self.users.values().collect();
                        Ok(CatalogResponse::List {
                            items: users
                                .iter()
                                .map(serde_json::to_value)
                                .collect::<Result<Vec<_>, _>>()?,
                        })
                    }
                    QueryType::ListRoles => {
                        let roles: Vec<&Role> = self.roles.values().collect();
                        Ok(CatalogResponse::List {
                            items: roles
                                .iter()
                                .map(serde_json::to_value)
                                .collect::<Result<Vec<_>, _>>()?,
                        })
                    }
                    QueryType::GetAcesForResource => {
                        if let Some(resource_path) =
                            params.get("resource_path").and_then(|v| v.as_str())
                        {
                            let aces = self.get_aces_for_resource(resource_path);
                            Ok(CatalogResponse::Query {
                                results: serde_json::to_value(aces)?,
                            })
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'resource_path' parameter".to_string(),
                            ))
                        }
                    }
                    QueryType::GetAcesForPrincipal => {
                        if let Some(principal_name) =
                            params.get("principal_name").and_then(|v| v.as_str())
                        {
                            let aces = self.get_aces_for_principal(principal_name);
                            Ok(CatalogResponse::Query {
                                results: serde_json::to_value(aces)?,
                            })
                        } else {
                            Err(CatalogError::InvalidParameters(
                                "Missing 'principal_name' parameter".to_string(),
                            ))
                        }
                    }
                    _ => {
                        // For any other query types not explicitly handled
                        Err(CatalogError::NotSupported(format!(
                            "Query type {:?} not supported in read-only mode",
                            query_type
                        )))
                    }
                }
            }
            CatalogOperation::List {
                entity_type,
                filters: _,
            } => match entity_type {
                EntityType::User => {
                    let users: Vec<&User> = self.users.values().collect();
                    Ok(CatalogResponse::List {
                        items: users
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                EntityType::Role => {
                    let roles: Vec<&Role> = self.roles.values().collect();
                    Ok(CatalogResponse::List {
                        items: roles
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                EntityType::Ace => {
                    let aces: Vec<&ACE> = self.aces.values().collect();
                    Ok(CatalogResponse::List {
                        items: aces
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },
            _ => {
                // Create, Update, Drop operations are not allowed in read-only mode
                Err(CatalogError::NotSupported(
                    "Write operations not allowed in read-only mode".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_catalog_creation() {
        let catalog = SecurityCatalog::new();
        assert_eq!(catalog.schema().name, "security");
    }

    #[test]
    fn test_create_and_get_user() {
        let mut catalog = SecurityCatalog {
            users: HashMap::new(),
            roles: HashMap::new(),
            aces: HashMap::new(),
            storage: None,
        };

        let params = json!({
            "password": "secret123",
            "enabled": true,
            "roles": ["admin"]
        });

        let result = catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::User,
            name: "testuser".to_string(),
            params,
        });

        assert!(result.is_ok());

        let query_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::GetUser,
            params: json!({ "name": "testuser" }),
        });

        assert!(query_result.is_ok());
    }

    #[test]
    fn test_user_authentication() {
        let mut catalog = SecurityCatalog {
            users: HashMap::new(),
            roles: HashMap::new(),
            aces: HashMap::new(),
            storage: None,
        };

        // Create user
        catalog
            .execute(CatalogOperation::Create {
                entity_type: EntityType::User,
                name: "testuser".to_string(),
                params: json!({ "password": "secret123", "enabled": true }),
            })
            .unwrap();

        // Test authentication
        let auth_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::Authenticate,
            params: json!({ "username": "testuser", "password": "secret123" }),
        });

        assert!(auth_result.is_ok());
        if let Ok(CatalogResponse::Query { results }) = auth_result {
            assert_eq!(results["authenticated"], true);
        }
    }

    #[test]
    fn test_create_role_and_ace() {
        let mut catalog = SecurityCatalog {
            users: HashMap::new(),
            roles: HashMap::new(),
            aces: HashMap::new(),
            storage: None,
        };

        // Create role
        let role_result = catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Role,
            name: "admin".to_string(),
            params: json!({
                "description": "Administrator role",
                "permissions": ["all"]
            }),
        });

        assert!(role_result.is_ok());

        // Create ACE
        let ace_result = catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Ace,
            name: "ace1".to_string(),
            params: json!({
                "principal_name": "admin",
                "principal_type": "role",
                "resource_path": "/schema1",
                "resource_type": "schema",
                "permissions": ["create_graph", "drop_graph"],
                "granted": true
            }),
        });

        assert!(ace_result.is_ok());
    }
}
