//! Query builder for fluent GQL query construction
//!
//! This module provides a builder API for constructing GQL queries in a
//! type-safe and ergonomic way.

use crate::connection::Session;
use crate::error::{Error, Result};
use graphlite::QueryResult;

/// Fluent API for building GQL queries
///
/// QueryBuilder provides a convenient way to construct complex GQL queries
/// without manually concatenating strings.
///
/// # Examples
///
/// ```no_run
/// # use graphlite_sdk::GraphLite;
/// # let db = GraphLite::open("./mydb")?;
/// # let session = db.session("admin")?;
/// // Using the builder
/// let result = session.query_builder()
///     .match_pattern("(p:Person)")
///     .where_clause("p.age > 25")
///     .return_clause("p.name, p.age")
///     .execute()?;
///
/// // Equivalent to:
/// // "MATCH (p:Person) WHERE p.age > 25 RETURN p.name, p.age"
/// # Ok::<(), graphlite_sdk::Error>(())
/// ```
pub struct QueryBuilder<'session> {
    session: &'session Session,
    match_patterns: Vec<String>,
    where_clauses: Vec<String>,
    with_clauses: Vec<String>,
    return_clause: Option<String>,
    order_by: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
}

impl<'session> QueryBuilder<'session> {
    /// Create a new query builder
    pub(crate) fn new(session: &'session Session) -> Self {
        QueryBuilder {
            session,
            match_patterns: Vec::new(),
            where_clauses: Vec::new(),
            with_clauses: Vec::new(),
            return_clause: None,
            order_by: None,
            skip: None,
            limit: None,
        }
    }

    /// Add a MATCH pattern
    ///
    /// Can be called multiple times to add multiple MATCH patterns.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Graph pattern to match (without the MATCH keyword)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .match_pattern("(p)-[:KNOWS]->(f:Person)")
    ///     .return_clause("p.name, f.name");
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn match_pattern(mut self, pattern: &str) -> Self {
        self.match_patterns.push(pattern.to_string());
        self
    }

    /// Add a WHERE clause condition
    ///
    /// Can be called multiple times - conditions are AND'ed together.
    ///
    /// # Arguments
    ///
    /// * `condition` - Condition to add (without the WHERE keyword)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .where_clause("p.age > 25")
    ///     .where_clause("p.name STARTS WITH 'A'")
    ///     .return_clause("p");
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn where_clause(mut self, condition: &str) -> Self {
        self.where_clauses.push(condition.to_string());
        self
    }

    /// Add a WITH clause
    ///
    /// WITH clauses are used for query chaining and intermediate results.
    ///
    /// # Arguments
    ///
    /// * `clause` - WITH clause content (without the WITH keyword)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .with_clause("p, p.age as age")
    ///     .where_clause("age > 25")
    ///     .return_clause("p.name");
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn with_clause(mut self, clause: &str) -> Self {
        self.with_clauses.push(clause.to_string());
        self
    }

    /// Set the RETURN clause
    ///
    /// Specifies what to return from the query. Required for MATCH queries.
    ///
    /// # Arguments
    ///
    /// * `clause` - Return clause content (without the RETURN keyword)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .return_clause("p.name, p.age");
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn return_clause(mut self, clause: &str) -> Self {
        self.return_clause = Some(clause.to_string());
        self
    }

    /// Set the ORDER BY clause
    ///
    /// # Arguments
    ///
    /// * `clause` - Order by clause (without the ORDER BY keywords)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .return_clause("p.name, p.age")
    ///     .order_by("p.age DESC");
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn order_by(mut self, clause: &str) -> Self {
        self.order_by = Some(clause.to_string());
        self
    }

    /// Set the SKIP value
    ///
    /// Skips the first N results.
    ///
    /// # Arguments
    ///
    /// * `n` - Number of results to skip
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .return_clause("p")
    ///     .skip(10);  // Skip first 10 results
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn skip(mut self, n: usize) -> Self {
        self.skip = Some(n);
        self
    }

    /// Set the LIMIT value
    ///
    /// Limits the number of results returned.
    ///
    /// # Arguments
    ///
    /// * `n` - Maximum number of results to return
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .return_clause("p")
    ///     .limit(10);  // Return max 10 results
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Build the query string without executing
    ///
    /// Returns the constructed GQL query as a string.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let query = session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .where_clause("p.age > 25")
    ///     .return_clause("p.name")
    ///     .build()?;
    ///
    /// assert_eq!(query, "MATCH (p:Person) WHERE p.age > 25 RETURN p.name");
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn build(&self) -> Result<String> {
        let mut query = String::new();

        // MATCH clauses
        if !self.match_patterns.is_empty() {
            for pattern in &self.match_patterns {
                if !query.is_empty() {
                    query.push(' ');
                }
                query.push_str("MATCH ");
                query.push_str(pattern);
            }
        }

        // WHERE clause
        if !self.where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.where_clauses.join(" AND "));
        }

        // WITH clauses
        for with_clause in &self.with_clauses {
            query.push_str(" WITH ");
            query.push_str(with_clause);
        }

        // RETURN clause
        if let Some(ref return_clause) = self.return_clause {
            query.push_str(" RETURN ");
            query.push_str(return_clause);
        } else if !self.match_patterns.is_empty() {
            return Err(Error::InvalidOperation(
                "MATCH query requires a RETURN clause".to_string()
            ));
        }

        // ORDER BY
        if let Some(ref order_by) = self.order_by {
            query.push_str(" ORDER BY ");
            query.push_str(order_by);
        }

        // SKIP
        if let Some(skip) = self.skip {
            query.push_str(&format!(" SKIP {}", skip));
        }

        // LIMIT
        if let Some(limit) = self.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        Ok(query.trim().to_string())
    }

    /// Execute the query and return results
    ///
    /// Builds and executes the query in one step.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let result = session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .where_clause("p.age > 25")
    ///     .return_clause("p.name, p.age")
    ///     .limit(10)
    ///     .execute()?;
    ///
    /// for row in result.rows() {
    ///     println!("{:?}", row);
    /// }
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn execute(&self) -> Result<QueryResult> {
        let query = self.build()?;
        self.session.query(&query)
    }
}

impl<'session> Session {
    /// Get a query builder for this session
    ///
    /// Convenience method for creating a query builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// let session = db.session("admin")?;
    /// let result = session.query_builder()
    ///     .match_pattern("(p:Person)")
    ///     .return_clause("p")
    ///     .execute()?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn query_builder(&self) -> QueryBuilder<'_> {
        QueryBuilder::new(self)
    }
}

#[cfg(test)]
mod tests {
    

    // Note: These are unit tests that test query building logic
    // Integration tests would require a real database

    #[test]
    fn test_query_builder_types_compile() {
        // Compilation test
    }
}
