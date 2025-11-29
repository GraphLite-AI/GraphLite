//! Error types for the GraphLite SDK

use thiserror::Error;

/// Result type alias for SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for GraphLite SDK operations
#[derive(Error, Debug)]
pub enum Error {
    /// Error from the core GraphLite library
    #[error("GraphLite error: {0}")]
    GraphLite(String),

    /// Session-related errors
    #[error("Session error: {0}")]
    Session(String),

    /// Query execution errors
    #[error("Query error: {0}")]
    Query(String),

    /// Transaction errors
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Type conversion errors
    #[error("Type conversion error: {0}")]
    TypeConversion(String),

    /// Invalid operation errors
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Resource not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Connection errors
    #[error("Connection error: {0}")]
    Connection(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::GraphLite(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::GraphLite(s.to_string())
    }
}
