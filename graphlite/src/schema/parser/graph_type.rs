// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Parser for ISO GQL CREATE/DROP/ALTER GRAPH TYPE statements

use nom::{
    branch::alt,
    combinator::{map, opt, value},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, preceded, tuple},
    IResult,
};

use crate::ast::lexer::Token;
use crate::schema::parser::ast::{
    AlterGraphTypeStatement, CreateGraphTypeStatement, DropGraphTypeStatement,
};
use crate::schema::types::{
    DataType, EdgeCardinality, EdgeTypeDefinition, GraphTypeVersion, NodeTypeDefinition,
    PropertyDefinition, SchemaChange,
};

/// Parse CREATE GRAPH TYPE statement
#[allow(dead_code)] // ROADMAP v0.4.0 - Graph type DDL for structured schema definitions
pub fn parse_create_graph_type(tokens: &[Token]) -> IResult<&[Token], CreateGraphTypeStatement> {
    let (tokens, _) = tag_token(Token::Create)(tokens)?;
    let (tokens, _) = tag_token(Token::Graph)(tokens)?;
    let (tokens, _) = tag_token(Token::Type)(tokens)?;

    // Parse IF NOT EXISTS clause
    let (tokens, if_not_exists) = opt(parse_if_not_exists)(tokens)?;

    // Parse graph type name
    let (tokens, name) = parse_identifier(tokens)?;

    // Parse optional VERSION clause
    let (tokens, version) = opt(parse_version_clause)(tokens)?;

    // Parse body within parentheses
    let (tokens, (node_types, edge_types)) = delimited(
        tag_token(Token::LeftParen),
        parse_graph_type_body,
        tag_token(Token::RightParen),
    )(tokens)?;

    Ok((
        tokens,
        CreateGraphTypeStatement {
            name,
            if_not_exists: if_not_exists.is_some(),
            version,
            node_types,
            edge_types,
        },
    ))
}

/// Parse DROP GRAPH TYPE statement
#[allow(dead_code)] // ROADMAP v0.4.0 - Graph type DDL for schema lifecycle management
pub fn parse_drop_graph_type(tokens: &[Token]) -> IResult<&[Token], DropGraphTypeStatement> {
    let (tokens, _) = tag_token(Token::Drop)(tokens)?;
    let (tokens, _) = tag_token(Token::Graph)(tokens)?;
    let (tokens, _) = tag_token(Token::Type)(tokens)?;

    // Parse IF EXISTS clause
    let (tokens, if_exists) = opt(parse_if_exists)(tokens)?;

    // Parse graph type name
    let (tokens, name) = parse_identifier(tokens)?;

    // Parse optional CASCADE/RESTRICT
    let (tokens, cascade) = opt(alt((
        value(true, tag_token(Token::Cascade)),
        value(false, tag_token(Token::Restrict)),
    )))(tokens)?;

    Ok((
        tokens,
        DropGraphTypeStatement {
            name,
            if_exists: if_exists.is_some(),
            cascade: cascade.unwrap_or(false),
        },
    ))
}

/// Parse ALTER GRAPH TYPE statement
#[allow(dead_code)] // ROADMAP v0.4.0 - Graph type DDL for schema evolution
pub fn parse_alter_graph_type(tokens: &[Token]) -> IResult<&[Token], AlterGraphTypeStatement> {
    let (tokens, _) = tag_token(Token::Alter)(tokens)?;
    let (tokens, _) = tag_token(Token::Graph)(tokens)?;
    let (tokens, _) = tag_token(Token::Type)(tokens)?;

    // Parse graph type name
    let (tokens, name) = parse_identifier(tokens)?;

    // Parse optional VERSION clause
    let (tokens, version) = opt(parse_version_clause)(tokens)?;

    // Parse schema changes
    let (tokens, changes) = many0(parse_schema_change)(tokens)?;

    Ok((
        tokens,
        AlterGraphTypeStatement {
            name,
            version,
            changes,
        },
    ))
}

/// Parse graph type body (node types and edge types)
#[allow(dead_code)] // ROADMAP v0.4.0 - Graph type body parser for node/edge type definitions
fn parse_graph_type_body(
    tokens: &[Token],
) -> IResult<&[Token], (Vec<NodeTypeDefinition>, Vec<EdgeTypeDefinition>)> {
    let mut tokens = tokens;
    let mut node_types = Vec::new();
    let mut edge_types = Vec::new();

    // Parse type definitions (can be in any order)
    loop {
        // Try to parse NODE TYPE
        if let Ok((remaining, node_type)) = parse_node_type(tokens) {
            node_types.push(node_type);
            tokens = remaining;

            // Skip optional comma
            if let Ok((remaining, _)) = tag_token(Token::Comma)(tokens) {
                tokens = remaining;
            }
            continue;
        }

        // Try to parse EDGE TYPE
        if let Ok((remaining, edge_type)) = parse_edge_type(tokens) {
            edge_types.push(edge_type);
            tokens = remaining;

            // Skip optional comma
            if let Ok((remaining, _)) = tag_token(Token::Comma)(tokens) {
                tokens = remaining;
            }
            continue;
        }

        // No more type definitions found
        break;
    }

    Ok((tokens, (node_types, edge_types)))
}

/// Parse NODE TYPE definition
#[allow(dead_code)] // ROADMAP v0.4.0 - Node type parser for structured graph schemas
fn parse_node_type(tokens: &[Token]) -> IResult<&[Token], NodeTypeDefinition> {
    let (tokens, _) = tag_token(Token::Node)(tokens)?;
    let (tokens, _) = tag_token(Token::Type)(tokens)?;

    // Parse node type label
    let (tokens, label) = parse_identifier(tokens)?;

    // Parse optional EXTENDS clause
    let (tokens, extends) = opt(preceded(tag_identifier("EXTENDS"), parse_identifier))(tokens)?;

    // Parse properties within parentheses
    let (tokens, properties) = delimited(
        tag_token(Token::LeftParen),
        separated_list0(tag_token(Token::Comma), parse_property_definition),
        tag_token(Token::RightParen),
    )(tokens)?;

    Ok((
        tokens,
        NodeTypeDefinition {
            label,
            properties,
            constraints: Vec::new(), // TODO: Parse constraints
            description: None,
            is_abstract: false,
            extends,
        },
    ))
}

/// Parse EDGE TYPE definition
#[allow(dead_code)] // ROADMAP v0.4.0 - Edge type parser for relationship schemas
fn parse_edge_type(tokens: &[Token]) -> IResult<&[Token], EdgeTypeDefinition> {
    let (tokens, _) = tag_token(Token::Edge)(tokens)?;
    let (tokens, _) = tag_token(Token::Type)(tokens)?;

    // Parse edge type name
    let (tokens, type_name) = parse_identifier(tokens)?;

    // Parse FROM and TO clauses
    let (tokens, (from_types, to_types)) = parse_edge_endpoints(tokens)?;

    // Parse optional properties
    let (tokens, properties) = opt(delimited(
        tag_token(Token::LeftParen),
        separated_list0(tag_token(Token::Comma), parse_property_definition),
        tag_token(Token::RightParen),
    ))(tokens)?;

    Ok((
        tokens,
        EdgeTypeDefinition {
            type_name,
            from_node_types: from_types,
            to_node_types: to_types,
            properties: properties.unwrap_or_default(),
            constraints: Vec::new(),
            description: None,
            cardinality: EdgeCardinality::default(),
        },
    ))
}

/// Parse edge endpoints (FROM ... TO ...)
#[allow(dead_code)] // ROADMAP v0.4.0 - Edge endpoint parser for relationship type constraints
fn parse_edge_endpoints(tokens: &[Token]) -> IResult<&[Token], (Vec<String>, Vec<String>)> {
    let (tokens, _) = tag_token(Token::LeftParen)(tokens)?;

    // Parse FROM clause
    let (tokens, _) = tag_token(Token::From)(tokens)?;
    let (tokens, from_types) = separated_list1(
        tag_token(Token::Pipe), // Use | for multiple types
        parse_identifier,
    )(tokens)?;

    // Parse TO clause
    let (tokens, _) = tag_token(Token::To)(tokens)?;
    let (tokens, to_types) = separated_list1(tag_token(Token::Pipe), parse_identifier)(tokens)?;

    let (tokens, _) = tag_token(Token::RightParen)(tokens)?;

    Ok((tokens, (from_types, to_types)))
}

/// Parse property definition
#[allow(dead_code)] // ROADMAP v0.4.0 - Property definition parser for schema attributes
fn parse_property_definition(tokens: &[Token]) -> IResult<&[Token], PropertyDefinition> {
    // Parse property name
    let (tokens, name) = parse_identifier(tokens)?;

    // Parse data type
    let (tokens, data_type) = parse_data_type(tokens)?;

    // Parse constraints (NOT NULL, UNIQUE, DEFAULT, etc.)
    let (tokens, (required, unique, default_value)) = parse_property_constraints(tokens)?;

    Ok((
        tokens,
        PropertyDefinition {
            name,
            data_type,
            required,
            unique,
            default_value,
            description: None,
            deprecated: false,
            deprecation_message: None,
            validation_pattern: None,
            constraints: Vec::new(), // Initialize with empty constraints for now
        },
    ))
}

/// Parse data type
#[allow(dead_code)] // ROADMAP v0.4.0 - Data type parser for property type specifications
fn parse_data_type(tokens: &[Token]) -> IResult<&[Token], DataType> {
    alt((
        value(DataType::String, tag_token(Token::StringType)),
        value(DataType::Integer, tag_token(Token::IntegerType)),
        value(DataType::BigInt, tag_token(Token::BigIntType)),
        value(DataType::Float, tag_token(Token::FloatType)),
        value(DataType::Double, tag_token(Token::DoubleType)),
        value(DataType::Boolean, tag_token(Token::BooleanType)),
        // TEXT type for full-text indexable content
        map(tag_identifier("TEXT"), |_| DataType::Text),
        // TIMESTAMP type
        map(tag_identifier("TIMESTAMP"), |_| DataType::Timestamp),
        // UUID type
        map(tag_identifier("UUID"), |_| DataType::Uuid),
        // Array type: ARRAY<ElementType>
        map(
            preceded(
                tag_identifier("ARRAY"),
                delimited(
                    tag_token(Token::LessThan),
                    parse_data_type,
                    tag_token(Token::GreaterThan),
                ),
            ),
            |element_type| DataType::Array(Box::new(element_type)),
        ),
        // Default to String for identifiers
        map(parse_identifier, |_| DataType::String),
    ))(tokens)
}

/// Parse property constraints
#[allow(dead_code)] // ROADMAP v0.4.0 - Property constraint parser for schema validation
fn parse_property_constraints(
    tokens: &[Token],
) -> IResult<&[Token], (bool, bool, Option<serde_json::Value>)> {
    let mut tokens = tokens;
    let mut required = false;
    let mut unique = false;
    let mut default_value = None;

    // Parse constraint keywords
    loop {
        // NOT NULL
        if let Ok((remaining, _)) = tuple((tag_token(Token::Not), tag_token(Token::Null)))(tokens) {
            required = true;
            tokens = remaining;
            continue;
        }

        // UNIQUE
        if let Ok((remaining, _)) = tag_identifier("UNIQUE")(tokens) {
            unique = true;
            tokens = remaining;
            continue;
        }

        // DEFAULT value
        if let Ok((remaining, _)) = tag_identifier("DEFAULT")(tokens) {
            // Parse default value (simplified - just capture as string for now)
            if let Ok((remaining, value)) = parse_literal_value(remaining) {
                default_value = Some(value);
                tokens = remaining;
            }
            continue;
        }

        // OPTIONAL (opposite of NOT NULL)
        if let Ok((remaining, _)) = tag_identifier("OPTIONAL")(tokens) {
            required = false;
            tokens = remaining;
            continue;
        }

        // No more constraints
        break;
    }

    Ok((tokens, (required, unique, default_value)))
}

/// Parse version clause (VERSION x.y.z)
#[allow(dead_code)] // ROADMAP v0.4.0 - Version clause parser for schema evolution
fn parse_version_clause(tokens: &[Token]) -> IResult<&[Token], GraphTypeVersion> {
    let (tokens, _) = tag_token(Token::Version)(tokens)?;

    // Parse version string (e.g., "1.0.0")
    let (tokens, version_str) = parse_string_literal(tokens)?;

    // Parse the version string
    let version = GraphTypeVersion::parse(&version_str).map_err(|_| {
        nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Verify,
        ))
    })?;

    Ok((tokens, version))
}

/// Parse schema change for ALTER operations
#[allow(dead_code)] // ROADMAP v0.4.0 - Schema change parser for ALTER GRAPH TYPE
fn parse_schema_change(tokens: &[Token]) -> IResult<&[Token], SchemaChange> {
    alt((
        // ADD NODE TYPE
        map(
            preceded(
                tuple((
                    tag_identifier("ADD"),
                    tag_token(Token::Node),
                    tag_token(Token::Type),
                )),
                parse_node_type,
            ),
            SchemaChange::AddNodeType,
        ),
        // DROP NODE TYPE
        map(
            preceded(
                tuple((
                    tag_token(Token::Drop),
                    tag_token(Token::Node),
                    tag_token(Token::Type),
                )),
                parse_identifier,
            ),
            SchemaChange::DropNodeType,
        ),
        // ADD PROPERTY TO NodeType
        map(
            tuple((
                preceded(
                    tuple((
                        tag_identifier("ADD"),
                        tag_token(Token::Property),
                        tag_token(Token::To),
                    )),
                    parse_identifier,
                ),
                parse_property_definition,
            )),
            |(type_name, property)| SchemaChange::AddProperty {
                type_name,
                is_node: true,
                property,
            },
        ),
    ))(tokens)
}

/// Parse IF NOT EXISTS clause
#[allow(dead_code)] // ROADMAP v0.4.0 - IF NOT EXISTS clause for idempotent DDL
fn parse_if_not_exists(tokens: &[Token]) -> IResult<&[Token], ()> {
    let (tokens, _) = tag_token(Token::If)(tokens)?;
    let (tokens, _) = tag_token(Token::Not)(tokens)?;
    let (tokens, _) = tag_token(Token::Exists)(tokens)?;
    Ok((tokens, ()))
}

/// Parse IF EXISTS clause
#[allow(dead_code)] // ROADMAP v0.4.0 - IF EXISTS clause for safe DDL operations
fn parse_if_exists(tokens: &[Token]) -> IResult<&[Token], ()> {
    let (tokens, _) = tag_token(Token::If)(tokens)?;
    let (tokens, _) = tag_token(Token::Exists)(tokens)?;
    Ok((tokens, ()))
}

/// Parse an identifier
#[allow(dead_code)] // ROADMAP v0.4.0 - Token-level identifier parser for DDL statements
fn parse_identifier(tokens: &[Token]) -> IResult<&[Token], String> {
    match tokens.first() {
        Some(Token::Identifier(name)) => Ok((&tokens[1..], name.clone())),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

/// Parse a string literal
#[allow(dead_code)] // ROADMAP v0.4.0 - String literal parser for DDL values
fn parse_string_literal(tokens: &[Token]) -> IResult<&[Token], String> {
    match tokens.first() {
        Some(Token::String(s)) => Ok((&tokens[1..], s.clone())),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

/// Parse a literal value (for default values)
#[allow(dead_code)] // ROADMAP v0.4.0 - Literal value parser for DEFAULT constraints
fn parse_literal_value(tokens: &[Token]) -> IResult<&[Token], serde_json::Value> {
    alt((
        // String literal
        map(parse_string_literal, serde_json::Value::String),
        // Number literal
        map(parse_number_literal, |n| {
            serde_json::Value::Number(serde_json::Number::from(n))
        }),
        // Boolean literal
        map(parse_boolean_literal, serde_json::Value::Bool),
        // NULL literal
        value(serde_json::Value::Null, tag_token(Token::Null)),
    ))(tokens)
}

/// Parse a boolean literal
#[allow(dead_code)] // ROADMAP v0.4.0 - Boolean literal parser for constraint values
fn parse_boolean_literal(tokens: &[Token]) -> IResult<&[Token], bool> {
    match tokens.first() {
        Some(Token::Boolean(b)) => Ok((&tokens[1..], *b)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

/// Parse a number literal
#[allow(dead_code)] // ROADMAP v0.4.0 - Number literal parser for numeric constraint values
fn parse_number_literal(tokens: &[Token]) -> IResult<&[Token], i64> {
    match tokens.first() {
        Some(Token::Integer(n)) => Ok((&tokens[1..], *n)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

/// Helper to match a specific token
#[allow(dead_code)] // ROADMAP v0.4.0 - Token matcher for parser combinators
fn tag_token(expected: Token) -> impl Fn(&[Token]) -> IResult<&[Token], &Token> {
    move |tokens: &[Token]| match tokens.first() {
        Some(token) if std::mem::discriminant(token) == std::mem::discriminant(&expected) => {
            Ok((&tokens[1..], token))
        }
        _ => Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

/// Helper to match an identifier with a specific value
#[allow(dead_code)] // ROADMAP v0.4.0 - Identifier matcher for keyword detection in DDL
fn tag_identifier(expected: &str) -> impl Fn(&[Token]) -> IResult<&[Token], ()> + '_ {
    move |tokens: &[Token]| match tokens.first() {
        Some(Token::Identifier(name)) if name == expected => Ok((&tokens[1..], ())),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::lexer::tokenize;

    #[test]
    fn test_parse_create_graph_type() {
        let input = "CREATE GRAPH TYPE UserGraphType (
            NODE TYPE User (
                username STRING NOT NULL,
                email STRING UNIQUE NOT NULL
            ),
            EDGE TYPE FOLLOWS (
                FROM User TO User
            )
        )";

        let tokens = tokenize(input).unwrap();
        let result = parse_create_graph_type(&tokens);

        if let Err(e) = &result {
            log::debug!("Parse error: {:?}", e);
        }
        assert!(result.is_ok());
        if let Ok((_, stmt)) = result {
            assert_eq!(stmt.name, "UserGraphType");
            assert_eq!(stmt.node_types.len(), 1);
            assert_eq!(stmt.edge_types.len(), 1);
        }
    }

    #[test]
    fn test_parse_drop_graph_type() {
        let input = "DROP GRAPH TYPE UserGraphType CASCADE";
        let tokens = tokenize(input).unwrap();
        let result = parse_drop_graph_type(&tokens);

        assert!(result.is_ok());
        if let Ok((_, stmt)) = result {
            assert_eq!(stmt.name, "UserGraphType");
            assert!(stmt.cascade);
        }
    }
}
