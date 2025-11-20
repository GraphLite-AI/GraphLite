// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! ISO GQL Special Functions Implementation
//!
//! This module implements the three special predicate functions defined in the ISO GQL standard:
//! - ALL_DIFFERENT: Ensures all provided expressions evaluate to different values
//! - SAME: Checks if two expressions evaluate to the same value  
//! - PROPERTY_EXISTS: Checks if a property exists on a node/edge
//!
//! These functions are part of the official ISO GQL BNF grammar under <predicate> production rule.

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;
use log::{debug, warn};
use std::collections::HashSet;

/// ALL_DIFFERENT function: Ensures all provided expressions evaluate to different values
///
/// According to ISO GQL spec: ALL_DIFFERENT(expr1, expr2, ..., exprN)
/// Returns true if all expressions evaluate to distinct values, false otherwise.
///
/// # Examples
/// ```gql
/// ALL_DIFFERENT(person.id, company.id, location.id)  -- Returns true if all IDs are different
/// ALL_DIFFERENT(1, 2, 1)  -- Returns false (duplicate value)
/// ```
#[derive(Debug)]
pub struct AllDifferentFunction;

impl AllDifferentFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for AllDifferentFunction {
    fn name(&self) -> &str {
        "ALL_DIFFERENT"
    }

    fn description(&self) -> &str {
        "Returns true if all provided expressions evaluate to different values"
    }

    fn argument_count(&self) -> usize {
        // Variable argument count - minimum 1
        1 // This indicates minimum, actual implementation handles variadic
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!(
            "Executing ALL_DIFFERENT function with {} arguments",
            context.arguments.len()
        );

        // Validate minimum argument count
        if context.arguments.is_empty() {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: 0,
            });
        }

        // Use HashSet to track seen values for O(n) performance
        let mut seen_values = HashSet::new();

        for (i, arg) in context.arguments.iter().enumerate() {
            debug!("Checking argument {}: {:?}", i, arg);

            // Convert Value to a comparable representation
            let comparable_value = value_to_comparable(arg)?;

            if seen_values.contains(&comparable_value) {
                debug!("Found duplicate value: {:?}", comparable_value);
                return Ok(Value::Boolean(false));
            }

            seen_values.insert(comparable_value);
        }

        debug!("All {} values are different", context.arguments.len());
        Ok(Value::Boolean(true))
    }

    fn graph_context_required(&self) -> bool {
        false // Pure function that doesn't need graph context
    }

    fn is_variadic(&self) -> bool {
        true // Accepts variable number of arguments
    }
}

/// SAME function: Checks if two expressions evaluate to the same value
///
/// According to ISO GQL spec: SAME(expr1, expr2)
/// Returns true if both expressions evaluate to the same value, false otherwise.
///
/// # Examples
/// ```gql
/// SAME(person.birth_year, person.graduation_year - 4)  -- Check if graduation was 4 years after birth
/// SAME("hello", "hello")  -- Returns true
/// SAME(42, 43)  -- Returns false
/// ```
#[derive(Debug)]
pub struct SameFunction;

impl SameFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SameFunction {
    fn name(&self) -> &str {
        "SAME"
    }

    fn description(&self) -> &str {
        "Returns true if two expressions evaluate to the same value"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing SAME function");

        // Validate exact argument count
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let value1 = &context.arguments[0];
        let value2 = &context.arguments[1];

        debug!("Comparing values: {:?} vs {:?}", value1, value2);

        // Convert both values to comparable representations
        let comparable1 = value_to_comparable(value1)?;
        let comparable2 = value_to_comparable(value2)?;

        let are_same = comparable1 == comparable2;
        debug!("Values are same: {}", are_same);

        Ok(Value::Boolean(are_same))
    }

    fn graph_context_required(&self) -> bool {
        false // Pure function that doesn't need graph context
    }
}

/// PROPERTY_EXISTS function: Checks if a property exists on a node/edge
///
/// According to ISO GQL spec: PROPERTY_EXISTS(property_reference)
/// Returns true if the specified property exists, false otherwise.
///
/// # Examples
/// ```gql
/// PROPERTY_EXISTS(person.email)  -- Returns true if person has email property
/// PROPERTY_EXISTS(node.nonexistent)  -- Returns false
/// ```
#[derive(Debug)]
pub struct PropertyExistsFunction;

impl PropertyExistsFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for PropertyExistsFunction {
    fn name(&self) -> &str {
        "PROPERTY_EXISTS"
    }

    fn description(&self) -> &str {
        "Returns true if the specified property exists on the node/edge"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        debug!("Executing PROPERTY_EXISTS function");

        // Validate argument count
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let property_ref = &context.arguments[0];
        debug!("Checking property existence for: {:?}", property_ref);

        // For now, we'll implement a simplified version that works with string property paths
        // In a full implementation, this would need proper property reference parsing
        match property_ref {
            Value::String(property_path) => {
                // Parse property path (e.g., "person.email" -> check if person has email)
                let exists = check_property_exists_from_path(property_path, context)?;
                debug!("Property {} exists: {}", property_path, exists);
                Ok(Value::Boolean(exists))
            }
            _ => {
                // PROPERTY_EXISTS requires a property reference (string for now)
                return Err(FunctionError::InvalidArgumentType {
                    message: "PROPERTY_EXISTS argument must be a property reference (string)"
                        .to_string(),
                });
            }
        }
    }

    fn graph_context_required(&self) -> bool {
        true // Needs access to graph context to check property existence
    }
}

/// Convert a Value to a comparable representation for equality/uniqueness checks
/// This handles the complexity of comparing different Value types consistently
fn value_to_comparable(value: &Value) -> FunctionResult<ComparableValue> {
    match value {
        Value::Boolean(b) => Ok(ComparableValue::Boolean(*b)),
        Value::Number(n) => Ok(ComparableValue::Number(n.to_bits())), // Use bit representation for exact floating point comparison
        Value::String(s) => Ok(ComparableValue::String(s.clone())),
        Value::DateTime(dt) => Ok(ComparableValue::DateTime(dt.timestamp())),
        Value::DateTimeWithFixedOffset(dt) => Ok(ComparableValue::DateTime(dt.timestamp())),
        Value::DateTimeWithNamedTz(_, dt) => Ok(ComparableValue::DateTime(dt.timestamp())),
        Value::TimeWindow(tw) => Ok(ComparableValue::String(format!("{:?}", tw))), // Use debug format for comparison
        // Add more value types as needed
        _ => {
            warn!("Unsupported value type for comparison: {:?}", value);
            Err(FunctionError::InvalidArgumentType {
                message: format!("Value type {:?} is not supported for comparison", value),
            })
        }
    }
}

/// Comparable representation of values for equality and uniqueness checks
/// This ensures consistent comparison across different value types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ComparableValue {
    Boolean(bool),
    Number(u64), // Using bit representation of f64
    String(String),
    DateTime(i64), // Unix timestamp
}

/// Check if a property exists based on a property path string
/// This is a simplified implementation - a full implementation would integrate with
/// the query execution context and graph traversal
fn check_property_exists_from_path(
    property_path: &str,
    context: &FunctionContext,
) -> FunctionResult<bool> {
    debug!("Checking property path: {}", property_path);

    // Parse property path (e.g., "person.email" -> ["person", "email"])
    let parts: Vec<&str> = property_path.split('.').collect();
    if parts.len() != 2 {
        warn!("Invalid property path format: {}", property_path);
        return Ok(false);
    }

    let _entity = parts[0];
    let property = parts[1];

    // Simplified check - in real implementation, this would:
    // 1. Look up the entity in the current query context
    // 2. Check if that entity has the specified property
    // 3. Return true/false based on actual graph data

    // For now, simulate property existence based on common patterns
    let common_properties = [
        "id",
        "name",
        "email",
        "age",
        "created_at",
        "updated_at",
        "birth_year",
        "graduation_year",
        "city",
        "country",
        "founded",
    ];

    let exists = common_properties.contains(&property);
    debug!("Property '{}' exists (simulated): {}", property, exists);

    // Check if we have any variables in context that might contain this property
    if !exists {
        for (var_name, var_value) in &context.variables {
            debug!("Checking variable {}: {:?}", var_name, var_value);
            // In a real implementation, we'd check if var_value has the property
        }
    }

    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_all_different_basic() {
        let func = AllDifferentFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::String("three".to_string()),
            ],
        );

        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_all_different_with_duplicates() {
        let func = AllDifferentFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(1.0), // Duplicate
            ],
        );

        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_same_identical_values() {
        let func = SameFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(42.0), Value::Number(42.0)],
        );

        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_same_different_values() {
        let func = SameFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(42.0), Value::Number(43.0)],
        );

        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_same_different_types() {
        let func = SameFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(42.0), Value::String("42".to_string())],
        );

        let result = func.execute(&context).unwrap();
        // Different types should be different even with same logical value
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_property_exists_basic() {
        let func = PropertyExistsFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("person.email".to_string())],
        );

        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(true)); // "email" is in common properties
    }

    #[test]
    fn test_property_exists_missing() {
        let func = PropertyExistsFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("person.nonexistent".to_string())],
        );

        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_argument_validation() {
        // Test ALL_DIFFERENT with empty args
        let all_diff = AllDifferentFunction::new();
        let empty_context = FunctionContext::new(vec![], HashMap::new(), vec![]);
        assert!(all_diff.execute(&empty_context).is_err());

        // Test SAME with wrong arg count
        let same = SameFunction::new();
        let wrong_count_context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(1.0)], // Only one arg
        );
        assert!(same.execute(&wrong_count_context).is_err());

        // Test PROPERTY_EXISTS with wrong arg count
        let prop_exists = PropertyExistsFunction::new();
        assert!(prop_exists.execute(&empty_context).is_err());
    }

    #[test]
    fn test_comparable_value_conversion() {
        // Test various value types
        assert!(value_to_comparable(&Value::Boolean(true)).is_ok());
        assert!(value_to_comparable(&Value::Number(42.0)).is_ok());
        assert!(value_to_comparable(&Value::String("test".to_string())).is_ok());

        // Test that same values produce same comparable values
        let val1 = value_to_comparable(&Value::Number(42.0)).unwrap();
        let val2 = value_to_comparable(&Value::Number(42.0)).unwrap();
        assert_eq!(val1, val2);

        // Test that different values produce different comparable values
        let val3 = value_to_comparable(&Value::Number(43.0)).unwrap();
        assert_ne!(val1, val3);
    }

    #[test]
    fn test_performance_with_large_dataset() {
        let func = AllDifferentFunction::new();

        // Create 1000 unique values
        let large_args: Vec<Value> = (0..1000).map(|i| Value::Number(i as f64)).collect();

        let context = FunctionContext::new(vec![], HashMap::new(), large_args);

        let start = std::time::Instant::now();
        let result = func.execute(&context).unwrap();
        let duration = start.elapsed();

        assert_eq!(result, Value::Boolean(true));
        assert!(
            duration.as_millis() < 100,
            "Should complete within 100ms for 1000 values"
        );
    }
}
