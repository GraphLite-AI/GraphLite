// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Lexer for GQL graph language using hybrid approach
//! 
//! # INFINITE LOOP PREVENTION GUIDE
//! 
//! This lexer has been designed to prevent infinite loops that commonly occur in parser implementations.
//! The following documentation explains the prevention strategies and how to avoid introducing new infinite loops.
//! 
//! ## Common Causes of Infinite Loops
//! 
//! 1. **Whitespace Functions Not Consuming Input**: When a whitespace parser returns a token
//!    without actually consuming any characters from the input string.
//! 
//! 2. **Function Parsers Not Validating Input**: When a function parser attempts to parse
//!    input that doesn't match its expected pattern, causing it to fail repeatedly.
//! 
//! 3. **Parser Order Issues**: When more general patterns are placed before more specific ones,
//!    causing the wrong parser to be called repeatedly.
//! 
//! ## Prevention Strategies
//! 
//! 1. **Input Consumption Validation**: Every parser function must either consume input or return an error.
//!    Never return a token without advancing the input position.
//! 
//! 2. **Proper Error Handling**: When a parser can't match its pattern, it must return an error
//!    to allow other parsers to try, rather than returning a token with unchanged input.
//! 
//! 3. **Parser Order**: More specific patterns must come before general patterns in the `alt()` chain.
//! 
//! 4. **Infinite Loop Detection**: The main tokenization loop includes iteration counting and
//!    input position validation to detect and report infinite loops.
//! 
//! ## Debugging Infinite Loops
//! 
//! If you encounter an infinite loop:
//! 
//! 1. Check if any parser function is returning a token without consuming input
//! 2. Verify that parser functions return errors when they can't match their pattern
//! 3. Ensure parser order is correct (specific before general)
//! 4. Look for patterns that might be consuming the same input repeatedly
//! 
//! ## Key Functions to Check
//! 
//! - `whitespace()`: Must validate that whitespace was actually consumed
//! - `function_call()`: Must validate identifier + '(' pattern before parsing
//! - Any new parser function: Must either consume input or return error

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, alpha1, alphanumeric1, digit1},
    combinator::{map, recognize, opt, peek},
    multi::many0,
    sequence::{pair, tuple},
    IResult,
};

/// Token types for GQL graph language
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Match,
    Where,
    Return,
    Select,
    From,
    With,
    Create,
    Insert,
    Delete,
    Set,
    Remove,
    Merge,
    Unwind,
    Call,
    Load,
    Store,
    Distinct,
    Order,
    By,
    Asc,
    Ascending,
    Desc,
    Descending,
    Nulls,
    First,
    Last,
    Limit,
    Group,
    Having,
    All,
    Any,
    Some,
    Offset,
    Skip,
    On,
    Detach,
    NoDetach,
    Yield,
    As,
    And,
    Or,
    Not,
    Xor,
    In,
    Union,
    Intersect,
    Except,
    Contains,
    Starts,
    Ends,
    Exists,
    Like,
    Matches,    // For MATCHES operator (pattern matching)
    Case,
    Cast,
    When,
    Then,
    Else,
    End,
    Within,  // Added for temporal queries
    Leading,  // For TRIM LEADING
    Trailing, // For TRIM TRAILING  
    Both,     // For TRIM BOTH
    Schema,
    Data,
    Graph,
    Home,
    If,
    Drop,
    Register,
    Unregister,
    Truncate,
    Clear,
    Cascade,
    Restrict,
    Type,
    File,
    Url,
    Inline,
    Validate,
    Against,
    Show,
    Describe,
    Alter,
    Version,
    Description,
    Copy,
    Of,
    Replace,    // For CREATE OR REPLACE syntax (as separate token)
    Property, 
    Vertex,
    Node,
    Edge,
    Types,
    Source,
    Destination,
    Is,
    Session,
    Reset,
    Close,
    Value,
    Binding,
    Table,
    Parameters,
    Characteristics,
    Time,
    Zone,
    Current,
    Parameter,
    User,
    Role,
    Password,
    Roles,
    Grant,
    Revoke,
    To,

    // IS predicate keywords
    Unknown,
    Normalized,
    Directed,
    Typed,

    // Query statement keywords
    Let,            // For LET variable definitions
    For,            // For FOR iteration statements
    Filter,         // For FILTER conditional statements

    // Stored procedure keywords
    Declare,        // For DECLARE variable definitions
    Next,           // For NEXT statement chaining
    At,             // For AT location prefix (keyword)
    Procedure,      // For CREATE/DROP PROCEDURE statements
    
    // Transaction control keywords
    Start,          // START TRANSACTION
    Commit,         // COMMIT
    Rollback,       // ROLLBACK
    Work,           // WORK (optional in COMMIT/ROLLBACK)
    Begin,          // Alternative to START TRANSACTION
    Isolation,      // ISOLATION LEVEL
    Level,          // ISOLATION LEVEL
    Read,           // READ COMMITTED/UNCOMMITTED
    Uncommitted,    // READ UNCOMMITTED
    Committed,      // READ COMMITTED
    Repeatable,     // REPEATABLE READ
    Serializable,   // SERIALIZABLE isolation level
    Only,           // READ ONLY
    Write,          // READ WRITE
    
    
    // Path type keywords
    Walk,
    Trail,
    Simple,
    Path,
    Acyclic,
    
    // Type keywords
    BooleanType,
    StringType,
    IntegerType,
    FloatType,
    RealType,
    DoubleType,
    BigIntType,
    SmallIntType,
    DecimalType,

    // Operators
    Plus,           // +
    Minus,          // -
    Star,           // *
    Slash,          // /
    Percent,        // %
    Caret,          // ^
    Equal,          // =
    NotEqual,       // !=
    LessThan,       // <
    LessEqual,      // <=
    GreaterThan,    // >
    GreaterEqual,   // >=
    Regex,          // =~
    FuzzyEqual,     // ~= (fuzzy/approximate equality)
    Concat,         // ||
    
    // Delimiters
    LeftParen,      // (
    RightParen,     // )
    LeftBracket,    // [
    RightBracket,   // ]
    LeftBrace,      // {
    RightBrace,     // }
    Comma,          // ,
    Semicolon,      // ;
    Colon,          // :
    Dot,            // .
    Arrow,          // ->
    ArrowLeft,      // <-
    ArrowBoth,      // <->
    Dash,           // -
    AtSign,         // @
    Dollar,         // $
    Ampersand,      // &
    Pipe,           // |
    Question,       // ?
    
    // Literals
    String(String),
    BacktickString(String),  // ISO GQL delimited identifier: `identifier`
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
    Vector(Vec<f64>),

    // Identifiers
    Identifier(String),
    Variable(String),

    // Property access
    PropertyAccess(String),
    
    // Whitespace and comments
    Whitespace,
    Comment(String),
    
    // End of file
    EOF,
}

/// Lexer state
#[derive(Debug, Clone)]
pub struct Lexer {
    input: String,
    _position: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    pub fn new(input: String) -> Self {
        Self {
            input,
            _position: 0,
            tokens: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut remaining = self.input.as_str();
        let mut tokens = Vec::new();
        let mut iteration = 0;

        // Main tokenization loop - processes input string character by character
        while !remaining.is_empty() {
            iteration += 1;
            
            // CRITICAL: Infinite loop protection
            // This prevents the lexer from getting stuck when no progress is made
            // Common causes of infinite loops:
            // 1. Whitespace function returning Token::Whitespace without consuming input
            // 2. Function parsers not advancing the input position
            // 3. Parser functions returning the same remaining string
            if iteration > 1000 {
                return Err("Infinite loop detected in lexer".to_string());
            }
            
            match token(remaining) {
                Ok((next_remaining, token)) => {
                    // CRITICAL: Ensure input position advances
                    // If next_remaining == remaining, we have an infinite loop
                    // This check helps debug parser functions that don't consume input
                    if next_remaining == remaining {
                        return Err(format!("Parser function not consuming input. Token: {:?}, Remaining: '{}'", token, remaining));
                    }
                    
                    // Only add non-whitespace/non-comment tokens to the result
                    if !matches!(token, Token::Whitespace | Token::Comment(_)) {
                        tokens.push(token);
                    }
                    remaining = next_remaining;
                }
                Err(e) => {
                    return Err(format!("Lexer error: {:?}", e));
                }
            }
        }
        tokens.push(Token::EOF);
        self.tokens = tokens.clone();
        Ok(tokens)
    }
}

/// Parse a single token using hybrid approach
/// 
/// CRITICAL: Token parsing order is crucial to prevent infinite loops and incorrect parsing
/// The order follows these principles:
/// 1. More specific patterns must come before general patterns
/// 2. Patterns that consume more characters must come before patterns that consume fewer
/// 3. Function calls must come after literals to avoid conflicts
/// 4. Keywords must come before identifiers to avoid keyword/identifier confusion
fn token(input: &str) -> IResult<&str, Token> {
    alt((
        // Whitespace and comments (use nom for complex patterns)
        // CRITICAL: whitespace function must properly consume input or return error
        whitespace,
        map(comment, |s| Token::Comment(s.to_string())),
        
        // Variables (must come before simple_patterns to avoid $ being parsed as Dollar)
        map(variable, |s| Token::Variable(s.to_string())),
        
        // Vector literals (must come before simple_patterns to avoid [ being parsed as LeftBracket)
        map(vector_literal, |v| Token::Vector(v)),
        
        // Float literals (must come before integer_literal to avoid . being parsed as Dot)
        map(float_literal, |f| Token::Float(f)),
        
        // Integer literals (must come before simple_patterns to avoid - being parsed as Minus)
        map(integer_literal, |n| Token::Integer(n)),
        
        // Complex literals (use nom for parsing) - must come before function calls
        map(backtick_identifier, |s| Token::BacktickString(s.to_string())), // ISO GQL delimited identifiers
        map(string_literal, |s| Token::String(s.to_string())),

        // Function calls removed - now handled by parser using individual tokens
        // This allows proper ISO GQL compliant parsing of nested function calls
        map(property_access, |s| Token::PropertyAccess(s.to_string())),
        
        // Simple patterns (keywords, operators, etc.) - must come after property access but before identifiers
        simple_patterns,
        
        // Identifiers (use nom for parsing) - must come after keywords
        map(identifier, |s| Token::Identifier(s.to_string())),
    ))(input)
}

/// Parse simple patterns using direct string matching
/// 
/// CRITICAL: The order of patterns in this function is crucial to prevent infinite loops
/// and ensure correct parsing. The order follows these principles:
/// 
/// 1. **Multi-character operators first**: `!=`, `<=`, `>=`, `=~`, `<->`, `->`, `<-`
///    These must come before single-character operators to avoid partial matches.
/// 
/// 2. **Longer keywords first**: `DISTINCT`, `CONTAINS`, `WITHIN`, etc.
///    This prevents shorter keywords from matching when longer ones should match.
/// 
/// 3. **Boolean literals**: `true`, `false`, `NULL`
///    These must come before single-character patterns to avoid parsing 't' as an identifier.
/// 
/// 4. **Single-character operators and delimiters**: `+`, `-`, `*`, `/`, etc.
///    These come last as they are the most general patterns.
/// 
/// INFINITE LOOP SCENARIO (if order is wrong):
/// Input: "!="
/// 1. If `=` comes before `!=`, it would parse `=` first
/// 2. Remaining input: "!"
/// 3. No pattern matches "!", causing infinite loop
/// 
/// CORRECT BEHAVIOR:
/// Input: "!="
/// 1. `!=` pattern matches first
/// 2. Remaining input: "" (empty)
/// 3. Lexer completes successfully
/// Check if character is a word boundary (not alphanumeric or underscore)
fn is_word_boundary(c: char) -> bool {
    !c.is_alphanumeric() && c != '_'
}

/// Check if a keyword match has a proper word boundary
fn is_keyword_match(input: &str, keyword: &str) -> bool {
    if input.len() < keyword.len() {
        return false;
    }
    
    // Check if the input starts with the keyword (case insensitive)
    if !input[..keyword.len()].eq_ignore_ascii_case(keyword) {
        return false;
    }
    
    // Check if the next character (if any) is a word boundary
    if input.len() > keyword.len() {
        let next_char = input.chars().nth(keyword.len()).unwrap_or(' ');
        is_word_boundary(next_char)
    } else {
        true // End of input is a word boundary
    }
}

fn simple_patterns(input: &str) -> IResult<&str, Token> {
    
    // Multi-character operators (must come before single character)
    if input.starts_with("||") {
        Ok((&input[2..], Token::Concat))
    } else if input.starts_with("!=") {
        Ok((&input[2..], Token::NotEqual))
    } else if input.starts_with("<>") {
        Ok((&input[2..], Token::NotEqual))
    } else if input.starts_with("<=") {
        Ok((&input[2..], Token::LessEqual))
    } else if input.starts_with(">=") {
        Ok((&input[2..], Token::GreaterEqual))
    } else if input.starts_with("=~") {
        Ok((&input[2..], Token::Regex))
    } else if input.starts_with("~=") {
        Ok((&input[2..], Token::FuzzyEqual))
    } else if input.starts_with("<->") {
        Ok((&input[3..], Token::ArrowBoth))
    } else if input.starts_with("->") {
        Ok((&input[2..], Token::Arrow))
    } else if input.starts_with("<-") {
        Ok((&input[2..], Token::ArrowLeft))
    }
    // Keywords (case insensitive check) - longer keywords first
    else if input.len() >= 11 && input[..11].eq_ignore_ascii_case("DESTINATION") &&
              (input.len() == 11 || (!input.chars().nth(11).unwrap().is_alphanumeric() && input.chars().nth(11).unwrap() != '_')) {
        Ok((&input[11..], Token::Destination))
    } else if input.len() >= 10 && input[..10].eq_ignore_ascii_case("NORMALIZED") &&
              (input.len() == 10 || (!input.chars().nth(10).unwrap().is_alphanumeric() && input.chars().nth(10).unwrap() != '_')) {
        Ok((&input[10..], Token::Normalized))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("DIRECTED") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Directed))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("PROPERTY") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Property))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("DISTINCT") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Distinct))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("CONTAINS") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Contains))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("WITHIN") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Within))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("REPLACE") &&
              (input.len() == 7 || (input.chars().nth(7).unwrap_or(' ') != '(' && !input.chars().nth(7).unwrap_or(' ').is_alphanumeric())) {
        Ok((&input[7..], Token::Replace))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("BOOLEAN") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::BooleanType))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("INTEGER") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::IntegerType))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("SMALLINT") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::SmallIntType))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("BIGINT") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::BigIntType))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("DECIMAL") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::DecimalType))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("DOUBLE") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::DoubleType))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("FLOAT") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::FloatType))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("REAL") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::RealType))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("CASCADE") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Cascade))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("UNKNOWN") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Unknown))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("RESTRICT") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Restrict))
    } else if input.len() >= 14 && input[..14].eq_ignore_ascii_case("CHARACTERISTICS") &&
              (input.len() == 14 || (!input.chars().nth(14).unwrap().is_alphanumeric() && input.chars().nth(14).unwrap() != '_')) {
        Ok((&input[14..], Token::Characteristics))
    } else if input.len() >= 10 && input[..10].eq_ignore_ascii_case("PARAMETERS") &&
              (input.len() == 10 || (!input.chars().nth(10).unwrap().is_alphanumeric() && input.chars().nth(10).unwrap() != '_')) {
        Ok((&input[10..], Token::Parameters))
    } else if input.len() >= 9 && input[..9].eq_ignore_ascii_case("PARAMETER") &&
              (input.len() == 9 || (!input.chars().nth(9).unwrap().is_alphanumeric() && input.chars().nth(9).unwrap() != '_')) {
        Ok((&input[9..], Token::Parameter))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("SESSION") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Session))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("BINDING") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Binding))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("CURRENT") && 
              (input.len() == 7 || !input.chars().nth(7).unwrap_or(' ').is_alphanumeric() && input.chars().nth(7).unwrap_or(' ') != '_') {
        Ok((&input[7..], Token::Current))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("PASSWORD") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Password))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("STRING") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::StringType))
    } else if input.len() >= 11 && input[..11].eq_ignore_ascii_case("DESCRIPTION") &&
              (input.len() == 11 || !input.chars().nth(11).unwrap_or(' ').is_alphanumeric() && input.chars().nth(11).unwrap_or(' ') != '_') {
        Ok((&input[11..], Token::Description))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("DESCRIBE") &&
              (input.len() == 8 || !input.chars().nth(8).unwrap_or(' ').is_alphanumeric() && input.chars().nth(8).unwrap_or(' ') != '_') {
        Ok((&input[8..], Token::Describe))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("VALIDATE") &&
              (input.len() == 8 || !input.chars().nth(8).unwrap_or(' ').is_alphanumeric() && input.chars().nth(8).unwrap_or(' ') != '_') {
        Ok((&input[8..], Token::Validate))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("VERSION") &&
              (input.len() == 7 || !input.chars().nth(7).unwrap_or(' ').is_alphanumeric() && input.chars().nth(7).unwrap_or(' ') != '_') {
        Ok((&input[7..], Token::Version))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("AGAINST") &&
              (input.len() == 7 || !input.chars().nth(7).unwrap_or(' ').is_alphanumeric() && input.chars().nth(7).unwrap_or(' ') != '_') {
        Ok((&input[7..], Token::Against))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("SCHEMA") &&
              (input.len() == 6 || !input.chars().nth(6).unwrap_or(' ').is_alphanumeric() && input.chars().nth(6).unwrap_or(' ') != '_') {
        Ok((&input[6..], Token::Schema))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("INLINE") &&
              (input.len() == 6 || !input.chars().nth(6).unwrap_or(' ').is_alphanumeric() && input.chars().nth(6).unwrap_or(' ') != '_') {
        Ok((&input[6..], Token::Inline))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("ALTER") &&
              (input.len() == 5 || !input.chars().nth(5).unwrap_or(' ').is_alphanumeric() && input.chars().nth(5).unwrap_or(' ') != '_') {
        Ok((&input[5..], Token::Alter))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("SHOW") &&
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric() && input.chars().nth(4).unwrap_or(' ') != '_') {
        Ok((&input[4..], Token::Show))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("DATA") &&
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric() && input.chars().nth(4).unwrap_or(' ') != '_') {
        Ok((&input[4..], Token::Data))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("FILE") &&
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric() && input.chars().nth(4).unwrap_or(' ') != '_') {
        Ok((&input[4..], Token::File))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("URL") &&
              (input.len() == 3 || !input.chars().nth(3).unwrap_or(' ').is_alphanumeric() && input.chars().nth(3).unwrap_or(' ') != '_') {
        Ok((&input[3..], Token::Url))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("ACYCLIC") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Acyclic))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("SIMPLE") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap_or(' ').is_alphanumeric() && input.chars().nth(6).unwrap_or(' ') != '_')) {
        Ok((&input[6..], Token::Simple))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("TRAIL") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Trail))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("WALK") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Walk))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("PATH") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Path))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("CREATE") && 
              (input.len() == 6 || !input.chars().nth(6).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[6..], Token::Create))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("INSERT") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Insert))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("DELETE") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Delete))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("REMOVE") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Remove))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("UNWIND") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Unwind))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("STARTS") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Starts))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("EXISTS") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Exists))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("MATCHES") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Matches))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("SOURCE") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Source))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("CASE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Case))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("CAST") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Cast))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("WHEN") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::When))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("THEN") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Then))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("ELSE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Else))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("END") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::End))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("LEADING") &&
              (input.len() == 7 || !input.chars().nth(7).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[7..], Token::Leading))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("TRAILING") &&
              (input.len() == 8 || !input.chars().nth(8).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[8..], Token::Trailing))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("BOTH") &&
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[4..], Token::Both))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("LET") &&
              (input.len() == 3 || !input.chars().nth(3).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[3..], Token::Let))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("FOR") &&
              (input.len() == 3 || !input.chars().nth(3).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[3..], Token::For))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("FILTER") &&
              (input.len() == 6 || !input.chars().nth(6).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[6..], Token::Filter))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("DECLARE") &&
              (input.len() == 7 || !input.chars().nth(7).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[7..], Token::Declare))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("NEXT") &&
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric()) {
        Ok((&input[4..], Token::Next))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("AT") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap_or(' ').is_alphanumeric() && input.chars().nth(2).unwrap_or(' ') != '_')) {
        Ok((&input[2..], Token::At))
    
    // Transaction control keywords
    // TRANSACTION removed from keywords - should be parsed as identifier
    // and handled by parser for SQL contexts
    } else if is_keyword_match(input, "UNCOMMITTED") {
        Ok((&input[11..], Token::Uncommitted))
    } else if is_keyword_match(input, "SERIALIZABLE") {
        Ok((&input[12..], Token::Serializable))
    } else if is_keyword_match(input, "REPEATABLE") {
        Ok((&input[10..], Token::Repeatable))
    } else if is_keyword_match(input, "ISOLATION") {
        Ok((&input[9..], Token::Isolation))
    } else if is_keyword_match(input, "COMMITTED") {
        Ok((&input[9..], Token::Committed))
    } else if is_keyword_match(input, "ROLLBACK") {
        Ok((&input[8..], Token::Rollback))
    } else if is_keyword_match(input, "START") {
        Ok((&input[5..], Token::Start))
    } else if is_keyword_match(input, "LEVEL") {
        Ok((&input[5..], Token::Level))
    } else if is_keyword_match(input, "BEGIN") {
        Ok((&input[5..], Token::Begin))
    } else if is_keyword_match(input, "WRITE") {
        Ok((&input[5..], Token::Write))
    } else if is_keyword_match(input, "WORK") {
        Ok((&input[4..], Token::Work))
    } else if is_keyword_match(input, "ONLY") {
        Ok((&input[4..], Token::Only))
    } else if is_keyword_match(input, "READ") {
        Ok((&input[4..], Token::Read))
    } else if is_keyword_match(input, "COMMIT") {
        Ok((&input[6..], Token::Commit))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("VERTEX") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Vertex))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("NODETACH") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::NoDetach))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("DETACH") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Detach))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("SELECT") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Select))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("FROM") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::From))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("RETURN") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Return))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("MATCH") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap_or(' ').is_alphanumeric() && input.chars().nth(5).unwrap_or(' ') != '_')) {
        Ok((&input[5..], Token::Match))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("WHERE") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Where))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("MERGE") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Merge))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("STORE") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Store))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("ORDER") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Order))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("LIMIT") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Limit))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("HAVING") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Having))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("OFFSET") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Offset))
    } else if is_keyword_match(input, "GRAPH") {
        Ok((&input[5..], Token::Graph))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("GROUP") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Group))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("TYPES") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Types))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("ROLES") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Roles))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("YIELD") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Yield))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("RESET") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Reset))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("CLOSE") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Close))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("VALUE") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Value))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("TABLE") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Table))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("TYPED") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Typed))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("TIME") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Time))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("ZONE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Zone))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("HOME") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Home))
    } else if input.len() >= 4 && &input[..4] == "USER" && 
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric() && input.chars().nth(4).unwrap_or(' ') != '_') {
        Ok((&input[4..], Token::User))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("ROLE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Role))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("GRANT") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Grant))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("REVOKE") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Revoke))
    } else if input.len() >= 9 && input[..9].eq_ignore_ascii_case("PROCEDURE") &&
              (input.len() == 9 || (!input.chars().nth(9).unwrap().is_alphanumeric() && input.chars().nth(9).unwrap() != '_')) {
        Ok((&input[9..], Token::Procedure))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("TO") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::To))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("NODE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        // Only treat as NODE keyword if it's actually uppercase "NODE"
        if &input[..4] == "NODE" {
            Ok((&input[4..], Token::Node))
        } else {
            // "node", "Node", etc. should be identifiers
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        }
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("EDGE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Edge))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("WITH") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::With))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("CALL") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Call))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("LOAD") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Load))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("ENDS") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Ends))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("LIKE") &&
              (input.len() == 4 || is_word_boundary(input.chars().nth(4).unwrap_or(' '))) {
        Ok((&input[4..], Token::Like))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("TYPE") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Type))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("COPY") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Copy))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("TRUNCATE") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Truncate))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("CLEAR") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Clear))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("DROP") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Drop))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("REGISTER") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Register))
    } else if input.len() >= 10 && input[..10].eq_ignore_ascii_case("UNREGISTER") &&
              (input.len() == 10 || (!input.chars().nth(10).unwrap().is_alphanumeric() && input.chars().nth(10).unwrap() != '_')) {
        Ok((&input[10..], Token::Unregister))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("SKIP") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Skip))
    } else if input.len() >= 7 && input[..7].eq_ignore_ascii_case("LEADING") &&
              (input.len() == 7 || (!input.chars().nth(7).unwrap().is_alphanumeric() && input.chars().nth(7).unwrap() != '_')) {
        Ok((&input[7..], Token::Leading))
    } else if input.len() >= 8 && input[..8].eq_ignore_ascii_case("TRAILING") &&
              (input.len() == 8 || (!input.chars().nth(8).unwrap().is_alphanumeric() && input.chars().nth(8).unwrap() != '_')) {
        Ok((&input[8..], Token::Trailing))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("BOTH") &&
              (input.len() == 4 || !input.chars().nth(4).unwrap_or(' ').is_alphanumeric() && input.chars().nth(4).unwrap_or(' ') != '_') {
        Ok((&input[4..], Token::Both))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("SET") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::Set))
    } else if input.len() >= 10 && input[..10].eq_ignore_ascii_case("DESCENDING") &&
              (input.len() == 10 || (!input.chars().nth(10).unwrap().is_alphanumeric() && input.chars().nth(10).unwrap() != '_')) {
        Ok((&input[10..], Token::Descending))
    } else if input.len() >= 9 && input[..9].eq_ignore_ascii_case("INTERSECT") &&
              (input.len() == 9 || (!input.chars().nth(9).unwrap().is_alphanumeric() && input.chars().nth(9).unwrap() != '_')) {
        Ok((&input[9..], Token::Intersect))
    } else if input.len() >= 9 && input[..9].eq_ignore_ascii_case("ASCENDING") &&
              (input.len() == 9 || (!input.chars().nth(9).unwrap().is_alphanumeric() && input.chars().nth(9).unwrap() != '_')) {
        Ok((&input[9..], Token::Ascending))
    } else if input.len() >= 6 && input[..6].eq_ignore_ascii_case("EXCEPT") &&
              (input.len() == 6 || (!input.chars().nth(6).unwrap().is_alphanumeric() && input.chars().nth(6).unwrap() != '_')) {
        Ok((&input[6..], Token::Except))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("UNION") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Union))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("NULLS") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Nulls))
    } else if is_keyword_match(input, "FIRST") {
        Ok((&input[5..], Token::First))
    } else if is_keyword_match(input, "LAST") {
        Ok((&input[4..], Token::Last))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("DESC") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Desc))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("ASC") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::Asc))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("ALL") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::All))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("ANY") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::Any))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("SOME") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Some))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("AND") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::And))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("NOT") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::Not))
    } else if input.len() >= 3 && input[..3].eq_ignore_ascii_case("XOR") &&
              (input.len() == 3 || (!input.chars().nth(3).unwrap().is_alphanumeric() && input.chars().nth(3).unwrap() != '_')) {
        Ok((&input[3..], Token::Xor))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("BY") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::By))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("ON") && 
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::On))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("AS") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::As))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("OR") && 
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::Or))  
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("OF") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::Of))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("IS") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap_or(' ').is_alphanumeric() && input.chars().nth(2).unwrap_or(' ') != '_')) {
        Ok((&input[2..], Token::Is))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("IF") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::If))
    } else if input.len() >= 2 && input[..2].eq_ignore_ascii_case("IN") &&
              (input.len() == 2 || (!input.chars().nth(2).unwrap().is_alphanumeric() && input.chars().nth(2).unwrap() != '_')) {
        Ok((&input[2..], Token::In))
    }
    // Boolean literals
    else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("true") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Boolean(true)))
    } else if input.len() >= 5 && input[..5].eq_ignore_ascii_case("false") &&
              (input.len() == 5 || (!input.chars().nth(5).unwrap().is_alphanumeric() && input.chars().nth(5).unwrap() != '_')) {
        Ok((&input[5..], Token::Boolean(false)))
    } else if input.len() >= 4 && input[..4].eq_ignore_ascii_case("NULL") &&
              (input.len() == 4 || (!input.chars().nth(4).unwrap().is_alphanumeric() && input.chars().nth(4).unwrap() != '_')) {
        Ok((&input[4..], Token::Null))
    }
    // Single character operators and delimiters
    else if input.starts_with('+') {
        Ok((&input[1..], Token::Plus))
    } else if input.starts_with('-') {
        // Check if this is part of a number (minus operator) or a dash delimiter
        if input.len() > 1 && input.chars().nth(1).unwrap().is_ascii_digit() {
            Ok((&input[1..], Token::Minus))
        } else {
            // For edge patterns, dash should be Dash token in most cases
            // Each dash is consumed individually, so -- becomes two Token::Dash
            Ok((&input[1..], Token::Dash))
        }
    } else if input.starts_with('*') {
        Ok((&input[1..], Token::Star))
    } else if input.starts_with('/') {
        Ok((&input[1..], Token::Slash))
    } else if input.starts_with('%') {
        Ok((&input[1..], Token::Percent))
    } else if input.starts_with('^') {
        Ok((&input[1..], Token::Caret))
    } else if input.starts_with('=') {
        Ok((&input[1..], Token::Equal))
    } else if input.starts_with('<') {
        Ok((&input[1..], Token::LessThan))
    } else if input.starts_with('>') {
        Ok((&input[1..], Token::GreaterThan))
    } else if input.starts_with('@') {
        Ok((&input[1..], Token::AtSign))
    } else if input.starts_with('$') {
        Ok((&input[1..], Token::Dollar))
    } else if input.starts_with('.') {
        Ok((&input[1..], Token::Dot))
    } else if input.starts_with(',') {
        Ok((&input[1..], Token::Comma))
    } else if input.starts_with(';') {
        Ok((&input[1..], Token::Semicolon))
    } else if input.starts_with(':') {
        Ok((&input[1..], Token::Colon))
    } else if input.starts_with('&') {
        Ok((&input[1..], Token::Ampersand))
    } else if input.starts_with('|') {
        Ok((&input[1..], Token::Pipe))
    } else if input.starts_with('(') {
        Ok((&input[1..], Token::LeftParen))
    } else if input.starts_with(')') {
        Ok((&input[1..], Token::RightParen))
    } else if input.starts_with('[') {
        Ok((&input[1..], Token::LeftBracket))
    } else if input.starts_with(']') {
        Ok((&input[1..], Token::RightBracket))
    } else if input.starts_with('{') {
        Ok((&input[1..], Token::LeftBrace))
    } else if input.starts_with('}') {
        Ok((&input[1..], Token::RightBrace))
    } else if input.starts_with('?') {
        Ok((&input[1..], Token::Question))
    } else {
        // No match found - this should not happen if lexer is working correctly
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

/// Parse whitespace
/// 
/// CRITICAL: This function was the source of infinite loops in the original implementation
/// 
/// PROBLEM: The original implementation used `map(take_while(...), |_| Token::Whitespace)`
/// which would return Token::Whitespace even when no whitespace was found, causing
/// the lexer to never advance and get stuck in an infinite loop.
/// 
/// SOLUTION: Explicitly check if whitespace was consumed and return an error if not.
/// This ensures the lexer only processes whitespace when it actually exists.
/// 
/// INFINITE LOOP SCENARIO:
/// Input: "MATCH WHERE"
/// 1. whitespace() called on "MATCH WHERE" (no leading whitespace)
/// 2. take_while() returns empty string, but map() still returns Token::Whitespace
/// 3. remaining = "MATCH WHERE" (unchanged)
/// 4. Loop repeats infinitely
/// 
/// FIXED BEHAVIOR:
/// Input: "MATCH WHERE"
/// 1. whitespace() called on "MATCH WHERE" (no leading whitespace)
/// 2. take_while() returns empty string
/// 3. Function returns Error, allowing other parsers to try
/// 4. simple_patterns() successfully parses "MATCH"
fn whitespace(input: &str) -> IResult<&str, Token> {
    let (remaining, whitespace_chars) = take_while(|c: char| c.is_whitespace())(input)?;
    if whitespace_chars.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
    }
    Ok((remaining, Token::Whitespace))
}

/// Parse comments
fn comment(input: &str) -> IResult<&str, &str> {
    alt((
        // C++-style single line comment
        recognize(pair(tag("//"), take_while(|c| c != '\n'))),
        // Multi-line comment
        recognize(tuple((
            tag("/*"),
            take_while(|c| c != '*'),
            tag("*/")
        ))),
        // Note: SQL-style comments (-- ...) are not handled at the lexer level
        // to avoid conflicts with edge patterns like -- or -->
        // They should be handled at the parser level if needed
    ))(input)
}

/// Parse function calls like upper(...), count(...), etc.
/// 
/// CRITICAL: This function was also a source of infinite loops in the original implementation
/// 
/// PROBLEM: The original implementation tried to parse any input containing '(' as a function call,
/// even when it wasn't actually a function call (e.g., "(a:User {id: 123})"). This caused
/// the lexer to never advance and get stuck in an infinite loop.
/// 
/// SOLUTION: Properly validate that the input starts with an identifier followed by '('
/// before attempting to parse it as a function call.
/// 
/// INFINITE LOOP SCENARIO:
/// Input: "(a:User {id: 123})"
/// 1. function_call() called on "(a:User {id: 123})"
/// 2. Original code found '(' and tried to parse as function call
/// 3. Failed to find matching ')', returned error
/// 4. But lexer didn't advance, so same input processed again
/// 5. Loop repeats infinitely
/// 
/// FIXED BEHAVIOR:
/// Input: "(a:User {id: 123})"
/// 1. function_call() called on "(a:User {id: 123})"
/// 2. Function validates: does it start with identifier + '('?
/// 3. No, it starts with '(' directly, so returns Error
/// Parse property access like user.risk_score
fn property_access(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        // Object name (identifier)
        pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"))))
        ),
        // Dot
        char('.'),
        // Property name (identifier)
        pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"))))
        )
    )))(input)
}

/// Parse string literals
fn string_literal(input: &str) -> IResult<&str, &str> {
    alt((
        // Double quoted string with escape handling
        map(
            recognize(pair(
                char('"'),
                pair(
                    escaped_string_content('"'),
                    char('"')
                )
            )),
            |s: &str| &s[1..s.len()-1] // Strip quotes
        ),
        // Single quoted string with escape handling
        map(
            recognize(pair(
                char('\''),
                pair(
                    escaped_string_content('\''),
                    char('\'')
                )
            )),
            |s: &str| &s[1..s.len()-1] // Strip quotes
        ),
    ))(input)
}

/// Parse backtick-delimited identifier (ISO GQL compliant)
/// Example: `My-Identifier`, `Property Name`, `Type-123`
/// Escape sequence: `` (double backtick) for literal backtick
fn backtick_identifier(input: &str) -> IResult<&str, &str> {
    map(
        recognize(pair(
            char('`'),
            pair(
                escaped_backtick_content,
                char('`')
            )
        )),
        |s: &str| &s[1..s.len()-1] // Strip backticks
    )(input)
}

/// Parse backtick-delimited content with double-backtick escape
fn escaped_backtick_content(input: &str) -> IResult<&str, &str> {
    let mut pos = 0;
    let input_bytes = input.as_bytes();

    while pos < input_bytes.len() {
        if input_bytes[pos] == b'`' {
            // Check for double backtick (escape sequence)
            if pos + 1 < input_bytes.len() && input_bytes[pos + 1] == b'`' {
                // Skip both backticks
                pos += 2;
            } else {
                // Found single backtick - end of identifier
                break;
            }
        } else {
            pos += 1;
        }
    }

    Ok((&input[pos..], &input[0..pos]))
}

/// Parse string content with escape sequences
fn escaped_string_content(quote_char: char) -> impl Fn(&str) -> IResult<&str, &str> {
    move |input: &str| {
        let mut pos = 0;
        let input_bytes = input.as_bytes();
        
        while pos < input_bytes.len() {
            if input_bytes[pos] == b'\\' && pos + 1 < input_bytes.len() {
                // Skip escaped character (including escaped quotes)
                pos += 2;
            } else if input_bytes[pos] == quote_char as u8 {
                // Found unescaped quote - end of string content
                break;
            } else {
                pos += 1;
            }
        }
        
        Ok((&input[pos..], &input[0..pos]))
    }
}

/// Parse integer literals
fn integer_literal(input: &str) -> IResult<&str, i64> {
    map(
        recognize(pair(
            opt(char('-')),
            digit1
        )),
        |s: &str| s.parse::<i64>().unwrap()
    )(input)
}

/// Parse float literals
fn float_literal(input: &str) -> IResult<&str, f64> {
    map(
        recognize(pair(
            opt(char('-')),
            pair(
                digit1,
                pair(
                    char('.'),
                    digit1
                )
            )
        )),
        |s: &str| s.parse::<f64>().unwrap()
    )(input)
}



/// Parse timewindow literals
/// 
/// CRITICAL: This function follows the same pattern as other literal parsers.
/// It must either consume input or return an error to prevent infinite loops.
/// 
/// Pattern: TIME_WINDOW(...) where ... can contain nested parentheses
/// 
/// INFINITE LOOP PREVENTION:
/// - Only matches if input starts with "TIME_WINDOW("
/// - Returns error if no matching closing parenthesis is found
/// - Ensures input position advances when successful
fn _timewindow_literal(input: &str) -> IResult<&str, &str> {
    // Simplified approach - just match the pattern
    if input.starts_with("TIME_WINDOW(") {
        // Find the matching closing parenthesis
        let mut paren_count = 0;
        let mut end_pos = 0;
        for (i, c) in input.chars().enumerate() {
            if c == '(' {
                paren_count += 1;
            } else if c == ')' {
                paren_count -= 1;
                if paren_count == 0 {
                    end_pos = i + 1;
                    break;
                }
            }
        }
        if end_pos > 0 {
            Ok((&input[end_pos..], &input[..end_pos]))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        }
    } else {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    }
}

/// Parse vector literals
fn vector_literal(input: &str) -> IResult<&str, Vec<f64>> {
    map(
        recognize(tuple((
            char('['),
            // Only match if there's at least one comma (indicating multiple elements) 
            // or multiple digits/floats, to avoid consuming single element array indexing
            alt((
                // Multiple elements with comma
                tuple((
                    take_while1(|c: char| c.is_ascii_digit() || c == '.' || c == ' ' || c == '-'),
                    char(','),
                    take_while(|c: char| c.is_ascii_digit() || c == '.' || c == ',' || c == ' ' || c == '-')
                )),
                // Or empty vector []
                tuple((
                    take_while(|c: char| c == ' '),
                    peek(char(']')),
                    take_while(|c: char| c == ' ')
                ))
            )),
            char(']')
        ))),
        |s: &str| {
            // Parse the vector content
            let content = &s[1..s.len()-1]; // Remove [ and ]
            if content.trim().is_empty() {
                vec![]
            } else {
                content.split(',')
                    .map(|x| x.trim().parse::<f64>().unwrap_or(0.0))
                    .collect()
            }
        }
    )(input)
}

/// Parse variables (starting with $)
fn variable(input: &str) -> IResult<&str, &str> {
    map(
        recognize(pair(
            char('$'),
            identifier
        )),
        |s: &str| &s[1..] // Strip the $ prefix
    )(input)
}

/// Parse identifiers
fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"))))
    ))(input)
}

/// Public function to tokenize input
pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut lexer = Lexer::new(input.to_string());
    lexer.tokenize()
}

// Removed unused test_lexer function

