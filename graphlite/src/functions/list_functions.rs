// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Advanced list operations for ISO GQL
//!
//! This module implements comprehensive list operations including:
//! - LIST_CONTAINS function
//! - LIST_SLICE function  
//! - LIST_APPEND/PREPEND functions
//! - LIST_LENGTH function
//! - LIST_REVERSE function

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

/// LIST_CONTAINS function: check if list contains element
#[derive(Debug)]
pub struct ListContainsFunction;

impl ListContainsFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ListContainsFunction {
    fn name(&self) -> &str {
        "LIST_CONTAINS"
    }

    fn description(&self) -> &str {
        "Check if list contains the specified element"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let list_arg = &context.arguments[0];
        let element_arg = &context.arguments[1];

        // Extract list values
        let list_values = list_arg
            .as_list()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a list".to_string(),
            })?;

        // Check if any element in the list matches the search element
        let contains = list_values.iter().any(|item| item == element_arg);

        Ok(Value::Boolean(contains))
    }
}

/// LIST_SLICE function: extract sublist from start to end indices
#[derive(Debug)]
pub struct ListSliceFunction;

impl ListSliceFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ListSliceFunction {
    fn name(&self) -> &str {
        "LIST_SLICE"
    }

    fn description(&self) -> &str {
        "Extract a slice of the list from start index to end index (exclusive)"
    }

    fn argument_count(&self) -> usize {
        3
    }

    fn return_type(&self) -> &str {
        "List"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 3 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 3,
                actual: context.arguments.len(),
            });
        }

        let list_arg = &context.arguments[0];
        let start_arg = &context.arguments[1];
        let end_arg = &context.arguments[2];

        // Extract list values
        let list_values = list_arg
            .as_list()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a list".to_string(),
            })?;

        // Extract start index
        let start_idx =
            start_arg
                .as_integer()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Start index must be an integer".to_string(),
                })? as usize;

        // Extract end index
        let end_idx = end_arg
            .as_integer()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "End index must be an integer".to_string(),
            })? as usize;

        // Validate indices
        if start_idx >= list_values.len() {
            return Ok(Value::List(Vec::new()));
        }

        let actual_end = end_idx.min(list_values.len());
        if start_idx >= actual_end {
            return Ok(Value::List(Vec::new()));
        }

        // Extract slice
        let slice = list_values[start_idx..actual_end].to_vec();
        Ok(Value::List(slice))
    }
}

/// LIST_APPEND function: append element(s) to end of list
#[derive(Debug)]
pub struct ListAppendFunction;

impl ListAppendFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ListAppendFunction {
    fn name(&self) -> &str {
        "LIST_APPEND"
    }

    fn description(&self) -> &str {
        "Append element or elements to the end of a list"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "List"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let list_arg = &context.arguments[0];
        let element_arg = &context.arguments[1];

        // Extract list values
        let mut list_values = list_arg
            .as_list()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a list".to_string(),
            })?
            .clone();

        // Append element(s)
        match element_arg {
            Value::List(elements) => {
                // Append all elements from the second list
                list_values.extend(elements.clone());
            }
            element => {
                // Append single element
                list_values.push(element.clone());
            }
        }

        Ok(Value::List(list_values))
    }
}

/// LIST_PREPEND function: prepend element(s) to beginning of list
#[derive(Debug)]
pub struct ListPrependFunction;

impl ListPrependFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ListPrependFunction {
    fn name(&self) -> &str {
        "LIST_PREPEND"
    }

    fn description(&self) -> &str {
        "Prepend element or elements to the beginning of a list"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "List"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let list_arg = &context.arguments[0];
        let element_arg = &context.arguments[1];

        // Extract list values
        let list_values = list_arg
            .as_list()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a list".to_string(),
            })?;

        let mut result = Vec::new();

        // Prepend element(s)
        match element_arg {
            Value::List(elements) => {
                // Prepend all elements from the second list
                result.extend(elements.clone());
            }
            element => {
                // Prepend single element
                result.push(element.clone());
            }
        }

        // Add original list elements
        result.extend(list_values.clone());

        Ok(Value::List(result))
    }
}

/// LIST_LENGTH function: get length of list
#[derive(Debug)]
pub struct ListLengthFunction;

impl ListLengthFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ListLengthFunction {
    fn name(&self) -> &str {
        "LIST_LENGTH"
    }

    fn description(&self) -> &str {
        "Get the length of a list"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Integer"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let list_arg = &context.arguments[0];

        // Extract list values
        let list_values = list_arg
            .as_list()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Argument must be a list".to_string(),
            })?;

        Ok(Value::Number(list_values.len() as f64))
    }
}

/// LIST_REVERSE function: reverse the order of elements in a list
#[derive(Debug)]
pub struct ListReverseFunction;

impl ListReverseFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ListReverseFunction {
    fn name(&self) -> &str {
        "LIST_REVERSE"
    }

    fn description(&self) -> &str {
        "Reverse the order of elements in a list"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "List"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let list_arg = &context.arguments[0];

        // Extract list values
        let list_values = list_arg
            .as_list()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Argument must be a list".to_string(),
            })?;

        let mut reversed = list_values.clone();
        reversed.reverse();

        Ok(Value::List(reversed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_context(args: Vec<Value>) -> FunctionContext {
        FunctionContext::new(Vec::new(), HashMap::new(), args)
    }

    #[test]
    fn test_list_contains() {
        let func = ListContainsFunction::new();

        // Test contains existing element
        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        let context = create_context(vec![list, Value::Number(2.0)]);
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(true));

        // Test does not contain element
        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        let context = create_context(vec![list, Value::Number(4.0)]);
        let result = func.execute(&context).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_list_slice() {
        let func = ListSliceFunction::new();

        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
            Value::Number(5.0),
        ]);

        // Test normal slice
        let context = create_context(vec![list.clone(), Value::Number(1.0), Value::Number(4.0)]);
        let result = func.execute(&context).unwrap();
        let expected = Value::List(vec![
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ]);
        assert_eq!(result, expected);

        // Test slice beyond bounds
        let context = create_context(vec![list.clone(), Value::Number(3.0), Value::Number(10.0)]);
        let result = func.execute(&context).unwrap();
        let expected = Value::List(vec![Value::Number(4.0), Value::Number(5.0)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_list_append() {
        let func = ListAppendFunction::new();

        let list = Value::List(vec![Value::Number(1.0), Value::Number(2.0)]);
        let context = create_context(vec![list, Value::Number(3.0)]);
        let result = func.execute(&context).unwrap();

        let expected = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_list_prepend() {
        let func = ListPrependFunction::new();

        let list = Value::List(vec![Value::Number(2.0), Value::Number(3.0)]);
        let context = create_context(vec![list, Value::Number(1.0)]);
        let result = func.execute(&context).unwrap();

        let expected = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_list_length() {
        let func = ListLengthFunction::new();

        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        let context = create_context(vec![list]);
        let result = func.execute(&context).unwrap();

        assert_eq!(result, Value::Number(3.0));
    }

    #[test]
    fn test_list_reverse() {
        let func = ListReverseFunction::new();

        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        let context = create_context(vec![list]);
        let result = func.execute(&context).unwrap();

        let expected = Value::List(vec![
            Value::Number(3.0),
            Value::Number(2.0),
            Value::Number(1.0),
        ]);
        assert_eq!(result, expected);
    }
}
