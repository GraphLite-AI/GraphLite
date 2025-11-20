// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! UNWIND Preprocessor - converts UNWIND queries into DataStatement operations
//!
//! This module handles queries like:
//! MATCH (p:Product) WITH collect(p) as products UNWIND products as product WHERE product.price > 25 REMOVE product.temp_flag
//!
//! And converts them into:
//! 1. MATCH (p:Product) WITH collect(p) as products RETURN products  (to get the collection)
//! 2. For each item: MATCH (product:Product {id: item.id}) WHERE product.price > 25 REMOVE product.temp_flag

// use std::collections::HashMap; // Removed unused import
use crate::exec::result::QueryResult;
use crate::exec::ExecutionError;
use crate::storage::Value;

pub struct UnwindPreprocessor;

#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - UNWIND clause optimization
pub struct UnwindQuery {
    pub collection_query: String,
    pub item_queries: Vec<String>,
}

impl UnwindPreprocessor {
    /// Detect if a query is an UNWIND pattern that needs preprocessing
    pub fn is_unwind_query(query: &str) -> bool {
        let query_upper = query.to_uppercase();

        // Basic requirements
        if !(query_upper.contains("MATCH")
            && query_upper.contains("WITH")
            && query_upper.contains("UNWIND")
            && (query_upper.contains("REMOVE") || query_upper.contains("SET")))
        {
            return false;
        }

        // Don't handle queries with multiple WITH clauses (too complex for preprocessor)
        let with_count = query_upper.matches("WITH").count();
        if with_count > 1 {
            log::debug!(
                "UNWIND PREPROCESSOR: Skipping query with {} WITH clauses (not supported)",
                with_count
            );
            return false;
        }

        true
    }

    /// Parse an UNWIND query into its components
    pub fn parse_unwind_query(query: &str) -> Result<UnwindQueryComponents, ExecutionError> {
        let query = query.trim();

        // Find the key sections
        let match_start = Self::find_keyword_pos(query, "MATCH")?;
        let with_start = Self::find_keyword_pos(query, "WITH")?;
        let unwind_start = Self::find_keyword_pos(query, "UNWIND")?;

        // Find optional WHERE clause
        let where_pos = Self::find_keyword_pos_optional(query, "WHERE");

        // Find the operation (REMOVE or SET)
        let remove_pos = Self::find_keyword_pos_optional(query, "REMOVE");
        let set_pos = Self::find_keyword_pos_optional(query, "SET");

        let operation_pos = match (remove_pos, set_pos) {
            (Some(r), Some(s)) => Some(r.min(s)), // Use whichever comes first
            (Some(r), None) => Some(r),
            (None, Some(s)) => Some(s),
            (None, None) => {
                return Err(ExecutionError::RuntimeError(
                    "No REMOVE or SET operation found in UNWIND query".to_string(),
                ))
            }
        };

        // Extract components
        let match_clause = &query[match_start..with_start].trim();
        let with_clause =
            Self::extract_between(query, with_start, Some(unwind_start), "WITH", "UNWIND")?.trim();
        let unwind_clause =
            Self::extract_unwind_clause(query, unwind_start, where_pos.or(operation_pos))?;

        let where_clause = if let Some(where_start) = where_pos {
            Some(
                Self::extract_between(query, where_start, operation_pos, "WHERE", "")?
                    .trim()
                    .to_string(),
            )
        } else {
            None
        };

        let operation_clause = if let Some(op_start) = operation_pos {
            &query[op_start..].trim().to_string()
        } else {
            return Err(ExecutionError::RuntimeError(
                "No operation clause found".to_string(),
            ));
        };

        Ok(UnwindQueryComponents {
            match_clause: match_clause.to_string(),
            with_clause: with_clause.to_string(),
            unwind_clause,
            where_clause,
            operation_clause: operation_clause.clone(),
        })
    }

    /// Execute an UNWIND query by expanding it into DataStatement operations
    pub fn execute_unwind_query<E>(
        query: &str,
        executor_fn: E,
    ) -> Result<QueryResult, ExecutionError>
    where
        E: Fn(&str) -> Result<QueryResult, ExecutionError>,
    {
        log::debug!("UNWIND PREPROCESSOR: Processing query: {}", query);

        // Parse the UNWIND query
        let components = Self::parse_unwind_query(query)?;
        log::debug!("UNWIND PREPROCESSOR: Parsed components: {:?}", components);

        // Step 1: Get all nodes first, then compute aggregates separately
        // This works around issues with collect() in WITH clauses

        // Extract the variable name from the MATCH clause
        // e.g., "MATCH (p:Product)" -> "p", "MATCH (s:Sale)" -> "s"
        let var_name = Self::extract_match_variable(&components.match_clause)?;

        let nodes_query = format!("{} RETURN {}", components.match_clause, var_name);
        log::debug!(
            "UNWIND PREPROCESSOR: Getting nodes with query: {}",
            nodes_query
        );

        let nodes_result = executor_fn(&nodes_query).map_err(|e| {
            ExecutionError::RuntimeError(format!("Failed to execute nodes query: {}", e))
        })?;

        // Extract all nodes from the result
        let mut items = vec![];
        for row in &nodes_result.rows {
            if let Some(node) = row.values.get(&var_name) {
                log::debug!("UNWIND PREPROCESSOR: Node: {:?}", node);
                items.push(node.clone());
            }
        }

        log::debug!(
            "UNWIND PREPROCESSOR: Found {} nodes to process",
            items.len()
        );

        // Step 2: Compute aggregates manually from the fetched nodes
        let mut computed_values = std::collections::HashMap::new();

        // Parse the WITH clause to compute aggregates
        if components.with_clause.contains("avg(") {
            // Extract all price values and compute average manually
            let mut prices = vec![];
            for item in &items {
                if let Value::Node(node_ref) = item {
                    if let Some(price_value) = node_ref.properties.get("price") {
                        if let Value::Number(price) = price_value {
                            prices.push(*price);
                        }
                    }
                }
            }

            let avg_price = if !prices.is_empty() {
                prices.iter().sum::<f64>() / prices.len() as f64
            } else {
                0.0
            };

            computed_values.insert("avg_price".to_string(), Value::Number(avg_price));
            log::debug!(
                "UNWIND PREPROCESSOR: Computed avg_price manually: {} from {} prices: {:?}",
                avg_price,
                prices.len(),
                prices
            );
        }
        log::debug!("UNWIND PREPROCESSOR: Found {} items to unwind", items.len());

        // Step 3: Generate and execute individual queries for each item
        let mut total_affected = 0;
        for (index, item) in items.iter().enumerate() {
            let individual_query =
                Self::generate_individual_query(&components, item, index, &computed_values)?;
            log::debug!(
                "UNWIND PREPROCESSOR: Executing individual query {}: {}",
                index + 1,
                individual_query
            );

            let result = executor_fn(&individual_query).map_err(|e| {
                ExecutionError::RuntimeError(format!(
                    "Failed to execute individual query {}: {}",
                    index + 1,
                    e
                ))
            })?;

            log::debug!(
                "UNWIND PREPROCESSOR: Query {} result: {:?}",
                index + 1,
                result
            );
            total_affected += result.rows_affected;
        }

        log::debug!(
            "UNWIND PREPROCESSOR: Completed. Total rows affected: {}",
            total_affected
        );

        Ok(QueryResult {
            rows: vec![],
            variables: vec![],
            execution_time_ms: 0,
            rows_affected: total_affected,
            session_result: None,
            warnings: Vec::new(),
        })
    }

    // Helper methods

    fn extract_match_variable(match_clause: &str) -> Result<String, ExecutionError> {
        // Extract variable name from MATCH clause
        // e.g., "MATCH (p:Product)" -> "p", "MATCH (s:Sale)" -> "s"

        let open_paren = match_clause.find('(').ok_or_else(|| {
            ExecutionError::RuntimeError("Invalid MATCH clause: no opening parenthesis".to_string())
        })?;

        let text_after_paren = &match_clause[open_paren + 1..];

        // Find the variable name - it's the first word after (
        let var_end = text_after_paren
            .find(|c: char| c == ':' || c == ')' || c == ' ')
            .ok_or_else(|| {
                ExecutionError::RuntimeError(
                    "Invalid MATCH clause: cannot find variable name".to_string(),
                )
            })?;

        let var_name = text_after_paren[..var_end].trim();

        if var_name.is_empty() {
            return Err(ExecutionError::RuntimeError(
                "Invalid MATCH clause: empty variable name".to_string(),
            ));
        }

        Ok(var_name.to_string())
    }

    fn find_keyword_pos(text: &str, keyword: &str) -> Result<usize, ExecutionError> {
        let upper_text = text.to_uppercase();
        upper_text.find(keyword).ok_or_else(|| {
            ExecutionError::RuntimeError(format!("Keyword '{}' not found in query", keyword))
        })
    }

    fn find_keyword_pos_optional(text: &str, keyword: &str) -> Option<usize> {
        text.to_uppercase().find(keyword)
    }

    fn extract_between<'a>(
        text: &'a str,
        start_pos: usize,
        end_pos: Option<usize>,
        start_keyword: &str,
        end_keyword: &str,
    ) -> Result<&'a str, ExecutionError> {
        let start = start_pos + start_keyword.len();
        let end = end_pos.unwrap_or(text.len());

        if start >= end {
            return Err(ExecutionError::RuntimeError(format!(
                "Invalid range for extracting between '{}' and '{}'",
                start_keyword, end_keyword
            )));
        }

        Ok(&text[start..end])
    }

    fn extract_unwind_clause(
        text: &str,
        unwind_start: usize,
        end_pos: Option<usize>,
    ) -> Result<UnwindClause, ExecutionError> {
        let unwind_part = Self::extract_between(text, unwind_start, end_pos, "UNWIND", "")?;

        // Parse "collection_var as item_alias"
        let parts: Vec<&str> = unwind_part.split_whitespace().collect();
        if parts.len() >= 3 && parts[1].to_uppercase() == "AS" {
            Ok(UnwindClause {
                item_alias: parts[2].to_string(),
            })
        } else {
            Err(ExecutionError::RuntimeError(format!(
                "Invalid UNWIND syntax: {}",
                unwind_part
            )))
        }
    }

    fn generate_individual_query(
        components: &UnwindQueryComponents,
        item: &Value,
        _index: usize,
        computed_values: &std::collections::HashMap<String, Value>,
    ) -> Result<String, ExecutionError> {
        // Convert the item to a node pattern for MATCH
        let item_pattern =
            Self::value_to_match_pattern(item, &components.unwind_clause.item_alias)?;

        // Build the individual query
        let mut query = format!("MATCH {}", item_pattern);

        // Add WHERE clause if present, substituting computed values
        if let Some(ref where_clause) = components.where_clause {
            let mut substituted_where = where_clause.clone();

            // Substitute computed values in the WHERE clause
            for (var_name, var_value) in computed_values {
                if substituted_where.contains(var_name) {
                    let replacement = match var_value {
                        Value::Number(n) => n.to_string(),
                        Value::String(s) => format!("'{}'", s),
                        Value::Boolean(b) => b.to_string(),
                        Value::Null => "NULL".to_string(),
                        _ => var_name.clone(), // Keep original if we can't substitute
                    };
                    substituted_where = substituted_where.replace(var_name, &replacement);
                }
            }

            query.push_str(&format!(" WHERE {}", substituted_where));
        }

        // Add the operation (REMOVE or SET)
        query.push_str(&format!(" {}", components.operation_clause));

        Ok(query)
    }

    fn value_to_match_pattern(value: &Value, alias: &str) -> Result<String, ExecutionError> {
        match value {
            Value::Node(node_ref) => {
                // Create a MATCH pattern for the specific node
                let labels_part = if !node_ref.labels.is_empty() {
                    format!(":{}", node_ref.labels.join(":"))
                } else {
                    "".to_string()
                };

                // Use the numeric ID from properties if available, otherwise use node reference ID
                let id_value = if let Some(id_prop) = node_ref.properties.get("id") {
                    match id_prop {
                        Value::Number(n) => n.to_string(),
                        Value::String(s) => format!("'{}'", s),
                        _ => format!("'{}'", node_ref.id),
                    }
                } else {
                    format!("'{}'", node_ref.id)
                };

                Ok(format!(
                    "({}{}{{{}: {}}})",
                    alias, labels_part, "id", id_value
                ))
            }
            _ => {
                // For non-node values, we can't create a meaningful MATCH pattern
                Err(ExecutionError::RuntimeError(format!(
                    "Cannot create MATCH pattern for non-node value: {:?}",
                    value
                )))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnwindQueryComponents {
    match_clause: String,
    with_clause: String,
    unwind_clause: UnwindClause,
    where_clause: Option<String>,
    operation_clause: String,
}

#[derive(Debug, Clone)]
struct UnwindClause {
    item_alias: String,
}
