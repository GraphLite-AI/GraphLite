// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Null handling function implementations
//!
//! This module contains functions for handling NULL values:
//! - NULLIF: Returns NULL if two expressions are equal, otherwise returns the first expression
//! - COALESCE: Returns the first non-NULL expression from a list of expressions

use super::function_trait::{Function, FunctionContext, FunctionResult, FunctionError};
use crate::storage::Value;

// ==============================================================================
// NULLIF FUNCTION
// ==============================================================================

/// NULLIF function - returns NULL if expr1 equals expr2, otherwise returns expr1
#[derive(Debug)]
pub struct NullIfFunction;

impl NullIfFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for NullIfFunction {
    fn name(&self) -> &str {
        "NULLIF"
    }
    
    fn description(&self) -> &str {
        "Returns NULL if expr1 equals expr2, otherwise returns expr1"
    }
    
    fn argument_count(&self) -> usize {
        2 // NULLIF(expr1, expr2)
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        context.validate_argument_count(2)?;
        
        let expr1 = context.get_argument(0)?;
        let expr2 = context.get_argument(1)?;
        
        // If either expression is NULL, return expr1 (following SQL semantics)
        if expr1.is_null() || expr2.is_null() {
            return Ok(expr1.clone());
        }
        
        // Compare the values
        if expr1 == expr2 {
            Ok(Value::Null)
        } else {
            Ok(expr1.clone())
        }
    }
    
    fn return_type(&self) -> &str {
        "Any" // Returns the type of the first expression or NULL
    }
}

// ==============================================================================
// COALESCE FUNCTION
// ==============================================================================

/// COALESCE function - returns the first non-NULL expression from a list
#[derive(Debug)]
pub struct CoalesceFunction;

impl CoalesceFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CoalesceFunction {
    fn name(&self) -> &str {
        "COALESCE"
    }
    
    fn description(&self) -> &str {
        "Returns the first non-NULL expression from a list of expressions"
    }
    
    fn argument_count(&self) -> usize {
        0 // COALESCE accepts variable number of arguments, but we need at least 1
    }
    
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // COALESCE must have at least one argument
        if context.argument_count() == 0 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: 0,
            });
        }
        
        // Return the first non-NULL value
        for i in 0..context.argument_count() {
            let arg = context.get_argument(i)?;
            if !arg.is_null() {
                return Ok(arg.clone());
            }
        }
        
        // If all arguments are NULL, return NULL
        Ok(Value::Null)
    }
    
    fn return_type(&self) -> &str {
        "Any" // Returns the type of the first non-NULL expression or NULL
    }
}

// ==============================================================================
// TESTS
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_nullif_equal_values() {
        let func = NullIfFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(5.0), Value::Number(5.0)]
        );
        
        let result = func.execute(&context).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_nullif_different_values() {
        let func = NullIfFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(5.0), Value::Number(3.0)]
        );
        
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Number(5.0));
    }

    #[test]
    fn test_nullif_first_null() {
        let func = NullIfFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Null, Value::Number(3.0)]
        );
        
        let result = func.execute(&context).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_nullif_second_null() {
        let func = NullIfFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(5.0), Value::Null]
        );
        
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Number(5.0));
    }

    #[test]
    fn test_nullif_strings() {
        let func = NullIfFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("hello".to_string()), Value::String("hello".to_string())]
        );
        
        let result = func.execute(&context).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_coalesce_first_non_null() {
        let func = CoalesceFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]
        );
        
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Number(1.0));
    }

    #[test]
    fn test_coalesce_skip_nulls() {
        let func = CoalesceFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Null, Value::Null, Value::String("found".to_string()), Value::Number(4.0)]
        );
        
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::String("found".to_string()));
    }

    #[test]
    fn test_coalesce_all_null() {
        let func = CoalesceFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Null, Value::Null, Value::Null]
        );
        
        let result = func.execute(&context).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_coalesce_single_argument() {
        let func = CoalesceFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::String("single".to_string())]
        );
        
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::String("single".to_string()));
    }

    #[test]
    fn test_coalesce_no_arguments() {
        let func = CoalesceFunction::new();
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![]
        );
        
        let result = func.execute(&context);
        assert!(result.is_err());
    }
}
