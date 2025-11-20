// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Type mapping utilities between schema types (TypeSpec) and runtime values (Value)
//!
//! This module provides utilities to:
//! - Convert between TypeSpec and Value
//! - Validate that runtime values conform to schema types
//! - Coerce values to match expected schema types

use crate::storage::Value;
use crate::types::GqlType;
use crate::types::{TypeError, TypeResult};

/// Utility for mapping between schema types and runtime values
pub struct TypeMapping;

impl TypeMapping {
    /// Check if a runtime Value is compatible with a schema TypeSpec
    #[allow(dead_code)] // ROADMAP v0.4.0 - Type compatibility checking for index key validation
    pub fn is_value_compatible_with_type(value: &Value, expected_type: &GqlType) -> bool {
        match (value, expected_type) {
            // Null values are compatible with any nullable type
            (Value::Null, _) => true, // In ISO GQL, most types can be null

            // String types
            (Value::String(s), GqlType::String { max_length }) => match max_length {
                Some(max_len) => s.len() <= *max_len,
                None => true,
            },

            // Numeric types
            (
                Value::Number(_),
                GqlType::Integer
                | GqlType::BigInt
                | GqlType::SmallInt
                | GqlType::Int128
                | GqlType::Int256
                | GqlType::Double
                | GqlType::Real
                | GqlType::Float { .. },
            ) => true,
            (Value::Number(_), GqlType::Decimal { .. }) => true,

            // Boolean types
            (Value::Boolean(_), GqlType::Boolean) => true,

            // Temporal types
            (
                Value::DateTime(_),
                GqlType::ZonedDateTime { .. }
                | GqlType::LocalDateTime { .. }
                | GqlType::Timestamp { .. }
                | GqlType::Date,
            ) => true,
            (Value::TimeWindow(_), GqlType::Duration { .. }) => true,

            // Collection types
            (
                Value::Array(arr),
                GqlType::List {
                    element_type,
                    max_length,
                },
            ) => {
                // Check length constraint
                if let Some(max_len) = max_length {
                    if arr.len() > *max_len {
                        return false;
                    }
                }
                // Check all elements are compatible with element type
                arr.iter()
                    .all(|elem| Self::is_value_compatible_with_type(elem, element_type))
            }
            (Value::Vector(_), GqlType::List { element_type, .. }) => {
                // Vectors are compatible with lists of numeric types
                matches!(
                    element_type.as_ref(),
                    GqlType::Double | GqlType::Float { .. } | GqlType::Float32 | GqlType::Real
                )
            }

            _ => false,
        }
    }

    /// Convert a runtime Value to match a schema TypeSpec (with coercion if possible)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Type coercion for index key normalization
    pub fn coerce_value_to_type(value: &Value, target_type: &GqlType) -> TypeResult<Value> {
        // If already compatible, return as-is
        if Self::is_value_compatible_with_type(value, target_type) {
            return Ok(value.clone());
        }

        match (value, target_type) {
            // String to numeric coercions
            (Value::String(s), GqlType::Integer | GqlType::BigInt | GqlType::SmallInt) => s
                .parse::<i64>()
                .map(|i| Value::Number(i as f64))
                .map_err(|_| TypeError::InvalidCast(s.clone(), "Integer".to_string())),
            (Value::String(s), GqlType::Double | GqlType::Real | GqlType::Float { .. }) => s
                .parse::<f64>()
                .map(Value::Number)
                .map_err(|_| TypeError::InvalidCast(s.clone(), "Number".to_string())),
            (Value::String(s), GqlType::Boolean) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(Value::Boolean(true)),
                "false" | "0" | "no" => Ok(Value::Boolean(false)),
                _ => Err(TypeError::InvalidCast(s.clone(), "Boolean".to_string())),
            },

            // Numeric to string coercion
            (Value::Number(n), GqlType::String { .. }) => Ok(Value::String(n.to_string())),

            // Boolean to string coercion
            (Value::Boolean(b), GqlType::String { .. }) => Ok(Value::String(b.to_string())),

            // Numeric type widening
            (Value::Number(n), GqlType::Integer | GqlType::BigInt | GqlType::SmallInt) => {
                // Check if the number is actually an integer
                if n.fract() == 0.0 {
                    Ok(Value::Number(*n))
                } else {
                    Err(TypeError::InvalidCast(n.to_string(), "Integer".to_string()))
                }
            }

            // Vector to Array coercion
            (Value::Vector(vec), GqlType::List { element_type, .. }) => {
                match element_type.as_ref() {
                    GqlType::Double | GqlType::Float { .. } | GqlType::Float32 | GqlType::Real => {
                        let array_values: Vec<Value> =
                            vec.iter().map(|&f| Value::Number(f as f64)).collect();
                        Ok(Value::Array(array_values))
                    }
                    _ => Err(TypeError::InvalidCast(
                        "Vector".to_string(),
                        format!("List<{:?}>", element_type),
                    )),
                }
            }

            // Array to Vector coercion (if all elements are numeric)
            (Value::Array(arr), GqlType::List { element_type, .. }) => {
                match element_type.as_ref() {
                    GqlType::Double | GqlType::Float { .. } | GqlType::Float32 | GqlType::Real => {
                        let mut vector_values = Vec::new();
                        for elem in arr {
                            match elem {
                                Value::Number(n) => vector_values.push(*n as f32),
                                _ => {
                                    return Err(TypeError::InvalidCast(
                                        "Array".to_string(),
                                        "Vector".to_string(),
                                    ))
                                }
                            }
                        }
                        Ok(Value::Vector(vector_values))
                    }
                    _ => Ok(value.clone()), // Keep as array for non-numeric types
                }
            }

            _ => Err(TypeError::InvalidCast(
                format!("{:?}", value),
                format!("{:?}", target_type),
            )),
        }
    }

    /// Infer the most specific TypeSpec for a runtime Value
    #[allow(dead_code)] // ROADMAP v0.4.0 - Type inference for schemaless data ingestion and index key type detection
    pub fn infer_type_from_value(value: &Value) -> GqlType {
        match value {
            Value::String(s) => GqlType::String {
                max_length: Some(s.len().max(255)), // Reasonable default max length
            },
            Value::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    // It's an integer
                    if *n >= i32::MIN as f64 && *n <= i32::MAX as f64 {
                        GqlType::Integer
                    } else {
                        GqlType::BigInt
                    }
                } else {
                    GqlType::Double
                }
            }
            Value::Boolean(_) => GqlType::Boolean,
            Value::DateTime(_) => GqlType::ZonedDateTime { precision: None },
            Value::DateTimeWithFixedOffset(_) => GqlType::ZonedDateTime { precision: None },
            Value::DateTimeWithNamedTz(_, _) => GqlType::ZonedDateTime { precision: None },
            Value::TimeWindow(_) => GqlType::Duration { precision: None },
            Value::Array(arr) => {
                // Infer element type from first element (or default to String)
                let element_type = arr
                    .first()
                    .map(|elem| Self::infer_type_from_value(elem))
                    .unwrap_or(GqlType::String { max_length: None });

                GqlType::List {
                    element_type: Box::new(element_type),
                    max_length: Some(arr.len().max(1000)), // Reasonable default max length
                }
            }
            Value::Vector(_) => GqlType::List {
                element_type: Box::new(GqlType::Float32),
                max_length: None,
            },
            Value::Path(_) => GqlType::Path,
            Value::Null => GqlType::String { max_length: None }, // Default for null
            Value::List(list_items) => {
                if list_items.is_empty() {
                    // Empty list - default to string element type
                    GqlType::List {
                        element_type: Box::new(GqlType::String { max_length: None }),
                        max_length: None,
                    }
                } else {
                    // Infer from first element
                    let first_element_type = Self::infer_type_from_value(&list_items[0]);
                    GqlType::List {
                        element_type: Box::new(first_element_type),
                        max_length: None,
                    }
                }
            }
            Value::Node(_) => GqlType::String { max_length: None }, // Nodes are complex objects, use String for now
            Value::Edge(_) => GqlType::String { max_length: None }, // Edges are complex objects, use String for now
            Value::Temporal(_) => GqlType::String { max_length: None }, // Temporal values are complex, use String for now
        }
    }

    /// Validate that a collection of values all conform to their expected types
    #[allow(dead_code)] // ROADMAP v0.4.0 - Batch type validation for index insertions and schema enforcement
    pub fn validate_value_types(
        values: &[(String, Value)],
        expected_types: &[(String, GqlType)],
    ) -> TypeResult<()> {
        let type_map: std::collections::HashMap<_, _> = expected_types
            .iter()
            .map(|(name, typ)| (name.as_str(), typ))
            .collect();

        for (name, value) in values {
            if let Some(expected_type) = type_map.get(name.as_str()) {
                if !Self::is_value_compatible_with_type(value, expected_type) {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", expected_type),
                        actual: format!("{:?}", Self::infer_type_from_value(value)),
                    });
                }
            }
        }

        Ok(())
    }

    /// Create a default Value for a given TypeSpec (useful for initialization)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Default value generation for schema constraints and property initialization
    pub fn default_value_for_type(type_spec: &GqlType) -> Value {
        match type_spec {
            GqlType::Boolean => Value::Boolean(false),
            GqlType::String { .. } => Value::String(String::new()),
            GqlType::Bytes { .. } => Value::String(String::new()), // Represent as string for now
            GqlType::Integer
            | GqlType::BigInt
            | GqlType::SmallInt
            | GqlType::Int128
            | GqlType::Int256 => Value::Number(0.0),
            GqlType::Decimal { .. }
            | GqlType::Float { .. }
            | GqlType::Float32
            | GqlType::Real
            | GqlType::Double => Value::Number(0.0),
            GqlType::Date
            | GqlType::Time { .. }
            | GqlType::Timestamp { .. }
            | GqlType::ZonedTime { .. }
            | GqlType::ZonedDateTime { .. }
            | GqlType::LocalTime { .. }
            | GqlType::LocalDateTime { .. } => Value::DateTime(chrono::Utc::now()),
            GqlType::Duration { .. } => Value::TimeWindow(
                crate::storage::TimeWindow::new(chrono::Utc::now(), chrono::Utc::now()).unwrap(),
            ),
            GqlType::List { .. } => {
                // Create an empty array with appropriate element type
                Value::Array(vec![])
            }
            GqlType::Reference { .. } => Value::Null,
            GqlType::Path => Value::String("()".to_string()), // Empty path
            GqlType::Record => Value::String("{}".to_string()), // Empty record
            GqlType::Graph { .. } => Value::String("GRAPH()".to_string()), // Empty graph
            GqlType::BindingTable => Value::Array(vec![]),    // Empty binding table
            GqlType::Vector { .. } => Value::Vector(vec![]),  // Empty vector
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_type_compatibility() {
        let string_val = Value::String("hello".to_string());
        let string_type = GqlType::String {
            max_length: Some(10),
        };
        let string_type_short = GqlType::String {
            max_length: Some(3),
        };

        assert!(TypeMapping::is_value_compatible_with_type(
            &string_val,
            &string_type
        ));
        assert!(!TypeMapping::is_value_compatible_with_type(
            &string_val,
            &string_type_short
        ));

        let num_val = Value::Number(42.0);
        let int_type = GqlType::Integer;
        let double_type = GqlType::Double;

        assert!(TypeMapping::is_value_compatible_with_type(
            &num_val, &int_type
        ));
        assert!(TypeMapping::is_value_compatible_with_type(
            &num_val,
            &double_type
        ));
    }

    #[test]
    fn test_value_coercion() {
        let string_val = Value::String("123".to_string());
        let int_type = GqlType::Integer;

        let coerced = TypeMapping::coerce_value_to_type(&string_val, &int_type).unwrap();
        assert!(matches!(coerced, Value::Number(n) if n == 123.0));

        let bool_val = Value::Boolean(true);
        let string_type = GqlType::String { max_length: None };

        let coerced = TypeMapping::coerce_value_to_type(&bool_val, &string_type).unwrap();
        assert!(matches!(coerced, Value::String(s) if s == "true"));
    }

    #[test]
    fn test_type_inference() {
        let string_val = Value::String("hello".to_string());
        let inferred = TypeMapping::infer_type_from_value(&string_val);
        assert!(matches!(
            inferred,
            GqlType::String {
                max_length: Some(255)
            }
        ));

        let int_val = Value::Number(42.0);
        let inferred = TypeMapping::infer_type_from_value(&int_val);
        assert!(matches!(inferred, GqlType::Integer));

        let float_val = Value::Number(3.14);
        let inferred = TypeMapping::infer_type_from_value(&float_val);
        assert!(matches!(inferred, GqlType::Double));
    }
}
