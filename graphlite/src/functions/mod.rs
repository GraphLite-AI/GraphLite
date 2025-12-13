// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Function execution system for query processing
//!
//! This module provides a generic function execution framework. Simply add new
//! functions by implementing the Function trait and registering them.

mod aggregate_functions;
mod function_trait;
mod graph_functions;
pub mod list_functions;
mod mathematical_functions;
mod null_functions;
mod numeric_functions;
mod special_functions;
mod string_functions;
mod temporal_functions;
mod text_search_functions;
mod timezone_functions;

pub use function_trait::{Function, FunctionContext};

use std::collections::HashMap;

/// Registry of all available functions
#[derive(Debug)]
pub struct FunctionRegistry {
    functions: HashMap<String, Box<dyn Function + 'static>>,
}

impl FunctionRegistry {
    /// Create a new function registry with default functions
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };

        // Register functions - just add them here
        registry.register("COUNT", Box::new(aggregate_functions::CountFunction::new()));
        registry.register(
            "AVERAGE",
            Box::new(aggregate_functions::AverageFunction::new()),
        );
        registry.register("AVG", Box::new(aggregate_functions::AverageFunction::new())); // Alias
        registry.register("SUM", Box::new(aggregate_functions::SumFunction::new()));
        registry.register("MIN", Box::new(aggregate_functions::MinFunction::new()));
        registry.register("MAX", Box::new(aggregate_functions::MaxFunction::new()));
        registry.register(
            "COLLECT",
            Box::new(aggregate_functions::CollectFunction::new()),
        );
        registry.register("UPPER", Box::new(string_functions::UpperFunction::new()));
        registry.register("LOWER", Box::new(string_functions::LowerFunction::new()));
        registry.register("ROUND", Box::new(numeric_functions::RoundFunction::new()));
        registry.register("TRIM", Box::new(string_functions::TrimFunction::new()));
        registry.register(
            "SUBSTRING",
            Box::new(string_functions::SubstringFunction::new()),
        );
        registry.register(
            "REPLACE",
            Box::new(string_functions::ReplaceFunction::new()),
        );
        registry.register(
            "REVERSE",
            Box::new(string_functions::ReverseFunction::new()),
        );

        // Register temporal functions
        registry.register(
            "DATETIME",
            Box::new(temporal_functions::DateTimeFunction::new()),
        );
        registry.register("NOW", Box::new(temporal_functions::NowFunction::new()));
        registry.register(
            "DURATION",
            Box::new(temporal_functions::DurationFunction::new()),
        );
        registry.register(
            "DURATION_NUMERIC",
            Box::new(temporal_functions::DurationNumericFunction::new()),
        );
        registry.register(
            "TIME_WINDOW",
            Box::new(temporal_functions::TimeWindowFunction::new()),
        );

        // Register standard SQL temporal convenience functions
        registry.register(
            "CURRENT_DATE",
            Box::new(temporal_functions::CurrentDateFunction::new()),
        );
        registry.register(
            "CURRENT_TIME",
            Box::new(temporal_functions::CurrentTimeFunction::new()),
        );
        registry.register(
            "EXTRACT",
            Box::new(temporal_functions::ExtractFunction::new()),
        );

        // Register timezone functions
        registry.register(
            "AT_TIME_ZONE",
            Box::new(timezone_functions::AtTimeZoneFunction::new()),
        );
        registry.register(
            "CONVERT_TZ",
            Box::new(timezone_functions::ConvertTzFunction::new()),
        );
        registry.register(
            "TIMEZONE",
            Box::new(timezone_functions::TimezoneFunction::new()),
        );
        registry.register(
            "EXTRACT_TIMEZONE",
            Box::new(timezone_functions::ExtractTimezoneFunction::new()),
        );
        registry.register(
            "GET_TIMEZONE_NAME",
            Box::new(timezone_functions::GetTimezoneNameFunction::new()),
        );
        registry.register(
            "GET_TIMEZONE_ABBREVIATION",
            Box::new(timezone_functions::GetTimezoneAbbreviationFunction::new()),
        );
        registry.register(
            "GET_TIMEZONE_OFFSET",
            Box::new(timezone_functions::GetTimezoneOffsetFunction::new()),
        );
        registry.register(
            "DATE_ADD",
            Box::new(temporal_functions::DateAddFunction::new()),
        );
        registry.register(
            "DATE_SUB",
            Box::new(temporal_functions::DateSubFunction::new()),
        );

        // Register timezone-aware functions
        registry.register(
            "AT_TIME_ZONE",
            Box::new(timezone_functions::AtTimeZoneFunction::new()),
        );
        registry.register(
            "CONVERT_TZ",
            Box::new(timezone_functions::ConvertTzFunction::new()),
        );
        registry.register(
            "TIMEZONE",
            Box::new(timezone_functions::TimezoneFunction::new()),
        );
        registry.register(
            "EXTRACT_TIMEZONE",
            Box::new(timezone_functions::ExtractTimezoneFunction::new()),
        );

        // Register mathematical functions
        registry.register("ABS", Box::new(mathematical_functions::AbsFunction::new()));
        registry.register(
            "CEIL",
            Box::new(mathematical_functions::CeilFunction::new()),
        );
        registry.register(
            "CEILING",
            Box::new(mathematical_functions::CeilFunction::new()),
        ); // Alias
        registry.register(
            "FLOOR",
            Box::new(mathematical_functions::FloorFunction::new()),
        );
        registry.register(
            "SQRT",
            Box::new(mathematical_functions::SqrtFunction::new()),
        );
        registry.register(
            "POWER",
            Box::new(mathematical_functions::PowerFunction::new()),
        );
        registry.register(
            "POW",
            Box::new(mathematical_functions::PowerFunction::new()),
        ); // Alias
        registry.register("LOG", Box::new(mathematical_functions::LogFunction::new()));
        registry.register(
            "LOG10",
            Box::new(mathematical_functions::Log10Function::new()),
        );
        registry.register("EXP", Box::new(mathematical_functions::ExpFunction::new()));
        registry.register("SIN", Box::new(mathematical_functions::SinFunction::new()));
        registry.register("COS", Box::new(mathematical_functions::CosFunction::new()));
        registry.register("TAN", Box::new(mathematical_functions::TanFunction::new()));
        registry.register("PI", Box::new(mathematical_functions::PiFunction::new()));
        registry.register(
            "SIGN",
            Box::new(mathematical_functions::SignFunction::new()),
        );
        registry.register("MOD", Box::new(mathematical_functions::ModFunction::new()));

        // Register null handling functions
        registry.register("NULLIF", Box::new(null_functions::NullIfFunction::new()));
        registry.register(
            "COALESCE",
            Box::new(null_functions::CoalesceFunction::new()),
        );

        // Register advanced list functions
        registry.register(
            "LIST_CONTAINS",
            Box::new(list_functions::ListContainsFunction::new()),
        );
        registry.register(
            "LIST_SLICE",
            Box::new(list_functions::ListSliceFunction::new()),
        );
        registry.register(
            "LIST_APPEND",
            Box::new(list_functions::ListAppendFunction::new()),
        );
        registry.register(
            "LIST_PREPEND",
            Box::new(list_functions::ListPrependFunction::new()),
        );
        registry.register(
            "LIST_LENGTH",
            Box::new(list_functions::ListLengthFunction::new()),
        );
        registry.register(
            "LIST_REVERSE",
            Box::new(list_functions::ListReverseFunction::new()),
        );

        // Register ISO GQL special functions (predicates)
        registry.register(
            "ALL_DIFFERENT",
            Box::new(special_functions::AllDifferentFunction::new()),
        );
        registry.register("SAME", Box::new(special_functions::SameFunction::new()));
        registry.register(
            "PROPERTY_EXISTS",
            Box::new(special_functions::PropertyExistsFunction::new()),
        );

        // Register ISO GQL graph functions
        registry.register("LABELS", Box::new(graph_functions::LabelsFunction::new()));
        registry.register("TYPE", Box::new(graph_functions::TypeFunction::new()));
        registry.register("ID", Box::new(graph_functions::IdFunction::new()));
        registry.register("KEYS", Box::new(graph_functions::KeysFunction::new()));
        registry.register(
            "PROPERTIES",
            Box::new(graph_functions::PropertiesFunction::new()),
        );
        registry.register("SIZE", Box::new(graph_functions::SizeFunction::new()));
        registry.register(
            "INFERRED_LABELS",
            Box::new(graph_functions::InferredLabelsFunction::new()),
        );

        // Register text search and fuzzy matching functions
        registry.register(
            "FT_FUZZY_MATCH",
            Box::new(text_search_functions::FuzzyMatchFunction::new()),
        );
        registry.register(
            "FT_SIMILARITY_SCORE",
            Box::new(text_search_functions::SimilarityScoreFunction::new()),
        );
        registry.register(
            "FT_CONTAINS_FUZZY",
            Box::new(text_search_functions::ContainsFuzzyFunction::new()),
        );
        registry.register(
            "FT_FUZZY_SEARCH",
            Box::new(text_search_functions::FuzzySearchFunction::new()),
        );

        // Register hybrid search functions
        registry.register(
            "FT_HYBRID_SEARCH",
            Box::new(text_search_functions::HybridSearchFunction::new()),
        );
        registry.register(
            "FT_KEYWORD_MATCH",
            Box::new(text_search_functions::KeywordMatchFunction::new()),
        );
        registry.register(
            "FT_KEYWORD_MATCH_ALL",
            Box::new(text_search_functions::KeywordMatchAllFunction::new()),
        );
        registry.register(
            "FT_WEIGHTED_SEARCH",
            Box::new(text_search_functions::WeightedSearchFunction::new()),
        );

        registry
    }

    /// Register a new function
    pub fn register(&mut self, name: &str, function: Box<dyn Function + 'static>) {
        self.functions.insert(name.to_uppercase(), function);
    }

    /// Get a function by name
    pub fn get(&self, name: &str) -> Option<&dyn Function> {
        self.functions.get(&name.to_uppercase()).map(|f| f.as_ref())
    }

    /// Check if a function exists
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function registry introspection (see ROADMAP.md §8)
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_uppercase())
    }

    /// Get all available function names
    pub fn function_names(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    /// List all functions with their metadata
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function registry introspection (see ROADMAP.md §8)
    pub fn list_functions(&self) -> impl Iterator<Item = (String, &Box<dyn Function>)> {
        self.functions
            .iter()
            .map(|(name, func)| (name.clone(), func))
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_functions_registered() {
        let registry = FunctionRegistry::new();

        // Test case insensitive lookup
        assert!(
            registry.get("upper").is_some(),
            "UPPER function should work"
        );
        assert!(
            registry.get("UPPER").is_some(),
            "UPPER (uppercase) should work"
        );

        log::debug!("Function registry tests passed!");
    }

    #[test]
    fn test_keys_function_registered() {
        let registry = FunctionRegistry::new();

        // Test that KEYS function is registered
        assert!(registry.get("KEYS").is_some(), "KEYS should be registered");
        assert!(
            registry.get("keys").is_some(),
            "keys (lowercase) should work"
        );

        // Test other graph functions are still registered
        assert!(
            registry.get("LABELS").is_some(),
            "LABELS should be registered"
        );
        assert!(
            registry.get("PROPERTIES").is_some(),
            "PROPERTIES should be registered"
        );
        assert!(registry.get("ID").is_some(), "ID should be registered");
        assert!(registry.get("TYPE").is_some(), "TYPE should be registered");

        log::debug!("KEYS function and other graph functions are properly registered!");
    }
}
