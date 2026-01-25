// Schema Engine - DDL Operations
//
// This module handles all schema definition operations (CREATE, DROP, ALTER) in GraphLite.
// It manages graph types, catalogs, and security (roles/users).

pub mod operations;
pub mod validators;

// Re-export for convenience (TODO: Enable when modules are populated)
// pub use operations::catalog;
// pub use operations::types;
// pub use operations::security;
