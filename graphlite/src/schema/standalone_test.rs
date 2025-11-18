// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Standalone test file to verify schema module functionality
// This can be run independently to test our implementation

use super::types::*;
use super::enforcement::config::*;

fn main() {
    log::debug!("Running Schema Module Unit Tests\n");

    // Test 1: Version parsing
    log::debug!("Test 1: Version Parsing");
    test_version_parsing();

    // Test 2: Version compatibility
    log::debug!("\nTest 2: Version Compatibility");
    test_version_compatibility();

    // Test 3: Data type compatibility
    log::debug!("\nTest 3: Data Type Compatibility");
    test_data_type_compatibility();

    // Test 4: Enforcement config
    log::debug!("\nTest 4: Enforcement Configuration");
    test_enforcement_config();

    log::debug!("\nAll tests passed!");
}

fn test_version_parsing() {
    let version = GraphTypeVersion::parse("1.2.3").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 2);
    assert_eq!(version.patch, 3);
    assert_eq!(version.to_string(), "1.2.3");
    log::debug!("  ✓ Basic version parsing: 1.2.3");

    let version_with_pre = GraphTypeVersion::parse("2.0.0-beta+build123").unwrap();
    assert_eq!(version_with_pre.major, 2);
    assert_eq!(version_with_pre.minor, 0);
    assert_eq!(version_with_pre.patch, 0);
    assert_eq!(version_with_pre.pre_release, Some("beta".to_string()));
    assert_eq!(version_with_pre.build_metadata, Some("build123".to_string()));
    log::debug!("  ✓ Version with pre-release and build metadata: 2.0.0-beta+build123");
}

fn test_version_compatibility() {
    let v1 = GraphTypeVersion::new(1, 2, 3);
    let v2 = GraphTypeVersion::new(1, 3, 0);
    let v3 = GraphTypeVersion::new(2, 0, 0);

    assert!(v1.is_compatible_with(&v2)); // Same major version
    log::debug!("  ✓ Version 1.2.3 is compatible with 1.3.0 (same major)");

    assert!(!v1.is_compatible_with(&v3)); // Different major version
    log::debug!("  ✓ Version 1.2.3 is NOT compatible with 2.0.0 (different major)");
}

fn test_data_type_compatibility() {
    // Compatible types
    assert!(DataType::String.is_compatible_with(&DataType::Text));
    log::debug!("  ✓ String is compatible with Text");

    assert!(DataType::Integer.is_compatible_with(&DataType::BigInt));
    log::debug!("  ✓ Integer is compatible with BigInt");

    assert!(DataType::Float.is_compatible_with(&DataType::Double));
    log::debug!("  ✓ Float is compatible with Double");

    // Incompatible types
    assert!(!DataType::String.is_compatible_with(&DataType::Integer));
    log::debug!("  ✓ String is NOT compatible with Integer");
}

fn test_enforcement_config() {
    // Test strict configuration
    let strict = SchemaEnforcementConfig::strict();
    assert_eq!(strict.mode, SchemaEnforcementMode::Strict);
    assert!(strict.validate_on_write);
    assert!(strict.validate_on_read);
    assert!(strict.allow_schema_evolution);
    assert!(!strict.allow_unknown_properties);
    log::debug!("  ✓ Strict config: validates on write/read, no unknown properties");

    // Test advisory configuration
    let advisory = SchemaEnforcementConfig::advisory();
    assert_eq!(advisory.mode, SchemaEnforcementMode::Advisory);
    assert!(advisory.validate_on_write);
    assert!(!advisory.validate_on_read);
    assert!(advisory.allow_schema_evolution);
    assert!(advisory.allow_unknown_properties);
    log::debug!("  ✓ Advisory config: validates on write, allows unknown properties");

    // Test disabled configuration
    let disabled = SchemaEnforcementConfig::disabled();
    assert_eq!(disabled.mode, SchemaEnforcementMode::Disabled);
    assert!(!disabled.validate_on_write);
    assert!(!disabled.validate_on_read);
    assert!(disabled.allow_schema_evolution);
    assert!(disabled.allow_unknown_properties);
    log::debug!("  ✓ Disabled config: no validation, all operations allowed");
}
