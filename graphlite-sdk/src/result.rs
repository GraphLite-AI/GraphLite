//! Result handling and typed deserialization
//!
//! This module provides utilities for working with query results, including
//! type-safe deserialization into Rust structs.

use crate::error::{Error, Result};
use graphlite::{QueryResult, Row, Value};
use serde::de::DeserializeOwned;

/// Wrapper around QueryResult with additional type-safe methods
///
/// TypedResult provides convenient methods for deserializing query results
/// into Rust types.
///
/// # Examples
///
/// ```no_run
/// use serde::Deserialize;
/// use graphlite_sdk::GraphLite;
///
/// #[derive(Deserialize, Debug)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// # fn main() -> Result<(), graphlite_sdk::Error> {
/// # let db = GraphLite::open("./mydb")?;
/// # let session = db.session("admin")?;
/// let result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")?;
/// let typed = TypedResult::from(result);
///
/// // Deserialize each row into a Person struct
/// for person in typed.deserialize_rows::<Person>()? {
///     println!("Person: {:?}", person);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TypedResult {
    inner: QueryResult,
}

impl TypedResult {
    /// Create a new TypedResult from a QueryResult
    pub fn new(result: QueryResult) -> Self {
        TypedResult { inner: result }
    }

    /// Get the underlying QueryResult
    pub fn inner(&self) -> &QueryResult {
        &self.inner
    }

    /// Consume and get the underlying QueryResult
    pub fn into_inner(self) -> QueryResult {
        self.inner
    }

    /// Get the number of rows
    pub fn row_count(&self) -> usize {
        self.inner.rows.len()
    }

    /// Get the column names (variables from RETURN clause)
    pub fn column_names(&self) -> Vec<String> {
        self.inner.variables.clone()
    }

    /// Get a specific row by index
    pub fn get_row(&self, index: usize) -> Option<&Row> {
        self.inner.rows.get(index)
    }

    /// Deserialize all rows into a vector of the given type
    ///
    /// Each row is converted to a JSON object and then deserialized
    /// into the target type using serde.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Type to deserialize each row into (must implement Deserialize)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use serde::Deserialize;
    /// # use graphlite_sdk::GraphLite;
    ///
    /// #[derive(Deserialize)]
    /// struct Person { name: String, age: u32 }
    ///
    /// # fn main() -> Result<(), graphlite_sdk::Error> {
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")?;
    /// let typed = TypedResult::from(result);
    /// let people: Vec<Person> = typed.deserialize_rows()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn deserialize_rows<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        let mut results = Vec::new();

        for row in &self.inner.rows {
            let item = self.deserialize_row::<T>(row)?;
            results.push(item);
        }

        Ok(results)
    }

    /// Deserialize a single row into the given type
    ///
    /// # Type Parameters
    ///
    /// * `T` - Type to deserialize the row into (must implement Deserialize)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use serde::Deserialize;
    /// # use graphlite_sdk::GraphLite;
    ///
    /// #[derive(Deserialize)]
    /// struct Person { name: String, age: u32 }
    ///
    /// # fn main() -> Result<(), graphlite_sdk::Error> {
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age LIMIT 1")?;
    /// let typed = TypedResult::from(result);
    /// if let Some(row) = typed.get_row(0) {
    ///     let person: Person = typed.deserialize_row(row)?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn deserialize_row<T: DeserializeOwned>(&self, row: &Row) -> Result<T> {
        // Convert row.values HashMap to JSON
        let json_value = serde_json::to_value(&row.values)?;
        let result = serde_json::from_value(json_value)?;
        Ok(result)
    }

    /// Get the first row as the given type
    ///
    /// Convenience method for queries that return a single row.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Type to deserialize into (must implement Deserialize)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use serde::Deserialize;
    /// # use graphlite_sdk::GraphLite;
    ///
    /// #[derive(Deserialize)]
    /// struct Count { count: i64 }
    ///
    /// # fn main() -> Result<(), graphlite_sdk::Error> {
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let result = session.query("MATCH (p:Person) RETURN count(p) as count")?;
    /// let typed = TypedResult::from(result);
    /// let count: Count = typed.first()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn first<T: DeserializeOwned>(&self) -> Result<T> {
        let row = self.get_row(0)
            .ok_or_else(|| Error::NotFound("No rows returned".to_string()))?;

        self.deserialize_row(row)
    }

    /// Get a single value from the first row and first column
    ///
    /// Useful for queries that return a single scalar value.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # fn main() -> Result<(), graphlite_sdk::Error> {
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let result = session.query("MATCH (p:Person) RETURN count(p)")?;
    /// let typed = TypedResult::from(result);
    /// let count: i64 = typed.scalar()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn scalar<T: DeserializeOwned>(&self) -> Result<T> {
        let row = self.get_row(0)
            .ok_or_else(|| Error::NotFound("No rows returned".to_string()))?;

        let columns = &self.inner.variables;
        if columns.is_empty() {
            return Err(Error::NotFound("No columns returned".to_string()));
        }

        let value = row.get_value(&columns[0])
            .ok_or_else(|| Error::NotFound("Column value not found".to_string()))?;

        value_to_type(value)
    }

    /// Check if the result is empty (no rows)
    pub fn is_empty(&self) -> bool {
        self.inner.rows.is_empty()
    }

    /// Iterate over rows
    pub fn rows(&self) -> &[Row] {
        &self.inner.rows
    }
}

impl From<QueryResult> for TypedResult {
    fn from(result: QueryResult) -> Self {
        TypedResult::new(result)
    }
}

/// Convert a GraphLite Value to a Rust type
fn value_to_type<T: DeserializeOwned>(value: &Value) -> Result<T> {
    let json_value = value_to_json(value);
    serde_json::from_value(json_value).map_err(|e| e.into())
}

/// Convert a GraphLite Value to a serde_json Value
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(arr) | Value::List(arr) => {
            let items: Vec<serde_json::Value> = arr.iter()
                .map(|v| value_to_json(v))
                .collect();
            serde_json::Value::Array(items)
        }
        // For complex types like Node, Edge, Path, etc., use serde serialization
        _ => serde_json::to_value(value).unwrap_or(serde_json::Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversion() {
        let value = Value::Number(42.0);
        let json = value_to_json(&value);
        assert_eq!(json, serde_json::json!(42.0));
    }
}
