// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Mathematical function implementations for ISO GQL compliance
//!
//! This module contains standard mathematical functions:
//! - ABS: Absolute value
//! - CEIL/CEILING: Round up to nearest integer
//! - FLOOR: Round down to nearest integer  
//! - SQRT: Square root
//! - POWER/POW: Exponentiation
//! - LOG: Natural logarithm
//! - LOG10: Base-10 logarithm
//! - EXP: Exponential function (e^x)
//! - SIN, COS, TAN: Trigonometric functions
//! - PI: Mathematical constant π
//! - SIGN: Sign function
//! - MOD: Modulo operation

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

// ==============================================================================
// ABS FUNCTION
// ==============================================================================

/// ABS function - returns absolute value
#[derive(Debug)]
pub struct AbsFunction;

impl AbsFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for AbsFunction {
    fn name(&self) -> &str {
        "ABS"
    }

    fn description(&self) -> &str {
        "Returns the absolute value of a number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("ABS requires a numeric argument, got {}", arg.type_name()),
            })?;

        Ok(Value::Number(number.abs()))
    }
}

// ==============================================================================
// CEIL/CEILING FUNCTION
// ==============================================================================

/// CEIL function - rounds up to nearest integer
#[derive(Debug)]
pub struct CeilFunction;

impl CeilFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CeilFunction {
    fn name(&self) -> &str {
        "CEIL"
    }

    fn description(&self) -> &str {
        "Returns the smallest integer greater than or equal to the given number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("CEIL requires a numeric argument, got {}", arg.type_name()),
            })?;

        Ok(Value::Number(number.ceil()))
    }
}

// ==============================================================================
// FLOOR FUNCTION
// ==============================================================================

/// FLOOR function - rounds down to nearest integer
#[derive(Debug)]
pub struct FloorFunction;

impl FloorFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for FloorFunction {
    fn name(&self) -> &str {
        "FLOOR"
    }

    fn description(&self) -> &str {
        "Returns the largest integer less than or equal to the given number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("FLOOR requires a numeric argument, got {}", arg.type_name()),
            })?;

        Ok(Value::Number(number.floor()))
    }
}

// ==============================================================================
// SQRT FUNCTION
// ==============================================================================

/// SQRT function - returns square root
#[derive(Debug)]
pub struct SqrtFunction;

impl SqrtFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SqrtFunction {
    fn name(&self) -> &str {
        "SQRT"
    }

    fn description(&self) -> &str {
        "Returns the square root of a number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("SQRT requires a numeric argument, got {}", arg.type_name()),
            })?;

        if number < 0.0 {
            return Err(FunctionError::ExecutionError {
                message: "SQRT of negative number is undefined".to_string(),
            });
        }

        Ok(Value::Number(number.sqrt()))
    }
}

// ==============================================================================
// POWER FUNCTION
// ==============================================================================

/// POWER function - returns base raised to exponent
#[derive(Debug)]
pub struct PowerFunction;

impl PowerFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for PowerFunction {
    fn name(&self) -> &str {
        "POWER"
    }

    fn description(&self) -> &str {
        "Returns base raised to the power of exponent"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(2)?;

        let base_arg = context.get_argument(0)?;
        let exp_arg = context.get_argument(1)?;

        let base = base_arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("POWER base must be numeric, got {}", base_arg.type_name()),
            })?;

        let exponent = exp_arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!(
                    "POWER exponent must be numeric, got {}",
                    exp_arg.type_name()
                ),
            })?;

        let result = base.powf(exponent);

        if result.is_nan() || result.is_infinite() {
            return Err(FunctionError::ExecutionError {
                message: format!("POWER({}, {}) results in invalid value", base, exponent),
            });
        }

        Ok(Value::Number(result))
    }
}

// ==============================================================================
// LOG FUNCTION (Natural Logarithm)
// ==============================================================================

/// LOG function - returns natural logarithm
#[derive(Debug)]
pub struct LogFunction;

impl LogFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for LogFunction {
    fn name(&self) -> &str {
        "LOG"
    }

    fn description(&self) -> &str {
        "Returns the natural logarithm of a number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("LOG requires a numeric argument, got {}", arg.type_name()),
            })?;

        if number <= 0.0 {
            return Err(FunctionError::ExecutionError {
                message: "LOG of zero or negative number is undefined".to_string(),
            });
        }

        Ok(Value::Number(number.ln()))
    }
}

// ==============================================================================
// LOG10 FUNCTION
// ==============================================================================

/// LOG10 function - returns base-10 logarithm
#[derive(Debug)]
pub struct Log10Function;

impl Log10Function {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Log10Function {
    fn name(&self) -> &str {
        "LOG10"
    }

    fn description(&self) -> &str {
        "Returns the base-10 logarithm of a number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("LOG10 requires a numeric argument, got {}", arg.type_name()),
            })?;

        if number <= 0.0 {
            return Err(FunctionError::ExecutionError {
                message: "LOG10 of zero or negative number is undefined".to_string(),
            });
        }

        Ok(Value::Number(number.log10()))
    }
}

// ==============================================================================
// EXP FUNCTION
// ==============================================================================

/// EXP function - returns e raised to the power of x
#[derive(Debug)]
pub struct ExpFunction;

impl ExpFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ExpFunction {
    fn name(&self) -> &str {
        "EXP"
    }

    fn description(&self) -> &str {
        "Returns e raised to the power of the given number"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("EXP requires a numeric argument, got {}", arg.type_name()),
            })?;

        let result = number.exp();

        if result.is_infinite() {
            return Err(FunctionError::ExecutionError {
                message: format!("EXP({}) results in overflow", number),
            });
        }

        Ok(Value::Number(result))
    }
}

// ==============================================================================
// TRIGONOMETRIC FUNCTIONS
// ==============================================================================

/// SIN function - returns sine
#[derive(Debug)]
pub struct SinFunction;

impl SinFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SinFunction {
    fn name(&self) -> &str {
        "SIN"
    }

    fn description(&self) -> &str {
        "Returns the sine of a number (in radians)"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("SIN requires a numeric argument, got {}", arg.type_name()),
            })?;

        Ok(Value::Number(number.sin()))
    }
}

/// COS function - returns cosine
#[derive(Debug)]
pub struct CosFunction;

impl CosFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for CosFunction {
    fn name(&self) -> &str {
        "COS"
    }

    fn description(&self) -> &str {
        "Returns the cosine of a number (in radians)"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("COS requires a numeric argument, got {}", arg.type_name()),
            })?;

        Ok(Value::Number(number.cos()))
    }
}

/// TAN function - returns tangent
#[derive(Debug)]
pub struct TanFunction;

impl TanFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for TanFunction {
    fn name(&self) -> &str {
        "TAN"
    }

    fn description(&self) -> &str {
        "Returns the tangent of a number (in radians)"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("TAN requires a numeric argument, got {}", arg.type_name()),
            })?;

        Ok(Value::Number(number.tan()))
    }
}

// ==============================================================================
// PI FUNCTION
// ==============================================================================

/// PI function - returns the mathematical constant π
#[derive(Debug)]
pub struct PiFunction;

impl PiFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for PiFunction {
    fn name(&self) -> &str {
        "PI"
    }

    fn description(&self) -> &str {
        "Returns the mathematical constant π (pi)"
    }

    fn argument_count(&self) -> usize {
        0
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(0)?;
        Ok(Value::Number(std::f64::consts::PI))
    }
}

// ==============================================================================
// SIGN FUNCTION
// ==============================================================================

/// SIGN function - returns the sign of a number
#[derive(Debug)]
pub struct SignFunction;

impl SignFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SignFunction {
    fn name(&self) -> &str {
        "SIGN"
    }

    fn description(&self) -> &str {
        "Returns -1 for negative numbers, 0 for zero, 1 for positive numbers"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(1)?;

        let arg = context.get_argument(0)?;
        let number = arg
            .as_number()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: format!("SIGN requires a numeric argument, got {}", arg.type_name()),
            })?;

        let sign = if number > 0.0 {
            1.0
        } else if number < 0.0 {
            -1.0
        } else {
            0.0
        };

        Ok(Value::Number(sign))
    }
}

// ==============================================================================
// MOD FUNCTION
// ==============================================================================

/// MOD function - returns the modulo (remainder) of division
#[derive(Debug)]
pub struct ModFunction;

impl ModFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ModFunction {
    fn name(&self) -> &str {
        "MOD"
    }

    fn description(&self) -> &str {
        "Returns the remainder after division of the first argument by the second"
    }

    fn argument_count(&self) -> usize {
        2
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false // Mathematical functions are pure scalar functions
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        context.validate_argument_count(2)?;

        let dividend_arg = context.get_argument(0)?;
        let divisor_arg = context.get_argument(1)?;

        let dividend =
            dividend_arg
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: format!(
                        "MOD dividend must be numeric, got {}",
                        dividend_arg.type_name()
                    ),
                })?;

        let divisor =
            divisor_arg
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: format!(
                        "MOD divisor must be numeric, got {}",
                        divisor_arg.type_name()
                    ),
                })?;

        if divisor == 0.0 {
            return Err(FunctionError::ExecutionError {
                message: "MOD by zero is undefined".to_string(),
            });
        }

        Ok(Value::Number(dividend % divisor))
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
    fn test_abs_function() {
        let func = AbsFunction::new();

        // Test positive number
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(5.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(5.0));

        // Test negative number
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(-5.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(5.0));

        // Test zero
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(0.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(0.0));
    }

    #[test]
    fn test_ceil_function() {
        let func = CeilFunction::new();

        // Test positive decimal
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(4.2)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(5.0));

        // Test negative decimal
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(-4.2)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(-4.0));

        // Test integer
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(5.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(5.0));
    }

    #[test]
    fn test_floor_function() {
        let func = FloorFunction::new();

        // Test positive decimal
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(4.8)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(4.0));

        // Test negative decimal
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(-4.2)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(-5.0));
    }

    #[test]
    fn test_sqrt_function() {
        let func = SqrtFunction::new();

        // Test perfect square
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(16.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(4.0));

        // Test negative number (should error)
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(-4.0)]);
        assert!(func.execute(&context).is_err());
    }

    #[test]
    fn test_power_function() {
        let func = PowerFunction::new();

        // Test 2^3 = 8
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(2.0), Value::Number(3.0)],
        );
        assert_eq!(func.execute(&context).unwrap(), Value::Number(8.0));

        // Test 4^0.5 = 2 (square root)
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(4.0), Value::Number(0.5)],
        );
        assert_eq!(func.execute(&context).unwrap(), Value::Number(2.0));
    }

    #[test]
    fn test_pi_function() {
        let func = PiFunction::new();
        let context = FunctionContext::new(vec![], HashMap::new(), vec![]);
        let result = func.execute(&context).unwrap();
        if let Value::Number(pi) = result {
            assert!((pi - std::f64::consts::PI).abs() < 1e-10);
        } else {
            panic!("PI function should return a number");
        }
    }

    #[test]
    fn test_sign_function() {
        let func = SignFunction::new();

        // Test positive
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(5.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(1.0));

        // Test negative
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(-5.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(-1.0));

        // Test zero
        let context = FunctionContext::new(vec![], HashMap::new(), vec![Value::Number(0.0)]);
        assert_eq!(func.execute(&context).unwrap(), Value::Number(0.0));
    }

    #[test]
    fn test_mod_function() {
        let func = ModFunction::new();

        // Test 10 % 3 = 1
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(10.0), Value::Number(3.0)],
        );
        assert_eq!(func.execute(&context).unwrap(), Value::Number(1.0));

        // Test division by zero (should error)
        let context = FunctionContext::new(
            vec![],
            HashMap::new(),
            vec![Value::Number(10.0), Value::Number(0.0)],
        );
        assert!(func.execute(&context).is_err());
    }
}
