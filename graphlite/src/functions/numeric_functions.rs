// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Consolidated numeric function implementations
//!
//! This module contains all numeric manipulation functions:
//! - ROUND: Rounds numbers to specified decimal places with Oracle-compatible logic

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

// ==============================================================================
// ROUND FUNCTION
// ==============================================================================

/// ROUND function - rounds numbers to specified decimal places
#[derive(Debug)]
pub struct RoundFunction;

impl RoundFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for RoundFunction {
    fn name(&self) -> &str {
        "ROUND"
    }

    fn description(&self) -> &str {
        "Rounds a number to specified decimal places"
    }

    fn argument_count(&self) -> usize {
        1 // ROUND(number) - can also accept optional second parameter
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Handle 1 or 2 arguments
        let arg_count = context.arguments.len();
        if arg_count == 0 || arg_count > 2 {
            return Err(FunctionError::InvalidArgumentType {
                message: "ROUND function expects 1 or 2 arguments".to_string(),
            });
        }

        // Get the number argument
        let value = context.get_argument(0)?;

        if value.is_null() {
            return Ok(Value::Null);
        }

        // Convert to number
        let number = if let Some(n) = value.as_number() {
            n
        } else if let Some(s) = value.as_string() {
            s.parse::<f64>()
                .map_err(|_| FunctionError::InvalidArgumentType {
                    message: format!("Cannot convert '{}' to number", s),
                })?
        } else {
            return Err(FunctionError::InvalidArgumentType {
                message: "ROUND argument must be a number or convertible to number".to_string(),
            });
        };

        // Get decimal places (default to 0)
        let decimal_places = if arg_count == 2 {
            let places_value = context.get_argument(1)?;
            if let Some(n) = places_value.as_number() {
                n as i32
            } else {
                return Err(FunctionError::InvalidArgumentType {
                    message: "ROUND decimal places argument must be a number".to_string(),
                });
            }
        } else {
            0
        };

        // Handle special case: if number is 0, always return 0
        if number == 0.0 {
            return Ok(Value::Number(0.0));
        }

        // Oracle ROUND logic
        let rounded = oracle_round(number, decimal_places);

        Ok(Value::Number(rounded))
    }

    fn return_type(&self) -> &str {
        "Number"
    }
}

/// Oracle-compatible ROUND function implementation
/// ROUND(n, integer) follows Oracle's rounding rules:
/// - If n is 0, always returns 0
/// - For negative n: ROUND(n, integer) = -ROUND(-n, integer)
/// - Uses "round half away from zero" behavior
fn oracle_round(n: f64, decimal_places: i32) -> f64 {
    if n == 0.0 {
        return 0.0;
    }

    // Handle negative numbers: ROUND(n, integer) = -ROUND(-n, integer)
    if n < 0.0 {
        return -oracle_round(-n, decimal_places);
    }

    // Calculate the multiplier for the decimal places
    let multiplier = 10.0_f64.powi(decimal_places);

    // Apply Oracle's rounding: ROUND(n, integer) = FLOOR(n * 10^integer + 0.5) / 10^integer
    let scaled = n * multiplier + 0.5;
    let floored = scaled.floor();

    floored / multiplier
}
