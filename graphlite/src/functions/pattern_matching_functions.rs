// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Pattern matching function implementations
//!
//! This module contains pattern matching functions for text search:
//! - FT_STARTS_WITH: Prefix matching (starts with pattern)
//! - FT_ENDS_WITH: Suffix matching (ends with pattern)
//! - FT_WILDCARD: Wildcard pattern matching (* and ?)
//! - FT_REGEX: Regular expression pattern matching
//! - FT_PHRASE_PREFIX: Phrase prefix matching (autocomplete)
//!
//! These functions use the FT_ prefix to indicate they are full-text search functions.
//! Phase 1 implementation uses string operations for immediate functionality.
//! Phase 2 will integrate Tantivy-backed indexed queries for better performance.

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;
use regex::Regex;

// ==============================================================================
// FT_STARTS_WITH FUNCTION
// ==============================================================================

/// FT_STARTS_WITH function - checks if string starts with a given prefix
///
/// Syntax: FT_STARTS_WITH(text, prefix)
/// Returns: Boolean - true if text starts with prefix, false otherwise
///
/// Examples:
/// - FT_STARTS_WITH("Hello World", "Hello") → true
/// - FT_STARTS_WITH("Hello World", "World") → false
/// - FT_STARTS_WITH("user@example.com", "user") → true
#[derive(Debug)]
pub struct StartsWithFunction;

impl StartsWithFunction {
    pub fn new() -> Self {
        Self
    }

    /// Helper method to convert value to string consistently
    fn value_to_string(&self, value: &Value) -> FunctionResult<String> {
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(String::new()),
            }
        };
        Ok(string_val)
    }
}

impl Function for StartsWithFunction {
    fn name(&self) -> &str {
        "FT_STARTS_WITH"
    }

    fn description(&self) -> &str {
        "Returns true if text starts with the given prefix. FT_STARTS_WITH(text, prefix)"
    }

    fn argument_count(&self) -> usize {
        2 // FT_STARTS_WITH(text, prefix)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let text_value = context.get_argument(0)?;
        let prefix_value = context.get_argument(1)?;

        // Handle null values
        if text_value.is_null() || prefix_value.is_null() {
            return Ok(Value::Boolean(false));
        }

        // Convert to strings
        let text = self.value_to_string(text_value)?;
        let prefix = self.value_to_string(prefix_value)?;

        // Check if text starts with prefix
        let result = text.starts_with(&prefix);

        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false // Pattern matching functions are pure scalar functions
    }
}

// ==============================================================================
// FT_ENDS_WITH FUNCTION
// ==============================================================================

/// FT_ENDS_WITH function - checks if string ends with a given suffix
///
/// Syntax: FT_ENDS_WITH(text, suffix)
/// Returns: Boolean - true if text ends with suffix, false otherwise
///
/// Examples:
/// - FT_ENDS_WITH("document.pdf", ".pdf") → true
/// - FT_ENDS_WITH("document.pdf", ".doc") → false
/// - FT_ENDS_WITH("user@example.com", ".com") → true
#[derive(Debug)]
pub struct EndsWithFunction;

impl EndsWithFunction {
    pub fn new() -> Self {
        Self
    }

    /// Helper method to convert value to string consistently
    fn value_to_string(&self, value: &Value) -> FunctionResult<String> {
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(String::new()),
            }
        };
        Ok(string_val)
    }
}

impl Function for EndsWithFunction {
    fn name(&self) -> &str {
        "FT_ENDS_WITH"
    }

    fn description(&self) -> &str {
        "Returns true if text ends with the given suffix. FT_ENDS_WITH(text, suffix)"
    }

    fn argument_count(&self) -> usize {
        2 // FT_ENDS_WITH(text, suffix)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let text_value = context.get_argument(0)?;
        let suffix_value = context.get_argument(1)?;

        // Handle null values
        if text_value.is_null() || suffix_value.is_null() {
            return Ok(Value::Boolean(false));
        }

        // Convert to strings
        let text = self.value_to_string(text_value)?;
        let suffix = self.value_to_string(suffix_value)?;

        // Check if text ends with suffix
        let result = text.ends_with(&suffix);

        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false // Pattern matching functions are pure scalar functions
    }
}

// ==============================================================================
// FT_WILDCARD FUNCTION
// ==============================================================================

/// FT_WILDCARD function - matches text against a wildcard pattern
///
/// Supports wildcards:
/// - * matches zero or more characters
/// - ? matches exactly one character
///
/// Syntax: FT_WILDCARD(text, pattern)
/// Returns: Boolean - true if text matches pattern, false otherwise
///
/// Examples:
/// - FT_WILDCARD("Hello World", "Hello*") → true
/// - FT_WILDCARD("document.pdf", "*.pdf") → true
/// - FT_WILDCARD("test123", "test???") → true
/// - FT_WILDCARD("user_admin", "*admin") → true
#[derive(Debug)]
pub struct WildcardFunction;

impl WildcardFunction {
    pub fn new() -> Self {
        Self
    }

    /// Helper method to convert value to string consistently
    fn value_to_string(&self, value: &Value) -> FunctionResult<String> {
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(String::new()),
            }
        };
        Ok(string_val)
    }

    /// Convert wildcard pattern to regex pattern
    /// - * becomes .*
    /// - ? becomes .
    /// - Escape other regex special characters
    fn wildcard_to_regex(&self, pattern: &str) -> String {
        let mut regex_pattern = String::new();
        regex_pattern.push('^'); // Anchor to start

        for ch in pattern.chars() {
            match ch {
                '*' => regex_pattern.push_str(".*"),
                '?' => regex_pattern.push('.'),
                // Escape regex special characters
                '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' |
                '^' | '$' | '|' | '\\' => {
                    regex_pattern.push('\\');
                    regex_pattern.push(ch);
                }
                _ => regex_pattern.push(ch),
            }
        }

        regex_pattern.push('$'); // Anchor to end
        regex_pattern
    }
}

impl Function for WildcardFunction {
    fn name(&self) -> &str {
        "FT_WILDCARD"
    }

    fn description(&self) -> &str {
        "Matches text against wildcard pattern (* and ?). FT_WILDCARD(text, pattern)"
    }

    fn argument_count(&self) -> usize {
        2 // FT_WILDCARD(text, pattern)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let text_value = context.get_argument(0)?;
        let pattern_value = context.get_argument(1)?;

        // Handle null values
        if text_value.is_null() || pattern_value.is_null() {
            return Ok(Value::Boolean(false));
        }

        // Convert to strings
        let text = self.value_to_string(text_value)?;
        let pattern = self.value_to_string(pattern_value)?;

        // Convert wildcard pattern to regex
        let regex_pattern = self.wildcard_to_regex(&pattern);

        // Compile and match regex
        let regex = Regex::new(&regex_pattern).map_err(|e| {
            FunctionError::InvalidArgumentType {
                message: format!("Invalid wildcard pattern: {}", e),
            }
        })?;

        let result = regex.is_match(&text);

        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false // Pattern matching functions are pure scalar functions
    }
}

// ==============================================================================
// FT_REGEX FUNCTION
// ==============================================================================

/// FT_REGEX function - matches text against a regular expression pattern
///
/// Syntax: FT_REGEX(text, pattern)
/// Returns: Boolean - true if text matches regex pattern, false otherwise
///
/// Supports standard regex syntax:
/// - . matches any character
/// - * matches zero or more of preceding
/// - + matches one or more of preceding
/// - ? matches zero or one of preceding
/// - [abc] matches any character in set
/// - [^abc] matches any character not in set
/// - ^ anchors to start
/// - $ anchors to end
/// - | alternation (OR)
/// - () grouping
/// - \d digit, \w word char, \s whitespace
///
/// Examples:
/// - FT_REGEX("test123", "^test[0-9]+$") → true
/// - FT_REGEX("user@example.com", "^[a-z]+@[a-z]+\\.com$") → true
/// - FT_REGEX("ABC-123", "^[A-Z]{3}-[0-9]{3}$") → true
#[derive(Debug)]
pub struct RegexFunction;

impl RegexFunction {
    pub fn new() -> Self {
        Self
    }

    /// Helper method to convert value to string consistently
    fn value_to_string(&self, value: &Value) -> FunctionResult<String> {
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(String::new()),
            }
        };
        Ok(string_val)
    }
}

impl Function for RegexFunction {
    fn name(&self) -> &str {
        "FT_REGEX"
    }

    fn description(&self) -> &str {
        "Matches text against regular expression pattern. FT_REGEX(text, pattern)"
    }

    fn argument_count(&self) -> usize {
        2 // FT_REGEX(text, pattern)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let text_value = context.get_argument(0)?;
        let pattern_value = context.get_argument(1)?;

        // Handle null values
        if text_value.is_null() || pattern_value.is_null() {
            return Ok(Value::Boolean(false));
        }

        // Convert to strings
        let text = self.value_to_string(text_value)?;
        let pattern = self.value_to_string(pattern_value)?;

        // Compile and match regex
        let regex = Regex::new(&pattern).map_err(|e| {
            FunctionError::InvalidArgumentType {
                message: format!("Invalid regex pattern: {}", e),
            }
        })?;

        let result = regex.is_match(&text);

        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false // Pattern matching functions are pure scalar functions
    }
}

// ==============================================================================
// FT_PHRASE_PREFIX FUNCTION
// ==============================================================================

/// FT_PHRASE_PREFIX function - matches phrases with prefix completion
///
/// Used for autocomplete functionality. Matches phrases where the last word
/// is a prefix match.
///
/// Syntax: FT_PHRASE_PREFIX(text, phrase_prefix)
/// Returns: Boolean - true if text contains phrase with prefix, false otherwise
///
/// Examples:
/// - FT_PHRASE_PREFIX("Machine Learning Fundamentals", "Machine Learn") → true
/// - FT_PHRASE_PREFIX("Deep Neural Networks", "Deep Neu") → true
/// - FT_PHRASE_PREFIX("Natural Language Processing", "Natural Lang") → true
///
/// Case-insensitive matching for better autocomplete experience.
#[derive(Debug)]
pub struct PhrasePrefixFunction;

impl PhrasePrefixFunction {
    pub fn new() -> Self {
        Self
    }

    /// Helper method to convert value to string consistently
    fn value_to_string(&self, value: &Value) -> FunctionResult<String> {
        let string_val = if let Some(s) = value.as_string() {
            s.to_string()
        } else if let Some(n) = value.as_number() {
            n.to_string()
        } else {
            match value {
                Value::Boolean(b) => b.to_string(),
                _ => return Ok(String::new()),
            }
        };
        Ok(string_val)
    }

    /// Check if text contains phrase prefix
    /// Algorithm:
    /// 1. Tokenize both text and phrase_prefix into words
    /// 2. For each position in text words:
    ///    a. Check if phrase words match (exact for all but last)
    ///    b. Check if last phrase word is prefix of text word
    fn matches_phrase_prefix(&self, text: &str, phrase_prefix: &str) -> bool {
        // Case-insensitive matching
        let text_lower = text.to_lowercase();
        let phrase_lower = phrase_prefix.to_lowercase();

        // Tokenize into words (split by whitespace and punctuation)
        let text_words: Vec<&str> = text_lower.split_whitespace().collect();
        let phrase_words: Vec<&str> = phrase_lower.split_whitespace().collect();

        if phrase_words.is_empty() {
            return false;
        }

        // Handle single word phrase (prefix match anywhere)
        if phrase_words.len() == 1 {
            let prefix = phrase_words[0];
            return text_words.iter().any(|word| word.starts_with(prefix));
        }

        // Multi-word phrase: scan for matching sequence
        let phrase_len = phrase_words.len();
        for i in 0..=text_words.len().saturating_sub(phrase_len) {
            let mut matched = true;

            // Check all words except the last (exact match)
            for j in 0..phrase_len - 1 {
                if text_words[i + j] != phrase_words[j] {
                    matched = false;
                    break;
                }
            }

            // Check last word (prefix match)
            if matched {
                let last_text_word = text_words[i + phrase_len - 1];
                let last_phrase_word = phrase_words[phrase_len - 1];
                if last_text_word.starts_with(last_phrase_word) {
                    return true;
                }
            }
        }

        false
    }
}

impl Function for PhrasePrefixFunction {
    fn name(&self) -> &str {
        "FT_PHRASE_PREFIX"
    }

    fn description(&self) -> &str {
        "Matches phrases with prefix completion (autocomplete). FT_PHRASE_PREFIX(text, phrase_prefix)"
    }

    fn argument_count(&self) -> usize {
        2 // FT_PHRASE_PREFIX(text, phrase_prefix)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        // Validate argument count
        if context.arguments.len() != 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.arguments.len(),
            });
        }

        let text_value = context.get_argument(0)?;
        let phrase_value = context.get_argument(1)?;

        // Handle null values
        if text_value.is_null() || phrase_value.is_null() {
            return Ok(Value::Boolean(false));
        }

        // Convert to strings
        let text = self.value_to_string(text_value)?;
        let phrase_prefix = self.value_to_string(phrase_value)?;

        // Check phrase prefix match
        let result = self.matches_phrase_prefix(&text, &phrase_prefix);

        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false // Pattern matching functions are pure scalar functions
    }
}

// ==============================================================================
// UNIT TESTS
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_context(args: Vec<Value>) -> FunctionContext {
        FunctionContext {
            rows: Vec::new(),
            variables: HashMap::new(),
            arguments: args,
            storage_manager: None,
            current_graph: None,
            graph_name: None,
        }
    }

    // FT_STARTS_WITH tests
    #[test]
    fn test_starts_with_basic() {
        let func = StartsWithFunction::new();
        let ctx = create_test_context(vec![
            Value::String("Hello World".to_string()),
            Value::String("Hello".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_starts_with_not_matching() {
        let func = StartsWithFunction::new();
        let ctx = create_test_context(vec![
            Value::String("Hello World".to_string()),
            Value::String("World".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_starts_with_null() {
        let func = StartsWithFunction::new();
        let ctx = create_test_context(vec![Value::Null, Value::String("test".to_string())]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    // FT_ENDS_WITH tests
    #[test]
    fn test_ends_with_basic() {
        let func = EndsWithFunction::new();
        let ctx = create_test_context(vec![
            Value::String("document.pdf".to_string()),
            Value::String(".pdf".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_ends_with_not_matching() {
        let func = EndsWithFunction::new();
        let ctx = create_test_context(vec![
            Value::String("document.pdf".to_string()),
            Value::String(".doc".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    // FT_WILDCARD tests
    #[test]
    fn test_wildcard_star() {
        let func = WildcardFunction::new();
        let ctx = create_test_context(vec![
            Value::String("Hello World".to_string()),
            Value::String("Hello*".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_wildcard_question() {
        let func = WildcardFunction::new();
        let ctx = create_test_context(vec![
            Value::String("test123".to_string()),
            Value::String("test???".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_wildcard_mixed() {
        let func = WildcardFunction::new();
        let ctx = create_test_context(vec![
            Value::String("user_admin".to_string()),
            Value::String("user_*".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    // FT_REGEX tests
    #[test]
    fn test_regex_digits() {
        let func = RegexFunction::new();
        let ctx = create_test_context(vec![
            Value::String("test123".to_string()),
            Value::String("^test[0-9]+$".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_regex_email() {
        let func = RegexFunction::new();
        let ctx = create_test_context(vec![
            Value::String("user@example.com".to_string()),
            Value::String("^[a-z]+@[a-z]+\\.com$".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    // FT_PHRASE_PREFIX tests
    #[test]
    fn test_phrase_prefix_single_word() {
        let func = PhrasePrefixFunction::new();
        let ctx = create_test_context(vec![
            Value::String("Machine Learning Fundamentals".to_string()),
            Value::String("Learn".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_phrase_prefix_multi_word() {
        let func = PhrasePrefixFunction::new();
        let ctx = create_test_context(vec![
            Value::String("Machine Learning Fundamentals".to_string()),
            Value::String("Machine Learn".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_phrase_prefix_not_matching() {
        let func = PhrasePrefixFunction::new();
        let ctx = create_test_context(vec![
            Value::String("Deep Neural Networks".to_string()),
            Value::String("Machine Learn".to_string()),
        ]);
        let result = func.execute(&ctx).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }
}
