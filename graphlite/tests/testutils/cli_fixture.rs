//! CLI-based test fixture for integration tests
//!
//! This fixture spawns the GraphLite CLI as separate processes, enforcing proper
//! architectural boundaries. All database operations go through the CLI interface
//! rather than directly instantiating internal components.

use std::process::Command;
use std::path::PathBuf;
use tempfile::TempDir;
use serde_json::Value as JsonValue;
use graphlite::Value;
use std::collections::HashMap;

/// CLI-based test fixture - all operations via command-line interface
pub struct CliFixture {
    _temp_dir: TempDir,
    db_path: PathBuf,
    admin_user: String,
    admin_password: String,
}

impl CliFixture {
    /// Initialize database via CLI
    pub fn empty() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("graphlite");
        let admin_user = "admin".to_string();
        let admin_password = "test_password_123".to_string();

        // Initialize database via CLI: cargo run --bin graphlite -- install
        let output = Command::new("cargo")
            .args(&["run", "--quiet", "--package", "graphlite-cli", "--bin", "graphlite", "--", "install"])
            .arg("--path").arg(&db_path)
            .arg("--admin-user").arg(&admin_user)
            .arg("--admin-password").arg(&admin_password)
            .arg("--yes")
            .env("RUST_LOG", "error")  // Suppress INFO logs in CLI output
            .output()?;

        if !output.status.success() {
            return Err(format!("Install failed: {}",
                String::from_utf8_lossy(&output.stderr)).into());
        }

        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            admin_user,
            admin_password,
        })
    }

    /// Execute query via CLI and expect success
    pub fn assert_query_succeeds(&self, query: &str) -> CliQueryResult {
        let output = Command::new("cargo")
            .args(&["run", "--quiet", "--package", "graphlite-cli", "--bin", "graphlite", "--", "query"])
            .arg("--path").arg(&self.db_path)
            .arg("--user").arg(&self.admin_user)
            .arg("--password").arg(&self.admin_password)
            .arg("--format").arg("json")
            .arg(query)
            .env("RUST_LOG", "error")  // Suppress INFO logs in CLI output
            .output()
            .expect("Failed to execute query");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("Query failed: {}\nQuery: {}", stderr, query);
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        CliQueryResult::from_json(&stdout)
            .unwrap_or_else(|e| panic!("Failed to parse JSON output: {}\nOutput was:\n{}", e, stdout))
    }

    /// Execute query via CLI and expect failure
    pub fn assert_query_fails(&self, query: &str) -> String {
        let output = Command::new("cargo")
            .args(&["run", "--quiet", "--package", "graphlite-cli", "--bin", "graphlite", "--", "query"])
            .arg("--path").arg(&self.db_path)
            .arg("--user").arg(&self.admin_user)
            .arg("--password").arg(&self.admin_password)
            .arg("--format").arg("json")
            .arg(query)
            .env("RUST_LOG", "error")  // Suppress INFO logs in CLI output
            .output()
            .expect("Failed to execute query");

        // Query should fail
        assert!(!output.status.success(), "Expected query to fail but it succeeded: {}", query);

        String::from_utf8_lossy(&output.stderr).to_string()
    }

    /// Get unique schema name for test isolation
    pub fn schema_name(&self) -> String {
        format!("test_{}", fastrand::u64(..))
    }

    /// Get the database path
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }
}

/// CLI query result (parsed from JSON)
pub struct CliQueryResult {
    pub rows: Vec<Row>,
}

impl CliQueryResult {
    /// Parse query result from JSON output
    fn from_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Find where JSON starts (skip any log lines before it)
        let json_start = json_str
            .find('{')
            .ok_or("No JSON found in output")?;

        let json_portion = &json_str[json_start..];
        let parsed: JsonValue = serde_json::from_str(json_portion)?;

        // Extract rows from JSON
        let empty_vec = vec![];
        let rows = parsed["rows"].as_array()
            .unwrap_or(&empty_vec);

        let converted_rows: Vec<Row> = rows.iter()
            .map(|row| {
                let mut values = HashMap::new();
                if let Some(obj) = row.as_object() {
                    for (key, val) in obj {
                        values.insert(key.clone(), json_value_to_storage_value(val));
                    }
                }
                Row { values }
            })
            .collect();

        Ok(CliQueryResult { rows: converted_rows })
    }

    /// Get the number of rows
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if result is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// A single row in the query result
#[derive(Debug)]
pub struct Row {
    pub values: HashMap<String, Value>,
}

impl Row {
    /// Get a value by column name
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// Get a string value
    pub fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key) {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Get a number value
    pub fn get_number(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(Value::Number(n)) => Some(*n),
            _ => None,
        }
    }

    /// Get a boolean value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(Value::Boolean(b)) => Some(*b),
            _ => None,
        }
    }
}

/// Convert JSON value to GraphLite storage value
fn json_value_to_storage_value(val: &JsonValue) -> Value {
    match val {
        JsonValue::String(s) => Value::String(s.clone()),
        JsonValue::Number(n) => {
            if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Null
            }
        }
        JsonValue::Bool(b) => Value::Boolean(*b),
        JsonValue::Null => Value::Null,
        JsonValue::Array(_arr) => {
            // For JSON arrays, convert to string representation
            // Value::Vector expects Vec<f32> specifically for embeddings
            Value::String(val.to_string())
        }
        JsonValue::Object(_) => {
            // For complex objects (Node, Edge), try to parse them
            // For now, just return as string representation
            Value::String(val.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_fixture_initialization() {
        let fixture = CliFixture::empty().expect("Failed to create fixture");
        assert!(fixture.db_path().exists());
    }

    #[test]
    fn test_simple_query() {
        let fixture = CliFixture::empty().expect("Failed to create fixture");
        let schema_name = fixture.schema_name();

        let result = fixture.assert_query_succeeds(&format!("CREATE SCHEMA /{};", schema_name));
        assert!(!result.is_empty());
    }
}
