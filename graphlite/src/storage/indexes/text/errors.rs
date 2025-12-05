// Error types for text search operations

use std::fmt;

/// Errors that can occur in text search operations
#[derive(Debug, Clone)]
pub enum TextSearchError {
    /// Unsupported language
    UnsupportedLanguage(String),
    /// Invalid analyzer configuration
    InvalidConfig(String),
    /// Index operation failed
    IndexError(String),
    /// IO error
    IoError(String),
    /// Tokenization error
    TokenizationError(String),
    /// Invalid query
    InvalidQuery(String),
}

impl fmt::Display for TextSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextSearchError::UnsupportedLanguage(lang) => {
                write!(f, "Unsupported language: {}", lang)
            }
            TextSearchError::InvalidConfig(msg) => {
                write!(f, "Invalid analyzer configuration: {}", msg)
            }
            TextSearchError::IndexError(msg) => {
                write!(f, "Index error: {}", msg)
            }
            TextSearchError::IoError(msg) => {
                write!(f, "IO error: {}", msg)
            }
            TextSearchError::TokenizationError(msg) => {
                write!(f, "Tokenization error: {}", msg)
            }
            TextSearchError::InvalidQuery(msg) => {
                write!(f, "Invalid query: {}", msg)
            }
        }
    }
}

impl std::error::Error for TextSearchError {}
