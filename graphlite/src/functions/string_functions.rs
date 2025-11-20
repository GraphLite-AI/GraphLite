// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Consolidated string function implementations
//!
//! This module contains all string manipulation functions:
//! - UPPER: Converts strings to uppercase
//! - LOWER: Converts strings to lowercase  
//! - TRIM: Removes leading/trailing characters
//! - SUBSTRING: Extracts substrings
//! - REPLACE: Replaces substring occurrences
//! - REVERSE: Reverses string characters

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

// ==============================================================================
// UPPER FUNCTION
// ==============================================================================

/// UPPER function - converts string values to uppercase
#[derive(Debug)]
pub struct UpperFunction;

impl UpperFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for UpperFunction {
    fn name(&self) -> &str {
        "UPPER"
    }

    fn description(&self) -> &str {
        "Converts string values to uppercase"
    }

    fn argument_count(&self) -> usize {
        1 // UPPER(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Get the actual value argument (not column name)
        let value = context.get_argument(0)?;

        if value.is_null() {
            return Ok(Value::Null);
        }

        // Handle both strings and numbers - convert to string first
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            // Try to convert other types to string representation
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(Value::Null), // Return null for non-convertible types
            }
        };

        Ok(Value::String(string_val.to_uppercase()))
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn graph_context_required(&self) -> bool {
        false // String functions are pure scalar functions
    }
}

// ==============================================================================
// LOWER FUNCTION
// ==============================================================================

/// LOWER function - converts string values to lowercase
#[derive(Debug)]
pub struct LowerFunction;

impl LowerFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for LowerFunction {
    fn name(&self) -> &str {
        "LOWER"
    }

    fn description(&self) -> &str {
        "Converts string values to lowercase"
    }

    fn argument_count(&self) -> usize {
        1 // LOWER(column)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Get the actual value argument (not column name)
        let value = context.get_argument(0)?;

        if value.is_null() {
            return Ok(Value::Null);
        }

        // Handle both strings and numbers - convert to string first
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            // Try to convert other types to string representation
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(Value::Null), // Return null for non-convertible types
            }
        };

        Ok(Value::String(string_val.to_lowercase()))
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn graph_context_required(&self) -> bool {
        false // String functions are pure scalar functions
    }
}

// ==============================================================================
// TRIM FUNCTION
// ==============================================================================

/// Enum representing the TRIM mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrimMode {
    Both,
    Leading,
    Trailing,
}

/// TRIM function - removes leading/trailing characters from string
/// Supports ISO GQL syntax: TRIM([LEADING|TRAILING|BOTH] [character FROM] string)
#[derive(Debug)]
pub struct TrimFunction;

impl TrimFunction {
    pub fn new() -> Self {
        Self
    }

    /// Parse trim mode from a string value
    fn parse_trim_mode(s: &str) -> Option<TrimMode> {
        match s.to_uppercase().as_str() {
            "LEADING" => Some(TrimMode::Leading),
            "TRAILING" => Some(TrimMode::Trailing),
            "BOTH" => Some(TrimMode::Both),
            _ => None,
        }
    }

    /// Perform the actual trimming based on mode
    fn trim_string(input: &str, trim_chars: &str, mode: TrimMode) -> String {
        match mode {
            TrimMode::Leading => input
                .trim_start_matches(|c: char| trim_chars.contains(c))
                .to_string(),
            TrimMode::Trailing => input
                .trim_end_matches(|c: char| trim_chars.contains(c))
                .to_string(),
            TrimMode::Both => input
                .trim_matches(|c: char| trim_chars.contains(c))
                .to_string(),
        }
    }
}

impl Function for TrimFunction {
    fn name(&self) -> &str {
        "TRIM"
    }

    fn description(&self) -> &str {
        "Removes leading and/or trailing characters from a string"
    }

    fn argument_count(&self) -> usize {
        1 // Minimum 1, but can accept up to 3 arguments
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let arg_count = context.arguments.len();
        if arg_count == 0 || arg_count > 3 {
            return Err(FunctionError::InvalidArgumentType {
                message: "TRIM function expects 1 to 3 arguments".to_string(),
            });
        }

        // Handle different argument patterns
        if arg_count == 1 {
            // TRIM(string) - trim whitespace from both ends
            let value = context.get_argument(0)?;
            if value.is_null() {
                return Ok(Value::Null);
            }

            let string_val = self.value_to_string(value)?;
            Ok(Value::String(string_val.trim().to_string()))
        } else if arg_count == 2 {
            // TRIM(mode, string) OR TRIM(string, character)
            let first_arg = context.get_argument(0)?;
            let second_arg = context.get_argument(1)?;

            // Check if first argument is a trim mode
            if let Some(mode_str) = first_arg.as_string() {
                if let Some(mode) = Self::parse_trim_mode(&mode_str) {
                    // TRIM(mode, string) - trim whitespace with specified mode
                    if second_arg.is_null() {
                        return Ok(Value::Null);
                    }
                    let string_val = self.value_to_string(second_arg)?;
                    let result = Self::trim_string(&string_val, " \t\n\r", mode);
                    return Ok(Value::String(result));
                }
            }

            // Otherwise treat as TRIM(string, character) - trim specified character from both ends
            if first_arg.is_null() {
                return Ok(Value::Null);
            }

            let string_val = self.value_to_string(first_arg)?;
            let trim_char = self.extract_trim_char(second_arg)?;
            Ok(Value::String(
                string_val.trim_matches(trim_char).to_string(),
            ))
        } else {
            // arg_count == 3: TRIM FROM syntax: mode, trim_char, string
            // Arguments are: [mode_string, trim_char_string, target_string]
            let mode_value = context.get_argument(0)?;
            let char_value = context.get_argument(1)?;
            let string_value = context.get_argument(2)?;

            if string_value.is_null() {
                return Ok(Value::Null);
            }

            let string_val = self.value_to_string(string_value)?;
            let trim_chars = self.value_to_string(char_value)?;
            let mode_str = if let Some(s) = mode_value.as_string() {
                s
            } else {
                "BOTH"
            };

            let mode = Self::parse_trim_mode(&mode_str).unwrap_or(TrimMode::Both);
            let result = Self::trim_string(&string_val, &trim_chars, mode);

            Ok(Value::String(result))
        }
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn graph_context_required(&self) -> bool {
        false // String functions are pure scalar functions
    }
}

impl TrimFunction {
    /// Helper method to convert value to string consistently
    fn value_to_string(&self, value: &Value) -> FunctionResult<String> {
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(String::new()), // Return empty for non-convertible types
            }
        };
        Ok(string_val)
    }

    /// Helper method to extract trim character from value
    fn extract_trim_char(&self, value: &Value) -> FunctionResult<char> {
        let trim_char = if let Some(s) = value.as_string() {
            if s.is_empty() {
                ' ' // Default to space if empty string
            } else {
                s.chars().next().unwrap_or(' ')
            }
        } else {
            ' ' // Default to space for non-string values
        };
        Ok(trim_char)
    }
}

// ==============================================================================
// SUBSTRING FUNCTION
// ==============================================================================

/// SUBSTRING function - extracts substring from string
#[derive(Debug)]
pub struct SubstringFunction;

impl SubstringFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SubstringFunction {
    fn name(&self) -> &str {
        "SUBSTRING"
    }

    fn description(&self) -> &str {
        "Extracts a substring from a string starting at a position with optional length"
    }

    fn argument_count(&self) -> usize {
        // Variable arguments: SUBSTRING(string, position) or SUBSTRING(string, position, length)
        // Return minimum required arguments (2), actual validation happens in execute()
        2
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Handle 2 or 3 arguments: SUBSTRING(string, start) or SUBSTRING(string, start, length)
        let arg_count = context.arguments.len();
        if arg_count < 2 || arg_count > 3 {
            return Err(FunctionError::InvalidArgumentType {
                message: "SUBSTRING function expects 2 or 3 arguments".to_string(),
            });
        }

        let value = context.get_argument(0)?;

        if value.is_null() {
            return Ok(Value::Null);
        }

        // Convert to string
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(Value::Null),
            }
        };

        // Get start position (1-based in GQL, 0-based in Rust)
        let start_value = context.get_argument(1)?;
        let start_pos = if let Some(n) = start_value.as_number() {
            let pos = n as i32;
            if pos <= 0 {
                0
            } else {
                (pos - 1) as usize // Convert to 0-based
            }
        } else {
            return Err(FunctionError::InvalidArgumentType {
                message: "SUBSTRING start position must be a number".to_string(),
            });
        };

        // Convert to character array for proper Unicode handling
        let chars: Vec<char> = string_val.chars().collect();

        // Check if start position is beyond string length (in characters, not bytes)
        if start_pos >= chars.len() {
            return Ok(Value::String("".to_string()));
        }

        // Get length if provided
        let result_string = if arg_count == 3 {
            let length_value = context.get_argument(2)?;
            let length = if let Some(n) = length_value.as_number() {
                let len = n as i32;
                if len <= 0 {
                    return Ok(Value::String("".to_string()));
                }
                len as usize
            } else {
                return Err(FunctionError::InvalidArgumentType {
                    message: "SUBSTRING length must be a number".to_string(),
                });
            };

            // Extract substring with length, ensuring we don't exceed string bounds
            let end_pos = std::cmp::min(start_pos + length, chars.len());
            chars[start_pos..end_pos].iter().collect()
        } else {
            // Extract substring from start to end
            chars[start_pos..].iter().collect()
        };

        Ok(Value::String(result_string))
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn graph_context_required(&self) -> bool {
        false // String functions are pure scalar functions
    }
}

// ==============================================================================
// REPLACE FUNCTION
// ==============================================================================

/// REPLACE function - replaces occurrences of a substring with another substring
#[derive(Debug)]
pub struct ReplaceFunction;

impl ReplaceFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ReplaceFunction {
    fn name(&self) -> &str {
        "REPLACE"
    }

    fn description(&self) -> &str {
        "Replaces all occurrences of a substring with another substring"
    }

    fn argument_count(&self) -> usize {
        3 // REPLACE(string, search, replacement)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        if context.arguments.len() != 3 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 3,
                actual: context.arguments.len(),
            });
        }

        let string_value = context.get_argument(0)?;
        let search_value = context.get_argument(1)?;
        let replacement_value = context.get_argument(2)?;

        // Handle null values
        if string_value.is_null() || search_value.is_null() || replacement_value.is_null() {
            return Ok(Value::Null);
        }

        // Helper function to convert value to string (consistent with UPPER/LOWER)
        let to_string = |value: &Value| -> Option<String> {
            if let Some(s) = value.as_string() {
                Some(s.to_string())
            } else if let Some(n) = value.as_number() {
                Some(n.to_string())
            } else {
                match value {
                    Value::Boolean(b) => Some(b.to_string()),
                    _ => None,
                }
            }
        };

        // Convert all arguments to strings
        let string_val =
            to_string(string_value).ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be convertible to string".to_string(),
            })?;

        let search_val =
            to_string(search_value).ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Search argument must be convertible to string".to_string(),
            })?;

        let replacement_val =
            to_string(replacement_value).ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Replacement argument must be convertible to string".to_string(),
            })?;

        // If search string is empty, return original string
        if search_val.is_empty() {
            return Ok(Value::String(string_val));
        }

        // Perform replacement
        let result = string_val.replace(&search_val, &replacement_val);

        Ok(Value::String(result))
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn graph_context_required(&self) -> bool {
        false // String functions are pure scalar functions
    }
}

// ==============================================================================
// REVERSE FUNCTION
// ==============================================================================

/// REVERSE function - reverses a string
#[derive(Debug)]
pub struct ReverseFunction;

impl ReverseFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ReverseFunction {
    fn name(&self) -> &str {
        "REVERSE"
    }

    fn description(&self) -> &str {
        "Reverses the characters in a string"
    }

    fn argument_count(&self) -> usize {
        1 // REVERSE(string)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        context.validate_argument_count(1)?;

        let value = context.get_argument(0)?;

        if value.is_null() {
            return Ok(Value::Null);
        }

        // Convert to string
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(Value::Null),
            }
        };

        // Reverse the string using proper Unicode handling
        let reversed: String = string_val.chars().rev().collect();

        Ok(Value::String(reversed))
    }

    fn return_type(&self) -> &str {
        "String"
    }

    fn graph_context_required(&self) -> bool {
        false // String functions are pure scalar functions
    }
}
