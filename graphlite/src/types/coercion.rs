// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Type coercion for ISO GQL
//!
//! Implements implicit type coercion rules using TypeSpec

use crate::types::{GqlType, TypeError, TypeResult};

/// Type coercion engine
#[derive(Debug)]
pub struct TypeCoercion;

impl TypeCoercion {
    /// Try to coerce a value from one type to another implicitly
    #[allow(dead_code)] // Called from executor.rs but compiler's dead code analysis doesn't detect cross-module usage
    pub fn coerce(from: &GqlType, to: &GqlType) -> TypeResult<CoercionStrategy> {
        // No coercion needed if types are the same
        if from == to {
            return Ok(CoercionStrategy::None);
        }

        match (from, to) {
            // Numeric coercions - widening integer types
            (GqlType::SmallInt, GqlType::Integer | GqlType::BigInt | GqlType::Int128 | GqlType::Int256) => {
                Ok(CoercionStrategy::IntegerWidening)
            }
            (GqlType::Integer, GqlType::BigInt | GqlType::Int128 | GqlType::Int256) => {
                Ok(CoercionStrategy::IntegerWidening)
            }
            (GqlType::BigInt, GqlType::Int128 | GqlType::Int256) => {
                Ok(CoercionStrategy::IntegerWidening)
            }
            (GqlType::Int128, GqlType::Int256) => {
                Ok(CoercionStrategy::IntegerWidening)
            }

            // Integer to decimal
            (GqlType::SmallInt | GqlType::Integer | GqlType::BigInt | GqlType::Int128 | GqlType::Int256, 
             GqlType::Decimal { .. }) => {
                Ok(CoercionStrategy::IntegerToDecimal)
            }

            // Integer to float
            (GqlType::SmallInt | GqlType::Integer, 
             GqlType::Float { .. } | GqlType::Real | GqlType::Double) => {
                Ok(CoercionStrategy::IntegerToFloat)
            }

            // Float widening
            (GqlType::Float { .. } | GqlType::Real, GqlType::Double) => {
                Ok(CoercionStrategy::FloatWidening)
            }

            // String to other types
            (GqlType::String { .. }, GqlType::Integer | GqlType::Double | GqlType::Boolean) => {
                Ok(CoercionStrategy::StringToOther)
            }

            // Temporal coercions
            (GqlType::Date, GqlType::Timestamp { .. }) => {
                Ok(CoercionStrategy::DateToTimestamp)
            }

            // REF type coercions
            (GqlType::Reference { target_type: Some(ref_type) }, target) => {
                // Can coerce REF(T) to T (dereference)
                if Self::coerce(ref_type, target).is_ok() {
                    Ok(CoercionStrategy::ReferenceDeference)
                } else {
                    Err(TypeError::InvalidCast(
                        format!("{:?}", from),
                        format!("{:?}", to),
                    ))
                }
            }
            (source, GqlType::Reference { target_type: Some(ref_type) }) => {
                // Can coerce T to REF(T) (create reference)
                if Self::coerce(source, ref_type).is_ok() {
                    Ok(CoercionStrategy::CreateReference)
                } else {
                    Err(TypeError::InvalidCast(
                        format!("{:?}", from),
                        format!("{:?}", to),
                    ))
                }
            }

            _ => Err(TypeError::InvalidCast(
                format!("{:?}", from),
                format!("{:?}", to),
            )),
        }
    }

    /// Find common type for two types (for type promotion)
    #[allow(dead_code)] // Used as fallback in executor.rs apply_coercion() but compiler doesn't detect cross-module usage
    pub fn find_common_type(type1: &GqlType, type2: &GqlType) -> Option<GqlType> {
        // Same type
        if type1 == type2 {
            return Some(type1.clone());
        }

        match (type1, type2) {
            // Numeric types - promote to wider type
            (GqlType::SmallInt, GqlType::Integer) | (GqlType::Integer, GqlType::SmallInt) => {
                Some(GqlType::Integer)
            }
            (GqlType::SmallInt, GqlType::BigInt) | (GqlType::BigInt, GqlType::SmallInt) => {
                Some(GqlType::BigInt)
            }
            (GqlType::Integer, GqlType::BigInt) | (GqlType::BigInt, GqlType::Integer) => {
                Some(GqlType::BigInt)
            }
            (_, GqlType::Double) | (GqlType::Double, _) => {
                Some(GqlType::Double)
            }

            // String types - use unbounded if different
            (GqlType::String { max_length: Some(l1) }, GqlType::String { max_length: Some(l2) }) => {
                Some(GqlType::String { max_length: Some(*l1.max(l2)) })
            }
            (GqlType::String { .. }, GqlType::String { .. }) => {
                Some(GqlType::String { max_length: None })
            }

            _ => None,
        }
    }
}

/// Strategy for type coercion
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Used in executor.rs via TypeCoercion::coerce() but compiler's dead code analysis doesn't detect cross-module enum construction
pub enum CoercionStrategy {
    /// No coercion needed
    None,

    // Numeric coercions
    /// Widen integer type
    IntegerWidening,
    /// Convert integer to decimal
    IntegerToDecimal,
    /// Convert integer to float
    IntegerToFloat,
    /// Widen float type
    FloatWidening,

    // String conversions
    /// String to other type
    StringToOther,

    // Temporal coercions
    /// Convert date to timestamp
    DateToTimestamp,

    // REF type coercions
    /// Dereference REF(T) to T
    ReferenceDeference,
    /// Create REF(T) from T
    CreateReference,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_coercion() {
        let small = GqlType::SmallInt;
        let big = GqlType::BigInt;
        
        let result = TypeCoercion::coerce(&small, &big).unwrap();
        assert_eq!(result, CoercionStrategy::IntegerWidening);
        
        assert!(TypeCoercion::coerce(&big, &small).is_err());
    }

    #[test]
    fn test_find_common_type() {
        let int = GqlType::Integer;
        let bigint = GqlType::BigInt;
        
        let common = TypeCoercion::find_common_type(&int, &bigint).unwrap();
        assert_eq!(common, GqlType::BigInt);
    }
}
