// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Text search and fuzzy matching functions
//!
//! This module contains text processing and search functions:
//! - FUZZY_MATCH: Approximate string matching with edit distance
//! - FUZZY_SEARCH: Search with similarity threshold
//! - CONTAINS_FUZZY: Fuzzy substring search
//! - SIMILARITY_SCORE: Levenshtein distance based similarity (0.0-1.0)

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

// ==============================================================================
// LEVENSHTEIN DISTANCE CALCULATION
// ==============================================================================

/// Calculate Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1, // deletion
                    matrix[i][j - 1] + 1, // insertion
                ),
                matrix[i - 1][j - 1] + cost, // substitution
            );
        }
    }

    matrix[len1][len2]
}

/// Calculate similarity score (0.0 to 1.0)
fn similarity_score(s1: &str, s2: &str) -> f64 {
    let distance = levenshtein_distance(s1, s2) as f64;
    let max_len = s1.len().max(s2.len()) as f64;

    if max_len == 0.0 {
        return 1.0; // Both empty strings are identical
    }

    1.0 - (distance / max_len)
}

// ==============================================================================
// FUZZY_MATCH FUNCTION
// ==============================================================================

/// FUZZY_MATCH function - returns true if strings are similar within edit distance threshold
#[derive(Debug)]
pub struct FuzzyMatchFunction;

impl FuzzyMatchFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for FuzzyMatchFunction {
    fn name(&self) -> &str {
        "FUZZY_MATCH"
    }

    fn description(&self) -> &str {
        "Returns true if two strings are similar within edit distance threshold. FUZZY_MATCH(str1, str2, max_distance)"
    }

    fn argument_count(&self) -> usize {
        3 // FUZZY_MATCH(string1, string2, max_distance)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let str1 = context.get_argument(0)?;
        let str2 = context.get_argument(1)?;
        let max_distance_val = context.get_argument(2)?;

        if str1.is_null() || str2.is_null() || max_distance_val.is_null() {
            return Ok(Value::Boolean(false));
        }

        let s1 = str1
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a string".to_string(),
            })?;

        let s2 = str2
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Second argument must be a string".to_string(),
            })?;

        let max_dist =
            max_distance_val
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Maximum distance must be a number".to_string(),
                })? as usize;

        let distance = levenshtein_distance(&s1.to_lowercase(), &s2.to_lowercase());
        Ok(Value::Boolean(distance <= max_dist))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// SIMILARITY_SCORE FUNCTION
// ==============================================================================

/// SIMILARITY_SCORE function - returns similarity score from 0.0 to 1.0
#[derive(Debug)]
pub struct SimilarityScoreFunction;

impl SimilarityScoreFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SimilarityScoreFunction {
    fn name(&self) -> &str {
        "SIMILARITY_SCORE"
    }

    fn description(&self) -> &str {
        "Returns similarity score (0.0-1.0) between two strings. 1.0 = identical, 0.0 = completely different. SIMILARITY_SCORE(str1, str2)"
    }

    fn argument_count(&self) -> usize {
        2 // SIMILARITY_SCORE(string1, string2)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let str1 = context.get_argument(0)?;
        let str2 = context.get_argument(1)?;

        if str1.is_null() || str2.is_null() {
            return Ok(Value::Null);
        }

        let s1 = str1
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a string".to_string(),
            })?;

        let s2 = str2
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Second argument must be a string".to_string(),
            })?;

        let score = similarity_score(&s1.to_lowercase(), &s2.to_lowercase());
        Ok(Value::Number(score))
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// HYBRID_SEARCH FUNCTION
// ==============================================================================

/// HYBRID_SEARCH function - combines multiple search strategies
/// Returns a combined score based on exact match, fuzzy match, and similarity
#[derive(Debug)]
pub struct HybridSearchFunction;

impl HybridSearchFunction {
    pub fn new() -> Self {
        Self
    }

    /// Calculate hybrid score combining multiple strategies
    fn calculate_hybrid_score(
        text: &str,
        query: &str,
        exact_weight: f64,
        fuzzy_weight: f64,
        similarity_weight: f64,
    ) -> f64 {
        let text_lower = text.to_lowercase();
        let query_lower = query.to_lowercase();

        // Strategy 1: Exact substring match (highest priority)
        let exact_score = if text_lower.contains(&query_lower) {
            1.0
        } else {
            0.0
        };

        // Strategy 2: Fuzzy substring match (check for close matches)
        let mut fuzzy_score: f64 = 0.0;
        let query_len = query_lower.len();
        if query_len > 0 && text_lower.len() >= query_len {
            for i in 0..=(text_lower.len().saturating_sub(query_len)) {
                let substring = &text_lower[i..i + query_len];
                let distance = levenshtein_distance(substring, &query_lower) as f64;
                // Normalize to 0-1 where 1 is perfect match
                let score = 1.0 - (distance / query_len as f64);
                fuzzy_score = fuzzy_score.max(score);
            }
        }

        // Strategy 3: Overall similarity (word-level matching)
        let similarity = similarity_score(&text_lower, &query_lower);

        // Combine scores with weights (normalized)
        let total_weight = exact_weight + fuzzy_weight + similarity_weight;
        if total_weight == 0.0 {
            return 0.0;
        }

        (exact_score * exact_weight + fuzzy_score * fuzzy_weight + similarity * similarity_weight)
            / total_weight
    }
}

impl Function for HybridSearchFunction {
    fn name(&self) -> &str {
        "HYBRID_SEARCH"
    }

    fn description(&self) -> &str {
        "Returns combined score using exact match, fuzzy match, and similarity. HYBRID_SEARCH(text, query) or HYBRID_SEARCH(text, query, exact_weight, fuzzy_weight, similarity_weight)"
    }

    fn argument_count(&self) -> usize {
        2 // Min args: text and query; max args: text, query, and three weights
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let text = context.get_argument(0)?;
        let query = context.get_argument(1)?;

        if text.is_null() || query.is_null() {
            return Ok(Value::Null);
        }

        let text_str = text
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a string".to_string(),
            })?;

        let query_str = query
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Second argument must be a string".to_string(),
            })?;

        // Default weights: equal distribution
        let (exact_weight, fuzzy_weight, similarity_weight) = if context.argument_count() >= 5 {
            let exact_w = context.get_argument(2)?.as_number().ok_or_else(|| {
                FunctionError::InvalidArgumentType {
                    message: "Exact weight must be a number".to_string(),
                }
            })?;
            let fuzzy_w = context.get_argument(3)?.as_number().ok_or_else(|| {
                FunctionError::InvalidArgumentType {
                    message: "Fuzzy weight must be a number".to_string(),
                }
            })?;
            let similarity_w = context.get_argument(4)?.as_number().ok_or_else(|| {
                FunctionError::InvalidArgumentType {
                    message: "Similarity weight must be a number".to_string(),
                }
            })?;
            (exact_w, fuzzy_w, similarity_w)
        } else {
            // Default: 0.4 exact, 0.4 fuzzy, 0.2 similarity
            (0.4, 0.4, 0.2)
        };

        let score = Self::calculate_hybrid_score(
            &text_str,
            &query_str,
            exact_weight,
            fuzzy_weight,
            similarity_weight,
        );

        Ok(Value::Number(score))
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// KEYWORD_MATCH FUNCTION
// ==============================================================================

/// KEYWORD_MATCH function - matches multiple keywords with OR/AND logic
#[derive(Debug)]
pub struct KeywordMatchFunction;

impl KeywordMatchFunction {
    pub fn new() -> Self {
        Self
    }

    /// Check if text contains any of the keywords (OR logic)
    fn contains_any_keyword(text: &str, keywords: &[&str]) -> bool {
        let text_lower = text.to_lowercase();
        keywords.iter().any(|keyword| {
            let keyword_lower = keyword.to_lowercase();
            text_lower.contains(&keyword_lower)
        })
    }

    /// Check if text contains all of the keywords (AND logic)
    fn contains_all_keywords(text: &str, keywords: &[&str]) -> bool {
        let text_lower = text.to_lowercase();
        keywords.iter().all(|keyword| {
            let keyword_lower = keyword.to_lowercase();
            text_lower.contains(&keyword_lower)
        })
    }
}

impl Function for KeywordMatchFunction {
    fn name(&self) -> &str {
        "KEYWORD_MATCH"
    }

    fn description(&self) -> &str {
        "Match text against multiple keywords. Returns true if any keyword matches (OR logic). KEYWORD_MATCH(text, 'keyword1', 'keyword2', ...)"
    }

    fn argument_count(&self) -> usize {
        2 // Minimum: text and one keyword
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.argument_count() < 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.argument_count(),
            });
        }

        let text = context.get_argument(0)?;
        if text.is_null() {
            return Ok(Value::Boolean(false));
        }

        let text_str = text
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a string".to_string(),
            })?;

        // Collect all keywords from remaining arguments
        let mut keywords = Vec::new();
        for i in 1..context.argument_count() {
            let keyword_val = context.get_argument(i)?;
            if !keyword_val.is_null() {
                let keyword_str =
                    keyword_val
                        .as_string()
                        .ok_or_else(|| FunctionError::InvalidArgumentType {
                            message: format!("Keyword {} must be a string", i),
                        })?;
                keywords.push(keyword_str);
            }
        }

        // Return true if any keyword matches
        let result = Self::contains_any_keyword(&text_str, &keywords);
        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// KEYWORD_MATCH_ALL FUNCTION
// ==============================================================================

/// KEYWORD_MATCH_ALL function - matches all keywords with AND logic
#[derive(Debug)]
pub struct KeywordMatchAllFunction;

impl KeywordMatchAllFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for KeywordMatchAllFunction {
    fn name(&self) -> &str {
        "KEYWORD_MATCH_ALL"
    }

    fn description(&self) -> &str {
        "Match text against multiple keywords. Returns true only if all keywords match (AND logic). KEYWORD_MATCH_ALL(text, 'keyword1', 'keyword2', ...)"
    }

    fn argument_count(&self) -> usize {
        2 // Minimum: text and one keyword
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.argument_count() < 2 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 2,
                actual: context.argument_count(),
            });
        }

        let text = context.get_argument(0)?;
        if text.is_null() {
            return Ok(Value::Boolean(false));
        }

        let text_str = text
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "First argument must be a string".to_string(),
            })?;

        // Collect all keywords from remaining arguments
        let mut keywords = Vec::new();
        for i in 1..context.argument_count() {
            let keyword_val = context.get_argument(i)?;
            if !keyword_val.is_null() {
                let keyword_str =
                    keyword_val
                        .as_string()
                        .ok_or_else(|| FunctionError::InvalidArgumentType {
                            message: format!("Keyword {} must be a string", i),
                        })?;
                keywords.push(keyword_str);
            }
        }

        // Return true only if all keywords match
        let result = KeywordMatchFunction::contains_all_keywords(&text_str, &keywords);
        Ok(Value::Boolean(result))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// WEIGHTED_SEARCH FUNCTION
// ==============================================================================

/// WEIGHTED_SEARCH function - assigns weights to different search strategies
#[derive(Debug)]
pub struct WeightedSearchFunction;

impl WeightedSearchFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for WeightedSearchFunction {
    fn name(&self) -> &str {
        "WEIGHTED_SEARCH"
    }

    fn description(&self) -> &str {
        "Calculate weighted search score with customizable strategy weights. WEIGHTED_SEARCH(text, query, exact_weight, fuzzy_weight, similarity_weight)"
    }

    fn argument_count(&self) -> usize {
        5 // text, query, exact_weight, fuzzy_weight, similarity_weight
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.argument_count() < 5 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 5,
                actual: context.argument_count(),
            });
        }

        let text = context.get_argument(0)?;
        let query = context.get_argument(1)?;
        let exact_weight = context.get_argument(2)?;
        let fuzzy_weight = context.get_argument(3)?;
        let similarity_weight = context.get_argument(4)?;

        if text.is_null() || query.is_null() {
            return Ok(Value::Null);
        }

        let text_str = text
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Text must be a string".to_string(),
            })?;

        let query_str = query
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Query must be a string".to_string(),
            })?;

        let exact_w =
            exact_weight
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Exact weight must be a number".to_string(),
                })?;

        let fuzzy_w =
            fuzzy_weight
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Fuzzy weight must be a number".to_string(),
                })?;

        let similarity_w =
            similarity_weight
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Similarity weight must be a number".to_string(),
                })?;

        let score = HybridSearchFunction::calculate_hybrid_score(
            &text_str,
            &query_str,
            exact_w,
            fuzzy_w,
            similarity_w,
        );
        Ok(Value::Number(score))
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// CONTAINS_FUZZY FUNCTION
// ==============================================================================

/// CONTAINS_FUZZY function - checks if text contains query with fuzzy matching
#[derive(Debug)]
pub struct ContainsFuzzyFunction;

impl ContainsFuzzyFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for ContainsFuzzyFunction {
    fn name(&self) -> &str {
        "CONTAINS_FUZZY"
    }

    fn description(&self) -> &str {
        "Returns true if text contains query as substring with fuzzy matching. CONTAINS_FUZZY(text, query, max_distance)"
    }

    fn argument_count(&self) -> usize {
        3 // CONTAINS_FUZZY(text, query, max_distance)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let text = context.get_argument(0)?;
        let query = context.get_argument(1)?;
        let max_distance_val = context.get_argument(2)?;

        if text.is_null() || query.is_null() || max_distance_val.is_null() {
            return Ok(Value::Boolean(false));
        }

        let text_str = text
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Text must be a string".to_string(),
            })?;

        let query_str = query
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Query must be a string".to_string(),
            })?;

        let max_dist =
            max_distance_val
                .as_number()
                .ok_or_else(|| FunctionError::InvalidArgumentType {
                    message: "Maximum distance must be a number".to_string(),
                })? as usize;

        let text_lower = text_str.to_lowercase();
        let query_lower = query_str.to_lowercase();

        // Check if query is exact substring first
        if text_lower.contains(&query_lower) {
            return Ok(Value::Boolean(true));
        }

        // Try fuzzy matching on all substrings of same length as query
        let query_len = query_lower.len();
        if query_len == 0 {
            return Ok(Value::Boolean(true)); // Empty query matches everything
        }

        for i in 0..=(text_lower.len().saturating_sub(query_len)) {
            let substring = &text_lower[i..i + query_len];
            let distance = levenshtein_distance(substring, &query_lower);
            if distance <= max_dist {
                return Ok(Value::Boolean(true));
            }
        }

        Ok(Value::Boolean(false))
    }

    fn return_type(&self) -> &str {
        "Boolean"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}

// ==============================================================================
// FUZZY_SEARCH FUNCTION
// ==============================================================================

/// FUZZY_SEARCH function - returns similarity score for string matching
#[derive(Debug)]
pub struct FuzzySearchFunction;

impl FuzzySearchFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for FuzzySearchFunction {
    fn name(&self) -> &str {
        "FUZZY_SEARCH"
    }

    fn description(&self) -> &str {
        "Returns similarity score for search query in text. Useful for ranking search results. FUZZY_SEARCH(text, query)"
    }

    fn argument_count(&self) -> usize {
        2 // FUZZY_SEARCH(text, query)
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        let text = context.get_argument(0)?;
        let query = context.get_argument(1)?;

        if text.is_null() || query.is_null() {
            return Ok(Value::Null);
        }

        let text_str = text
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Text must be a string".to_string(),
            })?;

        let query_str = query
            .as_string()
            .ok_or_else(|| FunctionError::InvalidArgumentType {
                message: "Query must be a string".to_string(),
            })?;

        let text_lower = text_str.to_lowercase();
        let query_lower = query_str.to_lowercase();

        // Exact substring match gets highest score
        if text_lower.contains(&query_lower) {
            return Ok(Value::Number(1.0));
        }

        // Find best match across all substrings
        let query_len = query_lower.len();
        if query_len == 0 {
            return Ok(Value::Number(1.0));
        }

        let mut best_score: f64 = 0.0;
        if text_lower.len() >= query_len {
            for i in 0..=(text_lower.len() - query_len) {
                let substring = &text_lower[i..i + query_len];
                let distance = levenshtein_distance(substring, &query_lower) as f64;
                let score = 1.0 - (distance / query_len as f64);
                best_score = best_score.max(score);
            }
        } else {
            // Text shorter than query, use overall similarity
            best_score = similarity_score(&text_lower, &query_lower);
        }

        Ok(Value::Number(best_score))
    }

    fn return_type(&self) -> &str {
        "Number"
    }

    fn graph_context_required(&self) -> bool {
        false
    }
}
