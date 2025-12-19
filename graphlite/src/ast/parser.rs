// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Parser for GQL graph language using nom parsers

use log::debug;
use nom::{
    branch::alt,
    combinator::{map, opt, success, value, verify},
    multi::{many0, many1, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};

use super::ast::*;
use super::lexer::{tokenize, Token};
use super::pretty_printer::pretty_print_ast;

/// Parser error type
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Lexer error: {0}")]
    LexerError(String),
    #[error("Unexpected token: {0:?}")]
    UnexpectedToken(Token),
    #[error("Expected token: {0:?}")]
    ExpectedToken(Token),
    #[error("Invalid syntax 'DELETE SCHEMA'. Schema deletion uses 'DROP SCHEMA'. Correct syntax: DROP SCHEMA [IF EXISTS] schema_name [CASCADE | RESTRICT]. Example: DROP SCHEMA analytics_db")]
    InvalidDeleteSchema,
    #[error("Invalid syntax 'DELETE GRAPH'. Graph deletion uses 'DROP GRAPH'. Correct syntax: DROP [PROPERTY] GRAPH [IF EXISTS] graph_path. Example: DROP GRAPH /test_schema/test_restrict_graph2")]
    InvalidDeleteGraph,
    #[error("Incomplete UNION operation. Expected a query after UNION. Syntax: query1 UNION [ALL] query2")]
    IncompleteUnion,
    #[error("Incomplete EXCEPT operation. Expected a query after EXCEPT. Syntax: query1 EXCEPT [ALL] query2")]
    IncompleteExcept,
    #[error("Incomplete INTERSECT operation. Expected a query after INTERSECT. Syntax: query1 INTERSECT [ALL] query2")]
    IncompleteIntersect,
}

/// Filter SQL-style comments from a token stream
/// SQL comments are two consecutive Dash tokens followed by any tokens until EOF or newline
fn filter_sql_comments(tokens: Vec<Token>) -> Vec<Token> {
    let mut filtered = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        // Check for SQL comment pattern: Dash, Dash, ...
        if i + 1 < tokens.len()
            && matches!(tokens[i], Token::Dash)
            && matches!(tokens[i + 1], Token::Dash)
        {
            // Check if this is actually a comment (not part of an edge pattern)
            // SQL comments are recognized when -- is followed by whitespace or identifiers
            // but NOT when followed by >, <, [, or (
            if i + 2 < tokens.len() {
                match &tokens[i + 2] {
                    Token::Arrow
                    | Token::ArrowLeft
                    | Token::ArrowBoth
                    | Token::LeftParen
                    | Token::LeftBracket => {
                        // This is an edge pattern, not a comment
                        filtered.push(tokens[i].clone());
                        i += 1;
                    }
                    Token::Eof => {
                        // Just -- at end of input, keep as edge pattern
                        filtered.push(tokens[i].clone());
                        i += 1;
                    }
                    _ => {
                        // This looks like a SQL comment, skip all tokens until EOF
                        // (In a real implementation, we'd skip until newline, but our lexer
                        // doesn't produce newline tokens, so we skip to EOF)
                        while i < tokens.len() && !matches!(tokens[i], Token::Eof) {
                            i += 1;
                        }
                    }
                }
            } else {
                // Just -- at end, keep as edge pattern
                filtered.push(tokens[i].clone());
                i += 1;
            }
        } else {
            filtered.push(tokens[i].clone());
            i += 1;
        }
    }

    filtered
}

/// Parse a GQL query or statement into an AST Document
pub fn parse_query(input: &str) -> Result<Document, ParserError> {
    // Debug: Print query text if it contains GROUP BY
    if input.contains("GROUP BY") {
        log::debug!(
            "PARSER: Parsing query with GROUP BY: {}",
            input.trim().split('\n').collect::<Vec<_>>().join(" ")
        );
    }

    // First tokenize the input
    let mut tokens = tokenize(input).map_err(ParserError::LexerError)?;

    // Filter out SQL-style comments at the parser level
    tokens = filter_sql_comments(tokens);

    // Debug: Check if GROUP BY tokens exist
    if input.contains("GROUP BY") {
        let has_group_token = tokens.iter().any(|t| matches!(t, Token::Group));
        let has_by_token = tokens.iter().any(|t| matches!(t, Token::By));
        log::debug!(
            "PARSER: Token stream has GROUP={}, BY={}",
            has_group_token,
            has_by_token
        );
    }

    // Check for invalid DELETE SCHEMA and DELETE GRAPH patterns before parsing
    if tokens.len() >= 2 {
        match (&tokens[0], &tokens[1]) {
            (Token::Delete, Token::Schema) => {
                return Err(ParserError::InvalidDeleteSchema);
            }
            (Token::Delete, Token::Graph) => {
                return Err(ParserError::InvalidDeleteGraph);
            }
            _ => {}
        }
    }

    // Check for incomplete set operations (trailing keywords without right-hand side)
    // Look for set operation keywords followed by EOF or at the end
    for i in 0..tokens.len() {
        match &tokens[i] {
            Token::Union => {
                // Check if this is followed by EOF or is the last meaningful token
                if i + 1 >= tokens.len() || matches!(tokens.get(i + 1), Some(Token::Eof) | None) {
                    return Err(ParserError::IncompleteUnion);
                }
            }
            Token::Except => {
                if i + 1 >= tokens.len() || matches!(tokens.get(i + 1), Some(Token::Eof) | None) {
                    return Err(ParserError::IncompleteExcept);
                }
            }
            Token::Intersect => {
                if i + 1 >= tokens.len() || matches!(tokens.get(i + 1), Some(Token::Eof) | None) {
                    return Err(ParserError::IncompleteIntersect);
                }
            }
            _ => {}
        }
    }

    // Handle comment-only or empty input
    if tokens.is_empty() || (tokens.len() == 1 && matches!(tokens[0], Token::Eof)) {
        return Err(ParserError::ExpectedToken(Token::Identifier(
            "statement".to_string(),
        )));
    }

    // Try to parse as different statement types
    if let Ok((_, at_stmt)) = at_location_statement(&tokens) {
        let document = Document {
            statement: Statement::AtLocation(at_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed AT location statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, declare_stmt)) = declare_statement(&tokens) {
        let document = Document {
            statement: Statement::Declare(declare_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed DECLARE statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    // NEXT statements are not allowed as standalone statements
    // They can only appear within procedure body contexts
    } else if let Ok((_, session_stmt)) = session_statement(&tokens) {
        let document = Document {
            statement: Statement::SessionStatement(session_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed session statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, transaction_stmt)) = transaction_statement(&tokens) {
        let document = Document {
            statement: Statement::TransactionStatement(transaction_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed transaction statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, catalog_stmt)) = catalog_statement(&tokens) {
        let document = Document {
            statement: Statement::CatalogStatement(catalog_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed catalog statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, index_stmt)) = index_statement(&tokens) {
        let document = Document {
            statement: Statement::IndexStatement(index_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed index statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, data_stmt)) = data_statement(&tokens) {
        log::debug!(
            "PARSER: Matched as DataStatement: {:?}",
            std::mem::discriminant(&data_stmt)
        );
        let document = Document {
            statement: Statement::DataStatement(data_stmt.clone()),
            location: Location::default(),
        };

        debug!(
            "Successfully parsed data modification statement into AST: {:?}",
            std::mem::discriminant(&data_stmt)
        );
        if let DataStatement::MatchSet(ref ms) = data_stmt {
            debug!(
                "PARSER: MatchSet has WITH clause: {}",
                ms.with_clause.is_some()
            );
        }
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, procedure_body)) = procedure_body_statement(&tokens) {
        let document = Document {
            statement: Statement::ProcedureBody(procedure_body),
            location: Location::default(),
        };

        debug!("Successfully parsed procedure body into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((remaining, call_stmt)) = call_statement(&tokens) {
        // Validate that only Semicolon/EOF remain after CALL statement
        // CALL cannot be combined with RETURN, MATCH, or other clauses
        let only_terminators = remaining
            .iter()
            .all(|t| matches!(t, Token::Semicolon | Token::Eof));

        if !only_terminators {
            // Found unexpected tokens after CALL statement
            // Get the first unexpected token
            let unexpected = remaining
                .iter()
                .find(|t| !matches!(t, Token::Semicolon | Token::Eof))
                .unwrap_or(&Token::Eof);
            return Err(ParserError::UnexpectedToken(unexpected.clone()));
        }

        let document = Document {
            statement: Statement::Call(call_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed CALL statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, select_stmt)) = select_statement(&tokens) {
        let document = Document {
            statement: Statement::Select(select_stmt),
            location: Location::default(),
        };

        debug!("Successfully parsed SELECT statement into AST");
        pretty_print_ast(&document);

        Ok(document)
    } else if let Ok((_, query)) = query(&tokens) {
        log::debug!(
            "PARSER: Matched as Query: {:?}",
            std::mem::discriminant(&query)
        );
        if let Query::Basic(ref basic_query) = query {
            log::debug!(
                "PARSER: BasicQuery has GROUP BY: {}",
                basic_query.group_clause.is_some()
            );
        } else if let Query::Limited { ref query, .. } = query {
            log::debug!(
                "PARSER: Limited query wrapping: {:?}",
                std::mem::discriminant(query.as_ref())
            );
            if let Query::Basic(ref basic_query) = query.as_ref() {
                log::debug!(
                    "PARSER: Wrapped BasicQuery has GROUP BY: {}",
                    basic_query.group_clause.is_some()
                );
            }
        }
        let document = Document {
            statement: Statement::Query(query.clone()),
            location: Location::default(),
        };

        debug!(
            "Successfully parsed GQL query into AST: {:?}",
            std::mem::discriminant(&query)
        );
        if let Query::MutationPipeline(ref mp) = query {
            debug!(
                "PARSER: MutationPipeline has {} segments",
                mp.segments.len()
            );
        }
        pretty_print_ast(&document);

        Ok(document)
    } else {
        debug!("PARSER: Failed to parse as any statement type");
        debug!(
            "PARSER: First few tokens: {:?}",
            tokens.get(0..10).unwrap_or(&[])
        );
        // Return the first token as unexpected since we couldn't parse it
        let unexpected = tokens.first().unwrap_or(&Token::Eof);
        Err(ParserError::UnexpectedToken(unexpected.clone()))
    }
}

/// Parse a complete query: MATCH [WHERE] RETURN [GROUP BY] [HAVING] [ORDER BY] [LIMIT]
/// Parse query with set operations support
/// <query-statement> ::= <query-term> (("UNION" | "EXCEPT") ["ALL"] <query-term> | "INTERSECT" ["ALL"] <query-term>)*
fn query(tokens: &[Token]) -> IResult<&[Token], Query> {
    parse_set_operation(tokens)
}

/// Parse core query logic (set operations, basic queries) without modifiers
/// This is the clean separation: core query parsing vs modifier parsing
fn parse_core_query(tokens: &[Token]) -> IResult<&[Token], Query> {
    parse_union_except(tokens)
}

/// Apply ORDER BY and LIMIT modifiers to any query type
/// This is the unified approach - one function handles all query variants
fn apply_query_modifiers(
    query: Query,
    order_clause: Option<OrderClause>,
    limit_clause: Option<LimitClause>,
) -> Query {
    match (order_clause, limit_clause) {
        // No modifiers - return query as-is
        (None, None) => query,

        // Has modifiers - apply them based on query type
        (order, limit) => {
            match query {
                // For set operations, apply modifiers directly to the SetOperation struct
                Query::SetOperation(mut set_op) => {
                    set_op.order_clause = order;
                    set_op.limit_clause = limit;
                    Query::SetOperation(set_op)
                }

                // For BasicQuery, apply modifiers directly to preserve GROUP BY/HAVING
                // BasicQuery already has order_clause and limit_clause fields
                Query::Basic(mut basic_query) => {
                    // Only override if not already set (basic_query parser may have consumed them)
                    if basic_query.order_clause.is_none() {
                        basic_query.order_clause = order;
                    }
                    if basic_query.limit_clause.is_none() {
                        basic_query.limit_clause = limit;
                    }
                    Query::Basic(basic_query)
                }

                // For other query types, use the Limited wrapper
                _ => Query::Limited {
                    query: Box::new(query),
                    order_clause: order,
                    limit_clause: limit,
                },
            }
        }
    }
}

/// Parse complete query with clean precedence: core query + modifiers
/// This implements Option 3: Restructured Parser Precedence
fn parse_set_operation(tokens: &[Token]) -> IResult<&[Token], Query> {
    // Step 1: Parse the core query (UNION, INTERSECT, MATCH, etc.)
    let (remaining, core_query) = parse_core_query(tokens)?;

    log::debug!(
        "parse_set_operation: core_query type = {:?}",
        std::mem::discriminant(&core_query)
    );
    match &core_query {
        Query::Basic(bq) => {
            log::debug!("parse_set_operation: BasicQuery has group_clause={}, order_clause={}, limit_clause={}",
                     bq.group_clause.is_some(), bq.order_clause.is_some(), bq.limit_clause.is_some());
        }
        Query::Limited {
            query,
            order_clause,
            limit_clause,
        } => {
            log::debug!(
                "parse_set_operation: Limited query has order={}, limit={}",
                order_clause.is_some(),
                limit_clause.is_some()
            );
            if let Query::Basic(bq) = query.as_ref() {
                log::debug!("parse_set_operation: Wrapped BasicQuery has group_clause={}, order_clause={}, limit_clause={}",
                         bq.group_clause.is_some(), bq.order_clause.is_some(), bq.limit_clause.is_some());
            }
        }
        _ => {
            log::debug!("parse_set_operation: Other query type");
        }
    }

    // Step 2: Parse optional modifiers that can apply to any query
    let (remaining, order_clause) = opt(order_clause)(remaining)?;
    let (remaining, limit_clause) = opt(limit_clause)(remaining)?;

    log::debug!(
        "parse_set_operation: Parsed order_clause={}, limit_clause={}",
        order_clause.is_some(),
        limit_clause.is_some()
    );

    // Step 3: Apply modifiers using unified logic
    let final_query = apply_query_modifiers(core_query, order_clause, limit_clause);

    log::debug!(
        "parse_set_operation: final_query type = {:?}",
        std::mem::discriminant(&final_query)
    );

    Ok((remaining, final_query))
}

/// Parse UNION and EXCEPT operations (lower precedence)
fn parse_union_except(tokens: &[Token]) -> IResult<&[Token], Query> {
    let (mut remaining, mut left) = parse_intersect(tokens)?;

    while let Ok((new_remaining, (operation, right))) = parse_union_except_op(remaining) {
        left = Query::SetOperation(SetOperation {
            left: Box::new(left),
            operation,
            right: Box::new(right),
            limit_clause: None,
            order_clause: None,
            location: Location::default(),
        });
        remaining = new_remaining;
    }

    // Check for incomplete set operations (UNION/EXCEPT without right-hand side)
    if !remaining.is_empty() {
        if let Some(token) = remaining.first() {
            match token {
                Token::Union => {
                    return Err(nom::Err::Error(nom::error::Error::new(
                        remaining,
                        nom::error::ErrorKind::Alt,
                    )));
                }
                Token::Except => {
                    return Err(nom::Err::Error(nom::error::Error::new(
                        remaining,
                        nom::error::ErrorKind::Alt,
                    )));
                }
                _ => {}
            }
        }
    }

    Ok((remaining, left))
}

/// Parse INTERSECT operations (higher precedence)
fn parse_intersect(tokens: &[Token]) -> IResult<&[Token], Query> {
    let (mut remaining, mut left) = parse_query_term(tokens)?;

    while let Ok((new_remaining, (operation, right))) = parse_intersect_op(remaining) {
        left = Query::SetOperation(SetOperation {
            left: Box::new(left),
            operation,
            right: Box::new(right),
            limit_clause: None,
            order_clause: None,
            location: Location::default(),
        });
        remaining = new_remaining;
    }

    // Check for incomplete INTERSECT operations
    if !remaining.is_empty() {
        if let Some(token) = remaining.first() {
            if token == &Token::Intersect {
                return Err(nom::Err::Error(nom::error::Error::new(
                    remaining,
                    nom::error::ErrorKind::Alt,
                )));
            }
        }
    }

    Ok((remaining, left))
}

/// Parse a single query term (basic query or parenthesized set operation)
fn parse_query_term(tokens: &[Token]) -> IResult<&[Token], Query> {
    alt((
        // Parenthesized query with optional ORDER BY and LIMIT
        parse_parenthesized_query_with_modifiers,
        // LET statement
        let_statement,
        // FOR statement
        for_statement,
        // FILTER statement
        filter_statement,
        // UNWIND statement
        unwind_statement,
        // Mutation pipeline (WITH...UNWIND...REMOVE/SET/DELETE)
        mutation_pipeline,
        // Basic query - MUST come before with_query!
        // BasicQuery (MATCH...RETURN) is more specific than WithQuery (MATCH...WITH...RETURN)
        // so it should be tried first to avoid WithQuery consuming BasicQuery patterns
        basic_query,
        // WITH query (chain of segments)
        with_query,
        // Standalone RETURN query
        return_query,
    ))(tokens)
}

/// Parse parenthesized query - only parse clauses that are actually inside parentheses
/// This prevents consuming trailing clauses that belong to outer set operations
fn parse_parenthesized_query_with_modifiers(tokens: &[Token]) -> IResult<&[Token], Query> {
    // Parse parenthesized query without trailing modifiers
    let (remaining, query) = delimited(
        expect_token(Token::LeftParen),
        parse_set_operation, // Use full parsing inside parentheses (this handles internal LIMIT/ORDER BY)
        expect_token(Token::RightParen),
    )(tokens)?;

    Ok((remaining, query))
}

/// Parse basic query (MATCH ... RETURN ...)
fn basic_query(tokens: &[Token]) -> IResult<&[Token], Query> {
    log::debug!("basic_query function called");
    map(
        tuple((
            match_clause,
            opt(where_clause),
            return_clause,
            opt(group_clause),
            opt(having_clause),
            opt(order_clause),
            opt(limit_clause),
        )),
        |(
            match_clause,
            where_clause,
            return_clause,
            group_clause,
            having_clause,
            order_clause,
            limit_clause,
        )| {
            log::debug!(
                "basic_query parser: group_clause={}, order_clause={}, limit_clause={}",
                group_clause.is_some(),
                order_clause.is_some(),
                limit_clause.is_some()
            );
            Query::Basic(BasicQuery {
                match_clause,
                where_clause,
                return_clause,
                group_clause,
                having_clause,
                order_clause,
                limit_clause,
                location: Location::default(),
            })
        },
    )(tokens)
}

/// Parse standalone RETURN query: RETURN [DISTINCT|ALL] items [GROUP BY] [HAVING] [ORDER BY] [LIMIT]
fn return_query(tokens: &[Token]) -> IResult<&[Token], Query> {
    map(
        tuple((
            return_clause,
            opt(group_clause),
            opt(having_clause),
            opt(order_clause),
            opt(limit_clause),
        )),
        |(return_clause, group_clause, having_clause, order_clause, limit_clause)| {
            Query::Return(ReturnQuery {
                return_clause,
                group_clause,
                having_clause,
                order_clause,
                limit_clause,
                location: Location::default(),
            })
        },
    )(tokens)
}

/// Parse final mutation action
fn final_mutation(tokens: &[Token]) -> IResult<&[Token], FinalMutation> {
    alt((
        // REMOVE items
        map(
            preceded(
                expect_token(Token::Remove),
                separated_list1(expect_token(Token::Comma), remove_item),
            ),
            FinalMutation::Remove,
        ),
        // SET items
        map(
            preceded(
                expect_token(Token::Set),
                separated_list1(expect_token(Token::Comma), set_item),
            ),
            FinalMutation::Set,
        ),
        // DELETE expressions
        map(
            tuple((
                opt(alt((
                    expect_token(Token::Detach),
                    expect_token(Token::NoDetach),
                ))),
                expect_token(Token::Delete),
                separated_list1(expect_token(Token::Comma), expression),
            )),
            |(detach_mode, _, expressions)| FinalMutation::Delete {
                expressions,
                detach: matches!(detach_mode, Some(Token::Detach)),
            },
        ),
    ))(tokens)
}

/// Parse mutation pipeline: MATCH ... WITH ... [UNWIND ... [WHERE ...]] REMOVE/SET/DELETE ...
fn mutation_pipeline(tokens: &[Token]) -> IResult<&[Token], Query> {
    log::debug!(
        "mutation_pipeline: trying to parse with tokens: {:?}",
        tokens.get(0..10)
    );

    // Try parsing: segments + UNWIND + WHERE + mutation
    let parse_with_unwind = map(
        tuple((
            many1(query_segment_no_unwind),
            unwind_clause,
            opt(where_clause),
            final_mutation,
        )),
        |(mut segments, unwind, where_cl, final_mutation)| {
            // Add the UNWIND and WHERE to the last segment
            if let Some(last_segment) = segments.last_mut() {
                last_segment.unwind_clause = Some(unwind);
                last_segment.post_unwind_where = where_cl;
            }
            Query::MutationPipeline(MutationPipeline {
                segments,
                final_mutation,
                location: Location::default(),
            })
        },
    );

    // Try parsing: segments (with possible UNWIND in last segment) + mutation
    let parse_without_unwind = map(
        tuple((many1(query_segment), final_mutation)),
        |(segments, final_mutation)| {
            Query::MutationPipeline(MutationPipeline {
                segments,
                final_mutation,
                location: Location::default(),
            })
        },
    );

    alt((parse_with_unwind, parse_without_unwind))(tokens)
}

/// Parse a query segment without UNWIND: MATCH [WHERE] WITH
fn query_segment_no_unwind(tokens: &[Token]) -> IResult<&[Token], QuerySegment> {
    map(
        tuple((match_clause, opt(where_clause), opt(with_clause))),
        |(match_clause, where_clause, with_clause)| QuerySegment {
            match_clause,
            where_clause,
            with_clause,
            unwind_clause: None,
            post_unwind_where: None,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse WITH query: MATCH ... WITH ... [MATCH ... WITH ...] RETURN ...
fn with_query(tokens: &[Token]) -> IResult<&[Token], Query> {
    map(
        tuple((
            many1(query_segment),
            return_clause,
            opt(group_clause),
            opt(having_clause),
            opt(order_clause),
            opt(limit_clause),
        )),
        |(segments, final_return, group_clause, having_clause, order_clause, limit_clause)| {
            Query::WithQuery(WithQuery {
                segments,
                final_return,
                group_clause,
                having_clause,
                order_clause,
                limit_clause,
                location: Location::default(),
            })
        },
    )(tokens)
}

/// Parse LET statement: LET variable = expression [, variable = expression]*
fn let_statement(tokens: &[Token]) -> IResult<&[Token], Query> {
    preceded(
        tag_token(Token::Let),
        map(
            separated_list1(tag_token(Token::Comma), variable_definition),
            |variable_definitions| {
                Query::Let(LetStatement {
                    variable_definitions,
                    location: Location::default(),
                })
            },
        ),
    )(tokens)
}

/// Parse variable definition: variable = expression
fn variable_definition(tokens: &[Token]) -> IResult<&[Token], VariableDefinition> {
    map(
        tuple((identifier, tag_token(Token::Equal), expression)),
        |(variable_name, _, expr)| VariableDefinition {
            variable_name,
            expression: expr,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse FOR statement: FOR [alias:] variable IN expression
fn for_statement(tokens: &[Token]) -> IResult<&[Token], Query> {
    preceded(
        tag_token(Token::For),
        map(
            tuple((
                opt(terminated(identifier, tag_token(Token::Colon))),
                identifier,
                tag_token(Token::In),
                expression,
            )),
            |(alias, variable, _, expr)| {
                Query::For(ForStatement {
                    variable,
                    alias,
                    expression: expr,
                    location: Location::default(),
                })
            },
        ),
    )(tokens)
}

/// Parse FILTER statement: FILTER [WHERE] expression
fn filter_statement(tokens: &[Token]) -> IResult<&[Token], Query> {
    preceded(
        tag_token(Token::Filter),
        map(
            alt((
                // FILTER WHERE expression
                preceded(tag_token(Token::Where), expression),
                // FILTER expression (without WHERE)
                expression,
            )),
            |expr| {
                Query::Filter(FilterStatement {
                    where_clause: WhereClause {
                        condition: expr,
                        location: Location::default(),
                    },
                    location: Location::default(),
                })
            },
        ),
    )(tokens)
}

/// Parse UNWIND statement: UNWIND expression AS variable
fn unwind_statement(tokens: &[Token]) -> IResult<&[Token], Query> {
    map(
        tuple((
            tag_token(Token::Unwind),
            expression,
            tag_token(Token::As),
            identifier,
        )),
        |(_, expr, _, var)| {
            Query::Unwind(UnwindStatement {
                expression: expr,
                variable: var,
                location: Location::default(),
            })
        },
    )(tokens)
}

/// Parse UNWIND clause: UNWIND expression AS variable
fn unwind_clause(tokens: &[Token]) -> IResult<&[Token], UnwindClause> {
    map(
        tuple((
            tag_token(Token::Unwind),
            expression,
            tag_token(Token::As),
            identifier,
        )),
        |(_, expr, _, var)| UnwindClause {
            expression: expr,
            variable: var,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse a query segment: MATCH [WHERE] [WITH] [WHERE] [ORDER BY] [LIMIT] [UNWIND ... AS ...] [WHERE]
fn query_segment(tokens: &[Token]) -> IResult<&[Token], QuerySegment> {
    map(
        tuple((
            match_clause,
            opt(where_clause),
            opt(with_clause),
            opt(unwind_clause),
            opt(where_clause),
        )),
        |(match_clause, where_clause, with_clause, unwind_clause, post_unwind_where)| {
            QuerySegment {
                match_clause,
                where_clause,
                with_clause,
                unwind_clause,
                post_unwind_where,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse UNION or EXCEPT operation
fn parse_union_except_op(tokens: &[Token]) -> IResult<&[Token], (SetOperationType, Query)> {
    alt((
        // UNION [ALL]
        map(
            tuple((
                expect_token(Token::Union),
                opt(expect_token(Token::All)),
                parse_intersect,
            )),
            |(_, all, right)| {
                let op = if all.is_some() {
                    SetOperationType::UnionAll
                } else {
                    SetOperationType::Union
                };
                (op, right)
            },
        ),
        // EXCEPT [ALL]
        map(
            tuple((
                expect_token(Token::Except),
                opt(expect_token(Token::All)),
                parse_intersect,
            )),
            |(_, all, right)| {
                let op = if all.is_some() {
                    SetOperationType::ExceptAll
                } else {
                    SetOperationType::Except
                };
                (op, right)
            },
        ),
    ))(tokens)
}

/// Parse INTERSECT operation
fn parse_intersect_op(tokens: &[Token]) -> IResult<&[Token], (SetOperationType, Query)> {
    map(
        tuple((
            expect_token(Token::Intersect),
            opt(expect_token(Token::All)),
            parse_query_term,
        )),
        |(_, all, right)| {
            let op = if all.is_some() {
                SetOperationType::IntersectAll
            } else {
                SetOperationType::Intersect
            };
            (op, right)
        },
    )(tokens)
}

/// Parse a SELECT statement: SELECT [DISTINCT|ALL] (* | return_items) [FROM graph_expression [match_statement]] [WHERE] [GROUP BY] [HAVING] [ORDER BY] [LIMIT]
fn select_statement(tokens: &[Token]) -> IResult<&[Token], SelectStatement> {
    map(
        tuple((
            expect_token(Token::Select),
            opt(distinct_qualifier),
            select_list,
            opt(from_clause),
            opt(where_clause),
            opt(group_clause),
            opt(having_clause),
            opt(order_clause),
            opt(limit_clause),
        )),
        |(
            _,
            distinct,
            return_items,
            from_clause,
            where_clause,
            group_clause,
            having_clause,
            order_clause,
            limit_clause,
        )| SelectStatement {
            distinct: distinct.unwrap_or(DistinctQualifier::None),
            return_items,
            from_clause,
            where_clause,
            group_clause,
            having_clause,
            order_clause,
            limit_clause,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse SELECT list: * | return_item (, return_item)*
fn select_list(tokens: &[Token]) -> IResult<&[Token], SelectItems> {
    alt((
        // SELECT *
        map(expect_token(Token::Star), |_| SelectItems::Wildcard {
            location: Location::default(),
        }),
        // SELECT return_item (, return_item)*
        map(
            separated_list1(expect_token(Token::Comma), return_item),
            |items| SelectItems::Explicit {
                items,
                location: Location::default(),
            },
        ),
    ))(tokens)
}

/// Parse FROM clause: FROM graph_expression [match_statement] (, graph_expression [match_statement])*
fn from_clause(tokens: &[Token]) -> IResult<&[Token], FromClause> {
    map(
        tuple((
            expect_token(Token::From),
            separated_list1(expect_token(Token::Comma), from_graph_expression),
        )),
        |(_, graph_expressions)| FromClause {
            graph_expressions,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse FROM graph expression: graph_expression [match_statement] OR just match_statement
///
/// NON-STANDARD EXTENSION: This parser supports "FROM MATCH ..." syntax for convenience,
/// which is not part of ISO GQL standard. In ISO GQL, FROM clause requires a graph expression.
/// Standard syntax: SELECT ... FROM /graph/path MATCH ...
/// Extension syntax: SELECT ... FROM MATCH ... (uses current session graph)
fn from_graph_expression(tokens: &[Token]) -> IResult<&[Token], FromGraphExpression> {
    alt((
        // NON-STANDARD: Support "FROM MATCH ..." syntax (Neo4j Cypher-style)
        // This uses the current session graph implicitly
        map(match_clause, |match_statement| FromGraphExpression {
            // Use CurrentGraph to indicate session graph should be used
            graph_expression: GraphExpression::CurrentGraph,
            match_statement: Some(match_statement),
            location: Location::default(),
        }),
        // STANDARD: FROM graph_expression [MATCH ...]
        map(
            tuple((graph_expression, opt(match_clause))),
            |(graph_expression, match_statement)| FromGraphExpression {
                graph_expression,
                match_statement,
                location: Location::default(),
            },
        ),
    ))(tokens)
}

/// Parse DISTINCT qualifier: DISTINCT | ALL
fn distinct_qualifier(tokens: &[Token]) -> IResult<&[Token], DistinctQualifier> {
    alt((
        value(DistinctQualifier::Distinct, expect_token(Token::Distinct)),
        value(DistinctQualifier::All, expect_token(Token::All)),
    ))(tokens)
}

/// Parse CALL statement: CALL procedure_name(args...) [YIELD ...] [WHERE ...]
/// NOTE: WHERE on CALL is a GraphLite extension (not in ISO GQL standard)
fn call_statement(tokens: &[Token]) -> IResult<&[Token], CallStatement> {
    map(
        tuple((
            expect_token(Token::Call),
            procedure_call,
            opt(yield_clause),
            opt(where_clause),
        )),
        |(_, (procedure_name, arguments), yield_clause, where_clause)| CallStatement {
            procedure_name,
            arguments,
            yield_clause,
            where_clause,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse procedure call: procedure_name(args...)
fn procedure_call(tokens: &[Token]) -> IResult<&[Token], (String, Vec<Expression>)> {
    map(
        tuple((
            property_access_as_string,
            expect_token(Token::LeftParen),
            opt(expression_list),
            expect_token(Token::RightParen),
        )),
        |(proc_name, _, args, _)| (proc_name, args.unwrap_or_default()),
    )(tokens)
}

/// Parse property access as string (for procedure names like gql.list_schemas)
fn property_access_as_string(tokens: &[Token]) -> IResult<&[Token], String> {
    if let Some(Token::PropertyAccess(prop_access)) = tokens.first() {
        // Handle PropertyAccess token (e.g., "gql.list_schemas")
        Ok((&tokens[1..], prop_access.clone()))
    } else if let Some(Token::Identifier(name)) = tokens.first() {
        let rest = &tokens[1..];

        // Check if this is a property access (name.property)
        if let Some(Token::Dot) = rest.first() {
            if let Some(Token::Identifier(property)) = rest.get(1) {
                return Ok((&rest[2..], format!("{}.{}", name, property)));
            }
        }

        // Simple identifier
        Ok((rest, name.clone()))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse YIELD clause: YIELD item1 [AS alias1], item2 [AS alias2], ...
fn yield_clause(tokens: &[Token]) -> IResult<&[Token], YieldClause> {
    map(
        tuple((
            expect_token(Token::Yield),
            yield_item,
            many0(tuple((expect_token(Token::Comma), yield_item))),
        )),
        |(_, first_item, additional_items)| {
            let mut items = vec![first_item];
            items.extend(additional_items.into_iter().map(|(_, item)| item));
            YieldClause {
                items,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse YIELD item: column_name [AS alias]
fn yield_item(tokens: &[Token]) -> IResult<&[Token], YieldItem> {
    map(
        tuple((
            identifier,
            opt(tuple((expect_token(Token::As), identifier))),
        )),
        |(column_name, alias_opt)| YieldItem {
            column_name,
            alias: alias_opt.map(|(_, alias)| alias),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse MATCH clause
fn match_clause(tokens: &[Token]) -> IResult<&[Token], MatchClause> {
    map(
        tuple((
            expect_token(Token::Match),
            path_pattern,
            many0(tuple((expect_token(Token::Comma), path_pattern))),
        )),
        |(_, first_pattern, additional_patterns)| {
            let mut patterns = vec![first_pattern];
            patterns.extend(additional_patterns.into_iter().map(|(_, pattern)| pattern));
            MatchClause {
                patterns,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse path pattern: [identifier =] [path_type] node (edge node)*
fn path_pattern(tokens: &[Token]) -> IResult<&[Token], PathPattern> {
    map(
        tuple((
            opt(tuple((identifier, expect_token(Token::Equal)))),
            opt(path_type_keywords),
            many1(pattern_element),
        )),
        |(assignment, path_type, elements)| PathPattern {
            assignment: assignment.map(|(id, _)| id),
            path_type,
            elements,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse path type keywords: WALK | TRAIL | SIMPLE PATH | ACYCLIC PATH
fn path_type_keywords(tokens: &[Token]) -> IResult<&[Token], PathType> {
    alt((
        // ACYCLIC PATH (must come before ACYCLIC alone)
        map(
            tuple((expect_token(Token::Acyclic), expect_token(Token::Path))),
            |_| PathType::AcyclicPath,
        ),
        // SIMPLE PATH (must come before SIMPLE alone)
        map(
            tuple((expect_token(Token::Simple), expect_token(Token::Path))),
            |_| PathType::SimplePath,
        ),
        // TRAIL
        map(expect_token(Token::Trail), |_| PathType::Trail),
        // WALK
        map(expect_token(Token::Walk), |_| PathType::Walk),
    ))(tokens)
}

/// Parse path quantifier: {n,m} | {n,} | {,m} | {n} | ? (per ISO GQL grammar)
fn path_quantifier(tokens: &[Token]) -> IResult<&[Token], PathQuantifier> {
    alt((
        // Optional pattern: ?
        map(expect_token(Token::Question), |_| PathQuantifier::Optional),
        // Braced quantifiers: {n,m} | {n,} | {,m} | {n}
        delimited(
            expect_token(Token::LeftBrace),
            alt((
                // {n,m} - range
                map(
                    tuple((integer_literal, expect_token(Token::Comma), integer_literal)),
                    |(min, _, max)| PathQuantifier::Range {
                        min: min as u32,
                        max: max as u32,
                    },
                ),
                // {n,} - at least
                map(
                    tuple((integer_literal, expect_token(Token::Comma))),
                    |(min, _)| PathQuantifier::AtLeast(min as u32),
                ),
                // {,m} - at most
                map(
                    tuple((expect_token(Token::Comma), integer_literal)),
                    |(_, max)| PathQuantifier::AtMost(max as u32),
                ),
                // {n} - exact
                map(integer_literal, |n| PathQuantifier::Exact(n as u32)),
            )),
            expect_token(Token::RightBrace),
        ),
    ))(tokens)
}

/// Parse pattern element (node or edge)
fn pattern_element(tokens: &[Token]) -> IResult<&[Token], PatternElement> {
    alt((
        map(node_pattern, PatternElement::Node),
        map(edge_pattern, PatternElement::Edge),
    ))(tokens)
}

/// Parse node pattern: (identifier? :label? {properties}?)
fn node_pattern(tokens: &[Token]) -> IResult<&[Token], Node> {
    map(
        tuple((
            expect_token(Token::LeftParen),
            opt(identifier),
            opt(label_list),
            opt(property_map),
            expect_token(Token::RightParen),
        )),
        |(_, identifier, labels, properties, _)| Node {
            identifier,
            labels: labels.unwrap_or_default(),
            label_expression: None, // TODO: Parse label conjunction when grammar support is added
            properties,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse edge pattern: -[:label {properties}]-
fn edge_pattern(tokens: &[Token]) -> IResult<&[Token], Edge> {
    map(
        tuple((
            edge_direction,
            expect_token(Token::LeftBracket),
            // Parse identifier and optional label expression, or just label expression, or nothing
            alt((
                map(
                    tuple((
                        identifier,
                        opt(preceded(expect_token(Token::Colon), label_expression)),
                    )),
                    |(identifier, label_expr)| {
                        (
                            Some(identifier),
                            label_expr.map(label_expression_to_strings),
                        )
                    },
                ),
                map(
                    preceded(expect_token(Token::Colon), label_expression),
                    |label_expr| (None, Some(label_expression_to_strings(label_expr))),
                ),
                // Allow empty patterns like -[]->
                value((None, None), success(())),
            )),
            opt(property_map),
            expect_token(Token::RightBracket),
            opt(path_quantifier), // ISO GQL: quantifier after ] but before final direction
            edge_direction,
        )),
        |(left_dir, _, (identifier, labels), properties, _, quantifier, right_dir)| {
            let direction = match (left_dir, right_dir) {
                (EdgeDirection::Incoming, EdgeDirection::Outgoing) => EdgeDirection::Both,
                (EdgeDirection::Incoming, _) => EdgeDirection::Incoming,
                (_, EdgeDirection::Outgoing) => EdgeDirection::Outgoing,
                _ => EdgeDirection::Undirected,
            };
            Edge {
                identifier,
                labels: labels.unwrap_or_default(),
                label_expression: None, // TODO: Parse label conjunction when grammar support is added
                properties,
                direction,
                quantifier, // ISO GQL: quantifier parsed after ] but before final direction
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse label list: :label (&label)* OR :label (:label)*  (BNF-compliant + backward compatible)
/// Supports ISO GQL delimited identifiers: :`Special-Label`
fn label_list(tokens: &[Token]) -> IResult<&[Token], Vec<String>> {
    map(
        tuple((
            expect_token(Token::Colon),
            identifier_or_quoted,
            many0(tuple((
                alt((
                    expect_token(Token::Ampersand), // BNF syntax: &label
                    expect_token(Token::Colon),     // Legacy syntax: :label
                )),
                identifier_or_quoted,
            ))),
        )),
        |(_, first_label, additional_labels)| {
            let mut labels = vec![first_label];
            labels.extend(additional_labels.into_iter().map(|(_, label)| label));
            labels
        },
    )(tokens)
}

/// Parse property map: {property, ...}
fn property_map(tokens: &[Token]) -> IResult<&[Token], PropertyMap> {
    map(
        delimited(
            expect_token(Token::LeftBrace),
            opt(tuple((
                property_pair,
                many0(tuple((expect_token(Token::Comma), property_pair))),
            ))),
            expect_token(Token::RightBrace),
        ),
        |opt_props| {
            let properties = if let Some((first, rest)) = opt_props {
                let mut props = vec![first];
                props.extend(rest.into_iter().map(|(_, prop)| prop));
                props
            } else {
                vec![]
            };
            PropertyMap {
                properties,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse property pair: key: value
/// Supports regular identifiers, backtick-delimited identifiers, and keywords
fn property_pair(tokens: &[Token]) -> IResult<&[Token], Property> {
    map(
        tuple((identifier_or_quoted, expect_token(Token::Colon), expression)),
        |(key, _, value)| Property {
            key,
            value,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse WHERE clause
fn where_clause(tokens: &[Token]) -> IResult<&[Token], WhereClause> {
    map(
        tuple((expect_token(Token::Where), expression)),
        |(_, condition)| WhereClause {
            condition,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse RETURN clause: RETURN [DISTINCT|ALL] items...
fn return_clause(tokens: &[Token]) -> IResult<&[Token], ReturnClause> {
    map(
        tuple((
            expect_token(Token::Return),
            opt(alt((
                value(DistinctQualifier::Distinct, expect_token(Token::Distinct)),
                value(DistinctQualifier::All, expect_token(Token::All)),
            ))),
            return_item,
            many0(tuple((expect_token(Token::Comma), return_item))),
        )),
        |(_, distinct_opt, first, rest)| {
            let mut items = vec![first];
            items.extend(rest.into_iter().map(|(_, item)| item));
            ReturnClause {
                distinct: distinct_opt.unwrap_or(DistinctQualifier::None),
                items,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse return item: expression [AS alias]
fn return_item(tokens: &[Token]) -> IResult<&[Token], ReturnItem> {
    map(
        tuple((
            expression,
            opt(tuple((expect_token(Token::As), identifier))),
        )),
        |(expression, opt_alias)| ReturnItem {
            expression,
            alias: opt_alias.map(|(_, alias)| alias),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse WITH clause: WITH [DISTINCT|ALL] expr [AS alias] [, expr [AS alias]]* [WHERE condition] [ORDER BY ...] [LIMIT ...]
fn with_clause(tokens: &[Token]) -> IResult<&[Token], WithClause> {
    map(
        tuple((
            expect_token(Token::With),
            opt(alt((
                value(DistinctQualifier::Distinct, expect_token(Token::Distinct)),
                value(DistinctQualifier::All, expect_token(Token::All)),
            ))),
            with_item,
            many0(tuple((expect_token(Token::Comma), with_item))),
            opt(where_clause),
            opt(order_clause),
            opt(limit_clause),
        )),
        |(_, distinct_opt, first, rest, where_clause, order_clause, limit_clause)| {
            let mut items = vec![first];
            items.extend(rest.into_iter().map(|(_, item)| item));
            WithClause {
                distinct: distinct_opt.unwrap_or(DistinctQualifier::None),
                items,
                where_clause,
                order_clause,
                limit_clause,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse WITH item: expression [AS alias]
fn with_item(tokens: &[Token]) -> IResult<&[Token], WithItem> {
    map(
        tuple((
            expression,
            opt(tuple((expect_token(Token::As), identifier))),
        )),
        |(expression, opt_alias)| WithItem {
            expression,
            alias: opt_alias.map(|(_, alias)| alias),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse ORDER BY clause: ORDER BY expr [ASC|DESC] [, expr [ASC|DESC]]*
fn order_clause(tokens: &[Token]) -> IResult<&[Token], OrderClause> {
    map(
        tuple((
            expect_token(Token::Order),
            expect_token(Token::By),
            order_item,
            many0(tuple((expect_token(Token::Comma), order_item))),
        )),
        |(_, _, first_item, additional_items)| {
            let mut items = vec![first_item];
            items.extend(additional_items.into_iter().map(|(_, item)| item));
            OrderClause {
                items,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse ORDER BY item: expression [ASC|ASCENDING|DESC|DESCENDING] [NULLS FIRST|LAST]
fn order_item(tokens: &[Token]) -> IResult<&[Token], OrderItem> {
    map(
        tuple((
            expression,
            opt(alt((
                value(OrderDirection::Ascending, expect_token(Token::Asc)),
                value(OrderDirection::Ascending, expect_token(Token::Ascending)),
                value(OrderDirection::Descending, expect_token(Token::Desc)),
                value(OrderDirection::Descending, expect_token(Token::Descending)),
            ))),
            opt(tuple((
                expect_token(Token::Nulls),
                alt((
                    value(NullsOrdering::First, expect_token(Token::First)),
                    value(NullsOrdering::Last, expect_token(Token::Last)),
                )),
            ))),
        )),
        |(expression, direction, nulls_clause)| OrderItem {
            expression,
            direction: direction.unwrap_or(OrderDirection::Ascending), // Default to ASC
            nulls_ordering: nulls_clause.map(|(_, nulls_order)| nulls_order),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse GROUP BY clause: GROUP BY expression, expression, ...
fn group_clause(tokens: &[Token]) -> IResult<&[Token], GroupClause> {
    map(
        tuple((
            expect_token(Token::Group),
            expect_token(Token::By),
            expression,
            many0(tuple((expect_token(Token::Comma), expression))),
        )),
        |(_, _, first_expr, additional_exprs)| {
            let mut expressions = vec![first_expr];
            expressions.extend(additional_exprs.into_iter().map(|(_, expr)| expr));
            GroupClause {
                expressions,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse HAVING clause: HAVING expression
fn having_clause(tokens: &[Token]) -> IResult<&[Token], HavingClause> {
    map(
        tuple((expect_token(Token::Having), expression)),
        |(_, condition)| HavingClause {
            condition,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse LIMIT clause: LIMIT count [OFFSET offset]
fn limit_clause(tokens: &[Token]) -> IResult<&[Token], LimitClause> {
    map(
        tuple((
            expect_token(Token::Limit),
            integer_literal,
            opt(tuple((expect_token(Token::Offset), integer_literal))),
        )),
        |(_, count, opt_offset)| LimitClause {
            count: count as usize,
            offset: opt_offset.map(|(_, offset)| offset as usize),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse expression with operator precedence
fn expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    or_expression(tokens)
}

/// Parse OR expressions: xor_expr OR xor_expr
fn or_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((
            xor_expression,
            many0(tuple((expect_token(Token::Or), xor_expression))),
        )),
        |(first, rest)| {
            rest.into_iter().fold(first, |left, (_, right)| {
                Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator: Operator::Or,
                    right: Box::new(right),
                    location: Location::default(),
                })
            })
        },
    )(tokens)
}

/// Parse XOR expressions: and_expr XOR and_expr
fn xor_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((
            and_expression,
            many0(tuple((expect_token(Token::Xor), and_expression))),
        )),
        |(first, rest)| {
            rest.into_iter().fold(first, |left, (_, right)| {
                Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator: Operator::Xor,
                    right: Box::new(right),
                    location: Location::default(),
                })
            })
        },
    )(tokens)
}

/// Parse AND expressions: not_expr AND not_expr
fn and_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((
            not_expression,
            many0(tuple((expect_token(Token::And), not_expression))),
        )),
        |(first, rest)| {
            rest.into_iter().fold(first, |left, (_, right)| {
                Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator: Operator::And,
                    right: Box::new(right),
                    location: Location::default(),
                })
            })
        },
    )(tokens)
}

/// Parse NOT expressions: [NOT] comparison
fn not_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    alt((
        map(
            tuple((expect_token(Token::Not), comparison)),
            |(_, expr)| {
                Expression::Unary(UnaryExpression {
                    operator: Operator::Not,
                    expression: Box::new(expr),
                    location: Location::default(),
                })
            },
        ),
        comparison,
    ))(tokens)
}

/// Parse shorthand label predicate: identifier:label (e.g., n:Person)
fn shorthand_label_predicate(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((identifier, expect_token(Token::Colon), identifier)),
        |(var_name, _, label_name)| {
            let label_expr = LabelExpression {
                terms: vec![LabelTerm {
                    factors: vec![LabelFactor::Identifier(label_name)],
                    location: Location::default(),
                }],
                location: Location::default(),
            };

            Expression::IsPredicate(IsPredicateExpression {
                subject: Box::new(Expression::Variable(Variable {
                    name: var_name,
                    location: Location::default(),
                })),
                predicate_type: IsPredicateType::Label(label_expr),
                negated: false,
                target: None,
                type_spec: None,
                location: Location::default(),
            })
        },
    )(tokens)
}

/// Parse comparison: expr op expr | expr IN (subquery) | expr IN (list) | expr NOT IN (subquery) | expr NOT IN (list) | expr IS [NOT] predicate
fn comparison(tokens: &[Token]) -> IResult<&[Token], Expression> {
    alt((
        // Handle shorthand label predicates: variable:label
        shorthand_label_predicate,
        // Handle IS predicates
        is_predicate,
        // Handle quantified comparisons: expr op ALL/ANY/SOME (subquery)
        map(
            tuple((
                additive_expression,
                comparison_operator,
                quantifier,
                expect_token(Token::LeftParen),
                basic_query,
                expect_token(Token::RightParen),
            )),
            |(left, op, quant, _, query, _)| {
                Expression::QuantifiedComparison(QuantifiedComparisonExpression {
                    left: Box::new(left),
                    operator: op,
                    quantifier: quant,
                    subquery: Box::new(Expression::Subquery(SubqueryExpression {
                        query: Box::new(query),
                        location: Location::default(),
                    })),
                    location: Location::default(),
                })
            },
        ),
        // Handle NOT IN subquery specially
        map(
            tuple((
                additive_expression,
                expect_token(Token::Not),
                expect_token(Token::In),
                alt((
                    // Try subquery first: NOT IN (SELECT ...)
                    map(
                        tuple((
                            expect_token(Token::LeftParen),
                            basic_query,
                            expect_token(Token::RightParen),
                        )),
                        |(_, query, _)| {
                            Expression::NotInSubquery(NotInSubqueryExpression {
                                expression: Box::new(Expression::Literal(Literal::String(
                                    "placeholder".to_string(),
                                ))), // Will be replaced
                                query: Box::new(query),
                                location: Location::default(),
                            })
                        },
                    ),
                    // Try parenthesized list: NOT IN ('a', 'b', 'c')
                    map(
                        tuple((
                            expect_token(Token::LeftParen),
                            expression_list,
                            expect_token(Token::RightParen),
                        )),
                        |(_, exprs, _)| {
                            Expression::Literal(Literal::List(
                                exprs
                                    .into_iter()
                                    .map(|e| {
                                        // Convert expressions to literals if possible
                                        match e {
                                            Expression::Literal(lit) => lit,
                                            _ => Literal::String(format!("{:?}", e)), // Fallback
                                        }
                                    })
                                    .collect(),
                            ))
                        },
                    ),
                    // Fallback: variable or array expression
                    additive_expression,
                )),
            )),
            |(left, _, _, right)| match right {
                Expression::NotInSubquery(mut not_in_subquery) => {
                    not_in_subquery.expression = Box::new(left);
                    Expression::NotInSubquery(not_in_subquery)
                }
                _ => Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator: Operator::NotIn,
                    right: Box::new(right),
                    location: Location::default(),
                }),
            },
        ),
        // Handle IN subquery specially
        map(
            tuple((
                additive_expression,
                expect_token(Token::In),
                alt((
                    // Try subquery first: IN (SELECT ...)
                    map(
                        tuple((
                            expect_token(Token::LeftParen),
                            basic_query,
                            expect_token(Token::RightParen),
                        )),
                        |(_, query, _)| {
                            Expression::InSubquery(InSubqueryExpression {
                                expression: Box::new(Expression::Literal(Literal::String(
                                    "placeholder".to_string(),
                                ))), // Will be replaced
                                query: Box::new(query),
                                location: Location::default(),
                            })
                        },
                    ),
                    // Try parenthesized list: IN ('a', 'b', 'c')
                    map(
                        tuple((
                            expect_token(Token::LeftParen),
                            expression_list,
                            expect_token(Token::RightParen),
                        )),
                        |(_, exprs, _)| {
                            Expression::Literal(Literal::List(
                                exprs
                                    .into_iter()
                                    .map(|e| {
                                        // Convert expressions to literals if possible
                                        match e {
                                            Expression::Literal(lit) => lit,
                                            _ => Literal::String(format!("{:?}", e)), // Fallback
                                        }
                                    })
                                    .collect(),
                            ))
                        },
                    ),
                    // Fallback: variable or array expression
                    additive_expression,
                )),
            )),
            |(left, _, right)| match right {
                Expression::InSubquery(mut in_subquery) => {
                    in_subquery.expression = Box::new(left);
                    Expression::InSubquery(in_subquery)
                }
                _ => Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator: Operator::In,
                    right: Box::new(right),
                    location: Location::default(),
                }),
            },
        ),
        // Regular comparison operations
        map(
            tuple((
                additive_expression,
                opt(tuple((comparison_operator, additive_expression))),
            )),
            |(left, opt_right)| {
                if let Some((op, right)) = opt_right {
                    Expression::Binary(BinaryExpression {
                        left: Box::new(left),
                        operator: op,
                        right: Box::new(right),
                        location: Location::default(),
                    })
                } else {
                    left
                }
            },
        ),
    ))(tokens)
}

/// Parse additive expressions: mult_expr (+|-|*|||) mult_expr
fn additive_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((
            multiplicative_expression,
            many0(tuple((
                alt((
                    expect_token(Token::Plus),
                    expect_token(Token::Minus),
                    expect_token(Token::Dash), // Accept Dash as arithmetic minus
                    expect_token(Token::Concat), // Concatenation operator
                )),
                multiplicative_expression,
            ))),
        )),
        |(first, rest)| {
            rest.into_iter().fold(first, |left, (op, right)| {
                let operator = match op {
                    Token::Plus => Operator::Plus,
                    Token::Minus => Operator::Minus,
                    Token::Dash => Operator::Minus, // Treat Dash as arithmetic minus
                    Token::Concat => Operator::Concat, // Concatenation
                    _ => Operator::Plus,            // Should not happen
                };
                Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                    location: Location::default(),
                })
            })
        },
    )(tokens)
}

/// Parse multiplicative expressions: postfix (*|/) postfix
fn multiplicative_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((
            postfix_expression,
            many0(tuple((
                alt((
                    expect_token(Token::Star),
                    expect_token(Token::Slash),
                    expect_token(Token::Percent),
                )),
                postfix_expression,
            ))),
        )),
        |(first, rest)| {
            rest.into_iter().fold(first, |left, (op, right)| {
                let operator = match op {
                    Token::Star => Operator::Star,
                    Token::Slash => Operator::Slash,
                    Token::Percent => Operator::Percent,
                    _ => Operator::Star, // Should not happen
                };
                Expression::Binary(BinaryExpression {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                    location: Location::default(),
                })
            })
        },
    )(tokens)
}

/// Parse array expression: [expr1, expr2, ...]
/// Returns a Literal::Vector if all elements are numeric, otherwise fails
/// This allows arrays to be used as arguments to functions that accept vector inputs
fn array_expression(tokens: &[Token]) -> IResult<&[Token], Literal> {
    // Parse [expr1, expr2, ...]
    let (tokens, _) = expect_token(Token::LeftBracket)(tokens)?;

    // Check for empty array
    if matches!(tokens.first(), Some(Token::RightBracket)) {
        let (tokens, _) = expect_token(Token::RightBracket)(tokens)?;
        return Ok((tokens, Literal::Vector(vec![])));
    }

    // Parse expressions
    let (tokens, expressions) = expression_list(tokens)?;
    let (tokens, _) = expect_token(Token::RightBracket)(tokens)?;

    // For now, we'll convert all expressions to a simple numeric vector
    // In the future, we might want to support more complex array types
    // For AI functions, we typically expect numeric arrays
    // Removed unused values vector since we now use literals directly

    // Try to convert to a Literal list
    let mut literals = Vec::new();
    let mut all_numeric = true;
    let mut numeric_values = Vec::new();

    for expr in expressions {
        // Try to extract a literal value from the expression
        match expr {
            Expression::Literal(literal) => match &literal {
                Literal::Integer(n) => {
                    numeric_values.push(*n as f64);
                    literals.push(literal);
                }
                Literal::Float(f) => {
                    numeric_values.push(*f);
                    literals.push(literal);
                }
                _ => {
                    all_numeric = false;
                    literals.push(literal);
                }
            },
            _ => {
                // For complex expressions, we can't evaluate them at parse time
                // This is problematic, so we'll return an error for now
                return Err(nom::Err::Error(nom::error::Error::new(
                    tokens,
                    nom::error::ErrorKind::Tag,
                )));
            }
        }
    }

    // If all elements are numeric, return a Vector for backwards compatibility
    // Otherwise return a generic List
    if all_numeric
        && !literals.is_empty()
        && literals
            .iter()
            .all(|lit| matches!(lit, Literal::Integer(_) | Literal::Float(_)))
    {
        Ok((tokens, Literal::Vector(numeric_values)))
    } else {
        Ok((tokens, Literal::List(literals)))
    }
}

/// Parse PATH constructor: PATH[expr1, expr2, ...] or PATH + vector
fn path_constructor(tokens: &[Token]) -> IResult<&[Token], PathConstructor> {
    let (tokens, _) = expect_token(Token::Path)(tokens)?;

    // Handle case where lexer parsed brackets with numbers as a vector literal
    if let Some(Token::Vector(values)) = tokens.first() {
        let (tokens, _) = expect_token_variant(&|t| matches!(t, Token::Vector(_)))(tokens)?;

        // Convert vector values to literal expressions
        let elements = values
            .iter()
            .map(|&v| Expression::Literal(crate::ast::Literal::Float(v)))
            .collect();

        return Ok((
            tokens,
            PathConstructor {
                elements,
                location: Location::default(),
            },
        ));
    }

    let (tokens, _) = expect_token(Token::LeftBracket)(tokens)?;

    // Check for empty PATH[]
    if matches!(tokens.first(), Some(Token::RightBracket)) {
        let (tokens, _) = expect_token(Token::RightBracket)(tokens)?;
        return Ok((
            tokens,
            PathConstructor {
                elements: vec![],
                location: Location::default(),
            },
        ));
    }

    // Parse expressions
    let (tokens, elements) = expression_list(tokens)?;
    let (tokens, _) = expect_token(Token::RightBracket)(tokens)?;

    Ok((
        tokens,
        PathConstructor {
            elements,
            location: Location::default(),
        },
    ))
}

/// Parse postfix expressions: primary_expr[index] | primary_expr
fn postfix_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    let (mut remaining, mut expr) = primary_expression(tokens)?;

    // Check for array indexing: [expression]
    while let Ok((tokens_after_bracket, _)) = expect_token(Token::LeftBracket)(remaining) {
        // Parse the index expression
        let (tokens_after_index, index_expr) = expression(tokens_after_bracket)?;
        let (tokens_after_close, _) = expect_token(Token::RightBracket)(tokens_after_index)?;

        // Create an ArrayIndexExpression
        expr = Expression::ArrayIndex(ArrayIndexExpression {
            array: Box::new(expr),
            index: Box::new(index_expr),
            location: Location::default(),
        });

        remaining = tokens_after_close;
    }

    Ok((remaining, expr))
}

/// Parse primary expression: (expr) | function_call | property_access | value | case_expr
fn primary_expression(tokens: &[Token]) -> IResult<&[Token], Expression> {
    alt((
        // NOT EXISTS subquery must be checked before EXISTS to avoid conflicts
        map(not_exists_subquery, Expression::NotExistsSubquery),
        // EXISTS subquery must be checked before general parenthesized expressions
        map(exists_subquery, Expression::ExistsSubquery),
        // Try pattern expression first (for WHERE clause patterns)
        map(pattern_expression, Expression::Pattern),
        // Try subquery first, then fall back to parenthesized expression
        map(
            alt((
                map(subquery_expression, Expression::Subquery),
                map(
                    tuple((
                        expect_token(Token::LeftParen),
                        expression,
                        expect_token(Token::RightParen),
                    )),
                    |(_, expr, _)| expr,
                ),
            )),
            |expr| expr,
        ),
        map(case_expression, Expression::Case),
        map(cast_expression, Expression::Cast),
        map(path_constructor, Expression::PathConstructor),
        map(array_expression, Expression::Literal),
        map(trim_function_call, Expression::FunctionCall), // Special TRIM FROM syntax
        map(function_call, Expression::FunctionCall),
        map(property_access, Expression::PropertyAccess),
        map(property_access_continued, Expression::PropertyAccess),
        map(property_access_token, Expression::PropertyAccess),
        map(parameter, Expression::Parameter),
        map(variable, Expression::Variable),
        map(literal, Expression::Literal),
    ))(tokens)
}

/// Parse expression list: expr1, expr2, expr3, ...
fn expression_list(tokens: &[Token]) -> IResult<&[Token], Vec<Expression>> {
    map(
        tuple((
            expression,
            many0(tuple((expect_token(Token::Comma), expression))),
        )),
        |(first_expr, additional_exprs)| {
            let mut expressions = vec![first_expr];
            expressions.extend(additional_exprs.into_iter().map(|(_, expr)| expr));
            expressions
        },
    )(tokens)
}

/// Parse function call: name(args...) using ISO GQL compliant token-based parsing
fn function_call(tokens: &[Token]) -> IResult<&[Token], FunctionCall> {
    // ISO GQL: <function-call> ::= <identifier> "(" [DISTINCT|ALL] [<expression> ("," <expression>)*] ")"

    // Parse function name (identifier)
    let (tokens, name) = identifier(tokens)?;
    let name = name.to_uppercase(); // Normalize function names to uppercase

    // Parse opening parenthesis
    let (tokens, _) = expect_token(Token::LeftParen)(tokens)?;

    // Parse optional DISTINCT/ALL qualifier for aggregate functions
    let (tokens, distinct_qualifier) = opt(distinct_qualifier)(tokens)?;
    let distinct = distinct_qualifier.unwrap_or(DistinctQualifier::None);

    // Parse arguments (expressions separated by commas)
    let mut arguments = Vec::new();
    let mut remaining = tokens;

    // Check if there are arguments (not immediate closing paren)
    if !matches!(remaining.first(), Some(Token::RightParen)) {
        // Special case for COUNT(*) and similar aggregate functions with *
        if matches!(remaining.first(), Some(Token::Star)) {
            // Create a wildcard variable expression for *
            arguments.push(Expression::Variable(Variable {
                name: "*".to_string(),
                location: Location::default(),
            }));
            remaining = &remaining[1..]; // consume the star
        } else {
            // Regular argument parsing loop
            loop {
                // Parse an expression as argument
                let (new_remaining, expr) = expression(remaining)?;
                arguments.push(expr);
                remaining = new_remaining;

                // Check for comma (more arguments) or closing paren (end)
                match remaining.first() {
                    Some(Token::Comma) => {
                        remaining = &remaining[1..]; // consume comma
                        continue;
                    }
                    Some(Token::RightParen) => break,
                    _ => {
                        return Err(nom::Err::Error(nom::error::Error::new(
                            remaining,
                            nom::error::ErrorKind::Tag,
                        )))
                    }
                }
            }
        }
    }

    // Parse closing parenthesis
    let (remaining, _) = expect_token(Token::RightParen)(remaining)?;

    Ok((
        remaining,
        FunctionCall {
            name,
            distinct,
            arguments,
            location: Location::default(),
        },
    ))
}

/// Parse special TRIM function with ISO GQL FROM clause syntax
/// TRIM "(" [("LEADING" | "TRAILING" | "BOTH") [<string-expr>] "FROM"] <string-expr> ")"
fn trim_function_call(tokens: &[Token]) -> IResult<&[Token], FunctionCall> {
    // Check if this is a TRIM function call
    if !matches!(tokens.first(), Some(Token::Identifier(name)) if name.eq_ignore_ascii_case("TRIM"))
    {
        return Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )));
    }

    let (tokens, _name) = identifier(tokens)?; // Consume TRIM
    let (tokens, _) = expect_token(Token::LeftParen)(tokens)?; // Consume (

    let mut arguments = Vec::new();
    let mut remaining = tokens;

    // Check for trim mode keywords: LEADING, TRAILING, BOTH
    let trim_mode = match remaining.first() {
        Some(Token::Leading) => {
            remaining = &remaining[1..];
            Some("LEADING")
        }
        Some(Token::Trailing) => {
            remaining = &remaining[1..];
            Some("TRAILING")
        }
        Some(Token::Both) => {
            remaining = &remaining[1..];
            Some("BOTH")
        }
        _ => None,
    };

    // If we have a trim mode, check for optional trim character and FROM keyword
    if let Some(mode) = trim_mode {
        // Add mode as first argument
        arguments.push(Expression::Literal(Literal::String(mode.to_string())));

        // Check if next token is FROM (no trim character specified) or an expression (trim character)
        if matches!(remaining.first(), Some(Token::From)) {
            // TRIM(MODE FROM string) - no trim character, use default whitespace
            remaining = &remaining[1..]; // consume FROM
            arguments.push(Expression::Literal(Literal::String(" ".to_string())));
        // default trim char
        } else {
            // TRIM(MODE char_expr FROM string) - parse trim character
            let (new_remaining, trim_char_expr) = expression(remaining)?;
            remaining = new_remaining;
            arguments.push(trim_char_expr); // trim character

            // Expect FROM keyword
            let (new_remaining, _) = expect_token(Token::From)(remaining)?;
            remaining = new_remaining;
        }

        // Parse the string expression to trim
        let (new_remaining, string_expr) = expression(remaining)?;
        remaining = new_remaining;
        arguments.push(string_expr);
    } else {
        // Check if this is TRIM(FROM string) - equivalent to TRIM(BOTH FROM string)
        if matches!(remaining.first(), Some(Token::From)) {
            remaining = &remaining[1..]; // consume FROM
            arguments.push(Expression::Literal(Literal::String("BOTH".to_string()))); // mode
            arguments.push(Expression::Literal(Literal::String(" ".to_string()))); // default trim char

            // Parse the string expression to trim
            let (new_remaining, string_expr) = expression(remaining)?;
            remaining = new_remaining;
            arguments.push(string_expr);
        } else {
            // This is regular TRIM(string) or TRIM(char, string) - let regular function_call handle it
            return Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )));
        }
    }

    // Parse closing parenthesis
    let (remaining, _) = expect_token(Token::RightParen)(remaining)?;

    Ok((
        remaining,
        FunctionCall {
            name: "TRIM".to_string(),
            distinct: DistinctQualifier::None,
            arguments,
            location: Location::default(),
        },
    ))
}

/// Parse property access: object.property
fn property_access(tokens: &[Token]) -> IResult<&[Token], PropertyAccess> {
    map(
        tuple((
            identifier,
            expect_token(Token::Dot),
            identifier,
            many0(tuple((expect_token(Token::Dot), identifier))),
        )),
        |(object, _, first_property, additional_properties)| {
            // Build the full property path: object.property1.property2...
            let mut property_path = first_property;
            for (_, prop) in additional_properties {
                property_path = format!("{}.{}", property_path, prop);
            }
            PropertyAccess {
                object,
                property: property_path,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse property access that starts with a PropertyAccess token: PropertyAccess.Dot.Identifier...
fn property_access_continued(tokens: &[Token]) -> IResult<&[Token], PropertyAccess> {
    map(
        tuple((
            property_access_token,
            many0(tuple((expect_token(Token::Dot), identifier))),
        )),
        |(base_access, additional_properties)| {
            // Build the full property path by extending the base property access
            let mut property_path = base_access.property;
            for (_, prop) in additional_properties {
                property_path = format!("{}.{}", property_path, prop);
            }
            PropertyAccess {
                object: base_access.object,
                property: property_path,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse variable reference: identifier (bound in MATCH/LET clauses)
fn variable(tokens: &[Token]) -> IResult<&[Token], Variable> {
    map(identifier, |name| Variable {
        name,
        location: Location::default(),
    })(tokens)
}

/// Parse property access token: object.property
fn property_access_token(tokens: &[Token]) -> IResult<&[Token], PropertyAccess> {
    if let Some(Token::PropertyAccess(s)) = tokens.first() {
        // Parse the property access string "object.property"
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 2 {
            Ok((
                &tokens[1..],
                PropertyAccess {
                    object: parts[0].to_string(),
                    property: parts[1].to_string(),
                    location: Location::default(),
                },
            ))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )))
        }
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse identifier (does NOT accept string literals - use identifier_or_quoted for that)
fn identifier(tokens: &[Token]) -> IResult<&[Token], String> {
    if let Some(token) = tokens.first() {
        let identifier_str = match token {
            Token::Identifier(s) => Some(s.clone()),
            // NOTE: String tokens should NOT be accepted here as it causes string literals
            // to be incorrectly parsed as variables in expressions
            // Allow certain keywords to be used as identifiers in contexts like aliases
            Token::Value => Some("value".to_string()),
            Token::Type => Some("type".to_string()),
            Token::User => Some("user".to_string()),
            Token::Role => Some("role".to_string()),
            Token::Schema => Some("schema".to_string()),
            Token::Data => Some("data".to_string()),
            Token::Graph => Some("graph".to_string()),
            Token::Node => Some("node".to_string()),
            Token::Edge => Some("edge".to_string()),
            Token::Path => Some("path".to_string()),
            Token::Table => Some("table".to_string()),
            Token::Property => Some("property".to_string()),
            Token::Source => Some("source".to_string()),
            Token::Destination => Some("destination".to_string()),
            Token::Zone => Some("zone".to_string()),
            Token::Time => Some("time".to_string()),
            Token::Parameter => Some("parameter".to_string()),
            Token::Order => Some("order".to_string()),
            Token::Contains => Some("contains".to_string()),
            Token::Next => Some("next".to_string()), // Allow NEXT as edge label
            Token::Start => Some("start".to_string()), // Allow START as identifier
            Token::End => Some("end".to_string()),   // Allow END as identifier
            Token::Register => Some("register".to_string()), // Allow REGISTER as identifier
            Token::Unregister => Some("unregister".to_string()), // Allow UNREGISTER as identifier
            Token::Description => Some("description".to_string()), // Allow DESCRIPTION in YIELD clauses
            _ => None,
        };

        if let Some(s) = identifier_str {
            Ok((&tokens[1..], s))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )))
        }
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse edge direction
fn edge_direction(tokens: &[Token]) -> IResult<&[Token], EdgeDirection> {
    alt((
        value(EdgeDirection::Both, expect_token(Token::ArrowBoth)),
        value(EdgeDirection::Outgoing, expect_token(Token::Arrow)),
        value(EdgeDirection::Incoming, expect_token(Token::ArrowLeft)),
        value(EdgeDirection::Undirected, expect_token(Token::Dash)),
    ))(tokens)
}

/// Parse comparison operator
fn comparison_operator(tokens: &[Token]) -> IResult<&[Token], Operator> {
    alt((
        value(Operator::Equal, expect_token(Token::Equal)),
        value(Operator::NotEqual, expect_token(Token::NotEqual)),
        value(Operator::LessThan, expect_token(Token::LessThan)),
        value(Operator::LessEqual, expect_token(Token::LessEqual)),
        value(Operator::GreaterThan, expect_token(Token::GreaterThan)),
        value(Operator::GreaterEqual, expect_token(Token::GreaterEqual)),
        value(Operator::Within, expect_token(Token::Within)),
        value(Operator::Like, expect_token(Token::Like)),
        value(Operator::Contains, expect_token(Token::Contains)),
        // Handle compound operators
        value(
            Operator::Starts,
            tuple((expect_token(Token::Starts), expect_token(Token::With))),
        ),
        value(
            Operator::Ends,
            tuple((expect_token(Token::Ends), expect_token(Token::With))),
        ),
    ))(tokens)
}

/// Parse quantifier
fn quantifier(tokens: &[Token]) -> IResult<&[Token], Quantifier> {
    alt((
        value(Quantifier::All, expect_token(Token::All)),
        value(Quantifier::Any, expect_token(Token::Any)),
        value(Quantifier::Some, expect_token(Token::Some)),
    ))(tokens)
}

/// Parse literals
fn literal(tokens: &[Token]) -> IResult<&[Token], Literal> {
    alt((
        map(string_literal, Literal::String),
        map(integer_literal, Literal::Integer),
        map(float_literal, Literal::Float),
        map(boolean_literal, Literal::Boolean),
        map(null_literal, |_| Literal::Null),
        map(vector_literal, Literal::Vector),
    ))(tokens)
}

/// Parse identifier or quoted identifier (ISO GQL compliant)
/// Accepts regular identifiers, backtick-delimited identifiers, and keywords as identifiers
/// Examples: myId, `My-Id`, `Property Name`, type, value
fn identifier_or_quoted(tokens: &[Token]) -> IResult<&[Token], String> {
    if let Some(token) = tokens.first() {
        let identifier_str = match token {
            Token::Identifier(s) => Some(s.clone()),
            Token::BacktickString(s) => Some(s.clone()), // ISO GQL delimited identifier: `identifier`
            Token::String(s) => Some(s.clone()), // Allow string literals as fallback (for backwards compatibility)
            // Allow certain keywords to be used as identifiers in contexts like aliases
            Token::Value => Some("value".to_string()),
            Token::Type => Some("type".to_string()),
            Token::User => Some("user".to_string()),
            Token::Role => Some("role".to_string()),
            Token::Schema => Some("schema".to_string()),
            Token::Data => Some("data".to_string()),
            Token::Graph => Some("graph".to_string()),
            Token::Node => Some("node".to_string()),
            Token::Edge => Some("edge".to_string()),
            Token::Path => Some("path".to_string()),
            Token::Table => Some("table".to_string()),
            Token::Property => Some("property".to_string()),
            Token::Source => Some("source".to_string()),
            Token::Destination => Some("destination".to_string()),
            Token::Zone => Some("zone".to_string()),
            Token::Time => Some("time".to_string()),
            Token::Parameter => Some("parameter".to_string()),
            Token::Order => Some("order".to_string()),
            Token::Contains => Some("contains".to_string()),
            Token::Next => Some("next".to_string()),
            Token::Start => Some("start".to_string()),
            Token::End => Some("end".to_string()),
            _ => None,
        };

        if let Some(s) = identifier_str {
            // Reject empty identifiers
            if s.is_empty() {
                return Err(nom::Err::Error(nom::error::Error::new(
                    tokens,
                    nom::error::ErrorKind::Verify,
                )));
            }
            Ok((&tokens[1..], s))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )))
        }
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse string literal
fn string_literal(tokens: &[Token]) -> IResult<&[Token], String> {
    if let Some(Token::String(s)) = tokens.first() {
        Ok((&tokens[1..], s.clone()))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse integer literal
fn integer_literal(tokens: &[Token]) -> IResult<&[Token], i64> {
    if let Some(Token::Integer(n)) = tokens.first() {
        Ok((&tokens[1..], *n))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse float literal
fn float_literal(tokens: &[Token]) -> IResult<&[Token], f64> {
    if let Some(Token::Float(f)) = tokens.first() {
        Ok((&tokens[1..], *f))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse boolean literal
fn boolean_literal(tokens: &[Token]) -> IResult<&[Token], bool> {
    if let Some(Token::Boolean(b)) = tokens.first() {
        Ok((&tokens[1..], *b))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse null literal
fn null_literal(tokens: &[Token]) -> IResult<&[Token], ()> {
    value((), expect_token(Token::Null))(tokens)
}

/// Parse vector literal
fn vector_literal(tokens: &[Token]) -> IResult<&[Token], Vec<f64>> {
    if let Some(Token::Vector(v)) = tokens.first() {
        Ok((&tokens[1..], v.clone()))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse catalog statement (DDL operations)
fn catalog_statement(tokens: &[Token]) -> IResult<&[Token], CatalogStatement> {
    alt((
        map(create_schema_statement, CatalogStatement::CreateSchema),
        map(drop_schema_statement, CatalogStatement::DropSchema),
        // IMPORTANT: Parse CREATE/DROP/ALTER GRAPH TYPE before CREATE/DROP GRAPH
        // because "CREATE GRAPH" would match "CREATE GRAPH TYPE" prematurely
        map(
            create_graph_type_statement,
            CatalogStatement::CreateGraphType,
        ),
        map(drop_graph_type_statement, CatalogStatement::DropGraphType),
        map(alter_graph_type_statement, CatalogStatement::AlterGraphType),
        map(create_graph_statement, CatalogStatement::CreateGraph),
        map(drop_graph_statement, CatalogStatement::DropGraph),
        map(truncate_graph_statement, CatalogStatement::TruncateGraph),
        map(clear_graph_statement, CatalogStatement::ClearGraph),
        map(create_user_statement, CatalogStatement::CreateUser),
        map(drop_user_statement, CatalogStatement::DropUser),
        map(create_role_statement, CatalogStatement::CreateRole),
        map(drop_role_statement, CatalogStatement::DropRole),
        map(grant_role_statement, CatalogStatement::GrantRole),
        map(revoke_role_statement, CatalogStatement::RevokeRole),
        map(
            create_procedure_statement,
            CatalogStatement::CreateProcedure,
        ),
        map(drop_procedure_statement, CatalogStatement::DropProcedure),
    ))(tokens)
}

/// Parse and validate schema name (catalog path with validation for schema names)
fn validated_schema_name(tokens: &[Token]) -> IResult<&[Token], CatalogPath> {
    // First parse as a regular catalog path
    let (remaining, schema_path) = catalog_path(tokens)?;

    // Validate schema name is not empty
    if schema_path.segments.is_empty() || schema_path.segments.iter().any(|s| s.trim().is_empty()) {
        // Return a custom error that will be converted to "Invalid schema name"
        return Err(nom::Err::Failure(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Verify,
        )));
    }

    Ok((remaining, schema_path))
}

/// Parse CREATE SCHEMA statement
fn create_schema_statement(tokens: &[Token]) -> IResult<&[Token], CreateSchemaStatement> {
    map(
        tuple((
            expect_token(Token::Create),
            expect_token(Token::Schema),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Not),
                expect_token(Token::Exists),
            ))),
            validated_schema_name,
        )),
        |(_, _, if_not_exists, schema_path)| CreateSchemaStatement {
            schema_path,
            if_not_exists: if_not_exists.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse DROP SCHEMA statement  
fn drop_schema_statement(tokens: &[Token]) -> IResult<&[Token], DropSchemaStatement> {
    map(
        tuple((
            expect_token(Token::Drop),
            expect_token(Token::Schema),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Exists),
            ))),
            catalog_path,
            opt(alt((
                value(true, expect_token(Token::Cascade)),
                value(false, expect_token(Token::Restrict)),
            ))),
        )),
        |(_, _, if_exists, schema_path, cascade)| DropSchemaStatement {
            schema_path,
            if_exists: if_exists.is_some(),
            cascade: cascade.unwrap_or(false),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse CREATE GRAPH statement
fn create_graph_statement(tokens: &[Token]) -> IResult<&[Token], CreateGraphStatement> {
    // Check if next token is Type - if so, fail early to let CREATE GRAPH TYPE parse it
    if !tokens.is_empty() {
        // Skip CREATE [OR REPLACE] [PROPERTY] GRAPH tokens
        let mut skip_count = 1; // CREATE
        if tokens.len() > skip_count && matches!(tokens[skip_count], Token::Or) {
            skip_count += 2; // OR REPLACE
        }
        if tokens.len() > skip_count && matches!(tokens[skip_count], Token::Property) {
            skip_count += 1; // PROPERTY
        }
        skip_count += 1; // GRAPH

        if tokens.len() > skip_count && matches!(tokens[skip_count], Token::Type) {
            // This is CREATE GRAPH TYPE, not CREATE GRAPH
            return Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Alt,
            )));
        }
    }

    map(
        tuple((
            expect_token(Token::Create),
            opt(tuple((
                expect_token(Token::Or),
                expect_token(Token::Replace),
            ))),
            opt(expect_token(Token::Property)),
            expect_token(Token::Graph),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Not),
                expect_token(Token::Exists),
            ))),
            catalog_path,
            opt(graph_type_spec),
            opt(tuple((expect_token(Token::As), query))),
        )),
        |(_, or_replace, _, _, if_not_exists, graph_path, graph_type_spec, as_query)| {
            CreateGraphStatement {
                graph_path,
                graph_type_spec,
                if_not_exists: if_not_exists.is_some(),
                or_replace: or_replace.is_some(),
                as_query: as_query.map(|(_, query)| Box::new(query)),
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse DROP GRAPH statement
fn drop_graph_statement(tokens: &[Token]) -> IResult<&[Token], DropGraphStatement> {
    // Check if next token is Type - if so, fail early to let DROP GRAPH TYPE parse it
    if !tokens.is_empty() {
        // Skip DROP [PROPERTY] GRAPH tokens
        let mut skip_count = 1; // DROP
        if tokens.len() > skip_count && matches!(tokens[skip_count], Token::Property) {
            skip_count += 1; // PROPERTY
        }
        skip_count += 1; // GRAPH

        if tokens.len() > skip_count && matches!(tokens[skip_count], Token::Type) {
            // This is DROP GRAPH TYPE, not DROP GRAPH
            return Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Alt,
            )));
        }
    }

    map(
        tuple((
            expect_token(Token::Drop),
            opt(expect_token(Token::Property)),
            expect_token(Token::Graph),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Exists),
            ))),
            catalog_path,
            opt(expect_token(Token::Cascade)),
        )),
        |(_, _, _, if_exists, graph_path, cascade)| DropGraphStatement {
            graph_path,
            if_exists: if_exists.is_some(),
            cascade: cascade.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse TRUNCATE GRAPH statement
fn truncate_graph_statement(tokens: &[Token]) -> IResult<&[Token], TruncateGraphStatement> {
    map(
        tuple((
            expect_token(Token::Truncate),
            expect_token(Token::Graph),
            catalog_path,
        )),
        |(_, _, graph_path)| TruncateGraphStatement {
            graph_path,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse CLEAR GRAPH statement
fn clear_graph_statement(tokens: &[Token]) -> IResult<&[Token], ClearGraphStatement> {
    map(
        tuple((
            expect_token(Token::Clear),
            expect_token(Token::Graph),
            opt(catalog_path),
        )),
        |(_, _, graph_path)| ClearGraphStatement {
            graph_path,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse CREATE GRAPH TYPE statement
fn create_graph_type_statement(tokens: &[Token]) -> IResult<&[Token], CreateGraphTypeStatement> {
    map(
        tuple((
            expect_token(Token::Create),
            opt(tuple((
                expect_token(Token::Or),
                expect_token(Token::Replace),
            ))),
            opt(expect_token(Token::Property)),
            expect_token(Token::Graph),
            expect_token(Token::Type),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Not),
                expect_token(Token::Exists),
            ))),
            catalog_path,
            opt(tuple((
                expect_token(Token::Copy),
                expect_token(Token::Of),
                catalog_path,
            ))),
            graph_type_spec,
        )),
        |(_, or_replace, _, _, _, if_not_exists, graph_type_path, copy_of, graph_type_spec)| {
            CreateGraphTypeStatement {
                graph_type_path,
                copy_of: copy_of.map(|(_, _, path)| path),
                graph_type_spec,
                if_not_exists: if_not_exists.is_some(),
                or_replace: or_replace.is_some(),
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse DROP GRAPH TYPE statement
fn drop_graph_type_statement(tokens: &[Token]) -> IResult<&[Token], DropGraphTypeStatement> {
    map(
        tuple((
            expect_token(Token::Drop),
            opt(expect_token(Token::Property)),
            expect_token(Token::Graph),
            expect_token(Token::Type),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Exists),
            ))),
            catalog_path,
            opt(alt((
                value(true, expect_token(Token::Cascade)),
                value(false, expect_token(Token::Restrict)),
            ))),
        )),
        |(_, _, _, _, if_exists, graph_type_path, cascade)| DropGraphTypeStatement {
            graph_type_path,
            if_exists: if_exists.is_some(),
            cascade: cascade.unwrap_or(false),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse ALTER GRAPH TYPE statement
fn alter_graph_type_statement(tokens: &[Token]) -> IResult<&[Token], AlterGraphTypeStatement> {
    map(
        tuple((
            expect_token(Token::Alter),
            expect_token(Token::Graph),
            expect_token(Token::Type),
            identifier,
        )),
        |(_, _, _, name)| AlterGraphTypeStatement {
            name,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse catalog path: /segment1/segment2/...
/// Supports ISO GQL delimited identifiers: /`My-Schema`/`My-Graph`
fn catalog_path(tokens: &[Token]) -> IResult<&[Token], CatalogPath> {
    map(
        tuple((
            // Optional leading slash
            opt(expect_token(Token::Slash)),
            // One or more segments separated by '/' - now accepts delimited identifiers
            separated_list1(expect_token(Token::Slash), identifier_or_quoted),
            // Optional trailing slash
            opt(expect_token(Token::Slash)),
        )),
        |(_, segments, _)| CatalogPath::new(segments, Location::default()),
    )(tokens)
}

/// Parse graph type specification: (VERTEX TYPE ... EDGE TYPE ...)
fn graph_type_spec(tokens: &[Token]) -> IResult<&[Token], GraphTypeSpec> {
    map(
        delimited(
            expect_token(Token::LeftParen),
            tuple((opt(vertex_types_clause), opt(edge_types_clause))),
            expect_token(Token::RightParen),
        ),
        |(vertex_types, edge_types)| GraphTypeSpec {
            vertex_types: vertex_types.unwrap_or_default(),
            edge_types: edge_types.unwrap_or_default(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse vertex types clause: VERTEX TYPE vertex_type, vertex_type, ...
fn vertex_types_clause(tokens: &[Token]) -> IResult<&[Token], Vec<VertexTypeSpec>> {
    map(
        tuple((
            alt((expect_token(Token::Vertex), expect_token(Token::Node))),
            alt((expect_token(Token::Type), expect_token(Token::Types))),
            vertex_type_spec,
            many0(tuple((expect_token(Token::Comma), vertex_type_spec))),
        )),
        |(_, _, first_type, additional_types)| {
            let mut types = vec![first_type];
            types.extend(additional_types.into_iter().map(|(_, spec)| spec));
            types
        },
    )(tokens)
}

/// Parse edge types clause: EDGE TYPE edge_type, edge_type, ...
fn edge_types_clause(tokens: &[Token]) -> IResult<&[Token], Vec<EdgeTypeSpec>> {
    map(
        tuple((
            expect_token(Token::Edge),
            alt((expect_token(Token::Type), expect_token(Token::Types))),
            edge_type_spec,
            many0(tuple((expect_token(Token::Comma), edge_type_spec))),
        )),
        |(_, _, first_type, additional_types)| {
            let mut types = vec![first_type];
            types.extend(additional_types.into_iter().map(|(_, spec)| spec));
            types
        },
    )(tokens)
}

/// Parse vertex type specification
fn vertex_type_spec(tokens: &[Token]) -> IResult<&[Token], VertexTypeSpec> {
    map(
        tuple((
            opt(identifier),
            opt(tuple((
                alt((expect_token(Token::Is), expect_token(Token::Colon))),
                label_expression,
            ))),
            opt(property_type_list),
        )),
        |(identifier, labels, properties)| VertexTypeSpec {
            identifier,
            labels: labels.map(|(_, expr)| expr),
            properties,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse edge type specification
fn edge_type_spec(tokens: &[Token]) -> IResult<&[Token], EdgeTypeSpec> {
    map(
        tuple((
            opt(identifier),
            opt(tuple((
                alt((expect_token(Token::Is), expect_token(Token::Colon))),
                label_expression,
            ))),
            opt(property_type_list),
            opt(tuple((expect_token(Token::Source), identifier))),
            opt(tuple((expect_token(Token::Destination), identifier))),
        )),
        |(identifier, labels, properties, source, destination)| EdgeTypeSpec {
            identifier,
            labels: labels.map(|(_, expr)| expr),
            properties,
            source_vertex: source.map(|(_, id)| id),
            destination_vertex: destination.map(|(_, id)| id),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse label expression: term1 | term2 | ...
fn label_expression(tokens: &[Token]) -> IResult<&[Token], LabelExpression> {
    map(
        tuple((
            label_term,
            many0(tuple((expect_token(Token::Pipe), label_term))),
        )),
        |(first_term, additional_terms)| {
            let mut terms = vec![first_term];
            terms.extend(additional_terms.into_iter().map(|(_, term)| term));
            LabelExpression {
                terms,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse label term: factor1 ! factor2 ! ... or :label1:label2:...
fn label_term(tokens: &[Token]) -> IResult<&[Token], LabelTerm> {
    map(
        tuple((
            label_factor,
            many0(alt((
                // Traditional NOT-separated factors
                tuple((expect_token(Token::Not), label_factor)),
                // Consecutive colon-separated labels (for syntax like :Manager:TeamLead or :`Special-Label`)
                map(
                    tuple((expect_token(Token::Colon), identifier_or_quoted)),
                    |(_, name)| (Token::Not, LabelFactor::Identifier(name)), // Dummy token, we only use the factor
                ),
            ))),
        )),
        |(first_factor, additional_factors)| {
            let mut factors = vec![first_factor];
            factors.extend(additional_factors.into_iter().map(|(_, factor)| factor));
            LabelTerm {
                factors,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse label factor: identifier | :identifier | % | (label_expression)
/// Supports ISO GQL delimited identifiers: `My-Label`, :`Special-Label`
fn label_factor(tokens: &[Token]) -> IResult<&[Token], LabelFactor> {
    alt((
        // Colon-prefixed identifier: :LABEL_NAME or :`Special-Label`
        map(
            tuple((expect_token(Token::Colon), identifier_or_quoted)),
            |(_, name)| LabelFactor::Identifier(name),
        ),
        // Regular identifier: LABEL_NAME or `Special-Label`
        map(identifier_or_quoted, LabelFactor::Identifier),
        // Wildcard: %
        value(LabelFactor::Wildcard, expect_token(Token::Percent)),
        // Parenthesized expression: (label_expression)
        map(
            delimited(
                expect_token(Token::LeftParen),
                label_expression,
                expect_token(Token::RightParen),
            ),
            |expr| LabelFactor::Parenthesized(Box::new(expr)),
        ),
    ))(tokens)
}

/// Convert LabelExpression to Vec<String> for backward compatibility
/// This extracts all identifier labels from a label expression like "Transaction|Purchase" -> ["Transaction", "Purchase"]
fn label_expression_to_strings(label_expr: LabelExpression) -> Vec<String> {
    let mut labels = Vec::new();

    for term in label_expr.terms {
        for factor in term.factors {
            match factor {
                LabelFactor::Identifier(name) => labels.push(name),
                LabelFactor::Wildcard => labels.push("%".to_string()),
                LabelFactor::Parenthesized(boxed_expr) => {
                    // Recursively handle parenthesized expressions
                    labels.extend(label_expression_to_strings(*boxed_expr));
                }
            }
        }
    }

    labels
}

/// Parse property type list: (prop1 type1, prop2 type2, ...)
/// ISO GQL uses parentheses, not braces, for property lists in graph types
fn property_type_list(tokens: &[Token]) -> IResult<&[Token], PropertyTypeList> {
    map(
        delimited(
            expect_token(Token::LeftParen),
            tuple((
                property_type_decl,
                many0(tuple((opt(expect_token(Token::Comma)), property_type_decl))),
            )),
            expect_token(Token::RightParen),
        ),
        |(first_prop, additional_props)| {
            let mut properties = vec![first_prop];
            properties.extend(additional_props.into_iter().map(|(_, prop)| prop));
            PropertyTypeList {
                properties,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse property type declaration: name type_spec
fn property_type_decl(tokens: &[Token]) -> IResult<&[Token], PropertyTypeDecl> {
    map(tuple((identifier, type_spec)), |(name, type_spec)| {
        PropertyTypeDecl {
            name,
            type_spec,
            location: Location::default(),
        }
    })(tokens)
}

/// Parse type specification (simplified for now)
fn type_spec(tokens: &[Token]) -> IResult<&[Token], TypeSpec> {
    alt((
        value(TypeSpec::Boolean, expect_token(Token::BooleanType)),
        value(TypeSpec::Integer, expect_token(Token::IntegerType)),
        value(
            TypeSpec::String { max_length: None },
            expect_token(Token::StringType),
        ),
        value(
            TypeSpec::Float { precision: None },
            expect_token(Token::FloatType),
        ),
        value(TypeSpec::Real, expect_token(Token::RealType)),
        value(TypeSpec::Double, expect_token(Token::DoubleType)),
        value(TypeSpec::BigInt, expect_token(Token::BigIntType)),
        value(TypeSpec::SmallInt, expect_token(Token::SmallIntType)),
        value(
            TypeSpec::Decimal {
                precision: None,
                scale: None,
            },
            expect_token(Token::DecimalType),
        ),
        // Handle VECTOR as a special identifier case instead of a dedicated token
        map(
            verify(identifier, |id: &str| id.eq_ignore_ascii_case("VECTOR")),
            |_| TypeSpec::Vector { dimension: None },
        ),
    ))(tokens)
}

/// Parse session statement: SESSION SET/RESET/CLOSE
fn session_statement(tokens: &[Token]) -> IResult<&[Token], SessionStatement> {
    alt((
        map(session_set_statement, SessionStatement::Set),
        map(session_reset_statement, SessionStatement::Reset),
        map(session_close_statement, SessionStatement::Close),
    ))(tokens)
}

/// Parse SESSION SET statement
fn session_set_statement(tokens: &[Token]) -> IResult<&[Token], SessionSetStatement> {
    map(
        tuple((
            expect_token(Token::Session),
            expect_token(Token::Set),
            session_set_clause,
        )),
        |(_, _, clause)| SessionSetStatement {
            clause,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse SESSION SET clauses
fn session_set_clause(tokens: &[Token]) -> IResult<&[Token], SessionSetClause> {
    alt((
        // SESSION SET SCHEMA schema_reference
        map(
            tuple((expect_token(Token::Schema), catalog_path)),
            |(_, schema_reference)| SessionSetClause::Schema { schema_reference },
        ),
        // SESSION SET [PROPERTY] GRAPH graph_expression
        map(
            tuple((
                opt(expect_token(Token::Property)),
                expect_token(Token::Graph),
                graph_expression,
            )),
            |(_, _, graph_expression)| SessionSetClause::Graph { graph_expression },
        ),
        // SESSION SET TIME ZONE time_zone_string
        map(
            tuple((
                expect_token(Token::Time),
                expect_token(Token::Zone),
                string_literal,
            )),
            |(_, _, time_zone)| SessionSetClause::TimeZone { time_zone },
        ),
        // SESSION SET [PROPERTY] GRAPH [IF NOT EXISTS] parameter graph_initializer
        map(
            tuple((
                opt(expect_token(Token::Property)),
                expect_token(Token::Graph),
                opt(tuple((
                    expect_token(Token::If),
                    expect_token(Token::Not),
                    expect_token(Token::Exists),
                ))),
                parameter_name,
                graph_initializer,
            )),
            |(_, _, if_not_exists, parameter, graph_initializer)| {
                SessionSetClause::GraphParameter {
                    parameter,
                    graph_initializer,
                    if_not_exists: if_not_exists.is_some(),
                }
            },
        ),
        // SESSION SET [BINDING] TABLE [IF NOT EXISTS] parameter binding_table_initializer
        map(
            tuple((
                opt(expect_token(Token::Binding)),
                expect_token(Token::Table),
                opt(tuple((
                    expect_token(Token::If),
                    expect_token(Token::Not),
                    expect_token(Token::Exists),
                ))),
                parameter_name,
                binding_table_initializer,
            )),
            |(_, _, if_not_exists, parameter, binding_table_initializer)| {
                SessionSetClause::BindingTableParameter {
                    parameter,
                    binding_table_initializer,
                    if_not_exists: if_not_exists.is_some(),
                }
            },
        ),
        // SESSION SET VALUE [IF NOT EXISTS] parameter value_initializer
        map(
            tuple((
                expect_token(Token::Value),
                opt(tuple((
                    expect_token(Token::If),
                    expect_token(Token::Not),
                    expect_token(Token::Exists),
                ))),
                parameter_name,
                value_initializer,
            )),
            |(_, if_not_exists, parameter, value_initializer)| SessionSetClause::ValueParameter {
                parameter,
                value_initializer,
                if_not_exists: if_not_exists.is_some(),
            },
        ),
    ))(tokens)
}

/// Parse SESSION RESET statement
fn session_reset_statement(tokens: &[Token]) -> IResult<&[Token], SessionResetStatement> {
    map(
        tuple((
            expect_token(Token::Session),
            expect_token(Token::Reset),
            opt(session_reset_args),
        )),
        |(_, _, args)| SessionResetStatement {
            args,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse SESSION RESET arguments
fn session_reset_args(tokens: &[Token]) -> IResult<&[Token], SessionResetArgs> {
    alt((
        // [ALL] (PARAMETERS | CHARACTERISTICS)
        map(
            tuple((
                opt(expect_token(Token::All)),
                alt((
                    value(
                        SessionResetTarget::Parameters,
                        expect_token(Token::Parameters),
                    ),
                    value(
                        SessionResetTarget::Characteristics,
                        expect_token(Token::Characteristics),
                    ),
                )),
            )),
            |(_, target)| SessionResetArgs::All { target },
        ),
        // SCHEMA
        value(SessionResetArgs::Schema, expect_token(Token::Schema)),
        // [PROPERTY] GRAPH
        map(
            tuple((
                opt(expect_token(Token::Property)),
                expect_token(Token::Graph),
            )),
            |_| SessionResetArgs::Graph,
        ),
        // TIME ZONE
        map(
            tuple((expect_token(Token::Time), expect_token(Token::Zone))),
            |_| SessionResetArgs::TimeZone,
        ),
        // [PARAMETER] parameter
        map(
            tuple((opt(expect_token(Token::Parameter)), parameter_name)),
            |(_, parameter_name)| SessionResetArgs::Parameter {
                parameter: parameter_name,
            },
        ),
    ))(tokens)
}

/// Parse SESSION CLOSE statement
fn session_close_statement(tokens: &[Token]) -> IResult<&[Token], SessionCloseStatement> {
    map(
        tuple((expect_token(Token::Session), expect_token(Token::Close))),
        |(_, _)| SessionCloseStatement {
            location: Location::default(),
        },
    )(tokens)
}

/// Parse graph expression for session context
fn graph_expression(tokens: &[Token]) -> IResult<&[Token], GraphExpression> {
    log::debug!(
        "graph_expression called with tokens: {:?}",
        tokens.get(0..3).unwrap_or(&[])
    );

    // Parse union operations with precedence
    fn union_expr(tokens: &[Token]) -> IResult<&[Token], GraphExpression> {
        let (tokens, left) = primary_graph_expr(tokens)?;

        // Check for UNION operators
        let mut current = left;
        let mut remaining = tokens;

        while let Ok((tokens_after_union, _)) = expect_token(Token::Union)(remaining) {
            let (tokens_after_all, union_all) = opt(expect_token(Token::All))(tokens_after_union)?;
            let all = union_all.is_some();
            let (tokens_after_right, right) = primary_graph_expr(tokens_after_all)?;

            current = GraphExpression::Union {
                left: Box::new(current),
                right: Box::new(right),
                all,
            };
            remaining = tokens_after_right;
        }

        Ok((remaining, current))
    }

    // Parse primary graph expressions (non-union)
    fn primary_graph_expr(tokens: &[Token]) -> IResult<&[Token], GraphExpression> {
        alt((
            // Parenthesized expressions
            delimited(
                expect_token(Token::LeftParen),
                union_expr,
                expect_token(Token::RightParen),
            ),
            // parameter or catalog_path
            map(parameter_or_catalog_path, |path_or_param| {
                log::debug!("graph_expression parsed catalog path: {:?}", path_or_param);
                GraphExpression::Reference(path_or_param)
            }),
        ))(tokens)
    }

    let result = union_expr(tokens);
    if result.is_err() {
        log::debug!("graph_expression parsing failed: {:?}", result);
    }
    result
}

/// Parse parameter: $identifier (returns Parameter struct for expressions)
fn parameter(tokens: &[Token]) -> IResult<&[Token], Parameter> {
    map(
        tuple((expect_token(Token::Dollar), identifier)),
        |(_, name)| Parameter {
            name,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse parameter name: $identifier (returns String for session contexts)
fn parameter_name(tokens: &[Token]) -> IResult<&[Token], String> {
    map(
        tuple((expect_token(Token::Dollar), identifier)),
        |(_, name)| name,
    )(tokens)
}

/// Parse graph initializer: = graph_expression
fn graph_initializer(tokens: &[Token]) -> IResult<&[Token], GraphExpression> {
    map(
        tuple((expect_token(Token::Equal), graph_expression)),
        |(_, graph_expr)| graph_expr,
    )(tokens)
}

/// Parse binding table initializer: = query_statement
fn binding_table_initializer(tokens: &[Token]) -> IResult<&[Token], Box<Query>> {
    map(tuple((expect_token(Token::Equal), query)), |(_, query)| {
        Box::new(query)
    })(tokens)
}

/// Parse value initializer: = expression
fn value_initializer(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((expect_token(Token::Equal), expression)),
        |(_, expr)| expr,
    )(tokens)
}

/// Parse parameter or catalog path (helper function)
fn parameter_or_catalog_path(tokens: &[Token]) -> IResult<&[Token], CatalogPath> {
    // For simplicity, just parse as catalog path for now
    catalog_path(tokens)
}

/// Expect a specific token
fn expect_token(expected: Token) -> impl Fn(&[Token]) -> IResult<&[Token], Token> {
    move |tokens: &[Token]| {
        if let Some(token) = tokens.first() {
            if std::mem::discriminant(token) == std::mem::discriminant(&expected) {
                Ok((&tokens[1..], token.clone()))
            } else {
                Err(nom::Err::Error(nom::error::Error::new(
                    tokens,
                    nom::error::ErrorKind::Tag,
                )))
            }
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )))
        }
    }
}

// Helper function for tag_token (alias for expect_token for compatibility)
fn tag_token(expected: Token) -> impl Fn(&[Token]) -> IResult<&[Token], Token> {
    expect_token(expected)
}

/// Expect a token that matches a predicate
fn expect_token_variant<F>(predicate: &F) -> impl Fn(&[Token]) -> IResult<&[Token], Token> + '_
where
    F: Fn(&Token) -> bool,
{
    move |tokens: &[Token]| {
        if let Some(token) = tokens.first() {
            if predicate(token) {
                Ok((&tokens[1..], token.clone()))
            } else {
                Err(nom::Err::Error(nom::error::Error::new(
                    tokens,
                    nom::error::ErrorKind::Tag,
                )))
            }
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )))
        }
    }
}

/// Helper function to expect a specific identifier (case-insensitive)
fn expect_identifier(name: &str) -> impl Fn(&[Token]) -> IResult<&[Token], Token> + '_ {
    move |tokens: &[Token]| {
        if let Some(token) = tokens.first() {
            match token {
                Token::Identifier(id) if id.eq_ignore_ascii_case(name) => {
                    Ok((&tokens[1..], token.clone()))
                }
                _ => Err(nom::Err::Error(nom::error::Error::new(
                    tokens,
                    nom::error::ErrorKind::Tag,
                ))),
            }
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            )))
        }
    }
}

/// Parse CREATE USER statement: CREATE USER username PASSWORD password [ROLES (role1, role2, ...)]
fn create_user_statement(tokens: &[Token]) -> IResult<&[Token], CreateUserStatement> {
    map(
        tuple((
            expect_token(Token::Create),
            expect_token(Token::User),
            string_literal, // username
            expect_token(Token::Password),
            string_literal, // password
            opt(tuple((
                expect_token(Token::Roles),
                expect_token(Token::LeftParen),
                separated_list1(expect_token(Token::Comma), string_literal),
                expect_token(Token::RightParen),
            ))),
        )),
        |(_, _, username, _, password, roles_opt)| CreateUserStatement {
            username,
            password: Some(password),
            roles: roles_opt.map(|(_, _, roles, _)| roles).unwrap_or_default(),
            if_not_exists: false, // TODO: Add IF NOT EXISTS support
            location: Location::default(),
        },
    )(tokens)
}

/// Parse DROP USER statement: DROP USER [IF EXISTS] username
fn drop_user_statement(tokens: &[Token]) -> IResult<&[Token], DropUserStatement> {
    map(
        tuple((
            expect_token(Token::Drop),
            expect_token(Token::User),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Exists),
            ))),
            string_literal, // username
        )),
        |(_, _, if_exists, username)| DropUserStatement {
            username,
            if_exists: if_exists.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse CREATE ROLE statement: CREATE ROLE role_name [DESCRIPTION 'description']
fn create_role_statement(tokens: &[Token]) -> IResult<&[Token], CreateRoleStatement> {
    map(
        tuple((
            expect_token(Token::Create),
            expect_token(Token::Role),
            string_literal, // role_name
                            // TODO: Add description and permissions support
        )),
        |(_, _, role_name)| CreateRoleStatement {
            role_name,
            description: None,
            permissions: vec![],
            if_not_exists: false,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse DROP ROLE statement: DROP ROLE [IF EXISTS] role_name
fn drop_role_statement(tokens: &[Token]) -> IResult<&[Token], DropRoleStatement> {
    map(
        tuple((
            expect_token(Token::Drop),
            expect_token(Token::Role),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Exists),
            ))),
            string_literal, // role_name
        )),
        |(_, _, if_exists, role_name)| DropRoleStatement {
            role_name,
            if_exists: if_exists.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse GRANT ROLE statement: GRANT ROLE role_name TO username
fn grant_role_statement(tokens: &[Token]) -> IResult<&[Token], GrantRoleStatement> {
    map(
        tuple((
            expect_token(Token::Grant),
            expect_token(Token::Role),
            string_literal, // role_name
            expect_token(Token::To),
            string_literal, // username
        )),
        |(_, _, role_name, _, username)| GrantRoleStatement {
            role_name,
            username,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse REVOKE ROLE statement: REVOKE ROLE role_name FROM username
fn revoke_role_statement(tokens: &[Token]) -> IResult<&[Token], RevokeRoleStatement> {
    map(
        tuple((
            expect_token(Token::Revoke),
            expect_token(Token::Role),
            string_literal, // role_name
            expect_token(Token::From),
            string_literal, // username
        )),
        |(_, _, role_name, _, username)| RevokeRoleStatement {
            role_name,
            username,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse CREATE PROCEDURE statement
/// Syntax: CREATE [OR REPLACE] PROCEDURE [IF NOT EXISTS] procedure_name ([params]) procedure_body
fn create_procedure_statement(tokens: &[Token]) -> IResult<&[Token], CreateProcedureStatement> {
    map(
        tuple((
            expect_token(Token::Create),
            opt(tuple((
                expect_token(Token::Or),
                expect_token(Token::Replace),
            ))),
            expect_token(Token::Procedure),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Not),
                expect_token(Token::Exists),
            ))),
            identifier_or_quoted, // procedure_name (supports backticks)
            procedure_parameters,
            procedure_body_statement,
        )),
        |(_, or_replace, _, if_not_exists, procedure_name, parameters, procedure_body)| {
            CreateProcedureStatement {
                procedure_name,
                parameters,
                procedure_body,
                or_replace: or_replace.is_some(),
                if_not_exists: if_not_exists.is_some(),
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse DROP PROCEDURE statement
/// Syntax: DROP PROCEDURE [IF EXISTS] procedure_name
fn drop_procedure_statement(tokens: &[Token]) -> IResult<&[Token], DropProcedureStatement> {
    map(
        tuple((
            expect_token(Token::Drop),
            expect_token(Token::Procedure),
            opt(tuple((
                expect_token(Token::If),
                expect_token(Token::Exists),
            ))),
            identifier_or_quoted, // procedure_name (supports backticks)
        )),
        |(_, _, if_exists, procedure_name)| DropProcedureStatement {
            procedure_name,
            if_exists: if_exists.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse procedure parameters: (param1 type1 [= default1], param2 type2, ...)
fn procedure_parameters(tokens: &[Token]) -> IResult<&[Token], Vec<ProcedureParameter>> {
    delimited(
        expect_token(Token::LeftParen),
        opt(map(
            tuple((
                procedure_parameter,
                many0(tuple((
                    opt(expect_token(Token::Comma)),
                    procedure_parameter,
                ))),
            )),
            |(first_param, additional_params)| {
                let mut params = vec![first_param];
                params.extend(additional_params.into_iter().map(|(_, param)| param));
                params
            },
        )),
        expect_token(Token::RightParen),
    )(tokens)
    .map(|(remaining, opt_params)| (remaining, opt_params.unwrap_or_default()))
}

/// Parse single procedure parameter: name type_spec [= default_value]
fn procedure_parameter(tokens: &[Token]) -> IResult<&[Token], ProcedureParameter> {
    map(
        tuple((
            identifier_or_quoted,                                  // parameter name
            type_spec,                                             // parameter type
            opt(preceded(expect_token(Token::Equal), expression)), // optional default value
        )),
        |(name, type_spec, default_value)| ProcedureParameter {
            name,
            type_spec,
            default_value,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse IS predicate: expression IS [NOT] predicate_type [target]
fn is_predicate(tokens: &[Token]) -> IResult<&[Token], Expression> {
    map(
        tuple((
            additive_expression,
            expect_token(Token::Is),
            opt(expect_token(Token::Not)),
            alt((
                // Value state predicates
                map(expect_token(Token::Null), |_| {
                    (IsPredicateType::Null, None, None)
                }),
                map(expect_token(Token::Boolean(true)), |_| {
                    (IsPredicateType::True, None, None)
                }),
                map(expect_token(Token::Boolean(false)), |_| {
                    (IsPredicateType::False, None, None)
                }),
                map(expect_token(Token::Unknown), |_| {
                    (IsPredicateType::Unknown, None, None)
                }),
                // Graph element predicates
                map(expect_token(Token::Normalized), |_| {
                    (IsPredicateType::Normalized, None, None)
                }),
                map(expect_token(Token::Directed), |_| {
                    (IsPredicateType::Directed, None, None)
                }),
                // Topology predicates with optional targets
                map(
                    tuple((
                        expect_token(Token::Source),
                        opt(tuple((expect_token(Token::Of), additive_expression))),
                    )),
                    |(_, target)| {
                        (
                            IsPredicateType::Source,
                            target.map(|(_, expr)| Box::new(expr)),
                            None,
                        )
                    },
                ),
                map(
                    tuple((
                        expect_token(Token::Destination),
                        opt(tuple((expect_token(Token::Of), additive_expression))),
                    )),
                    |(_, target)| {
                        (
                            IsPredicateType::Destination,
                            target.map(|(_, expr)| Box::new(expr)),
                            None,
                        )
                    },
                ),
                // Type predicates
                map(tuple((expect_token(Token::Typed), type_spec)), |(_, ts)| {
                    (IsPredicateType::Typed, None, Some(ts))
                }),
                // Label predicates (enhanced)
                map(label_expression, |label_expr| {
                    (IsPredicateType::Label(label_expr), None, None)
                }),
            )),
        )),
        |(subject, _, not_token, (predicate_type, target, type_spec))| {
            Expression::IsPredicate(IsPredicateExpression {
                subject: Box::new(subject),
                predicate_type,
                negated: not_token.is_some(),
                target,
                type_spec,
                location: Location::default(),
            })
        },
    )(tokens)
}

/// Parse CASE expression: CASE [expr] WHEN ... THEN ... [ELSE ...] END
fn case_expression(tokens: &[Token]) -> IResult<&[Token], CaseExpression> {
    alt((
        map(simple_case_expression, |simple| CaseExpression {
            case_type: CaseType::Simple(simple),
            location: Location::default(),
        }),
        map(searched_case_expression, |searched| CaseExpression {
            case_type: CaseType::Searched(searched),
            location: Location::default(),
        }),
    ))(tokens)
}

/// Parse CAST expression: CAST(expr AS type-spec)
fn cast_expression(tokens: &[Token]) -> IResult<&[Token], CastExpression> {
    map(
        tuple((
            expect_token(Token::Cast),
            expect_token(Token::LeftParen),
            expression,
            expect_token(Token::As),
            type_spec,
            expect_token(Token::RightParen),
        )),
        |(_, _, expr, _, target_type, _)| CastExpression {
            expression: Box::new(expr),
            target_type,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse simple CASE: CASE expr WHEN value1 [, value2] THEN result1 ... [ELSE default] END
fn simple_case_expression(tokens: &[Token]) -> IResult<&[Token], SimpleCaseExpression> {
    map(
        tuple((
            expect_token(Token::Case),
            expression, // test expression
            many1(simple_when_branch),
            opt(tuple((expect_token(Token::Else), expression))),
            expect_token(Token::End),
        )),
        |(_, test_expr, when_branches, else_clause, _)| SimpleCaseExpression {
            test_expression: Box::new(test_expr),
            when_branches,
            else_expression: else_clause.map(|(_, expr)| Box::new(expr)),
        },
    )(tokens)
}

/// Parse searched CASE: CASE WHEN condition1 THEN result1 ... [ELSE default] END
fn searched_case_expression(tokens: &[Token]) -> IResult<&[Token], SearchedCaseExpression> {
    map(
        tuple((
            expect_token(Token::Case),
            many1(searched_when_branch),
            opt(tuple((expect_token(Token::Else), expression))),
            expect_token(Token::End),
        )),
        |(_, when_branches, else_clause, _)| SearchedCaseExpression {
            when_branches,
            else_expression: else_clause.map(|(_, expr)| Box::new(expr)),
        },
    )(tokens)
}

/// Parse WHEN branch for simple CASE: WHEN value1 [, value2, ...] THEN result
fn simple_when_branch(tokens: &[Token]) -> IResult<&[Token], SimpleWhenBranch> {
    map(
        tuple((
            expect_token(Token::When),
            separated_list1(expect_token(Token::Comma), expression), // multiple values
            expect_token(Token::Then),
            expression,
        )),
        |(_, when_values, _, then_expr)| SimpleWhenBranch {
            when_values,
            then_expression: Box::new(then_expr),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse WHEN branch for searched CASE: WHEN condition THEN result
fn searched_when_branch(tokens: &[Token]) -> IResult<&[Token], SearchedWhenBranch> {
    map(
        tuple((
            expect_token(Token::When),
            expression, // condition
            expect_token(Token::Then),
            expression, // result
        )),
        |(_, condition, _, then_expr)| SearchedWhenBranch {
            condition: Box::new(condition),
            then_expression: Box::new(then_expr),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse data modification statements (INSERT, MATCH INSERT, SET, REMOVE, DELETE)
fn data_statement(tokens: &[Token]) -> IResult<&[Token], DataStatement> {
    log::debug!(
        "data_statement called with tokens: {:?}",
        tokens.get(0..3).unwrap_or(&[])
    );
    let result = alt((
        // Try MATCH-* patterns first (more specific)
        map(match_insert_statement, DataStatement::MatchInsert),
        map(match_set_statement, DataStatement::MatchSet),
        map(match_remove_statement, DataStatement::MatchRemove),
        map(match_delete_statement, DataStatement::MatchDelete),
        // Then try standalone patterns
        map(insert_statement, DataStatement::Insert),
        map(set_statement, DataStatement::Set),
        map(remove_statement, DataStatement::Remove),
        map(delete_statement, DataStatement::Delete),
    ))(tokens);
    if result.is_err() {
        log::debug!("data_statement parsing failed");
    }
    result
}

/// Parse MATCH INSERT statement: MATCH pattern... [WITH ...] [WHERE condition] INSERT graph_pattern [, graph_pattern]*
fn match_insert_statement(tokens: &[Token]) -> IResult<&[Token], MatchInsertStatement> {
    log::debug!(
        "match_insert_statement called with tokens: {:?}",
        tokens.get(0..5).unwrap_or(&[])
    );
    let result = map(
        tuple((
            match_clause,
            opt(with_clause),
            opt(where_clause),
            expect_token(Token::Insert),
            separated_list1(expect_token(Token::Comma), graph_pattern),
        )),
        |(match_clause, with_clause_opt, where_clause_opt, _, insert_graph_patterns)| {
            MatchInsertStatement {
                match_clause,
                with_clause: with_clause_opt,
                where_clause: where_clause_opt,
                insert_graph_patterns,
                location: Location::default(),
            }
        },
    )(tokens);

    match &result {
        Ok(_) => log::debug!("match_insert_statement successfully parsed MATCH INSERT"),
        Err(e) => log::debug!("match_insert_statement failed: {:?}", e),
    }

    result
}

/// Parse MATCH SET statement: MATCH pattern... [WITH ...] [WHERE condition] SET item, item
fn match_set_statement(tokens: &[Token]) -> IResult<&[Token], MatchSetStatement> {
    log::debug!(
        "PARSER: match_set_statement called with first 5 tokens: {:?}",
        tokens.get(0..5).unwrap_or(&[])
    );
    let result = map(
        tuple((
            match_clause,
            opt(with_clause),
            opt(where_clause),
            expect_token(Token::Set),
            separated_list1(expect_token(Token::Comma), set_item),
        )),
        |(match_clause, with_clause_opt, where_clause_opt, _, items)| {
            log::debug!(
                "PARSER: Successfully parsed MatchSetStatement with WITH clause: {}",
                with_clause_opt.is_some()
            );
            MatchSetStatement {
                match_clause,
                with_clause: with_clause_opt,
                where_clause: where_clause_opt,
                items,
                location: Location::default(),
            }
        },
    )(tokens);

    match &result {
        Ok((remaining, _)) => log::debug!(
            "PARSER: match_set_statement succeeded, remaining tokens: {}",
            remaining.len()
        ),
        Err(e) => log::debug!("PARSER: match_set_statement failed: {:?}", e),
    }

    result
}

/// Parse MATCH REMOVE statement: MATCH pattern... [WITH ...] [WHERE condition] REMOVE item, item
fn match_remove_statement(tokens: &[Token]) -> IResult<&[Token], MatchRemoveStatement> {
    map(
        tuple((
            match_clause,
            opt(with_clause),
            opt(where_clause),
            expect_token(Token::Remove),
            separated_list1(expect_token(Token::Comma), remove_item),
        )),
        |(match_clause, with_clause_opt, where_clause_opt, _, items)| MatchRemoveStatement {
            match_clause,
            with_clause: with_clause_opt,
            where_clause: where_clause_opt,
            items,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse MATCH DELETE statement: MATCH pattern... [WITH ...] [WHERE condition] [DETACH] DELETE expression, expression
fn match_delete_statement(tokens: &[Token]) -> IResult<&[Token], MatchDeleteStatement> {
    map(
        tuple((
            match_clause,
            opt(with_clause),
            opt(where_clause),
            opt(alt((
                expect_token(Token::Detach),
                expect_token(Token::NoDetach),
            ))),
            expect_token(Token::Delete),
            separated_list1(expect_token(Token::Comma), expression),
        )),
        |(match_clause, with_clause_opt, where_clause_opt, detach_mode, _, expressions)| {
            MatchDeleteStatement {
                match_clause,
                with_clause: with_clause_opt,
                where_clause: where_clause_opt,
                expressions,
                detach: match detach_mode {
                    Some(Token::Detach) => true,
                    Some(Token::NoDetach) => false,
                    None => false,
                    _ => false, // Default to false for any other token
                },
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse INSERT statement: INSERT graph_pattern
fn insert_statement(tokens: &[Token]) -> IResult<&[Token], InsertStatement> {
    map(
        tuple((
            alt((expect_token(Token::Insert), expect_token(Token::Create))),
            separated_list1(expect_token(Token::Comma), graph_pattern),
        )),
        |(_, graph_patterns)| InsertStatement {
            graph_patterns,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse graph pattern - a single node or path pattern for INSERT
fn graph_pattern(tokens: &[Token]) -> IResult<&[Token], PathPattern> {
    alt((
        // Try full path pattern first (node-edge-node sequences)
        path_pattern,
        // If that fails, try single node pattern
        map(node_pattern, |node| PathPattern {
            assignment: None,
            path_type: None,
            elements: vec![PatternElement::Node(node)],
            location: Location::default(),
        }),
    ))(tokens)
}

/// Parse SET statement: SET set_item [, set_item]*
fn set_statement(tokens: &[Token]) -> IResult<&[Token], SetStatement> {
    log::debug!(
        "set_statement called with tokens: {:?}",
        tokens.get(0..10).unwrap_or(&[])
    );
    let result = map(
        tuple((
            expect_token(Token::Set),
            separated_list1(expect_token(Token::Comma), set_item),
        )),
        |(_, items)| {
            log::debug!("SET parsing succeeded");
            SetStatement {
                items,
                location: Location::default(),
            }
        },
    )(tokens);
    if result.is_err() {
        log::debug!("set_statement parsing failed: {:?}", result);
    }
    result
}

/// Parse SET item: property = value | variable = value | variable:label
fn set_item(tokens: &[Token]) -> IResult<&[Token], SetItem> {
    alt((
        // Property assignment: object.property = value
        map(
            tuple((
                property_access_token,
                expect_token(Token::Equal),
                expression,
            )),
            |(property, _, value)| SetItem::Property { property, value },
        ),
        // Label assignment: variable:label or variable IS label
        map(
            tuple((
                identifier,
                alt((expect_token(Token::Colon), expect_token(Token::Is))),
                label_expression,
            )),
            |(variable, _, labels)| SetItem::Label { variable, labels },
        ),
        // Variable assignment: variable = value
        map(
            tuple((identifier, expect_token(Token::Equal), expression)),
            |(variable, _, value)| SetItem::Variable { variable, value },
        ),
    ))(tokens)
}

/// Parse REMOVE statement: REMOVE remove_item [, remove_item]*
fn remove_statement(tokens: &[Token]) -> IResult<&[Token], RemoveStatement> {
    log::debug!(
        "remove_statement called with tokens: {:?}",
        tokens.get(0..10).unwrap_or(&[])
    );
    let result = map(
        tuple((
            expect_token(Token::Remove),
            separated_list1(expect_token(Token::Comma), remove_item),
        )),
        |(_, items)| RemoveStatement {
            items,
            location: Location::default(),
        },
    )(tokens);
    if result.is_err() {
        log::debug!("remove_statement parsing failed");
    }
    result
}

/// Parse REMOVE item: property | variable:label | variable
fn remove_item(tokens: &[Token]) -> IResult<&[Token], RemoveItem> {
    alt((
        // Property removal: object.property
        map(property_access_token, RemoveItem::Property),
        // Label removal: variable:label or variable IS label
        map(
            tuple((
                identifier,
                alt((expect_token(Token::Colon), expect_token(Token::Is))),
                label_expression,
            )),
            |(variable, _, labels)| RemoveItem::Label { variable, labels },
        ),
        // Variable removal: variable
        map(identifier, RemoveItem::Variable),
    ))(tokens)
}

/// Parse DELETE statement: [DETACH | NODETACH] DELETE expression [, expression]*
fn delete_statement(tokens: &[Token]) -> IResult<&[Token], DeleteStatement> {
    map(
        tuple((
            opt(alt((
                expect_token(Token::Detach),
                expect_token(Token::NoDetach),
            ))),
            expect_token(Token::Delete),
            separated_list1(expect_token(Token::Comma), expression),
        )),
        |(detach_mode, _, expressions)| DeleteStatement {
            expressions,
            detach: match detach_mode {
                Some(Token::Detach) => true,
                Some(Token::NoDetach) => false,
                None => true, // Default to DETACH when not specified
                _ => true,
            },
            location: Location::default(),
        },
    )(tokens)
}

/// Parse EXISTS subquery: EXISTS(subquery)
fn exists_subquery(tokens: &[Token]) -> IResult<&[Token], ExistsSubqueryExpression> {
    map(
        tuple((
            expect_token(Token::Exists),
            expect_token(Token::LeftParen),
            basic_query,
            expect_token(Token::RightParen),
        )),
        |(_, _, query, _)| ExistsSubqueryExpression {
            query: Box::new(query),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse NOT EXISTS subquery: NOT EXISTS(subquery)
fn not_exists_subquery(tokens: &[Token]) -> IResult<&[Token], NotExistsSubqueryExpression> {
    map(
        tuple((
            expect_token(Token::Not),
            expect_token(Token::Exists),
            expect_token(Token::LeftParen),
            basic_query,
            expect_token(Token::RightParen),
        )),
        |(_, _, _, query, _)| NotExistsSubqueryExpression {
            query: Box::new(query),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse general subquery: (subquery)
fn subquery_expression(tokens: &[Token]) -> IResult<&[Token], SubqueryExpression> {
    map(
        tuple((
            expect_token(Token::LeftParen),
            basic_query,
            expect_token(Token::RightParen),
        )),
        |(_, query, _)| SubqueryExpression {
            query: Box::new(query),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse DECLARE statement: DECLARE variable type [= initial_value] [, variable type [= initial_value]]*
fn declare_statement(tokens: &[Token]) -> IResult<&[Token], DeclareStatement> {
    map(
        tuple((
            expect_token(Token::Declare),
            separated_list1(expect_token(Token::Comma), variable_declaration),
        )),
        |(_, variable_declarations)| DeclareStatement {
            variable_declarations,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse variable declaration: variable_name type_spec [= initial_value]
fn variable_declaration(tokens: &[Token]) -> IResult<&[Token], VariableDeclaration> {
    map(
        tuple((
            identifier,
            type_spec,
            opt(preceded(expect_token(Token::Equal), expression)),
        )),
        |(variable_name, type_spec, initial_value)| VariableDeclaration {
            variable_name,
            type_spec,
            initial_value,
            location: Location::default(),
        },
    )(tokens)
}

// NEXT statement parsing removed - NEXT can only appear within procedure body contexts
// The procedure_body_statement function handles NEXT parsing directly

/// Parse procedure body: [variable_definition+] statement (NEXT [yield_clause] statement)*
/// This handles the specific case where MATCH clauses don't require RETURN when followed by NEXT
fn procedure_body_statement(tokens: &[Token]) -> IResult<&[Token], ProcedureBodyStatement> {
    // A procedure body must have either:
    // 1. Variable definitions at the start, OR
    // 2. At least one NEXT statement (for chaining)
    // This prevents simple statements from being incorrectly parsed as procedure bodies

    alt((
        // Case 1: Has variable definitions
        map(
            tuple((
                many1(variable_declaration_for_procedure_body), // At least one variable definition
                alt((
                    map(query, Statement::Query), // Accept any query including LET
                    map(data_statement, Statement::DataStatement),
                    map(catalog_statement, Statement::CatalogStatement),
                )),
                many0(tuple((
                    expect_token(Token::Next),
                    opt(yield_clause),
                    alt((
                        map(query, Statement::Query), // Accept any query after NEXT
                        map(data_statement, Statement::DataStatement),
                    )),
                ))),
            )),
            |(variable_defs, initial_statement, chained)| {
                (variable_defs, initial_statement, chained)
            },
        ),
        // Case 2: No variable definitions but has NEXT statements
        map(
            tuple((
                alt((
                    map(query, Statement::Query), // Accept any query including LET
                    map(data_statement, Statement::DataStatement),
                    map(catalog_statement, Statement::CatalogStatement),
                )),
                many1(tuple((
                    // At least one NEXT statement
                    expect_token(Token::Next),
                    opt(yield_clause),
                    alt((
                        map(query, Statement::Query), // Accept any query after NEXT
                        map(data_statement, Statement::DataStatement),
                    )),
                ))),
            )),
            |(initial_statement, chained)| (vec![], initial_statement, chained),
        ),
    ))(tokens)
    .map(|(remaining, (variable_defs, initial_statement, chained))| {
        // Convert variable declarations to DeclareStatements
        let variable_definitions = if variable_defs.is_empty() {
            vec![]
        } else {
            vec![DeclareStatement {
                variable_declarations: variable_defs,
                location: Location::default(),
            }]
        };

        // Convert chained statements
        let chained_statements = chained
            .into_iter()
            .map(|(_, yield_clause, statement)| ChainedStatement {
                yield_clause,
                statement: Box::new(statement),
                location: Location::default(),
            })
            .collect();

        (
            remaining,
            ProcedureBodyStatement {
                variable_definitions,
                initial_statement: Box::new(initial_statement),
                chained_statements,
                location: Location::default(),
            },
        )
    })
}

/// Parse variable declaration for procedure body: [<type-spec>] <identifier> ["=" <expression>]
fn variable_declaration_for_procedure_body(
    tokens: &[Token],
) -> IResult<&[Token], VariableDeclaration> {
    map(
        tuple((
            // Optional type specification
            opt(type_spec),
            // Variable identifier
            identifier,
            // Optional initial value
            opt(preceded(tag_token(Token::Equal), expression)),
        )),
        |(type_spec, variable_name, initial_value)| {
            VariableDeclaration {
                variable_name,
                type_spec: type_spec.unwrap_or(TypeSpec::Vector { dimension: None }), // Default to vector type when unspecified
                initial_value,
                location: Location::default(),
            }
        },
    )(tokens)
}

/// Parse a MATCH clause for procedure body (only the MATCH part, stopping before WHERE)
///
/// # Purpose
/// This function is reserved for future ISO GQL AT/NEXT full implementation.
/// It handles the special case of parsing MATCH clauses in procedure bodies where:
/// - MATCH may not require a RETURN clause (when followed by NEXT)
/// - Parsing must stop at WHERE or NEXT boundaries
/// - The standard `match_clause()` parser is too greedy for procedure contexts
///
/// # Current Status
/// Not yet wired to the parser. The current `procedure_body_statement()` uses the
/// full `query` parser which works correctly for common cases due to nom's backtracking.
///
/// # Future Use Case
/// When implementing full support for MATCH without RETURN:
/// ```gql
/// CREATE PROCEDURE example()
///   MATCH (n:Person) WHERE n.age > 30
///   NEXT
///   MATCH (m:Company)
/// ```
///
/// # Implementation Plan
/// See ROADMAP.md v0.3.0 - ISO GQL Procedure Body Enhancement for complete implementation details (effort: 3-4 hours)
///
/// # Related
/// - `where_clause_for_procedure_body()` - Companion function for WHERE parsing
/// - `procedure_body_statement()` - Will use this when enhanced
/// - ROADMAP.md - Full AT/NEXT implementation roadmap
#[allow(dead_code)] // Reserved for ISO GQL AT/NEXT full implementation (ROADMAP.md v0.3.0)
fn match_clause_for_procedure_body(tokens: &[Token]) -> IResult<&[Token], MatchClause> {
    // Find where WHERE starts or NEXT if no WHERE
    let stop_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Where | Token::Next))
        .ok_or_else(|| {
            nom::Err::Error(nom::error::Error::new(tokens, nom::error::ErrorKind::Alt))
        })?;

    // Parse just the MATCH part
    let match_tokens = &tokens[..stop_pos];

    // Parse the MATCH clause
    let (remaining, match_clause) = match_clause(match_tokens)?;

    // Ensure we consumed all MATCH tokens
    if !remaining.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            remaining,
            nom::error::ErrorKind::Complete,
        )));
    }

    // Return the remaining tokens starting from WHERE/NEXT
    Ok((&tokens[stop_pos..], match_clause))
}

/// Parse optional WHERE clause for procedure body (stopping at NEXT)
///
/// # Purpose
/// This function is reserved for future ISO GQL AT/NEXT full implementation.
/// It handles the special case of parsing WHERE clauses in procedure bodies where:
/// - WHERE may appear without a following RETURN (when followed by NEXT)
/// - Parsing must stop at NEXT boundaries
/// - The standard `where_clause()` parser may consume too many tokens
///
/// # Current Status
/// Not yet wired to the parser. The current `procedure_body_statement()` uses the
/// full `query` parser which handles WHERE clauses correctly in most cases.
///
/// # Future Use Case
/// When implementing full support for WHERE without RETURN:
/// ```gql
/// CREATE PROCEDURE example()
///   MATCH (n:Person)
///   WHERE n.age > 30
///   NEXT
///   RETURN count(n) as adults
/// ```
///
/// # Design Pattern
/// This function works in tandem with `match_clause_for_procedure_body()`:
/// 1. Parse MATCH up to WHERE
/// 2. Parse WHERE up to NEXT
/// 3. Create BasicQuery without RETURN clause
///
/// # Implementation Plan
/// See ROADMAP.md v0.3.0 - ISO GQL Procedure Body Enhancement for complete implementation details (effort: 3-4 hours)
///
/// # Related
/// - `match_clause_for_procedure_body()` - Companion function for MATCH parsing
/// - `procedure_body_statement()` - Will use this when enhanced
/// - ROADMAP.md - Full AT/NEXT implementation roadmap
#[allow(dead_code)] // Reserved for ISO GQL AT/NEXT full implementation (ROADMAP.md v0.3.0)
fn where_clause_for_procedure_body(tokens: &[Token]) -> IResult<&[Token], Option<WhereClause>> {
    // Check if we start with WHERE
    if tokens.is_empty() || !matches!(tokens[0], Token::Where) {
        return Ok((tokens, None));
    }

    // Find where NEXT starts
    let next_pos = tokens
        .iter()
        .position(|t| matches!(t, Token::Next))
        .ok_or_else(|| {
            nom::Err::Error(nom::error::Error::new(tokens, nom::error::ErrorKind::Alt))
        })?;

    // Parse WHERE clause tokens
    let where_tokens = &tokens[..next_pos];

    // Parse the WHERE clause
    let (remaining, where_clause) = where_clause(where_tokens)?;

    // Ensure we consumed all WHERE tokens
    if !remaining.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            remaining,
            nom::error::ErrorKind::Complete,
        )));
    }

    // Return the remaining tokens starting from NEXT
    Ok((&tokens[next_pos..], Some(where_clause)))
}

/// Parse AT location statement: AT location_path statements*
fn at_location_statement(tokens: &[Token]) -> IResult<&[Token], AtLocationStatement> {
    map(
        tuple((
            expect_token(Token::At),
            catalog_path,
            many1(map(
                tuple((
                    alt((
                        map(declare_statement, Statement::Declare),
                        // NEXT statements removed - only allowed in procedure body context
                        map(basic_query, Statement::Query),
                        map(select_statement, Statement::Select),
                        map(set_statement, |s| {
                            Statement::DataStatement(DataStatement::Set(s))
                        }),
                    )),
                    opt(expect_token(Token::Semicolon)),
                )),
                |(statement, _)| statement,
            )),
        )),
        |(_, location_path, statements)| AtLocationStatement {
            location_path,
            statements,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse transaction statement
fn transaction_statement(tokens: &[Token]) -> IResult<&[Token], TransactionStatement> {
    alt((
        map(
            start_transaction_statement,
            TransactionStatement::StartTransaction,
        ),
        map(commit_statement, TransactionStatement::Commit),
        map(rollback_statement, TransactionStatement::Rollback),
        map(
            set_transaction_characteristics_statement,
            TransactionStatement::SetTransactionCharacteristics,
        ),
    ))(tokens)
}

/// Parse START TRANSACTION [characteristics]
fn start_transaction_statement(tokens: &[Token]) -> IResult<&[Token], StartTransactionStatement> {
    alt((
        // START TRANSACTION
        map(
            tuple((
                expect_token(Token::Start),
                expect_identifier("TRANSACTION"),
                opt(transaction_characteristics),
            )),
            |(_, _, characteristics)| StartTransactionStatement {
                characteristics,
                location: Location::default(),
            },
        ),
        // BEGIN [characteristics] (alternative syntax)
        map(
            tuple((expect_token(Token::Begin), opt(transaction_characteristics))),
            |(_, characteristics)| StartTransactionStatement {
                characteristics,
                location: Location::default(),
            },
        ),
    ))(tokens)
}

/// Parse COMMIT [WORK]
fn commit_statement(tokens: &[Token]) -> IResult<&[Token], CommitStatement> {
    map(
        tuple((expect_token(Token::Commit), opt(expect_token(Token::Work)))),
        |(_, work)| CommitStatement {
            work: work.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse ROLLBACK [WORK]
fn rollback_statement(tokens: &[Token]) -> IResult<&[Token], RollbackStatement> {
    map(
        tuple((
            expect_token(Token::Rollback),
            opt(expect_token(Token::Work)),
        )),
        |(_, work)| RollbackStatement {
            work: work.is_some(),
            location: Location::default(),
        },
    )(tokens)
}

/// Parse SET TRANSACTION characteristics
fn set_transaction_characteristics_statement(
    tokens: &[Token],
) -> IResult<&[Token], SetTransactionCharacteristicsStatement> {
    map(
        tuple((
            expect_token(Token::Set),
            expect_identifier("TRANSACTION"),
            transaction_characteristics,
        )),
        |(_, _, characteristics)| SetTransactionCharacteristicsStatement {
            characteristics,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse transaction characteristics
fn transaction_characteristics(tokens: &[Token]) -> IResult<&[Token], TransactionCharacteristics> {
    map(
        tuple((opt(isolation_level_clause), opt(access_mode_clause))),
        |(isolation_level, access_mode)| TransactionCharacteristics {
            isolation_level,
            access_mode,
            location: Location::default(),
        },
    )(tokens)
}

/// Parse isolation level: ISOLATION LEVEL (READ UNCOMMITTED | READ COMMITTED | REPEATABLE READ | SERIALIZABLE)
fn isolation_level_clause(tokens: &[Token]) -> IResult<&[Token], IsolationLevel> {
    map(
        tuple((
            expect_token(Token::Isolation),
            expect_token(Token::Level),
            alt((
                map(
                    tuple((expect_token(Token::Read), expect_token(Token::Uncommitted))),
                    |_| IsolationLevel::ReadUncommitted,
                ),
                map(
                    tuple((expect_token(Token::Read), expect_token(Token::Committed))),
                    |_| IsolationLevel::ReadCommitted,
                ),
                map(
                    tuple((expect_token(Token::Repeatable), expect_token(Token::Read))),
                    |_| IsolationLevel::RepeatableRead,
                ),
                map(expect_token(Token::Serializable), |_| {
                    IsolationLevel::Serializable
                }),
            )),
        )),
        |(_, _, level)| level,
    )(tokens)
}

/// Parse access mode: READ ONLY | READ WRITE
fn access_mode_clause(tokens: &[Token]) -> IResult<&[Token], AccessMode> {
    alt((
        map(
            tuple((expect_token(Token::Read), expect_token(Token::Only))),
            |_| AccessMode::ReadOnly,
        ),
        map(
            tuple((expect_token(Token::Read), expect_token(Token::Write))),
            |_| AccessMode::ReadWrite,
        ),
    ))(tokens)
}

/// Parse pattern expression for WHERE clauses: (node)-[edge]->(node)
fn pattern_expression(tokens: &[Token]) -> IResult<&[Token], PatternExpression> {
    // Look ahead to see if this starts with a node pattern (open paren followed by identifier or label)
    // This helps distinguish patterns from regular parenthesized expressions
    if let Some(&Token::LeftParen) = tokens.first() {
        // Look at the second token to see if this could be a node pattern
        if let Some(second_token) = tokens.get(1) {
            match second_token {
                // (identifier...
                Token::Identifier(_) => {
                    // Check if there's a colon (label) or close paren after identifier
                    if let Some(third_token) = tokens.get(2) {
                        match third_token {
                            Token::Colon | Token::RightParen | Token::LeftBrace => {
                                // This looks like a node pattern, try to parse it
                                return map(path_pattern, |pattern| PatternExpression {
                                    pattern,
                                    location: Location::default(),
                                })(tokens);
                            }
                            _ => {}
                        }
                    }
                }
                // (:Label...
                Token::Colon => {
                    // This looks like a label-only node pattern
                    return map(path_pattern, |pattern| PatternExpression {
                        pattern,
                        location: Location::default(),
                    })(tokens);
                }
                // ()... empty node pattern
                Token::RightParen => {
                    // Check if there's an edge pattern after the node
                    if tokens.len() > 3 {
                        if let Some(fourth_token) = tokens.get(3) {
                            match fourth_token {
                                Token::Dash | Token::LessThan => {
                                    // This looks like a pattern starting with empty node
                                    return map(path_pattern, |pattern| PatternExpression {
                                        pattern,
                                        location: Location::default(),
                                    })(tokens);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // If we get here, this doesn't look like a pattern, so fail
    Err(nom::Err::Error(nom::error::Error::new(
        tokens,
        nom::error::ErrorKind::Tag,
    )))
}

// =============================================================================
// INDEX DDL STATEMENT PARSERS
// =============================================================================

/// Parse index statement (CREATE INDEX, DROP INDEX, ALTER INDEX, OPTIMIZE INDEX, REINDEX)
fn index_statement(tokens: &[Token]) -> IResult<&[Token], IndexStatement> {
    alt((
        map(create_index_statement, IndexStatement::Create),
        map(drop_index_statement, IndexStatement::Drop),
        map(alter_index_statement, IndexStatement::Alter),
        map(optimize_index_statement, IndexStatement::Optimize),
        map(reindex_statement, IndexStatement::Reindex),
    ))(tokens)
}

/// Parse index name - lenient parser that accepts various token types for better error messages
/// This allows invalid names to be parsed so we can provide meaningful validation errors
fn parse_index_name(tokens: &[Token]) -> IResult<&[Token], String> {
    if let Some(token) = tokens.first() {
        let name = match token {
            // Valid identifier tokens
            Token::Identifier(s) => Some(s.clone()),
            Token::String(s) => Some(s.clone()),

            // Accept integers so we can validate "cannot start with digit" later
            Token::Integer(n) => Some(n.to_string()),
            Token::Float(f) => Some(f.to_string()),

            // Accept keywords that might be used as identifiers
            Token::Value => Some("value".to_string()),
            Token::Type => Some("type".to_string()),
            Token::User => Some("user".to_string()),
            Token::Role => Some("role".to_string()),
            Token::Schema => Some("schema".to_string()),
            Token::Data => Some("data".to_string()),
            Token::Graph => Some("graph".to_string()),
            Token::Node => Some("node".to_string()),
            Token::Edge => Some("edge".to_string()),
            Token::Path => Some("path".to_string()),
            Token::Table => Some("table".to_string()),
            Token::Property => Some("property".to_string()),

            _ => None,
        };

        if let Some(mut name_str) = name {
            let mut remaining = &tokens[1..];

            // Check if next token is a minus sign followed by identifier (e.g., "invalid-name")
            // Consume the pattern to give better error message later
            while let Some(Token::Minus) = remaining.first() {
                if let Some(Token::Identifier(suffix)) = remaining.get(1) {
                    name_str.push('-');
                    name_str.push_str(suffix);
                    remaining = &remaining[2..];
                } else {
                    break;
                }
            }

            // Check if this was an integer followed by underscore and identifier (e.g., "123_invalid")
            // This handles the case where lexer splits it as: Integer(123), Identifier("_invalid")
            if let Token::Integer(_) = token {
                while let Some(Token::Identifier(suffix)) = remaining.first() {
                    if suffix.starts_with('_') {
                        name_str.push_str(suffix);
                        remaining = &remaining[1..];
                    } else {
                        break;
                    }
                }
            }

            return Ok((remaining, name_str));
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(
        tokens,
        nom::error::ErrorKind::Tag,
    )))
}

/// Parse CREATE GRAPH INDEX [IF NOT EXISTS] statement
fn create_index_statement(tokens: &[Token]) -> IResult<&[Token], CreateIndexStatement> {
    let (tokens, _) = expect_token(Token::Create)(tokens)?;

    // Parse index type specifier
    let (tokens, index_type) = alt((
        map(
            pair(
                preceded(expect_identifier("GRAPH"), expect_identifier("INDEX")),
                opt(graph_index_type),
            ),
            |(_, gtype)| {
                IndexTypeSpecifier::Graph(gtype.unwrap_or(GraphIndexTypeSpecifier::AdjacencyList))
            },
        ),
        // Default to adjacency list if just "CREATE INDEX"
        map(expect_identifier("INDEX"), |_| {
            IndexTypeSpecifier::Graph(GraphIndexTypeSpecifier::AdjacencyList)
        }),
    ))(tokens)?;

    // Parse optional IF NOT EXISTS
    let (tokens, if_not_exists) = opt(tuple((
        expect_token(Token::If),
        expect_token(Token::Not),
        expect_token(Token::Exists),
    )))(tokens)?;
    let if_not_exists = if_not_exists.is_some();

    // Parse index name - use lenient parser to accept various tokens for better error messages
    let (tokens, name) = parse_index_name(tokens)?;

    // Parse ON table_name
    let (tokens, _) = expect_token(Token::On)(tokens)?;
    let (tokens, table) = parse_table_name(tokens)?;

    // Parse optional column list (column1, column2, ...)
    let (tokens, columns) = opt(delimited(
        expect_token(Token::LeftParen),
        separated_list1(expect_token(Token::Comma), identifier),
        expect_token(Token::RightParen),
    ))(tokens)?;

    // Parse optional USING clause
    let (tokens, _using_type) = opt(preceded(
        expect_identifier("USING"),
        alt((
            expect_identifier("IVF"),
            expect_identifier("FLAT"),
            expect_identifier("INVERTED"),
            expect_identifier("BM25"),
            expect_identifier("NGRAM"),
            expect_identifier("ADJACENCY_LIST"),
            expect_identifier("PATH_INDEX"),
            expect_identifier("REACHABILITY"),
            expect_identifier("PATTERN_INDEX"),
        )),
    ))(tokens)?;

    // Parse optional WITH clause (parameter=value, ...)
    let (tokens, options) = opt(preceded(expect_identifier("WITH"), index_options))(tokens)?;

    Ok((
        tokens,
        CreateIndexStatement {
            name,
            table,
            columns: columns.unwrap_or_default(),
            index_type,
            options: options.unwrap_or_default(),
            if_not_exists,
            location: Location::default(),
        },
    ))
}

/// Parse DROP INDEX statement
fn drop_index_statement(tokens: &[Token]) -> IResult<&[Token], DropIndexStatement> {
    let (tokens, _) = expect_token(Token::Drop)(tokens)?;
    let (tokens, _) = expect_identifier("INDEX")(tokens)?;

    // Parse optional IF EXISTS
    let (tokens, if_exists) = opt(preceded(
        expect_token(Token::If),
        expect_token(Token::Exists),
    ))(tokens)?;

    // Parse index name - use lenient parser to accept various tokens for better error messages
    let (tokens, name) = parse_index_name(tokens)?;

    Ok((
        tokens,
        DropIndexStatement {
            name,
            if_exists: if_exists.is_some(),
            location: Location::default(),
        },
    ))
}

/// Parse ALTER INDEX statement
fn alter_index_statement(tokens: &[Token]) -> IResult<&[Token], AlterIndexStatement> {
    let (tokens, _) = expect_token(Token::Alter)(tokens)?;
    let (tokens, _) = expect_identifier("INDEX")(tokens)?;

    // Parse index name
    let (tokens, name) = identifier(tokens)?;

    // Parse operation
    let (tokens, operation) = alt((
        map(expect_identifier("REBUILD"), |_| {
            AlterIndexOperation::Rebuild
        }),
        map(expect_identifier("OPTIMIZE"), |_| {
            AlterIndexOperation::Optimize
        }),
        map(
            preceded(
                tuple((expect_identifier("SET"), expect_identifier("OPTION"))),
                tuple((
                    identifier,
                    preceded(expect_token(Token::Equal), parse_value),
                )),
            ),
            |(key, value)| AlterIndexOperation::SetOption(key, value),
        ),
    ))(tokens)?;

    Ok((
        tokens,
        AlterIndexStatement {
            name,
            operation,
            location: Location::default(),
        },
    ))
}

/// Parse OPTIMIZE INDEX statement
fn optimize_index_statement(tokens: &[Token]) -> IResult<&[Token], OptimizeIndexStatement> {
    let (tokens, _) = expect_identifier("OPTIMIZE")(tokens)?;
    let (tokens, _) = expect_identifier("INDEX")(tokens)?;

    // Parse index name
    let (tokens, name) = identifier(tokens)?;

    Ok((
        tokens,
        OptimizeIndexStatement {
            name,
            location: Location::default(),
        },
    ))
}

/// Parse REINDEX statement
/// Syntax: REINDEX index_name
fn reindex_statement(tokens: &[Token]) -> IResult<&[Token], ReindexStatement> {
    let (tokens, _) = expect_identifier("REINDEX")(tokens)?;

    // Parse index name
    let (tokens, name) = identifier(tokens)?;

    Ok((
        tokens,
        ReindexStatement {
            name,
            location: Location::default(),
        },
    ))
}

/// Parse graph index type specifier
fn graph_index_type(tokens: &[Token]) -> IResult<&[Token], GraphIndexTypeSpecifier> {
    alt((
        map(expect_identifier("ADJACENCY_LIST"), |_| {
            GraphIndexTypeSpecifier::AdjacencyList
        }),
        map(expect_identifier("PATH_INDEX"), |_| {
            GraphIndexTypeSpecifier::PathIndex
        }),
        map(expect_identifier("REACHABILITY"), |_| {
            GraphIndexTypeSpecifier::ReachabilityIndex
        }),
        map(expect_identifier("PATTERN_INDEX"), |_| {
            GraphIndexTypeSpecifier::PatternIndex
        }),
    ))(tokens)
}

/// Parse index options (parameter=value, ...)
fn index_options(tokens: &[Token]) -> IResult<&[Token], IndexOptions> {
    let (tokens, params) = delimited(
        expect_token(Token::LeftParen),
        separated_list1(
            expect_token(Token::Comma),
            tuple((
                identifier,
                preceded(expect_token(Token::Equal), parse_value),
            )),
        ),
        expect_token(Token::RightParen),
    )(tokens)?;

    let parameters = params.into_iter().collect();

    Ok((
        tokens,
        IndexOptions {
            parameters,
            location: Location::default(),
        },
    ))
}

/// Parse value for index parameters
fn parse_value(tokens: &[Token]) -> IResult<&[Token], Value> {
    alt((
        map(parse_string_literal, Value::String),
        map(parse_number, Value::Number),
        map(parse_integer, Value::Integer),
        map(expect_identifier("true"), |_| Value::Boolean(true)),
        map(expect_identifier("false"), |_| Value::Boolean(false)),
        map(expect_identifier("null"), |_| Value::Null),
    ))(tokens)
}

/// Parse string literal
fn parse_string_literal(tokens: &[Token]) -> IResult<&[Token], String> {
    if let Some((Token::String(s), rest)) = tokens.split_first() {
        Ok((rest, s.clone()))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse number (float)
fn parse_number(tokens: &[Token]) -> IResult<&[Token], f64> {
    if let Some((token, rest)) = tokens.split_first() {
        match token {
            Token::Integer(n) => Ok((rest, *n as f64)),
            Token::Float(f) => Ok((rest, *f)),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                tokens,
                nom::error::ErrorKind::Tag,
            ))),
        }
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse integer
fn parse_integer(tokens: &[Token]) -> IResult<&[Token], i64> {
    if let Some((Token::Integer(i), rest)) = tokens.split_first() {
        Ok((rest, *i))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            tokens,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Parse table name
fn parse_table_name(tokens: &[Token]) -> IResult<&[Token], String> {
    identifier(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_statement_with_where_clause() {
        let query = r#"CALL system.list_functions()
YIELD name, category, description
WHERE category = 'text' OR category = 'hybrid' OR category = 'fuzzy';"#;

        log::debug!("\n=== Testing WHERE clause parsing");
        let result = parse_query(query);

        assert!(result.is_ok(), "Query parsing should succeed");

        let doc = result.unwrap();
        if let Statement::Call(ref call_stmt) = doc.statement {
            log::debug!("Procedure: {}", call_stmt.procedure_name);
            log::debug!("Has YIELD: {}", call_stmt.yield_clause.is_some());
            log::debug!("Has WHERE: {}", call_stmt.where_clause.is_some());

            assert!(
                call_stmt.where_clause.is_some(),
                "WHERE clause should be captured!"
            );
        } else {
            panic!("Expected Call statement");
        }
    }
}

// ========================================================================
// CALL Statement Parser Tests
// ========================================================================
// Tests for CALL statement parser bug fixes:
// 1. 'description' keyword not recognized in YIELD clauses
// 2. IN operator not parsing parenthesized lists
// 3. Invalid CALL+RETURN syntax not rejected

#[test]
fn test_call_with_description_in_yield() {
    // Bug #1: 'description' is a keyword but should be allowed as column name in YIELD
    let query = r#"CALL system.list_functions()
YIELD name, category, description;"#;

    let result = parse_query(query);
    assert!(
        result.is_ok(),
        "Failed to parse CALL with 'description' in YIELD"
    );

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        assert_eq!(call_stmt.procedure_name, "system.list_functions");

        let yield_clause = call_stmt
            .yield_clause
            .as_ref()
            .expect("YIELD clause should be present");

        assert_eq!(yield_clause.items.len(), 3, "Should have 3 YIELD items");
        assert_eq!(yield_clause.items[0].column_name, "name");
        assert_eq!(yield_clause.items[1].column_name, "category");
        assert_eq!(yield_clause.items[2].column_name, "description");
    } else {
        panic!(
            "Expected CallStatement, got {:?}",
            std::mem::discriminant(&doc.statement)
        );
    }
}

#[test]
fn test_call_with_where_and_description() {
    // Bug #1 + WHERE extension: Ensure WHERE is parsed after 'description' in YIELD
    let query = r#"CALL system.list_functions()
YIELD name, category, description
WHERE category = 'string';"#;

    let result = parse_query(query);
    assert!(
        result.is_ok(),
        "Failed to parse CALL with WHERE after description"
    );

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        assert!(
            call_stmt.where_clause.is_some(),
            "WHERE clause should be present after description in YIELD"
        );

        let yield_clause = call_stmt
            .yield_clause
            .as_ref()
            .expect("YIELD clause should be present");
        assert_eq!(
            yield_clause.items.len(),
            3,
            "All 3 YIELD items should be parsed"
        );
    } else {
        panic!("Expected CallStatement");
    }
}

#[test]
fn test_call_where_in_with_list() {
    // Bug #2: IN operator should parse parenthesized lists like IN ('a', 'b', 'c')
    let query = r#"CALL system.list_functions()
YIELD name, category
WHERE category IN ('string', 'numeric', 'aggregate');"#;

    let result = parse_query(query);
    assert!(
        result.is_ok(),
        "Failed to parse WHERE IN with parenthesized list"
    );

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        let where_clause = call_stmt
            .where_clause
            .as_ref()
            .expect("WHERE clause should be present");

        // Check that the WHERE condition is a Binary expression with IN operator
        match &where_clause.condition {
            Expression::Binary(binary) => {
                assert_eq!(binary.operator, Operator::In, "Expected IN operator");

                // Check that right side is a List literal
                match &*binary.right {
                    Expression::Literal(Literal::List(list)) => {
                        assert_eq!(list.len(), 3, "Should have 3 items in IN list");
                    }
                    other => panic!("Expected Literal::List, got {:?}", other),
                }
            }
            other => panic!("Expected Binary expression with IN, got {:?}", other),
        }
    } else {
        panic!("Expected CallStatement");
    }
}

#[test]
fn test_call_where_in_simple() {
    // Bug #2: Simple IN test with 2 items
    let query = r#"CALL system.list_functions()
YIELD name
WHERE name IN ('UPPER', 'LOWER');"#;

    let result = parse_query(query);
    assert!(result.is_ok(), "Failed to parse simple WHERE IN");

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        assert!(
            call_stmt.where_clause.is_some(),
            "WHERE clause should be present"
        );
    } else {
        panic!("Expected CallStatement");
    }
}

#[test]
fn test_call_where_not_in() {
    // Bug #2: NOT IN should also work with parenthesized lists
    let query = r#"CALL system.list_functions()
YIELD name, category
WHERE category NOT IN ('aggregate', 'utility');"#;

    let result = parse_query(query);
    assert!(result.is_ok(), "Failed to parse WHERE NOT IN");

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        let where_clause = call_stmt
            .where_clause
            .as_ref()
            .expect("WHERE clause should be present");

        match &where_clause.condition {
            Expression::Binary(binary) => {
                assert_eq!(binary.operator, Operator::NotIn, "Expected NotIn operator");
            }
            other => panic!("Expected Binary expression with NOT IN, got {:?}", other),
        }
    } else {
        panic!("Expected CallStatement");
    }
}

#[test]
fn test_call_with_return_rejected() {
    // Bug #3: CALL+RETURN should be rejected as invalid syntax
    let query = r#"CALL system.list_functions()
YIELD name, category, description
WHERE category = 'string'
RETURN name;"#;

    let result = parse_query(query);
    assert!(
        result.is_err(),
        "Parser should reject CALL with RETURN clause"
    );

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("CALL statements cannot have additional clauses")
            || error_msg.contains("unexpected tokens")
            || error_msg.contains("Unexpected token"),
        "Error message should mention invalid CALL syntax, got: {}",
        error_msg
    );
}

#[test]
fn test_call_with_match_rejected() {
    // Bug #3: CALL+MATCH should also be rejected
    let query = r#"CALL system.list_functions()
YIELD name
MATCH (n);"#;

    let result = parse_query(query);
    assert!(
        result.is_err(),
        "Parser should reject CALL with MATCH clause"
    );
}

#[test]
fn test_call_complex_where_expression() {
    // Combined test: Complex WHERE with IN, OR, and description in YIELD
    let query = r#"CALL system.list_functions()
YIELD name, category, description
WHERE category IN ('string', 'numeric') OR category = 'aggregate';"#;

    let result = parse_query(query);
    assert!(result.is_ok(), "Failed to parse complex WHERE expression");

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        assert!(call_stmt.yield_clause.is_some(), "YIELD should be present");
        assert!(call_stmt.where_clause.is_some(), "WHERE should be present");

        // Verify it's an OR expression
        match &call_stmt.where_clause.as_ref().unwrap().condition {
            Expression::Binary(binary) => {
                assert_eq!(
                    binary.operator,
                    Operator::Or,
                    "Top-level should be OR operator"
                );
            }
            other => panic!("Expected OR binary expression, got {:?}", other),
        }
    } else {
        panic!("Expected CallStatement");
    }
}

#[test]
fn test_call_valid_without_where() {
    // Sanity test: CALL without WHERE should still work
    let query = r#"CALL system.list_functions()
YIELD name, category, description;"#;

    let result = parse_query(query);
    assert!(result.is_ok(), "Valid CALL without WHERE should parse");

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        assert!(call_stmt.yield_clause.is_some(), "YIELD should be present");
        assert!(
            call_stmt.where_clause.is_none(),
            "WHERE should not be present"
        );
    } else {
        panic!("Expected CallStatement");
    }
}

#[test]
fn test_call_valid_without_yield() {
    // Sanity test: CALL without YIELD should work
    let query = r#"CALL system.list_functions();"#;

    let result = parse_query(query);
    assert!(result.is_ok(), "Valid CALL without YIELD should parse");

    let doc = result.unwrap();
    if let Statement::Call(ref call_stmt) = doc.statement {
        assert!(
            call_stmt.yield_clause.is_none(),
            "YIELD should not be present"
        );
        assert!(
            call_stmt.where_clause.is_none(),
            "WHERE should not be present"
        );
    } else {
        panic!("Expected CallStatement");
    }
}

// ========================================================================
// Pattern and Complex Query Parser Tests
// Extracted from integration tests - tests lexer and parser for patterns
// ========================================================================

#[test]
fn test_lexer_basic_match_pattern() {
    let result = crate::ast::lexer::tokenize("MATCH (a:User)");
    assert!(result.is_ok(), "Basic MATCH pattern should tokenize");
}

#[test]
fn test_lexer_simple_edge_pattern() {
    let result = crate::ast::lexer::tokenize("MATCH (a)-[:NEXT]->(b) RETURN a");
    assert!(result.is_ok(), "Simple edge pattern should tokenize");
}

#[test]
fn test_lexer_complex_where_pattern() {
    let query = "MATCH (start:TestNode)-[:CONNECTS_TO]->(end:TestNode) WHERE start.id = 1 RETURN count(end) as connected_count";
    let result = crate::ast::lexer::tokenize(query);
    assert!(result.is_ok(), "Complex WHERE pattern should tokenize");
}

#[test]
fn test_lexer_variable_length_pattern() {
    let result = crate::ast::lexer::tokenize("-[:NEXT]{1,3}->");
    assert!(result.is_ok(), "Variable-length pattern should tokenize");
}

#[test]
fn test_parser_match_user_return() {
    let result = parse_query("MATCH (a:User) RETURN a");
    assert!(result.is_ok(), "MATCH User RETURN should parse");
}

#[test]
fn test_parser_match_with_label() {
    let result = parse_query("MATCH (node0:ChainNode) RETURN node0");
    assert!(result.is_ok(), "MATCH with label should parse");
}

#[test]
fn test_parser_simple_edge_pattern_no_quantifier() {
    let result = parse_query("MATCH (a)-[:NEXT]->(b) RETURN a");
    assert!(
        result.is_ok(),
        "Simple edge pattern without quantifier should parse"
    );
}

#[test]
fn test_parser_variable_length_edge_pattern() {
    let result = parse_query("MATCH (a)-[:NEXT]{1,3}->(b) RETURN a");
    assert!(result.is_ok(), "Variable-length edge pattern should parse");
}

#[test]
fn test_parser_variable_length_with_path_assignment() {
    let query = "MATCH path = (node0:ChainNode {id: 0})-[:NEXT]{1,3}->(node_end) RETURN count(path) as one_to_three_hop_paths";
    let result = parse_query(query);
    assert!(
        result.is_ok(),
        "Variable-length pattern with path assignment should parse"
    );
}

#[test]
fn test_parser_connects_to_pattern() {
    let query = "MATCH (start:TestNode)-[:CONNECTS_TO]->(end:TestNode) WHERE start.id = 1 RETURN count(end) as connected_count";
    let result = parse_query(query);
    assert!(result.is_ok(), "CONNECTS_TO pattern should parse");
}

#[test]
fn test_parser_pattern_comprehension() {
    let query = "MATCH (a:Account)
         RETURN a.account_number,
                [(a)-[t:Transaction]->(m) | t.amount] as transaction_amounts,
                [(a)-[t:Transaction]->(m) | m.category] as merchant_categories
         LIMIT 10";
    let result = parse_query(query);
    assert!(result.is_ok(), "Pattern comprehension should parse");
}

#[test]
fn test_parser_with_clause_aggregation() {
    let query = "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WITH a, m, count(t) as transaction_count, sum(t.amount) as total_spent
         WHERE transaction_count > 5
         MATCH (m)<-[:Transaction]-(other:Account)
         WHERE other <> a
         RETURN a.account_number,
                m.name,
                transaction_count,
                total_spent,
                count(DISTINCT other) as fellow_customers
         ORDER BY total_spent DESC
         LIMIT 10";
    let result = parse_query(query);
    assert!(
        result.is_ok(),
        "WITH clause with aggregation should parse. Error: {:?}",
        result.err()
    );
}

#[test]
fn test_parser_simple_with_clause() {
    let query = "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WITH a, m, count(t) as transaction_count
         RETURN a.account_number, transaction_count";
    let result = parse_query(query);
    assert!(result.is_ok(), "Simple WITH clause should parse");
}

#[test]
fn test_parser_with_where_clause() {
    let query = "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WITH a, m, count(t) as transaction_count
         WHERE transaction_count > 5
         RETURN a.account_number, transaction_count";
    let result = parse_query(query);
    assert!(result.is_ok(), "WITH + WHERE clause should parse");
}

#[test]
fn test_parser_with_then_match() {
    let query = "MATCH (a:Account)-[t:Transaction]->(m:Merchant)
         WITH a, m, count(t) as transaction_count
         MATCH (m)<-[:Transaction]-(other:Account)
         RETURN a.account_number, other.account_number";
    let result = parse_query(query);
    assert!(
        result.is_ok(),
        "WITH then MATCH should parse. Error: {:?}",
        result.err()
    );
}
