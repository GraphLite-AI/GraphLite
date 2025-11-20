// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Schema enforcement configuration

use crate::schema::types::SchemaEnforcementMode;
use serde::{Deserialize, Serialize};

/// Configuration for schema enforcement behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEnforcementConfig {
    /// Global enforcement mode
    pub mode: SchemaEnforcementMode,

    /// Whether to validate on write operations
    pub validate_on_write: bool,

    /// Whether to validate on read operations
    pub validate_on_read: bool,

    /// Whether to auto-create indexes based on schema
    pub auto_create_indexes: bool,

    /// Whether to log validation warnings
    pub log_warnings: bool,

    /// Whether to allow unknown properties not defined in schema
    pub allow_unknown_properties: bool,

    /// Whether to allow schema evolution without version change
    pub allow_schema_drift: bool,
}

impl Default for SchemaEnforcementConfig {
    fn default() -> Self {
        Self {
            mode: SchemaEnforcementMode::Advisory,
            validate_on_write: true,
            validate_on_read: false,
            auto_create_indexes: false,
            log_warnings: true,
            allow_unknown_properties: false,
            allow_schema_drift: false,
        }
    }
}

impl SchemaEnforcementConfig {
    /// Create a strict configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Strict schema enforcement preset for production graph type validation
    pub fn strict() -> Self {
        Self {
            mode: SchemaEnforcementMode::Strict,
            validate_on_write: true,
            validate_on_read: true,
            auto_create_indexes: true,
            log_warnings: true,
            allow_unknown_properties: false,
            allow_schema_drift: false,
        }
    }

    /// Create an advisory configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Advisory schema enforcement preset (warnings only, no blocking)
    pub fn advisory() -> Self {
        Self {
            mode: SchemaEnforcementMode::Advisory,
            validate_on_write: true,
            validate_on_read: false,
            auto_create_indexes: false,
            log_warnings: true,
            allow_unknown_properties: true,
            allow_schema_drift: true,
        }
    }

    /// Create a disabled configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Disabled schema enforcement preset (schemaless mode)
    pub fn disabled() -> Self {
        Self {
            mode: SchemaEnforcementMode::Disabled,
            validate_on_write: false,
            validate_on_read: false,
            auto_create_indexes: false,
            log_warnings: false,
            allow_unknown_properties: true,
            allow_schema_drift: true,
        }
    }

    /// Check if validation should be performed for write operations
    #[allow(dead_code)] // ROADMAP v0.4.0 - Write validation check for INSERT/UPDATE operations
    pub fn should_validate_write(&self) -> bool {
        self.mode != SchemaEnforcementMode::Disabled && self.validate_on_write
    }

    /// Check if validation should be performed for read operations
    #[allow(dead_code)] // ROADMAP v0.4.0 - Read validation check for SELECT/MATCH operations
    pub fn should_validate_read(&self) -> bool {
        self.mode != SchemaEnforcementMode::Disabled && self.validate_on_read
    }

    /// Check if validation errors should block operations
    #[allow(dead_code)] // ROADMAP v0.4.0 - Error blocking check (strict mode blocks, advisory mode warns)
    pub fn should_block_on_error(&self) -> bool {
        self.mode == SchemaEnforcementMode::Strict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_config() {
        let config = SchemaEnforcementConfig::strict();
        assert_eq!(config.mode, SchemaEnforcementMode::Strict);
        assert!(config.validate_on_write);
        assert!(config.validate_on_read);
        assert!(config.should_block_on_error());
    }

    #[test]
    fn test_advisory_config() {
        let config = SchemaEnforcementConfig::advisory();
        assert_eq!(config.mode, SchemaEnforcementMode::Advisory);
        assert!(config.validate_on_write);
        assert!(!config.validate_on_read);
        assert!(!config.should_block_on_error());
    }

    #[test]
    fn test_disabled_config() {
        let config = SchemaEnforcementConfig::disabled();
        assert_eq!(config.mode, SchemaEnforcementMode::Disabled);
        assert!(!config.should_validate_write());
        assert!(!config.should_validate_read());
    }
}
