// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Type casting for ISO GQL CAST operations
//!
//! Implements explicit type casting rules as per ISO/IEC 39075 using TypeSpec

use crate::types::{GqlType, TypeError, TypeResult};

/// Type caster for explicit CAST operations
#[derive(Debug)]
pub struct TypeCaster;

impl TypeCaster {
    /// Check if a type can be explicitly cast to another
    #[allow(dead_code)] // ISO GQL - CAST expression validation for type conversions
    pub fn can_cast(from: &GqlType, to: &GqlType) -> bool {
        // Same type can always be cast
        if from == to {
            return true;
        }

        match (from, to) {
            // Boolean casts
            (GqlType::Boolean, GqlType::String { .. }) => true,
            (GqlType::Boolean, GqlType::Integer | GqlType::SmallInt | GqlType::BigInt) => true,
            (GqlType::String { .. }, GqlType::Boolean) => true,
            (GqlType::Integer | GqlType::SmallInt | GqlType::BigInt, GqlType::Boolean) => true,

            // Numeric casts (all numeric types can cast to each other)
            (from_t, to_t) if from_t.is_numeric() && to_t.is_numeric() => true,
            (GqlType::String { .. }, to_t) if to_t.is_numeric() => true,

            // Temporal casts
            (from_t, to_t) if from_t.is_temporal() && to_t.is_temporal() => {
                Self::can_cast_temporal(from_t, to_t)
            }
            (GqlType::String { .. }, to_t) if to_t.is_temporal() => true,

            // Duration casts
            (GqlType::String { .. }, GqlType::Duration { .. }) => true,

            // String casts (most things can cast to string) - put this last to avoid unreachable patterns
            (_, GqlType::String { .. }) => !matches!(from, GqlType::Graph { .. } | GqlType::BindingTable),

            // List casts (only if element types can be cast)
            (GqlType::List { element_type: from_elem, .. }, GqlType::List { element_type: to_elem, .. }) => {
                Self::can_cast(from_elem, to_elem)
            }

            _ => false,
        }
    }

    /// Check if temporal types can be cast
    fn can_cast_temporal(from: &GqlType, to: &GqlType) -> bool {
        match (from, to) {
            // Same temporal type
            _ if std::mem::discriminant(from) == std::mem::discriminant(to) => true,

            // Date conversions
            (GqlType::Date, GqlType::Timestamp { .. } | GqlType::ZonedDateTime { .. } | GqlType::LocalDateTime { .. }) => true,

            // Time conversions
            (GqlType::Time { .. }, GqlType::ZonedTime { .. } | GqlType::LocalTime { .. }) => true,
            (GqlType::ZonedTime { .. }, GqlType::Time { .. } | GqlType::LocalTime { .. }) => true,
            (GqlType::LocalTime { .. }, GqlType::Time { .. }) => true,

            // Timestamp/DateTime conversions
            (GqlType::Timestamp { .. }, GqlType::ZonedDateTime { .. } | GqlType::LocalDateTime { .. } | GqlType::Date) => true,
            (GqlType::ZonedDateTime { .. }, GqlType::LocalDateTime { .. } | GqlType::Date) => true,
            (GqlType::LocalDateTime { .. }, GqlType::Date) => true,

            _ => false,
        }
    }

    /// Perform explicit type cast
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type casting for static analysis (see ROADMAP.md §7)
    pub fn cast(from: &GqlType, to: &GqlType) -> TypeResult<CastOperation> {
        if !Self::can_cast(from, to) {
            return Err(TypeError::InvalidCast(
                format!("{:?}", from),
                format!("{:?}", to),
            ));
        }

        // Same type - no operation needed
        if from == to {
            return Ok(CastOperation::Identity);
        }

        match (from, to) {
            // Boolean casts
            (GqlType::Boolean, GqlType::String { .. }) => Ok(CastOperation::BooleanToString),
            (GqlType::Boolean, to_t) if to_t.is_numeric() => Ok(CastOperation::BooleanToNumeric),
            (GqlType::String { .. }, GqlType::Boolean) => Ok(CastOperation::StringToBoolean),
            (from_t, GqlType::Boolean) if from_t.is_numeric() => Ok(CastOperation::NumericToBoolean),

            // Numeric casts
            (from_t, to_t) if from_t.is_numeric() && to_t.is_numeric() => {
                Ok(CastOperation::NumericCast)
            }
            (from_t, GqlType::String { .. }) if from_t.is_numeric() => {
                Ok(CastOperation::NumericToString)
            }
            (GqlType::String { .. }, to_t) if to_t.is_numeric() => {
                Ok(CastOperation::StringToNumeric)
            }

            // Temporal casts
            (from_t, to_t) if from_t.is_temporal() && to_t.is_temporal() => {
                Ok(CastOperation::TemporalCast)
            }
            (GqlType::String { .. }, to_t) if to_t.is_temporal() => {
                Ok(CastOperation::StringToTemporal)
            }
            (from_t, GqlType::String { .. }) if from_t.is_temporal() => {
                Ok(CastOperation::TemporalToString)
            }

            _ => Ok(CastOperation::Custom),
        }
    }

    /// Check if a value satisfies IS TYPED predicate
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type casting for static analysis (see ROADMAP.md §7)
    pub fn is_typed(value_type: &GqlType, check_type: &GqlType) -> bool {
        match (value_type, check_type) {
            // Exact type match
            _ if value_type == check_type => true,

            // Numeric type hierarchy - larger types satisfy smaller type checks
            (GqlType::Integer, GqlType::SmallInt) => true,
            (GqlType::BigInt, GqlType::SmallInt | GqlType::Integer) => true,
            (GqlType::Int128, GqlType::SmallInt | GqlType::Integer | GqlType::BigInt) => true,
            (GqlType::Int256, GqlType::SmallInt | GqlType::Integer | GqlType::BigInt | GqlType::Int128) => true,
            (GqlType::Double, GqlType::Float { .. }) => true,

            _ => false,
        }
    }
}

/// Cast operation descriptor
#[derive(Debug, Clone)]
#[allow(dead_code)] // ISO GQL - Cast operation descriptors for execution
pub enum CastOperation {
    /// No operation needed (same type)
    Identity,
    
    // Boolean casts
    BooleanToString,
    BooleanToNumeric,
    StringToBoolean,
    NumericToBoolean,
    
    // Numeric casts
    NumericCast,
    NumericToString,
    StringToNumeric,
    
    // Temporal casts
    TemporalCast,
    StringToTemporal,
    TemporalToString,
    
    // Custom cast
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_cast() {
        // Boolean casts
        assert!(TypeCaster::can_cast(&GqlType::Boolean, &GqlType::String { max_length: None }));
        assert!(TypeCaster::can_cast(&GqlType::String { max_length: None }, &GqlType::Boolean));
        
        // Numeric casts
        let int = GqlType::Integer;
        let double = GqlType::Double;
        assert!(TypeCaster::can_cast(&int, &double));
        assert!(TypeCaster::can_cast(&double, &int));
        
        // Temporal casts
        let date = GqlType::Date;
        let timestamp = GqlType::Timestamp { precision: None, with_timezone: false };
        assert!(TypeCaster::can_cast(&date, &timestamp));
    }

    #[test]
    fn test_is_typed() {
        let small = GqlType::SmallInt;
        let int = GqlType::Integer;
        let big = GqlType::BigInt;
        
        // Exact match
        assert!(TypeCaster::is_typed(&int, &int));
        
        // Hierarchy - larger types satisfy smaller type checks
        assert!(!TypeCaster::is_typed(&small, &int));
        assert!(TypeCaster::is_typed(&int, &small));
        assert!(TypeCaster::is_typed(&big, &int));
    }

    #[test]
    fn test_cast_operation() {
        let bool_type = GqlType::Boolean;
        let string_type = GqlType::String { max_length: None };
        
        let cast_op = TypeCaster::cast(&bool_type, &string_type).unwrap();
        assert!(matches!(cast_op, CastOperation::BooleanToString));
        
        let int_type = GqlType::Integer;
        let double_type = GqlType::Double;
        
        let cast_op = TypeCaster::cast(&int_type, &double_type).unwrap();
        assert!(matches!(cast_op, CastOperation::NumericCast));
    }
}
