// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Type inference engine for ISO GQL
//!
//! Implements type inference for expressions and operations using TypeSpec

use crate::types::{GqlType, TypeError, TypeResult};
use std::collections::HashMap;

/// Type inference engine
#[derive(Debug)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for query optimization and static analysis
pub struct TypeInferenceEngine {
    /// Variable type bindings
    #[allow(dead_code)] // ROADMAP v0.5.0 - Variable type tracking for static analysis
    variable_types: HashMap<String, GqlType>,
    /// Function signatures
    #[allow(dead_code)] // ROADMAP v0.5.0 - Builtin function signatures for type checking
    function_signatures: HashMap<String, FunctionSignature>,
}

/// Function signature for type inference
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Function signature tracking for type inference
pub struct FunctionSignature {
    pub param_types: Vec<GqlType>,
    pub return_type: GqlType,
    pub variadic: bool,
}

impl TypeInferenceEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            variable_types: HashMap::new(),
            function_signatures: HashMap::new(),
        };
        engine.register_builtin_functions();
        engine
    }

    /// Register built-in function signatures
    fn register_builtin_functions(&mut self) {
        // Aggregate functions
        self.register_function(
            "COUNT",
            vec![],
            GqlType::BigInt,
            true, // variadic
        );
        self.register_function("SUM", vec![GqlType::Integer], GqlType::BigInt, false);
        self.register_function("AVG", vec![GqlType::Integer], GqlType::Double, false);
        self.register_function(
            "MIN",
            vec![],           // Will be inferred from arguments
            GqlType::Integer, // Placeholder
            true,             // variadic
        );
        self.register_function(
            "MAX",
            vec![],           // Will be inferred from arguments
            GqlType::Integer, // Placeholder
            true,             // variadic
        );

        // String functions
        self.register_function(
            "UPPER",
            vec![GqlType::String { max_length: None }],
            GqlType::String { max_length: None },
            false,
        );
        self.register_function(
            "LOWER",
            vec![GqlType::String { max_length: None }],
            GqlType::String { max_length: None },
            false,
        );
        self.register_function(
            "LENGTH",
            vec![GqlType::String { max_length: None }],
            GqlType::Integer,
            false,
        );
        self.register_function(
            "SUBSTRING",
            vec![
                GqlType::String { max_length: None },
                GqlType::Integer,
                GqlType::Integer,
            ],
            GqlType::String { max_length: None },
            false,
        );

        // Temporal functions
        self.register_function("CURRENT_DATE", vec![], GqlType::Date, false);
        self.register_function(
            "CURRENT_TIME",
            vec![],
            GqlType::Time {
                precision: None,
                with_timezone: true,
            },
            false,
        );
        self.register_function(
            "CURRENT_TIMESTAMP",
            vec![],
            GqlType::Timestamp {
                precision: None,
                with_timezone: true,
            },
            false,
        );

        // Type check functions
        self.register_function(
            "IS_NULL",
            vec![],
            GqlType::Boolean,
            true, // variadic - accepts any type
        );
        self.register_function(
            "IS_BOOLEAN",
            vec![],
            GqlType::Boolean,
            true, // variadic - accepts any type
        );
        self.register_function(
            "IS_STRING",
            vec![],
            GqlType::Boolean,
            true, // variadic - accepts any type
        );
        self.register_function(
            "IS_NUMERIC",
            vec![],
            GqlType::Boolean,
            true, // variadic - accepts any type
        );
    }

    /// Register a function signature
    pub fn register_function(
        &mut self,
        name: &str,
        param_types: Vec<GqlType>,
        return_type: GqlType,
        variadic: bool,
    ) {
        self.function_signatures.insert(
            name.to_uppercase(),
            FunctionSignature {
                param_types,
                return_type,
                variadic,
            },
        );
    }

    /// Bind a variable to a type
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn bind_variable(&mut self, name: String, var_type: GqlType) {
        self.variable_types.insert(name, var_type);
    }

    /// Get the type of a variable
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn get_variable_type(&self, name: &str) -> Option<&GqlType> {
        self.variable_types.get(name)
    }

    /// Infer type from a literal value
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn infer_literal_type(&self, literal: &str) -> GqlType {
        // Boolean literals
        if literal.eq_ignore_ascii_case("true") || literal.eq_ignore_ascii_case("false") {
            return GqlType::Boolean;
        }

        // String literal (quoted)
        if (literal.starts_with('\'') && literal.ends_with('\''))
            || (literal.starts_with('"') && literal.ends_with('"'))
        {
            return GqlType::String { max_length: None };
        }

        // Numeric literals
        if literal.parse::<i64>().is_ok() {
            return GqlType::Integer;
        }
        if literal.parse::<f64>().is_ok() {
            return GqlType::Double;
        }

        // Date/Time literals (prefixed)
        if literal.starts_with("DATE ") {
            return GqlType::Date;
        }
        if literal.starts_with("TIME ") {
            return GqlType::Time {
                precision: None,
                with_timezone: false,
            };
        }
        if literal.starts_with("TIMESTAMP ") {
            return GqlType::Timestamp {
                precision: None,
                with_timezone: false,
            };
        }
        if literal.starts_with("DURATION ") {
            return GqlType::Duration { precision: None };
        }

        // Default to integer for now (should be unknown type)
        GqlType::Integer
    }

    /// Infer type of a binary operation
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn infer_binary_op_type(
        &self,
        op: &str,
        left_type: &GqlType,
        right_type: &GqlType,
    ) -> TypeResult<GqlType> {
        match op {
            // Arithmetic operators
            "+" | "-" | "*" | "/" | "%" => self.infer_arithmetic_op_type(op, left_type, right_type),

            // Comparison operators
            "=" | "<>" | "!=" | "<" | ">" | "<=" | ">=" => {
                self.infer_comparison_op_type(left_type, right_type)
            }

            // Logical operators
            "AND" | "OR" | "XOR" => self.infer_logical_op_type(left_type, right_type),

            // String concatenation
            "||" => self.infer_string_concat_type(left_type, right_type),

            // IN operator
            "IN" => self.infer_in_op_type(left_type, right_type),

            _ => Err(TypeError::UnknownType(format!("Unknown operator: {}", op))),
        }
    }

    /// Infer type of arithmetic operation
    fn infer_arithmetic_op_type(
        &self,
        op: &str,
        left_type: &GqlType,
        right_type: &GqlType,
    ) -> TypeResult<GqlType> {
        match (left_type, right_type) {
            // Numeric arithmetic
            (left, right) if left.is_numeric() && right.is_numeric() => {
                Ok(self.promote_numeric_types(left, right))
            }

            // Duration arithmetic
            (GqlType::Duration { .. }, GqlType::Duration { .. }) if op == "+" || op == "-" => {
                Ok(GqlType::Duration { precision: None })
            }

            // Temporal + Duration
            (left, GqlType::Duration { .. }) if left.is_temporal() && (op == "+" || op == "-") => {
                Ok(left.clone()) // Result is same temporal type
            }
            (GqlType::Duration { .. }, right) if right.is_temporal() && op == "+" => {
                Ok(right.clone()) // Result is same temporal type
            }

            // Temporal - Temporal = Duration
            (left, right) if left.is_temporal() && right.is_temporal() && op == "-" => {
                Ok(GqlType::Duration { precision: None })
            }

            _ => Err(TypeError::IncompatibleTypes(
                format!("{}", left_type),
                format!("{}", right_type),
            )),
        }
    }

    /// Infer type of comparison operation
    fn infer_comparison_op_type(
        &self,
        left_type: &GqlType,
        right_type: &GqlType,
    ) -> TypeResult<GqlType> {
        // Comparisons always return boolean
        if self.are_comparable(left_type, right_type) {
            Ok(GqlType::Boolean)
        } else {
            Err(TypeError::IncompatibleTypes(
                format!("{}", left_type),
                format!("{}", right_type),
            ))
        }
    }

    /// Infer type of logical operation
    fn infer_logical_op_type(
        &self,
        left_type: &GqlType,
        right_type: &GqlType,
    ) -> TypeResult<GqlType> {
        match (left_type, right_type) {
            (GqlType::Boolean, GqlType::Boolean) => Ok(GqlType::Boolean),
            _ => Err(TypeError::TypeMismatch {
                expected: "BOOLEAN".to_string(),
                actual: format!("{} and {}", left_type, right_type),
            }),
        }
    }

    /// Infer type of string concatenation
    fn infer_string_concat_type(
        &self,
        left_type: &GqlType,
        right_type: &GqlType,
    ) -> TypeResult<GqlType> {
        match (left_type, right_type) {
            (GqlType::String { .. }, GqlType::String { .. }) => {
                Ok(GqlType::String { max_length: None })
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "STRING".to_string(),
                actual: format!("{} and {}", left_type, right_type),
            }),
        }
    }

    /// Infer type of IN operation
    fn infer_in_op_type(&self, left_type: &GqlType, right_type: &GqlType) -> TypeResult<GqlType> {
        match right_type {
            GqlType::List { element_type, .. } => {
                if self.are_comparable(left_type, element_type) {
                    Ok(GqlType::Boolean)
                } else {
                    Err(TypeError::CollectionTypeMismatch(format!(
                        "Cannot check if {} is in list of {}",
                        left_type, element_type
                    )))
                }
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "LIST".to_string(),
                actual: format!("{}", right_type),
            }),
        }
    }

    /// Infer type of function call
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn infer_function_type(
        &self,
        function_name: &str,
        arg_types: &[GqlType],
    ) -> TypeResult<GqlType> {
        let name = function_name.to_uppercase();

        if let Some(signature) = self.function_signatures.get(&name) {
            // Check argument count
            if !signature.variadic && arg_types.len() != signature.param_types.len() {
                return Err(TypeError::InvalidTypeSpecification(format!(
                    "Function {} expects {} arguments, got {}",
                    function_name,
                    signature.param_types.len(),
                    arg_types.len()
                )));
            }

            // For aggregate functions with polymorphic return type, infer from input
            if signature.return_type == GqlType::Double {
                // Use Double as polymorphic marker
                if (name == "MIN" || name == "MAX") && !arg_types.is_empty() {
                    return Ok(arg_types[0].clone());
                }
            }

            Ok(signature.return_type.clone())
        } else {
            // Unknown function - return String as default
            Ok(GqlType::String { max_length: None })
        }
    }

    /// Check if two types are comparable
    fn are_comparable(&self, left: &GqlType, right: &GqlType) -> bool {
        match (left, right) {
            // Same type is always comparable
            _ if left == right => true,

            // Remove null comparison as TypeSpec doesn't have Null variant

            // String is comparable with most types
            (GqlType::String { .. }, _) | (_, GqlType::String { .. }) => true,

            // Numeric types are comparable
            (left, right) if left.is_numeric() && right.is_numeric() => true,

            // Temporal types are comparable if they're both temporal
            (left, right) if left.is_temporal() && right.is_temporal() => {
                left.has_date_component() == right.has_date_component()
                    && left.has_time_component() == right.has_time_component()
            }

            _ => false,
        }
    }

    /// Promote numeric types to common type using TypeSpec
    fn promote_numeric_types(&self, left: &GqlType, right: &GqlType) -> GqlType {
        match (left, right) {
            // Same type
            _ if left == right => left.clone(),

            // Promote to wider integer type
            (GqlType::SmallInt, GqlType::Integer) | (GqlType::Integer, GqlType::SmallInt) => {
                GqlType::Integer
            }
            (GqlType::SmallInt, GqlType::BigInt) | (GqlType::BigInt, GqlType::SmallInt) => {
                GqlType::BigInt
            }
            (GqlType::Integer, GqlType::BigInt) | (GqlType::BigInt, GqlType::Integer) => {
                GqlType::BigInt
            }

            // Promote to Int128/Int256
            (_, GqlType::Int256) | (GqlType::Int256, _) => GqlType::Int256,
            (_, GqlType::Int128) | (GqlType::Int128, _) => GqlType::Int128,

            // Promote to floating point
            (_, GqlType::Double) | (GqlType::Double, _) => GqlType::Double,
            (_, GqlType::Float { .. }) | (GqlType::Float { .. }, _) => GqlType::Double,
            (_, GqlType::Real) | (GqlType::Real, _) => GqlType::Real,

            // Promote to decimal
            (
                GqlType::Decimal {
                    precision: p1,
                    scale: s1,
                },
                GqlType::Decimal {
                    precision: p2,
                    scale: s2,
                },
            ) => GqlType::Decimal {
                precision: match (p1, p2) {
                    (Some(x), Some(y)) => Some((*x).max(*y)),
                    _ => None,
                },
                scale: match (s1, s2) {
                    (Some(x), Some(y)) => Some((*x).max(*y)),
                    _ => None,
                },
            },

            // Default to double for mixed types
            _ => GqlType::Double,
        }
    }

    /// Clear all type bindings
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn clear_bindings(&mut self) {
        self.variable_types.clear();
    }

    /// Create a new scope with current bindings
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn push_scope(&self) -> HashMap<String, GqlType> {
        self.variable_types.clone()
    }

    /// Restore previous scope
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn pop_scope(&mut self, scope: HashMap<String, GqlType>) {
        self.variable_types = scope;
    }

    /// Infer type for unary operations
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type inference for static analysis (see ROADMAP.md §7)
    pub fn infer_unary_operation_type(
        &self,
        operand_type: &GqlType,
        operator: &crate::ast::ast::Operator,
    ) -> TypeResult<GqlType> {
        use crate::ast::ast::Operator;

        match operator {
            Operator::Not => {
                // NOT requires boolean operand and returns boolean
                if matches!(operand_type, GqlType::Boolean) {
                    Ok(GqlType::Boolean)
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "BOOLEAN".to_string(),
                        actual: format!("{}", operand_type),
                    })
                }
            }
            Operator::Minus => {
                // Unary minus requires numeric operand and preserves type
                if operand_type.is_numeric() {
                    Ok(operand_type.clone())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "NUMERIC".to_string(),
                        actual: format!("{}", operand_type),
                    })
                }
            }
            Operator::Plus => {
                // Unary plus requires numeric operand and preserves type
                if operand_type.is_numeric() {
                    Ok(operand_type.clone())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: "NUMERIC".to_string(),
                        actual: format!("{}", operand_type),
                    })
                }
            }
            _ => Err(TypeError::InvalidTypeSpecification(format!(
                "Operator {:?} is not valid for unary operations",
                operator
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_inference() {
        let engine = TypeInferenceEngine::new();

        assert_eq!(engine.infer_literal_type("true"), GqlType::Boolean);
        assert_eq!(engine.infer_literal_type("FALSE"), GqlType::Boolean);
        // Null doesn't have its own type in TypeSpec, defaults to Integer
        assert_eq!(engine.infer_literal_type("null"), GqlType::Integer);
        assert_eq!(
            engine.infer_literal_type("'hello'"),
            GqlType::String { max_length: None }
        );
        assert_eq!(engine.infer_literal_type("42"), GqlType::Integer);
        assert_eq!(engine.infer_literal_type("3.14"), GqlType::Double);
        assert_eq!(
            engine.infer_literal_type("DATE '2024-01-01'"),
            GqlType::Date
        );
    }

    #[test]
    fn test_arithmetic_inference() {
        let engine = TypeInferenceEngine::new();

        let int_type = GqlType::Integer;
        let float_type = GqlType::Double;

        let result = engine
            .infer_binary_op_type("+", &int_type, &int_type)
            .unwrap();
        assert_eq!(result, GqlType::Integer);

        let result = engine
            .infer_binary_op_type("+", &int_type, &float_type)
            .unwrap();
        assert_eq!(result, GqlType::Double);
    }

    #[test]
    fn test_comparison_inference() {
        let engine = TypeInferenceEngine::new();

        let int_type = GqlType::Integer;
        let string_type = GqlType::String { max_length: None };

        let result = engine
            .infer_binary_op_type("=", &int_type, &int_type)
            .unwrap();
        assert_eq!(result, GqlType::Boolean);

        let result = engine
            .infer_binary_op_type("<", &string_type, &string_type)
            .unwrap();
        assert_eq!(result, GqlType::Boolean);
    }

    #[test]
    fn test_function_inference() {
        let engine = TypeInferenceEngine::new();

        let result = engine.infer_function_type("COUNT", &[]).unwrap();
        assert_eq!(result, GqlType::BigInt);

        let result = engine
            .infer_function_type("UPPER", &[GqlType::String { max_length: None }])
            .unwrap();
        assert_eq!(result, GqlType::String { max_length: None });

        let result = engine.infer_function_type("CURRENT_DATE", &[]).unwrap();
        assert_eq!(result, GqlType::Date);
    }
}
