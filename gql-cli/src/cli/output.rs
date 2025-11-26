// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Result formatting for CLI output

use colored::*;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use graphlite::{QueryResult, Value};

/// Result formatter for different output formats
pub struct ResultFormatter;

impl ResultFormatter {
    /// Format query results in the specified format
    pub fn format(result: &QueryResult, format: crate::cli::commands::OutputFormat) -> String {
        match format {
            crate::cli::commands::OutputFormat::Table => Self::format_table(result),
            crate::cli::commands::OutputFormat::Json => Self::format_json(result),
            crate::cli::commands::OutputFormat::Csv => Self::format_csv(result),
        }
    }

    /// Format results as a table using comfy-table
    fn format_table(result: &QueryResult) -> String {
        // Check if this is a session command
        if result.is_session_command() {
            if let Some(msg) = result.get_session_message() {
                return format!("{}\n", format!("✅ {}", msg).green());
            }
        }

        if result.rows.is_empty() {
            return format!("{}\n", "No results found".yellow());
        }

        let mut output = String::new();

        // Header
        output.push_str(&format!("{}\n", "Query Results".bold().green()));
        output.push_str(&format!(
            "Execution time: {} ms\n",
            result.execution_time_ms
        ));
        output.push_str(&format!("Rows returned: {}\n\n", result.rows.len()));

        // Create table
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        // Table header
        let header_cells: Vec<Cell> = result
            .variables
            .iter()
            .map(|col| Cell::new(col).fg(Color::Green))
            .collect();
        table.set_header(header_cells);

        // Table rows
        for row in &result.rows {
            let row_values: Vec<String> = result
                .variables
                .iter()
                .map(|col| {
                    row.get_value(col)
                        .map(|v| Self::value_to_string(v))
                        .unwrap_or_else(|| "NULL".to_string())
                })
                .collect();
            table.add_row(row_values);
        }

        output.push_str(&table.to_string());
        output.push('\n');

        // Display warnings if any
        if !result.warnings.is_empty() {
            output.push_str(&format!("\n{}\n", "Warnings:".bold().yellow()));
            for (i, warning) in result.warnings.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, warning.yellow()));
            }
        }

        output
    }

    /// Format results as JSON
    fn format_json(result: &QueryResult) -> String {
        // Create a JSON-friendly representation
        let mut json_obj = serde_json::json!({
            "status": "success",
            "columns": result.variables,
            "rows": result.rows.iter().map(|row| {
                let mut row_map = serde_json::Map::new();
                for col in &result.variables {
                    let value = row.get_value(col)
                        .map(|v| Self::value_to_json(v))
                        .unwrap_or(serde_json::Value::Null);
                    row_map.insert(col.clone(), value);
                }
                serde_json::Value::Object(row_map)
            }).collect::<Vec<_>>(),
            "rows_affected": result.rows.len(),
            "execution_time_ms": result.execution_time_ms,
        });

        // Add warnings if any
        if !result.warnings.is_empty() {
            if let serde_json::Value::Object(ref mut map) = json_obj {
                map.insert("warnings".to_string(), serde_json::json!(result.warnings));
            }
        }

        let json_result = json_obj;

        serde_json::to_string_pretty(&json_result).unwrap_or_else(|_| {
            format!("{{\"status\": \"error\", \"error\": \"Could not serialize results to JSON\"}}")
        })
    }

    /// Format results as CSV
    fn format_csv(result: &QueryResult) -> String {
        let mut output = String::new();

        // CSV header
        output.push_str(&result.variables.join(","));
        output.push('\n');

        // CSV rows
        for row in &result.rows {
            let row_values: Vec<String> = result
                .variables
                .iter()
                .map(|col| {
                    row.get_value(col)
                        .map(|v| Self::value_to_csv_string(v))
                        .unwrap_or_else(|| "".to_string())
                })
                .collect();
            output.push_str(&row_values.join(","));
            output.push('\n');
        }

        // Add warnings as CSV comments
        if !result.warnings.is_empty() {
            output.push_str("\n# Warnings:\n");
            for (i, warning) in result.warnings.iter().enumerate() {
                output.push_str(&format!("# {}. {}\n", i + 1, warning));
            }
        }

        output
    }

    /// Convert a Value to a display string
    fn value_to_string(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Null => "NULL".to_string(),
            Value::DateTime(dt) => dt.to_string(),
            Value::DateTimeWithFixedOffset(dt) => dt.to_string(),
            Value::DateTimeWithNamedTz(tz, dt) => format!("{} {}", dt, tz),
            Value::TimeWindow(tw) => format!("TIME_WINDOW({} to {})", tw.start, tw.end),
            Value::Path(path) => format!("{:?}", path),
            Value::Array(arr) | Value::List(arr) => format!(
                "[{}]",
                arr.iter()
                    .map(|v| Self::value_to_string(v))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Vector(vec) => format!(
                "VECTOR[{}]",
                vec.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Node(node) => {
                let labels = if node.labels.is_empty() {
                    String::new()
                } else {
                    format!(":{}", node.labels.join(":"))
                };
                format!("({}{})", node.id, labels)
            }
            Value::Edge(edge) => {
                format!("[{}:{}]", edge.id, edge.label)
            }
            Value::Temporal(temporal) => format!("TEMPORAL({:?})", temporal),
        }
    }

    /// Convert a Value to a JSON value
    fn value_to_json(value: &Value) -> serde_json::Value {
        match value {
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Number(n) => serde_json::json!(n),
            Value::Boolean(b) => serde_json::Value::Bool(*b),
            Value::Null => serde_json::Value::Null,
            Value::DateTime(dt) => serde_json::Value::String(dt.to_string()),
            Value::DateTimeWithFixedOffset(dt) => serde_json::Value::String(dt.to_string()),
            Value::DateTimeWithNamedTz(tz, dt) => serde_json::json!({
                "datetime": dt.to_string(),
                "timezone": tz,
            }),
            Value::TimeWindow(tw) => serde_json::json!({
                "start": tw.start.to_string(),
                "end": tw.end.to_string(),
            }),
            Value::Path(path) => serde_json::json!(format!("{:?}", path)),
            Value::Array(arr) | Value::List(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| Self::value_to_json(v)).collect())
            }
            Value::Vector(vec) => {
                serde_json::Value::Array(vec.iter().map(|v| serde_json::json!(v)).collect())
            }
            Value::Node(node) => {
                serde_json::json!({
                    "type": "node",
                    "id": node.id,
                    "labels": node.labels,
                    "properties": node.properties,
                })
            }
            Value::Edge(edge) => {
                serde_json::json!({
                    "type": "edge",
                    "id": edge.id,
                    "label": edge.label,
                    "from": edge.from_node,
                    "to": edge.to_node,
                    "properties": edge.properties,
                })
            }
            Value::Temporal(temporal) => {
                serde_json::json!(format!("{:?}", temporal))
            }
        }
    }

    /// Convert a Value to a CSV-safe string
    fn value_to_csv_string(value: &Value) -> String {
        let s = Self::value_to_string(value);
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s
        }
    }
}
