// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Type validation for ISO GQL
//!
//! Implements type checking and validation rules using TypeSpec

use crate::types::{GqlType, GraphTypeSpec, TypeError, TypeResult};

/// Runtime type constraints for validation
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Runtime type constraints for data validation
pub struct TypeConstraints {
    pub allow_null: bool,
    pub max_collection_size: Option<usize>,
    pub max_numeric_precision: Option<u8>,
}

impl Default for TypeConstraints {
    fn default() -> Self {
        Self {
            allow_null: true,
            max_collection_size: None,
            max_numeric_precision: None,
        }
    }
}

/// Type validator
#[derive(Debug)]
pub struct TypeValidator;

impl TypeValidator {
    /// Check if two types are compatible (for assignment/comparison)
    pub fn are_compatible(left: &GqlType, right: &GqlType) -> bool {
        match (left, right) {
            // Same type is always compatible
            _ if left == right => true,

            // Numeric type compatibility - can implicit cast between numeric types
            (
                GqlType::SmallInt,
                GqlType::Integer | GqlType::BigInt | GqlType::Int128 | GqlType::Int256,
            ) => true,
            (GqlType::Integer, GqlType::BigInt | GqlType::Int128 | GqlType::Int256) => true,
            (GqlType::BigInt, GqlType::Int128 | GqlType::Int256) => true,
            (GqlType::Int128, GqlType::Int256) => true,

            // Integer to decimal
            (
                GqlType::SmallInt
                | GqlType::Integer
                | GqlType::BigInt
                | GqlType::Int128
                | GqlType::Int256,
                GqlType::Decimal { .. },
            ) => true,

            // Integer to float
            (
                GqlType::SmallInt | GqlType::Integer,
                GqlType::Float { .. } | GqlType::Real | GqlType::Double,
            ) => true,

            // Float widening
            (GqlType::Float { .. }, GqlType::Double) => true,
            (GqlType::Real, GqlType::Double) => true,

            // String types with different max lengths
            (
                GqlType::String {
                    max_length: Some(l_max),
                },
                GqlType::String {
                    max_length: Some(r_max),
                },
            ) => l_max <= r_max,
            (
                GqlType::String {
                    max_length: Some(_),
                },
                GqlType::String { max_length: None },
            ) => true, // Bounded to unbounded is ok
            (
                GqlType::String { max_length: None },
                GqlType::String {
                    max_length: Some(_),
                },
            ) => false, // Unbounded to bounded is not ok
            (GqlType::String { max_length: None }, GqlType::String { max_length: None }) => true,

            // List type compatibility
            (
                GqlType::List {
                    element_type: l_elem,
                    max_length: l_max,
                },
                GqlType::List {
                    element_type: r_elem,
                    max_length: r_max,
                },
            ) => {
                // Check element type compatibility first
                if !Self::are_compatible(l_elem, r_elem) {
                    return false;
                }
                // Check max_length constraints (similar to strings)
                match (l_max, r_max) {
                    (Some(l), Some(r)) => l <= r, // Left max must be <= right max
                    (Some(_), None) => true,      // Bounded to unbounded is ok
                    (None, Some(_)) => false,     // Unbounded to bounded is not ok
                    (None, None) => true,         // Both unbounded is ok
                }
            }

            // Reference type compatibility
            (
                GqlType::Reference {
                    target_type: Some(l),
                },
                GqlType::Reference {
                    target_type: Some(r),
                },
            ) => Self::are_compatible(l, r),

            // Temporal type compatibility
            (
                l @ (GqlType::Date
                | GqlType::Time { .. }
                | GqlType::Timestamp { .. }
                | GqlType::ZonedTime { .. }
                | GqlType::ZonedDateTime { .. }
                | GqlType::LocalTime { .. }
                | GqlType::LocalDateTime { .. }),
                r @ (GqlType::Date
                | GqlType::Time { .. }
                | GqlType::Timestamp { .. }
                | GqlType::ZonedTime { .. }
                | GqlType::ZonedDateTime { .. }
                | GqlType::LocalTime { .. }
                | GqlType::LocalDateTime { .. }),
            ) => Self::are_temporal_compatible(l, r),

            _ => false,
        }
    }

    /// Check if temporal types are compatible
    fn are_temporal_compatible(left: &GqlType, right: &GqlType) -> bool {
        match (left, right) {
            // Same temporal type
            _ if std::mem::discriminant(left) == std::mem::discriminant(right) => true,

            // Date to timestamp conversions
            (
                GqlType::Date,
                GqlType::Timestamp { .. }
                | GqlType::ZonedDateTime { .. }
                | GqlType::LocalDateTime { .. },
            ) => true,

            // Time conversions
            (GqlType::Time { .. }, GqlType::ZonedTime { .. } | GqlType::LocalTime { .. }) => true,
            (GqlType::ZonedTime { .. }, GqlType::Time { .. } | GqlType::LocalTime { .. }) => true,
            (GqlType::LocalTime { .. }, GqlType::Time { .. }) => true,

            _ => false,
        }
    }

    /// Validate list size constraints
    #[allow(dead_code)] // ROADMAP v0.4.0 - List size validation for schema constraints
    pub fn validate_list_size(list_type: &GqlType, size: usize) -> bool {
        match list_type {
            GqlType::List {
                max_length: Some(max),
                ..
            } => size <= *max,
            GqlType::List {
                max_length: None, ..
            } => true,
            _ => false,
        }
    }

    /// Validate that a value type matches expected type
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_value_type(value_type: &GqlType, expected_type: &GqlType) -> TypeResult<()> {
        if Self::are_compatible(value_type, expected_type) {
            Ok(())
        } else {
            Err(TypeError::TypeMismatch {
                expected: format!("{}", expected_type),
                actual: format!("{}", value_type),
            })
        }
    }

    /// Validate function argument types
    pub fn validate_function_args(
        function_name: &str,
        arg_types: &[GqlType],
        expected_types: &[GqlType],
        variadic: bool,
    ) -> TypeResult<()> {
        if !variadic && arg_types.len() != expected_types.len() {
            return Err(TypeError::InvalidTypeSpecification(format!(
                "Function {} expects {} arguments, got {}",
                function_name,
                expected_types.len(),
                arg_types.len()
            )));
        }

        let check_count = if variadic {
            expected_types.len().min(arg_types.len())
        } else {
            expected_types.len()
        };

        for i in 0..check_count {
            if !Self::are_compatible(&arg_types[i], &expected_types[i]) {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{}", expected_types[i]),
                    actual: format!("{}", arg_types[i]),
                });
            }
        }

        Ok(())
    }

    /// Validate graph schema compliance (simplified)
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_graph_element(
        _graph_type: &GraphTypeSpec,
        element_type: &GqlType,
    ) -> TypeResult<()> {
        // Simplified validation - just check if it's a graph-related type
        match element_type {
            GqlType::Graph { .. } => Ok(()),
            _ => Err(TypeError::GraphSchemaViolation(
                "Element is not a graph type".to_string(),
            )),
        }
    }

    /// Validate nullability constraints
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_nullability(_value_type: &GqlType, allow_null: bool) -> TypeResult<()> {
        // Note: TypeSpec doesn't have a Null variant - null handling is done at the Value level
        if !allow_null {
            // In a full implementation, we'd check if the value is null at runtime
            // For now, we'll assume all types can be nullable
            Err(TypeError::NullabilityViolation(
                "Null value not allowed in non-nullable context".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// Validate collection element types (simplified)
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_collection_elements(
        list_type: &GqlType,
        element_types: &[GqlType],
    ) -> TypeResult<()> {
        match list_type {
            GqlType::List {
                element_type,
                max_length,
            } => {
                // Check size constraint
                if let Some(max) = max_length {
                    if element_types.len() > *max {
                        return Err(TypeError::CollectionTypeMismatch(format!(
                            "List size {} exceeds maximum {}",
                            element_types.len(),
                            max
                        )));
                    }
                }

                // Check element types
                for element_type_actual in element_types {
                    if !Self::are_compatible(element_type_actual, element_type) {
                        return Err(TypeError::CollectionTypeMismatch(format!(
                            "Element type {:?} not compatible with list element type {:?}",
                            element_type_actual, element_type
                        )));
                    }
                }

                Ok(())
            }
            _ => Err(TypeError::CollectionTypeMismatch(
                "Not a list type".to_string(),
            )),
        }
    }

    /// Enhanced validation for deeply nested complex types
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_nested_type_structure(
        value_type: &GqlType,
        expected_type: &GqlType,
        max_depth: usize,
    ) -> TypeResult<()> {
        Self::validate_nested_type_structure_impl(value_type, expected_type, max_depth, 0)
    }

    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    fn validate_nested_type_structure_impl(
        value_type: &GqlType,
        expected_type: &GqlType,
        max_depth: usize,
        current_depth: usize,
    ) -> TypeResult<()> {
        // Prevent infinite recursion
        if current_depth > max_depth {
            return Err(TypeError::InvalidTypeSpecification(
                "Type structure too deeply nested".to_string(),
            ));
        }

        match (value_type, expected_type) {
            // Direct type match
            _ if Self::are_compatible(value_type, expected_type) => Ok(()),

            // Nested list validation
            (
                GqlType::List {
                    element_type: value_elem,
                    max_length: value_max,
                },
                GqlType::List {
                    element_type: expected_elem,
                    max_length: expected_max,
                },
            ) => {
                // Validate max length constraint
                match (value_max, expected_max) {
                    (Some(v_max), Some(e_max)) if v_max > e_max => {
                        return Err(TypeError::CollectionTypeMismatch(format!(
                            "List max length {} exceeds expected maximum {}",
                            v_max, e_max
                        )));
                    }
                    _ => {}
                }

                // Recursively validate element types
                Self::validate_nested_type_structure_impl(
                    value_elem,
                    expected_elem,
                    max_depth,
                    current_depth + 1,
                )
            }

            // Nested reference validation
            (
                GqlType::Reference {
                    target_type: Some(value_target),
                },
                GqlType::Reference {
                    target_type: Some(expected_target),
                },
            ) => Self::validate_nested_type_structure_impl(
                value_target,
                expected_target,
                max_depth,
                current_depth + 1,
            ),

            // Reference dereferencing validation
            (
                GqlType::Reference {
                    target_type: Some(value_target),
                },
                expected_type,
            ) => Self::validate_nested_type_structure_impl(
                value_target,
                expected_type,
                max_depth,
                current_depth + 1,
            ),

            _ => Err(TypeError::TypeMismatch {
                expected: format!("{}", expected_type),
                actual: format!("{}", value_type),
            }),
        }
    }

    /// Validate reference target type resolution
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_reference_target(
        ref_type: &GqlType,
        target_value_type: &GqlType,
    ) -> TypeResult<()> {
        match ref_type {
            GqlType::Reference {
                target_type: Some(expected_target),
            } => Self::validate_value_type(target_value_type, expected_target),
            GqlType::Reference { target_type: None } => {
                // Untyped reference - accepts any type
                Ok(())
            }
            _ => Err(TypeError::InvalidTypeSpecification(
                "Not a reference type".to_string(),
            )),
        }
    }

    /// Validate complex type constraints at runtime
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    pub fn validate_runtime_constraints(
        value_type: &GqlType,
        constraints: &TypeConstraints,
    ) -> TypeResult<()> {
        // Check nullability
        if !constraints.allow_null && Self::is_nullable_value(value_type) {
            return Err(TypeError::NullabilityViolation(
                "Null value not allowed".to_string(),
            ));
        }

        // Check size constraints for collections
        if let Some(max_size) = constraints.max_collection_size {
            match value_type {
                GqlType::List {
                    max_length: Some(list_max),
                    ..
                } => {
                    if *list_max > max_size {
                        return Err(TypeError::CollectionTypeMismatch(format!(
                            "List max size {} exceeds constraint {}",
                            list_max, max_size
                        )));
                    }
                }
                GqlType::String {
                    max_length: Some(str_max),
                } => {
                    if *str_max > max_size {
                        return Err(TypeError::InvalidTypeSpecification(format!(
                            "String max length {} exceeds constraint {}",
                            str_max, max_size
                        )));
                    }
                }
                _ => {}
            }
        }

        // Check precision constraints for numeric types
        if let Some(max_precision) = constraints.max_numeric_precision {
            match value_type {
                GqlType::Decimal {
                    precision: Some(p), ..
                } if *p > max_precision => {
                    return Err(TypeError::NumericOverflow(format!(
                        "Decimal precision {} exceeds maximum {}",
                        p, max_precision
                    )));
                }
                GqlType::Float { precision: Some(p) } if *p > max_precision => {
                    return Err(TypeError::NumericOverflow(format!(
                        "Float precision {} exceeds maximum {}",
                        p, max_precision
                    )));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Check if a type represents a nullable value
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type validation for static analysis (see ROADMAP.md §7)
    fn is_nullable_value(value_type: &GqlType) -> bool {
        matches!(value_type, GqlType::Reference { target_type: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_compatibility() {
        assert!(TypeValidator::are_compatible(
            &GqlType::Boolean,
            &GqlType::Boolean
        ));
        assert!(TypeValidator::are_compatible(
            &GqlType::Integer,
            &GqlType::BigInt
        ));
        assert!(!TypeValidator::are_compatible(
            &GqlType::BigInt,
            &GqlType::Integer
        ));
    }

    #[test]
    fn test_string_compatibility() {
        let unbounded = GqlType::String { max_length: None };
        let bounded_10 = GqlType::String {
            max_length: Some(10),
        };
        let bounded_20 = GqlType::String {
            max_length: Some(20),
        };

        assert!(TypeValidator::are_compatible(&bounded_10, &unbounded));
        assert!(!TypeValidator::are_compatible(&unbounded, &bounded_10));
        assert!(TypeValidator::are_compatible(&bounded_10, &bounded_20));
        assert!(!TypeValidator::are_compatible(&bounded_20, &bounded_10));
    }

    #[test]
    fn test_list_validation() {
        let list_type = GqlType::List {
            element_type: Box::new(GqlType::Integer),
            max_length: Some(3),
        };

        let elements = vec![GqlType::Integer, GqlType::SmallInt];
        assert!(TypeValidator::validate_collection_elements(&list_type, &elements).is_ok());

        let too_many = vec![GqlType::Integer; 4];
        assert!(TypeValidator::validate_collection_elements(&list_type, &too_many).is_err());
    }
}
