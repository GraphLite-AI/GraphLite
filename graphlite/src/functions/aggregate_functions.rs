// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Consolidated aggregate function implementations
//!
//! This module contains all aggregate/statistical functions:
//! - COUNT: Counts rows or non-null values
//! - AVERAGE: Calculates arithmetic mean
//! - SUM: Calculates sum of numeric values
//! - MIN: Finds minimum value
//! - MAX: Finds maximum value

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

// ==============================================================================
// COUNT FUNCTION
// ==============================================================================

/// COUNT function - counts rows or non-null values
#[derive(Debug)]
pub struct CountFunction;

impl CountFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CountFunction {
    fn name(&self) -> &str {
        "COUNT"
    }

    fn description(&self) -> &str {
        "Counts the number of non-null values in a column or all rows if no column specified"
    }

    fn argument_count(&self) -> usize {
        0 // COUNT() or COUNT(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // If no arguments, count all rows
        if context.argument_count() == 0 {
            return Ok(Value::Number(context.rows.len() as f64));
        }

        // If argument provided, count non-null values in that column
        let column_name = context.get_argument(0)?.as_string().ok_or_else(|| {
            FunctionError::InvalidArgumentType {
                message: "COUNT argument must be a string column name".to_string(),
            }
        })?;

        // Special case: COUNT(*) should count all rows
        if column_name == "*" {
            return Ok(Value::Number(context.rows.len() as f64));
        }

        let mut count = 0;
        for row in &context.rows {
            if let Some(value) = row.values.get(column_name) {
                if !value.is_null() {
                    count += 1;
                }
            }
        }
        Ok(Value::Number(count as f64))
    }

    fn return_type(&self) -> &str {
        "Number"
    }
}

// ==============================================================================
// AVERAGE FUNCTION
// ==============================================================================

/// AVERAGE function - calculates arithmetic mean
#[derive(Debug)]
pub struct AverageFunction;

impl AverageFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for AverageFunction {
    fn name(&self) -> &str {
        "AVERAGE"
    }

    fn description(&self) -> &str {
        "Calculates the arithmetic mean of numeric values in a column"
    }

    fn argument_count(&self) -> usize {
        1 // AVERAGE(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;

        // Handle both aggregation path (string column name) and regular function path (numeric value)
        match arg {
            // Aggregation path: receive column name as string, process all rows
            Value::String(column_name) => {
                let mut sum = 0.0;
                let mut count = 0;

                for row in &context.rows {
                    if let Some(value) = row.values.get(column_name) {
                        if !value.is_null() {
                            let number = value.as_number().ok_or_else(|| {
                                FunctionError::InvalidArgumentType {
                                    message: format!(
                                        "Cannot convert {} to number for AVERAGE",
                                        value.type_name()
                                    ),
                                }
                            })?;

                            sum += number;
                            count += 1;
                        }
                    }
                }

                if count == 0 {
                    log::debug!(
                        "DEBUG: AverageFunction::execute - returning NULL (no values found)"
                    );
                    Ok(Value::Null)
                } else {
                    let avg = sum / count as f64;
                    log::debug!("DEBUG: AverageFunction::execute - returning Number({}) from sum={}, count={}", avg, sum, count);
                    Ok(Value::Number(avg))
                }
            }

            // Regular function path: receive individual numeric values
            // This handles the case where AVERAGE is called as a regular function instead of aggregate
            Value::Number(num) => {
                // For single values, just return the value (average of one number is itself)
                Ok(Value::Number(*num))
            }

            _ => Err(FunctionError::InvalidArgumentType {
                message: "AVERAGE argument must be a string column name or numeric value"
                    .to_string(),
            }),
        }
    }

    fn return_type(&self) -> &str {
        "Number"
    }
}

// ==============================================================================
// SUM FUNCTION
// ==============================================================================

/// SUM function - calculates the sum of numeric values in a column
#[derive(Debug)]
pub struct SumFunction;

impl SumFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SumFunction {
    fn name(&self) -> &str {
        "SUM"
    }

    fn description(&self) -> &str {
        "Calculates the sum of numeric values in a column"
    }

    fn argument_count(&self) -> usize {
        1 // SUM(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Get the column name argument
        let column_name = context.get_argument(0)?.as_string().ok_or_else(|| {
            FunctionError::InvalidArgumentType {
                message: "SUM argument must be a string column name".to_string(),
            }
        })?;

        let mut sum = 0.0;
        let mut has_values = false;

        for row in &context.rows {
            if let Some(value) = row.values.get(column_name) {
                if !value.is_null() {
                    if let Some(num) = value.as_number() {
                        sum += num;
                        has_values = true;
                    }
                }
            }
        }

        // Return NULL if no numeric values found (ISO GQL behavior)
        if !has_values {
            return Ok(Value::Null);
        }

        Ok(Value::Number(sum))
    }

    fn return_type(&self) -> &str {
        "Number"
    }
}

// ==============================================================================
// MIN FUNCTION
// ==============================================================================

/// MIN function - finds the minimum numeric value in a column
#[derive(Debug)]
pub struct MinFunction;

impl MinFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for MinFunction {
    fn name(&self) -> &str {
        "MIN"
    }

    fn description(&self) -> &str {
        "Finds the minimum numeric value in a column"
    }

    fn argument_count(&self) -> usize {
        1 // MIN(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Get the column name argument
        let column_name = context.get_argument(0)?.as_string().ok_or_else(|| {
            FunctionError::InvalidArgumentType {
                message: "MIN argument must be a string column name".to_string(),
            }
        })?;

        let mut min_value: Option<f64> = None;

        for row in &context.rows {
            if let Some(value) = row.values.get(column_name) {
                if !value.is_null() {
                    if let Some(num) = value.as_number() {
                        match min_value {
                            None => min_value = Some(num),
                            Some(current_min) => {
                                if num < current_min {
                                    min_value = Some(num);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Return null if no numeric values found
        match min_value {
            Some(min) => Ok(Value::Number(min)),
            None => Ok(Value::Null),
        }
    }

    fn return_type(&self) -> &str {
        "Number"
    }
}

// ==============================================================================
// MAX FUNCTION
// ==============================================================================

/// MAX function - finds the maximum numeric value in a column
#[derive(Debug)]
pub struct MaxFunction;

impl MaxFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for MaxFunction {
    fn name(&self) -> &str {
        "MAX"
    }

    fn description(&self) -> &str {
        "Finds the maximum numeric value in a column"
    }

    fn argument_count(&self) -> usize {
        1 // MAX(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Get the column name argument
        let column_name = context.get_argument(0)?.as_string().ok_or_else(|| {
            FunctionError::InvalidArgumentType {
                message: "MAX argument must be a string column name".to_string(),
            }
        })?;

        let mut max_value: Option<f64> = None;

        for row in &context.rows {
            if let Some(value) = row.values.get(column_name) {
                if !value.is_null() {
                    if let Some(num) = value.as_number() {
                        match max_value {
                            None => max_value = Some(num),
                            Some(current_max) => {
                                if num > current_max {
                                    max_value = Some(num);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Return null if no numeric values found
        match max_value {
            Some(max) => Ok(Value::Number(max)),
            None => Ok(Value::Null),
        }
    }

    fn return_type(&self) -> &str {
        "Number"
    }
}

// ==============================================================================
// COLLECT FUNCTION
// ==============================================================================

/// COLLECT function - collects values into a list/array
#[derive(Debug)]
pub struct CollectFunction;

impl CollectFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CollectFunction {
    fn name(&self) -> &str {
        "COLLECT"
    }

    fn description(&self) -> &str {
        "Collects values from a column into a list/array"
    }

    fn argument_count(&self) -> usize {
        1 // COLLECT(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Get the column name argument
        let column_name = context.get_argument(0)?.as_string().ok_or_else(|| {
            FunctionError::InvalidArgumentType {
                message: "COLLECT argument must be a string column name".to_string(),
            }
        })?;

        let mut collected_values = Vec::new();

        for row in &context.rows {
            if let Some(value) = row.values.get(column_name) {
                // Include all values, including nulls, to match ISO GQL behavior
                collected_values.push(value.clone());
            }
        }

        // Return as List type for ISO GQL compatibility
        Ok(Value::List(collected_values))
    }

    fn return_type(&self) -> &str {
        "List"
    }
}
