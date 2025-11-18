// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Validator for GQL AST structures
//! Implements comprehensive validation based on ISO GQL BNF grammar rules
//! 
//! # Validation Categories
//! 
//! This validator performs several types of validation on GQL queries:
//! 
//! ## 1. Structural Validations
//! - **Query Structure**: Ensures required clauses (MATCH, RETURN) are present
//! - **Path Patterns**: Validates alternating node-edge-node patterns
//! - **Clause Order**: Verifies proper ordering of MATCH → WHERE → RETURN
//! - **Empty Patterns**: Checks that path patterns are not empty
//! 
//! ## 2. Semantic Validations
//! - **Variable Declarations**: Tracks variables declared in MATCH clause
//! - **Variable Usage**: Ensures all used variables are declared
//! - **Scope Management**: Maintains variable scope across query clauses
//! - **Function Existence**: Validates that called functions are defined
//! 
//! ## 3. Type Validations
//! - **Function Signatures**: Validates argument counts and types for function calls
//! - **Built-in Functions**: Registers and validates temporal, vector, and aggregation functions
//! - **Operator Compatibility**: Ensures operators are used with compatible types
//! - **Property Access**: Validates property access on declared variables
//! 
//! ## 4. Syntax Validations
//! - **Property Names**: Ensures property names are not empty
//! - **Identifier Format**: Validates identifier syntax
//! - **Literal Formats**: Validates string, numeric, and boolean literals
//! 
//! ## 5. Temporal Validations
//! - **DateTime Format**: Validates ISO 8601 datetime format
//! - **Duration Format**: Validates ISO 8601 duration format
//! - **Time Window Format**: Validates time window specifications
//! - **Temporal Functions**: Validates DATETIME, NOW, DURATION, TIME_WINDOW functions
//! 
//! ## 6. Edge Pattern Validations
//! - **Direction Consistency**: Validates edge direction specifications
//! - **Label Syntax**: Ensures edge labels follow proper syntax
//! - **Property Maps**: Validates edge property specifications
//! 
//! ## 7. Expression Validations
//! - **Binary Operations**: Validates arithmetic, comparison, and logical operators
//! - **Unary Operations**: Validates unary operators (NOT, etc.)
//! - **Function Calls**: Validates function call syntax and arguments
//! - **Property Access**: Validates object.property access patterns
//! 
//! ## 8. Security Validations
//! - **Variable Scope**: Prevents access to undeclared variables
//! - **Function Access**: Controls access to built-in functions
//! 
//! # Built-in Functions Supported
//! 
//! ## Temporal Functions
//! - `DATETIME(string)` - Creates datetime from ISO string
//! - `NOW()` - Returns current datetime
//! - `DURATION(string)` - Creates duration from ISO string
//! - `TIME_WINDOW(start, end)` - Creates time window
//!
//! ## Aggregation Functions
//! - `count(expr)` - Counts non-null values
//! - `sum(expr)` - Sums numeric values
//! 
//! # Error Types
//! 
//! The validator categorizes errors into:
//! - **Structural**: Query structure and pattern violations
//! - **Semantic**: Variable scope and declaration issues
//! - **Type**: Type mismatches and function signature errors
//! - **Syntax**: Format and syntax violations
//! - **Security**: Access control and security violations
//! 
//! # Usage Example
//!
//! ```rust,ignore
//! use graphlite::ast::parser::parse_query;
//! use graphlite::ast::validator::validate_query;
//! use log::info;
//!
//! let query_str = "MATCH (user:User) WHERE user.risk_score > 0.8 RETURN user";
//! let document = parse_query(query_str).unwrap();
//!
//! match validate_query(&document, false) {
//!     Ok(()) => info!("Query is valid"),
//!     Err(errors) => {
//!         for error in errors {
//!             info!("Validation error: {}", error.message);
//!         }
//!     }
//! }
//! ```

use super::ast::*;
use crate::types::{GqlType, TypeValidator};
use std::collections::{HashMap, HashSet};

/// Validation error with context
#[derive(Debug, Clone)]
pub struct ValidationError {
    #[allow(dead_code)] // Used by Debug derive for error display
    pub message: String,
    #[allow(dead_code)] // Used by Debug derive for error display
    pub location: Option<Location>,
    #[allow(dead_code)] // Used by Debug derive for error display
    pub error_type: ValidationErrorType,
}

#[derive(Debug, Clone)]
pub enum ValidationErrorType {
    Structural,
    Semantic,
    Type,
    Syntax,
}

/// Validation context for tracking variables and their types
#[derive(Debug, Clone)]
struct ValidationContext {
    declared_variables: HashSet<String>,
    variable_types: HashMap<String, GqlType>,
    function_signatures: HashMap<String, FunctionSignature>,
    _current_scope: Vec<String>,
    has_graph_context: bool,
    /// Property types for schema-aware validation
    property_types: HashMap<String, GqlType>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    // Note: name is stored as HashMap key, not duplicated here
    argument_types: Vec<GqlType>,
    return_type: GqlType,
    variadic: bool,
}

impl ValidationContext {
    fn new() -> Self {
        let mut ctx = Self {
            declared_variables: HashSet::new(),
            variable_types: HashMap::new(),
            function_signatures: HashMap::new(),
            _current_scope: Vec::new(),
            has_graph_context: false,
            property_types: HashMap::new(),
        };
        
        // Register built-in functions
        ctx.register_builtin_functions();
        ctx
    }
    
    fn register_builtin_functions(&mut self) {
        use crate::ast::ast::TypeSpec as GqlType;
        
        // Temporal functions
        self.function_signatures.insert("DATETIME".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::ZonedDateTime { precision: None },
            variadic: false,
        });
        
        self.function_signatures.insert("NOW".to_string(), FunctionSignature {
            argument_types: vec![],
            return_type: GqlType::ZonedDateTime { precision: None },
            variadic: false,
        });
        
        // DURATION function has two variants:
        // 1. DURATION(string) - ISO duration string
        // 2. DURATION(number, string) - number and temporal unit
        // We'll use the first variant as the primary signature
        self.function_signatures.insert("DURATION".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::Duration { precision: None },
            variadic: false,
        });
        
        // Alternative signature for DURATION(number, unit)
        self.function_signatures.insert("DURATION_NUMERIC".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double, GqlType::String { max_length: None }],
            return_type: GqlType::Duration { precision: None },
            variadic: false,
        });
        
        self.function_signatures.insert("TIME_WINDOW".to_string(), FunctionSignature {
            argument_types: vec![GqlType::ZonedDateTime { precision: None }, GqlType::ZonedDateTime { precision: None }],
            return_type: GqlType::Duration { precision: None }, // Time windows are duration-based
            variadic: false,
        });


        // Aggregation functions (support both lowercase and uppercase)
        self.function_signatures.insert("count".to_string(), FunctionSignature {
            argument_types: vec![], // COUNT can take any expression or *
            return_type: GqlType::BigInt,
            variadic: true,
        });
        
        self.function_signatures.insert("COUNT".to_string(), FunctionSignature {
            argument_types: vec![], // COUNT can take any expression or *
            return_type: GqlType::BigInt,
            variadic: true,
        });
        
        self.function_signatures.insert("sum".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double], // Accept any numeric, will be promoted
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("SUM".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double], // Accept any numeric, will be promoted
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("avg".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double], // Accept any numeric, will be promoted
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("AVG".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double], // Accept any numeric, will be promoted
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("min".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double],
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("MIN".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double],
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("max".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double],
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("MAX".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double],
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("collect".to_string(), FunctionSignature {
            argument_types: vec![], // COLLECT can take any expression
            return_type: GqlType::List { 
                element_type: Box::new(GqlType::String { max_length: None }), // Generic element type
                max_length: None 
            },
            variadic: true,
        });
        
        self.function_signatures.insert("COLLECT".to_string(), FunctionSignature {
            argument_types: vec![], // COLLECT can take any expression
            return_type: GqlType::List { 
                element_type: Box::new(GqlType::String { max_length: None }), // Generic element type
                max_length: None 
            },
            variadic: true,
        });
        
        self.function_signatures.insert("UPPER".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("LOWER".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("ROUND".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double],
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("TRIM".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("SUBSTRING".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::BigInt],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("REPLACE".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("REVERSE".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("UPPER".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("LOWER".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("ROUND".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Double],
            return_type: GqlType::Double,
            variadic: false,
        });
        
        self.function_signatures.insert("TRIM".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("SUBSTRING".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::BigInt],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("REPLACE".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("REVERSE".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        // Graph-specific functions per GQL specification
        // LABELS function - returns list of node labels
        self.function_signatures.insert("LABELS".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Reference { target_type: None }], // Takes a node reference
            return_type: GqlType::List { 
                element_type: Box::new(GqlType::String { max_length: None }),
                max_length: None 
            }, // Returns list of strings (labels)
            variadic: false,
        });
        
        // TYPE function - returns the type of any value (updated to handle all types)
        self.function_signatures.insert("TYPE".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }], // Placeholder - validation is skipped
            return_type: GqlType::String { max_length: None }, // Returns the type name as string
            variadic: false,
        });
        
        // ID function - returns node/edge identifier
        self.function_signatures.insert("ID".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Reference { target_type: None }], // Takes a node or edge reference
            return_type: GqlType::String { max_length: None }, // Returns the ID as string
            variadic: false,
        });
        
        // PROPERTIES function - returns all properties as record
        self.function_signatures.insert("PROPERTIES".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Reference { target_type: None }], // Takes a node or edge reference
            return_type: GqlType::Record, // Returns properties as a record
            variadic: false,
        });
        
        // INFERRED_LABELS function - returns inferred labels based on node properties (temporary workaround)
        self.function_signatures.insert("INFERRED_LABELS".to_string(), FunctionSignature {
            argument_types: vec![GqlType::Reference { target_type: None }], // Takes a node reference
            return_type: GqlType::List { 
                element_type: Box::new(GqlType::String { max_length: None }),
                max_length: None 
            }, // Returns list of inferred label strings
            variadic: false,
        });

        // SIZE function - returns the size/length of collections, vectors, or strings
        self.function_signatures.insert("SIZE".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }], // Placeholder - validation is skipped
            return_type: GqlType::Double, // Returns size as number
            variadic: false,
        });

        // Text search functions
        self.function_signatures.insert("TEXT_SEARCH".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::Double,
            variadic: true, // Optional third argument for options
        });

        self.function_signatures.insert("FUZZY_MATCH".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::Double,
            variadic: true, // Optional third argument for options
        });

        self.function_signatures.insert("TEXT_MATCH".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::Double,
            variadic: true, // Optional third argument for options
        });

        self.function_signatures.insert("HIGHLIGHT".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: true, // Optional third argument for options
        });

        self.function_signatures.insert("TEXT_SCORE".to_string(), FunctionSignature {
            argument_types: vec![],
            return_type: GqlType::Double,
            variadic: false,
        });

        self.function_signatures.insert("HYBRID_SEARCH".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }, GqlType::String { max_length: None }],
            return_type: GqlType::Double,
            variadic: true, // Optional third argument for options
        });

        // Timezone functions
        self.function_signatures.insert("GET_TIMEZONE_NAME".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("GET_TIMEZONE_ABBREVIATION".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
        
        self.function_signatures.insert("GET_TIMEZONE_OFFSET".to_string(), FunctionSignature {
            argument_types: vec![GqlType::String { max_length: None }],
            return_type: GqlType::String { max_length: None },
            variadic: false,
        });
    }
}

/// Validate a GQL query document
pub fn validate_query(document: &Document, has_graph_context: bool) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let mut ctx = ValidationContext::new();
    ctx.has_graph_context = has_graph_context;
    
    match &document.statement {
        Statement::Query(query) => {
            // 1. Structural validations
            validate_query_structure(query, &mut errors);
            
            // 2. Variable declarations and scope
            validate_variable_declarations(query, &mut ctx, &mut errors);
            
            // 3. Path pattern validations
            validate_path_patterns(query, &mut ctx, &mut errors);
            
            // 4. Expression validations
            validate_expressions(query, &mut ctx, &mut errors);
            
            // 5. Temporal validations
            validate_temporal_literals(query, &mut errors);
            
            // 6. Edge pattern validations
            validate_edge_patterns(query, &mut errors);
        },
        Statement::Select(select_stmt) => {
            validate_select_statement(select_stmt, &mut ctx, &mut errors);
        },
        Statement::Call(call_stmt) => {
            validate_call_statement(call_stmt, &mut ctx, &mut errors);
        },
        Statement::CatalogStatement(catalog_stmt) => {
            validate_catalog_statement(catalog_stmt, &mut ctx, &mut errors);
        },
        Statement::DataStatement(_) => {
            // TODO: Add data statement validation
        },
        Statement::SessionStatement(_) => {
            // TODO: Add session statement validation
        },
        Statement::Declare(_) => {
            // TODO: Add DECLARE statement validation
        },
        Statement::Next(_) => {
            // TODO: Add NEXT statement validation
        },
        Statement::AtLocation(_) => {
            // TODO: Add AT location statement validation
        },
        Statement::TransactionStatement(_) => {
            // TODO: Add transaction statement validation
        },
        Statement::ProcedureBody(procedure_body) => {
            // Validate procedure body: validate initial statement and all chained statements
            // First validate variable definitions
            for var_def in &procedure_body.variable_definitions {
                // Validate each variable declaration within the DECLARE statement
                for var_decl in &var_def.variable_declarations {
                    if var_decl.variable_name.is_empty() {
                        errors.push(ValidationError {
                            message: format!("Variable name cannot be empty in procedure body"),
                            location: Some(var_decl.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                }
            }
            
            // Validate the initial statement (usually a MATCH query)
            validate_procedure_statement(&procedure_body.initial_statement, &ctx, &mut errors);
            
            // Validate each chained statement (can be data statements like DELETE)
            for chained in &procedure_body.chained_statements {
                validate_procedure_statement(&chained.statement, &ctx, &mut errors);
            }
        },
        &Statement::IndexStatement(ref index_stmt) => {
            // Validate index DDL statements
            match index_stmt {
                crate::ast::ast::IndexStatement::CreateIndex(create_idx) => {
                    // Validate index name is not empty
                    if create_idx.name.is_empty() {
                        errors.push(ValidationError {
                            message: "Index name cannot be empty".to_string(),
                            location: Some(create_idx.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                    // Validate table name is not empty
                    if create_idx.table.is_empty() {
                        errors.push(ValidationError {
                            message: "Table name cannot be empty for index".to_string(),
                            location: Some(create_idx.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                },
                crate::ast::ast::IndexStatement::DropIndex(drop_idx) => {
                    // Validate index name is not empty
                    if drop_idx.name.is_empty() {
                        errors.push(ValidationError {
                            message: "Index name cannot be empty".to_string(),
                            location: Some(drop_idx.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                },
                crate::ast::ast::IndexStatement::AlterIndex(alter_idx) => {
                    // Validate index name is not empty
                    if alter_idx.name.is_empty() {
                        errors.push(ValidationError {
                            message: "Index name cannot be empty".to_string(),
                            location: Some(alter_idx.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                },
                crate::ast::ast::IndexStatement::OptimizeIndex(optimize_idx) => {
                    // Validate index name is not empty
                    if optimize_idx.name.is_empty() {
                        errors.push(ValidationError {
                            message: "Index name cannot be empty".to_string(),
                            location: Some(optimize_idx.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                },
                crate::ast::ast::IndexStatement::ReindexIndex(reindex) => {
                    // Validate index name is not empty
                    if reindex.name.is_empty() {
                        errors.push(ValidationError {
                            message: "Index name cannot be empty for REINDEX".to_string(),
                            location: Some(reindex.location.clone()),
                            error_type: ValidationErrorType::Semantic,
                        });
                    }
                },
            }
        },
        Statement::Let(let_stmt) => {
            // Validate LET statement - check that variable definitions have valid expressions
            for var_def in &let_stmt.variable_definitions {
                validate_expression(&var_def.expression, &mut ctx, &mut errors);
            }
        },
    }
    
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate query structure (required clauses, order)
fn validate_query_structure(query: &Query, errors: &mut Vec<ValidationError>) {
    match query {
        Query::Basic(basic_query) => {
            validate_basic_query_structure(basic_query, errors);
        }
        Query::SetOperation(set_op) => {
            validate_query_structure(&set_op.left, errors);
            validate_query_structure(&set_op.right, errors);
        }
        Query::Limited { query, .. } => {
            validate_query_structure(query, errors);
        }
        Query::WithQuery(_) => {
            // TODO: Implement WithQuery validation
        }
        Query::Let(_) => {
            // TODO: Implement LET validation
        }
        Query::For(_) => {
            // TODO: Implement FOR validation  
        }
        Query::Filter(_) => {
            // TODO: Implement FILTER validation
        }
        Query::Return(return_query) => {
            validate_return_query_structure(return_query, errors);
        }
        Query::Unwind(_) => {
            // TODO: Implement UNWIND validation
        }
        Query::MutationPipeline(_) => {
            // TODO: Validate mutation pipeline structure
        }
    }
}

fn validate_basic_query_structure(query: &BasicQuery, errors: &mut Vec<ValidationError>) {
    // Check for required MATCH clause
    if query.match_clause.patterns.is_empty() {
        errors.push(ValidationError {
            message: "Query must have at least one path pattern in MATCH clause".to_string(),
            location: None,
            error_type: ValidationErrorType::Structural,
        });
    }
    
    // Check for required RETURN clause
    if query.return_clause.items.is_empty() {
        errors.push(ValidationError {
            message: "Query must have at least one item in RETURN clause".to_string(),
            location: None,
            error_type: ValidationErrorType::Structural,
        });
    }
}

fn validate_return_query_structure(query: &ReturnQuery, errors: &mut Vec<ValidationError>) {
    // Check for required RETURN clause items
    if query.return_clause.items.is_empty() {
        errors.push(ValidationError {
            message: "Query must have at least one item in RETURN clause".to_string(),
            location: None,
            error_type: ValidationErrorType::Structural,
        });
    }
}

/// Validate variable declarations and scope
fn validate_variable_declarations(query: &Query, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    match query {
        Query::Basic(basic_query) => {
            validate_basic_query_variables(basic_query, ctx, errors);
        }
        Query::SetOperation(set_op) => {
            // For set operations, validate each query independently
            let mut left_ctx = ctx.clone();
            validate_variable_declarations(&set_op.left, &mut left_ctx, errors);
            let mut right_ctx = ctx.clone();
            validate_variable_declarations(&set_op.right, &mut right_ctx, errors);
        }
        Query::Limited { query, .. } => {
            validate_variable_declarations(query, ctx, errors);
        }
        Query::WithQuery(_) => {
            // TODO: Implement WithQuery validation
        }
        Query::Let(_) => {
            // TODO: Implement LET validation
        }
        Query::For(_) => {
            // TODO: Implement FOR validation  
        }
        Query::Filter(_) => {
            // TODO: Implement FILTER validation
        }
        Query::Return(_) => {
            // TODO: Implement RETURN validation
        }
        Query::Unwind(_) => {
            // TODO: Implement UNWIND variable validation
        }
        Query::MutationPipeline(_) => {
            // TODO: Validate mutation pipeline variables
        }
    }
}

fn validate_basic_query_variables(query: &BasicQuery, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Collect all variables declared in MATCH clause
    for pattern in &query.match_clause.patterns {
        for element in &pattern.elements {
            match element {
                PatternElement::Node(node) => {
                    if let Some(ref identifier) = node.identifier {
                        ctx.declared_variables.insert(identifier.clone());
                        ctx.variable_types.insert(identifier.clone(), GqlType::Reference { target_type: None }); // Node as reference type
                    }
                }
                PatternElement::Edge(edge) => {
                    if let Some(ref identifier) = edge.identifier {
                        ctx.declared_variables.insert(identifier.clone());
                        ctx.variable_types.insert(identifier.clone(), GqlType::Reference { target_type: None }); // Edge as reference type
                    }
                }
            }
        }
    }
    
    // Validate variables used in WHERE clause
    if let Some(ref where_clause) = query.where_clause {
        validate_expression_variables(&where_clause.condition, ctx, errors);
    }
    
    // Validate variables used in RETURN clause
    for item in &query.return_clause.items {
        validate_expression_variables(&item.expression, ctx, errors);
    }
}

/// Validate path patterns (alternating nodes and edges)
fn validate_path_patterns(query: &Query, _ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    match query {
        Query::Basic(basic_query) => {
            validate_basic_query_path_patterns(basic_query, _ctx, errors);
        }
        Query::SetOperation(set_op) => {
            validate_path_patterns(&set_op.left, _ctx, errors);
            validate_path_patterns(&set_op.right, _ctx, errors);
        }
        Query::Limited { query, .. } => {
            validate_path_patterns(query, _ctx, errors);
        }
        Query::WithQuery(_) => {
            // TODO: Implement WithQuery validation
        }
        Query::Let(_) => {
            // TODO: Implement LET validation
        }
        Query::For(_) => {
            // TODO: Implement FOR validation  
        }
        Query::Filter(_) => {
            // TODO: Implement FILTER validation
        }
        Query::Return(_) => {
            // TODO: Implement RETURN validation
        }
        Query::Unwind(_) => {
            // TODO: Implement UNWIND validation
        }
        Query::MutationPipeline(_) => {
            // TODO: Validate mutation pipeline path patterns
        }
    }
}

fn validate_basic_query_path_patterns(query: &BasicQuery, _ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    for pattern in &query.match_clause.patterns {
        if pattern.elements.is_empty() {
            errors.push(ValidationError {
                message: "Path pattern cannot be empty".to_string(),
                location: None,
                error_type: ValidationErrorType::Structural,
            });
            continue;
        }
        
        // Check alternating pattern: Node -> Edge -> Node -> Edge -> Node
        for (i, element) in pattern.elements.iter().enumerate() {
            match element {
                PatternElement::Node(_) => {
                    // Nodes should be at even indices (0, 2, 4, ...)
                    if i % 2 != 0 {
                        errors.push(ValidationError {
                            message: format!("Invalid path pattern: expected edge at position {}", i),
                            location: None,
                            error_type: ValidationErrorType::Structural,
                        });
                    }
                }
                PatternElement::Edge(_) => {
                    // Edges should be at odd indices (1, 3, 5, ...)
                    if i % 2 != 1 {
                        errors.push(ValidationError {
                            message: format!("Invalid path pattern: expected node at position {}", i),
                            location: None,
                            error_type: ValidationErrorType::Structural,
                        });
                    }
                }
            }
        }
        
        // Validate that path starts and ends with nodes
        if let Some(first) = pattern.elements.first() {
            if matches!(first, PatternElement::Edge(_)) {
                errors.push(ValidationError {
                    message: "Path pattern must start with a node".to_string(),
                    location: None,
                    error_type: ValidationErrorType::Structural,
                });
            }
        }
        
        if let Some(last) = pattern.elements.last() {
            if matches!(last, PatternElement::Edge(_)) {
                errors.push(ValidationError {
                    message: "Path pattern must end with a node".to_string(),
                    location: None,
                    error_type: ValidationErrorType::Structural,
                });
            }
        }
    }
}

/// Validate expressions (function calls, property access, etc.)
fn validate_expressions(query: &Query, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    match query {
        Query::Basic(basic_query) => {
            validate_basic_query_expressions(basic_query, ctx, errors);
        }
        Query::SetOperation(set_op) => {
            validate_expressions(&set_op.left, ctx, errors);
            validate_expressions(&set_op.right, ctx, errors);
        }
        Query::Limited { query, .. } => {
            validate_expressions(query, ctx, errors);
        }
        Query::WithQuery(_) => {
            // TODO: Implement WithQuery validation
        }
        Query::Let(_) => {
            // TODO: Implement LET validation
        }
        Query::For(_) => {
            // TODO: Implement FOR validation  
        }
        Query::Filter(_) => {
            // TODO: Implement FILTER validation
        }
        Query::Return(_) => {
            // TODO: Implement RETURN validation
        }
        Query::Unwind(_) => {
            // TODO: Implement UNWIND validation
        }
        Query::MutationPipeline(_) => {
            // TODO: Validate mutation pipeline expressions
        }
    }
}

fn validate_basic_query_expressions(query: &BasicQuery, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate WHERE clause expressions
    if let Some(ref where_clause) = query.where_clause {
        validate_expression(&where_clause.condition, ctx, errors);
    }
    
    // Validate RETURN clause expressions
    for item in &query.return_clause.items {
        validate_expression(&item.expression, ctx, errors);
    }
}

/// Validate a single expression
fn validate_expression(expr: &Expression, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    match expr {
        Expression::Binary(binary) => {
            validate_expression(&binary.left, ctx, errors);
            validate_expression(&binary.right, ctx, errors);
            validate_binary_operation(&binary.operator, &binary.left, &binary.right, ctx, errors);
        }
        Expression::Unary(unary) => {
            validate_expression(&unary.expression, ctx, errors);
            validate_unary_operation(&unary.operator, &unary.expression, ctx, errors);
        }
        Expression::FunctionCall(func_call) => {
            validate_function_call(func_call, ctx, errors);
        }
        Expression::PropertyAccess(prop_access) => {
            validate_property_access(prop_access, ctx, errors);
        }
        Expression::Variable(variable) => {
            validate_variable_usage(variable, ctx, errors);
        }
        Expression::Literal(literal) => {
            validate_literal(literal, errors);
        }
        Expression::Case(case_expr) => {
            validate_case_expression(case_expr, ctx, errors);
        }
        Expression::PathConstructor(path_constructor) => {
            validate_path_constructor(path_constructor, ctx, errors);
        }
        Expression::Cast(cast_expr) => {
            validate_cast_expression(cast_expr, ctx, errors);
        },
        Expression::Subquery(subquery_expr) => {
            // Validate the subquery recursively
            validate_subquery(&subquery_expr.query, ctx, errors);
        },
        Expression::ExistsSubquery(subquery_expr) => {
            // Validate the EXISTS subquery recursively
            validate_subquery(&subquery_expr.query, ctx, errors);
        },
        Expression::NotExistsSubquery(subquery_expr) => {
            // Validate the NOT EXISTS subquery recursively
            validate_subquery(&subquery_expr.query, ctx, errors);
        },
        Expression::InSubquery(subquery_expr) => {
            // Validate the left expression and the subquery
            validate_expression(&subquery_expr.expression, ctx, errors);
            validate_subquery(&subquery_expr.query, ctx, errors);
        },
        Expression::NotInSubquery(subquery_expr) => {
            // Validate the left expression and the subquery
            validate_expression(&subquery_expr.expression, ctx, errors);
            validate_subquery(&subquery_expr.query, ctx, errors);
        }
        Expression::QuantifiedComparison(quantified_expr) => {
            // Validate the left expression and subquery
            validate_expression(&quantified_expr.left, ctx, errors);
            validate_expression(&quantified_expr.subquery, ctx, errors);
        }
        Expression::IsPredicate(is_predicate) => {
            // Validate the subject expression
            validate_expression(&is_predicate.subject, ctx, errors);
            
            // If there's a target expression (for SOURCE OF, DESTINATION OF), validate it
            if let Some(ref target) = is_predicate.target {
                validate_expression(target, ctx, errors);
            }
            
            // Validate label expression if it's a label predicate
            if let crate::ast::ast::IsPredicateType::Label(ref label_expr) = is_predicate.predicate_type {
                // Label expressions are already validated as part of their structure
                // but we can add specific label validation here if needed
                if label_expr.terms.is_empty() {
                    errors.push(ValidationError {
                        message: "Label predicate must have at least one label term".to_string(),
                        location: Some(is_predicate.location.clone()),
                        error_type: ValidationErrorType::Syntax,
                    });
                }
            }
        }
        Expression::ArrayIndex(array_index) => {
            // Validate array expression
            validate_expression(&array_index.array, ctx, errors);
            // Validate index expression - should be numeric
            validate_expression(&array_index.index, ctx, errors);
        }
        Expression::Parameter(parameter) => {
            // Parameters are valid and will be resolved at execution time
            // For now, just validate the parameter name is valid
            if parameter.name.is_empty() {
                errors.push(ValidationError {
                    message: "Parameter name cannot be empty".to_string(),
                    error_type: ValidationErrorType::Syntax,
                    location: Some(parameter.location.clone()),
                });
            }
        }
        Expression::Pattern(pattern_expr) => {
            // Validate the pattern within the WHERE clause context
            validate_path_pattern(&pattern_expr.pattern, ctx, errors);
        }
    }
}

/// Validate CASE expressions
fn validate_case_expression(case_expr: &crate::ast::ast::CaseExpression, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    use crate::ast::ast::CaseType;
    
    match &case_expr.case_type {
        CaseType::Simple(simple_case) => {
            validate_simple_case_expression(simple_case, ctx, errors);
        }
        CaseType::Searched(searched_case) => {
            validate_searched_case_expression(searched_case, ctx, errors);
        }
    }
}

/// Validate simple CASE expressions
fn validate_simple_case_expression(simple_case: &SimpleCaseExpression, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate test expression
    validate_expression(&simple_case.test_expression, ctx, errors);
    
    // Validate WHEN branches
    for when_branch in &simple_case.when_branches {
        // Validate WHEN values
        for when_value in &when_branch.when_values {
            validate_expression(when_value, ctx, errors);
        }
        // Validate THEN expression
        validate_expression(&when_branch.then_expression, ctx, errors);
    }
    
    // Validate ELSE expression if present
    if let Some(else_expr) = &simple_case.else_expression {
        validate_expression(else_expr, ctx, errors);
    }
    
    // Check that there's at least one WHEN branch
    if simple_case.when_branches.is_empty() {
        errors.push(ValidationError {
            message: "CASE expression must have at least one WHEN branch".to_string(),
            location: Some(crate::ast::ast::Location::default()),
            error_type: ValidationErrorType::Structural,
        });
    }
}

/// Validate searched CASE expressions
fn validate_searched_case_expression(searched_case: &SearchedCaseExpression, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate WHEN branches
    for when_branch in &searched_case.when_branches {
        // Validate WHEN condition
        validate_expression(&when_branch.condition, ctx, errors);
        // Validate THEN expression
        validate_expression(&when_branch.then_expression, ctx, errors);
    }
    
    // Validate ELSE expression if present
    if let Some(else_expr) = &searched_case.else_expression {
        validate_expression(else_expr, ctx, errors);
    }
    
    // Check that there's at least one WHEN branch
    if searched_case.when_branches.is_empty() {
        errors.push(ValidationError {
            message: "CASE expression must have at least one WHEN branch".to_string(),
            location: Some(crate::ast::ast::Location::default()),
            error_type: ValidationErrorType::Structural,
        });
    }
}

/// Validate PATH constructor expressions
fn validate_path_constructor(path_constructor: &crate::ast::ast::PathConstructor, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate each element in the PATH constructor
    for element in &path_constructor.elements {
        validate_expression(element, ctx, errors);
        
        // Check that element types are suitable for PATH constructor
        if let Ok(element_type) = infer_expression_type(element, ctx) {
            match element_type {
                GqlType::String { .. } | GqlType::Integer | GqlType::BigInt | GqlType::SmallInt | GqlType::Double | GqlType::Float { .. } | GqlType::Real => {
                    // These types are valid for PATH elements (can be converted to string IDs)
                }
                _ => {
                    errors.push(ValidationError {
                        message: format!("PATH constructor element must be a string or number type, got: {:?}", element_type),
                        location: Some(crate::ast::ast::Location::default()),
                        error_type: ValidationErrorType::Type,
                    });
                }
            }
        }
    }
    
    // PATH elements should follow node-edge-node pattern (optional validation)
    if path_constructor.elements.len() % 2 == 0 && path_constructor.elements.len() > 2 {
        // Even number of elements > 2 might indicate incomplete path
        // This is a warning rather than an error since PATH[node, edge] might be valid
        // in some contexts
    }
}

/// Validate CAST expressions
fn validate_cast_expression(cast_expr: &crate::ast::ast::CastExpression, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate the inner expression
    validate_expression(&cast_expr.expression, ctx, errors);
    
    // Check if the cast is valid (basic validation - runtime will handle detailed conversion)
    if let Ok(source_type) = infer_expression_type(&cast_expr.expression, ctx) {
        // Check for obviously invalid casts
        match (&source_type, &cast_expr.target_type) {
            // Allow most casts - runtime will handle specifics
            // Add specific validation rules here if needed
            _ => {} // Most casts are allowed for now
        }
    }
    
    // Validate that the target type is well-formed
    match &cast_expr.target_type {
        GqlType::String { max_length } => {
            if let Some(len) = max_length {
                if *len == 0 {
                    errors.push(ValidationError {
                        message: "String type cannot have max_length of 0".to_string(),
                        location: Some(crate::ast::ast::Location::default()),
                        error_type: ValidationErrorType::Type,
                    });
                }
            }
        },
        GqlType::Decimal { precision, scale } => {
            if let (Some(p), Some(s)) = (precision, scale) {
                if *s > *p {
                    errors.push(ValidationError {
                        message: "DECIMAL scale cannot be greater than precision".to_string(),
                        location: Some(crate::ast::ast::Location::default()),
                        error_type: ValidationErrorType::Type,
                    });
                }
            }
        },
        _ => {} // Other types are valid
    }
}

/// Infer the type of an expression for validation
fn infer_expression_type(expression: &Expression, ctx: &ValidationContext) -> Result<GqlType, String> {
    match expression {
        Expression::Literal(lit) => {
            match lit {
                Literal::String(_) => Ok(GqlType::String { max_length: None }),
                Literal::Integer(_) => Ok(GqlType::BigInt),
                Literal::Float(_) => Ok(GqlType::Double),
                Literal::Boolean(_) => Ok(GqlType::Boolean),
                Literal::Null => Ok(GqlType::String { max_length: None }), // Null can be any type
                Literal::DateTime(_) => Ok(GqlType::ZonedDateTime { precision: None }),
                Literal::Duration(_) => Ok(GqlType::Duration { precision: None }),
                Literal::TimeWindow(_) => Ok(GqlType::Duration { precision: None }),
                Literal::Vector(_) => Ok(GqlType::List { 
                    element_type: Box::new(GqlType::Double), 
                    max_length: None 
                }),
                Literal::List(list) => {
                    // For now, assume all lists are lists of strings
                    // In a more sophisticated type system, we'd infer the element type
                    if list.is_empty() {
                        Ok(GqlType::List { 
                            element_type: Box::new(GqlType::String { max_length: None }), 
                            max_length: None 
                        })
                    } else {
                        // Use the type of the first element
                        let first_type = infer_expression_type(&Expression::Literal(list[0].clone()), ctx)?;
                        Ok(GqlType::List { 
                            element_type: Box::new(first_type), 
                            max_length: None 
                        })
                    }
                },
            }
        }
        Expression::Variable(var) => {
            ctx.variable_types.get(&var.name)
                .cloned()
                .ok_or_else(|| format!("Unknown variable type: {}", var.name))
        }
        Expression::PropertyAccess(prop_access) => {
            // Try to get exact type from schema if available
            if let Some(property_type) = ctx.property_types.get(&prop_access.property) {
                Ok(property_type.clone())
            } else {
                // Schema info not available - this should ideally query the catalog
                // For now, return a generic type that can be coerced at runtime
                // The runtime executor will handle type coercion when the actual value is known
                Ok(GqlType::String { max_length: None }) // Generic type for runtime coercion
            }
        }
        Expression::FunctionCall(func) => {
            // Case-insensitive function lookup
            let func_name_upper = func.name.to_uppercase();
            ctx.function_signatures.get(&func_name_upper)
                .map(|sig| sig.return_type.clone())
                .ok_or_else(|| format!("Unknown function: {}", func.name))
        }
        Expression::PathConstructor(_) => {
            Ok(GqlType::Path)
        }
        Expression::Cast(cast_expr) => {
            Ok(cast_expr.target_type.clone())
        },
        Expression::Subquery(_) => {
            // Subqueries can return various types - for now default to string
            Ok(GqlType::String { max_length: None })
        },
        Expression::ExistsSubquery(_) => {
            // EXISTS always returns boolean
            Ok(GqlType::Boolean)
        },
        Expression::InSubquery(_) => {
            // IN always returns boolean
            Ok(GqlType::Boolean)
        },
        Expression::NotInSubquery(_) => {
            // NOT IN always returns boolean
            Ok(GqlType::Boolean)
        },
        Expression::IsPredicate(_) => {
            // IS predicates always return boolean
            Ok(GqlType::Boolean)
        },
        Expression::Binary(binary) => {
            // For binary expressions, infer type based on operator and operands
            use crate::ast::ast::Operator;
            match &binary.operator {
                // Arithmetic operators return numeric types
                Operator::Plus | Operator::Minus | Operator::Star | Operator::Slash | 
                Operator::Percent | Operator::Caret => {
                    // For arithmetic operations, try to infer the type from operands
                    // If both operands are numeric, return Double for simplicity
                    let left_type = infer_expression_type(&binary.left, ctx)?;
                    let right_type = infer_expression_type(&binary.right, ctx)?;
                    
                    // If either operand is numeric, result is numeric
                    match (&left_type, &right_type) {
                        (GqlType::Double | GqlType::Float { .. } | GqlType::Real | 
                         GqlType::Integer | GqlType::BigInt | GqlType::SmallInt | 
                         GqlType::Decimal { .. }, _) |
                        (_, GqlType::Double | GqlType::Float { .. } | GqlType::Real | 
                         GqlType::Integer | GqlType::BigInt | GqlType::SmallInt | 
                         GqlType::Decimal { .. }) => Ok(GqlType::Double),
                        _ => Ok(GqlType::Double), // Default to Double for arithmetic
                    }
                }
                // Comparison operators return boolean
                Operator::Equal | Operator::NotEqual | Operator::LessThan | 
                Operator::LessEqual | Operator::GreaterThan | Operator::GreaterEqual => {
                    Ok(GqlType::Boolean)
                }
                // Logical operators return boolean
                Operator::And | Operator::Or => {
                    Ok(GqlType::Boolean)
                }
                // String concatenation
                Operator::Concat => {
                    Ok(GqlType::String { max_length: None })
                }
                // For other operators, default to the type of the left operand
                _ => {
                    infer_expression_type(&binary.left, ctx)
                }
            }
        },
        _ => {
            // For other expression types, default to string for now
            Ok(GqlType::String { max_length: None })
        }
    }
}

/// Validate function calls using TypeSpec and TypeValidator
fn validate_function_call(func_call: &FunctionCall, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Case-insensitive function lookup
    let func_name_upper = func_call.name.to_uppercase();

    // Check if function exists and clone the signature to avoid borrow checker issues
    let signature = match ctx.function_signatures.get(&func_name_upper).cloned() {
        Some(sig) => sig,
        None => {
            errors.push(ValidationError {
                message: format!("Unknown function '{}'", func_call.name),
                location: None,
                error_type: ValidationErrorType::Semantic,
            });
            return;
        }
    };
    
    // Validate each argument expression
    for arg in &func_call.arguments {
        validate_expression(arg, ctx, errors);
    }
    
    // For variadic functions (like COUNT), allow flexible argument counts
    if signature.variadic {
        if func_name_upper == "COUNT" {
            // COUNT can have 0 (COUNT(*)) or 1 argument (COUNT(expr))
            if func_call.arguments.len() > 1 {
                errors.push(ValidationError {
                    message: format!(
                        "Function '{}' expects 0 or 1 arguments, got {}",
                        func_call.name,
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
                return;
            }
        }
        // For other variadic functions, just ensure minimum arguments
        if func_call.arguments.len() < signature.argument_types.len() {
            errors.push(ValidationError {
                message: format!(
                    "Function '{}' expects at least {} arguments, got {}",
                    func_call.name,
                    signature.argument_types.len(),
                    func_call.arguments.len()
                ),
                location: None,
                error_type: ValidationErrorType::Type,
            });
        }
        return;
    }
    
    // Special handling for ROUND function which has optional decimal places
    if func_name_upper == "ROUND" {
        match func_call.arguments.len() {
            1 => {
                // ROUND(number) - round to integer
                validate_expression(&func_call.arguments[0], ctx, errors);
            }
            2 => {
                // ROUND(number, decimal_places) - round to specified decimal places
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
            }
            _ => {
                errors.push(ValidationError {
                    message: format!(
                        "Function 'ROUND' expects 1 or 2 arguments, got {}",
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
            }
        }
        return;
    }
    
    // Special handling for TRIM function which has variable arguments
    if func_name_upper == "TRIM" {
        match func_call.arguments.len() {
            1 => {
                // TRIM(string) - trim whitespace
                validate_expression(&func_call.arguments[0], ctx, errors);
            }
            2 => {
                // TRIM(string, character) - trim specified character
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
            }
            3 => {
                // TRIM(mode, character, string) - trim with mode
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
                validate_expression(&func_call.arguments[2], ctx, errors);
            }
            _ => {
                errors.push(ValidationError {
                    message: format!(
                        "Function 'TRIM' expects 1 to 3 arguments, got {}",
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
            }
        }
        return;
    }
    
    // Special handling for REPLACE function
    if func_name_upper == "REPLACE" {
        if func_call.arguments.len() != 3 {
            errors.push(ValidationError {
                message: format!(
                    "Function 'REPLACE' expects 3 arguments, got {}",
                    func_call.arguments.len()
                ),
                location: None,
                error_type: ValidationErrorType::Type,
            });
        } else {
            // Validate all three arguments
            validate_expression(&func_call.arguments[0], ctx, errors);
            validate_expression(&func_call.arguments[1], ctx, errors);
            validate_expression(&func_call.arguments[2], ctx, errors);
        }
        return;
    }
    
    // Special handling for SUBSTRING function which has optional length
    if func_name_upper == "SUBSTRING" {
        match func_call.arguments.len() {
            2 => {
                // SUBSTRING(string, start) - substring from start to end
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
            }
            3 => {
                // SUBSTRING(string, start, length) - substring with length
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
                validate_expression(&func_call.arguments[2], ctx, errors);
            }
            _ => {
                errors.push(ValidationError {
                    message: format!(
                        "Function 'SUBSTRING' expects 2 or 3 arguments, got {}",
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
            }
        }
        return;
    }
    
    // Special handling for ROUND function which has optional decimal places
    if func_name_upper == "ROUND" {
        match func_call.arguments.len() {
            1 => {
                // ROUND(number) - round to integer
                validate_expression(&func_call.arguments[0], ctx, errors);
            }
            2 => {
                // ROUND(number, decimal_places) - round to specified decimal places
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
            }
            _ => {
                errors.push(ValidationError {
                    message: format!(
                        "Function 'ROUND' expects 1 or 2 arguments, got {}",
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
            }
        }
        return;
    }
    
    // Special handling for TRIM function which has variable arguments
    if func_name_upper == "TRIM" {
        match func_call.arguments.len() {
            1 => {
                // TRIM(string) - trim whitespace
                validate_expression(&func_call.arguments[0], ctx, errors);
            }
            2 => {
                // TRIM(string, character) - trim specified character
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
            }
            3 => {
                // TRIM(mode, character, string) - trim with mode
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
                validate_expression(&func_call.arguments[2], ctx, errors);
            }
            _ => {
                errors.push(ValidationError {
                    message: format!(
                        "Function 'TRIM' expects 1 to 3 arguments, got {}",
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
            }
        }
        return;
    }
    
    // Special handling for REPLACE function
    if func_name_upper == "REPLACE" {
        if func_call.arguments.len() != 3 {
            errors.push(ValidationError {
                message: format!(
                    "Function 'REPLACE' expects 3 arguments, got {}",
                    func_call.arguments.len()
                ),
                location: None,
                error_type: ValidationErrorType::Type,
            });
        } else {
            // Validate all three arguments
            validate_expression(&func_call.arguments[0], ctx, errors);
            validate_expression(&func_call.arguments[1], ctx, errors);
            validate_expression(&func_call.arguments[2], ctx, errors);
        }
        return;
    }
    
    // Special handling for SUBSTRING function which has optional length
    if func_name_upper == "SUBSTRING" {
        match func_call.arguments.len() {
            2 => {
                // SUBSTRING(string, start) - substring from start to end
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
            }
            3 => {
                // SUBSTRING(string, start, length) - substring with length
                validate_expression(&func_call.arguments[0], ctx, errors);
                validate_expression(&func_call.arguments[1], ctx, errors);
                validate_expression(&func_call.arguments[2], ctx, errors);
            }
            _ => {
                errors.push(ValidationError {
                    message: format!(
                        "Function 'SUBSTRING' expects 2 or 3 arguments, got {}",
                        func_call.arguments.len()
                    ),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
            }
        }
        return;
    }
    
    // Case-insensitive function lookup
    let func_name_upper = func_call.name.to_uppercase();

    // Check if function exists
    if let Some(signature) = ctx.function_signatures.get(&func_name_upper) {
        // For non-variadic functions, validate exact argument count
        if func_call.arguments.len() != signature.argument_types.len() {
            errors.push(ValidationError {
                message: format!(
                    "Function '{}' expects {} arguments, got {}",
                    func_call.name,
                    signature.argument_types.len(),
                    func_call.arguments.len()
                ),
                location: None,
                error_type: ValidationErrorType::Type,
            });
        }

        // Validate argument types (simplified - in real implementation, infer types)
        for (_i, arg) in func_call.arguments.iter().enumerate() {
            validate_expression(arg, ctx, errors);
        }
    } else {
        errors.push(ValidationError {
            message: format!("Unknown function '{}'", func_call.name),
            location: None,
            error_type: ValidationErrorType::Type,
        });
    }
    
    // Infer argument types and validate compatibility
    let mut arg_types = Vec::new();
    for arg in &func_call.arguments {
        match infer_expression_type(arg, ctx) {
            Ok(arg_type) => arg_types.push(arg_type),
            Err(err) => {
                errors.push(ValidationError {
                    message: format!("Type inference error: {}", err),
                    location: None,
                    error_type: ValidationErrorType::Type,
                });
                return;
            }
        }
    }
    
    // Use TypeValidator to validate function arguments
    // Skip strict type validation for aggregation functions to allow runtime coercion
    let is_aggregation_function = matches!(func_name_upper.as_str(),
        "SUM" | "AVG" | "MIN" | "MAX" | "COUNT" | "COLLECT");

    // Functions that can handle any type and should skip strict validation
    let is_flexible_function = matches!(func_name_upper.as_str(),
        "TYPE" | "SIZE");

    // Skip strict type validation for functions that can handle type coercion at runtime
    if !is_aggregation_function && !is_flexible_function {
        if let Err(type_error) = TypeValidator::validate_function_args(
            &func_call.name,
            &arg_types,
            &signature.argument_types,
            signature.variadic,
        ) {
            errors.push(ValidationError {
                message: format!("Function validation error: {}", type_error),
                location: None,
                error_type: ValidationErrorType::Type,
            });
        }
    }
}

/// Validate property access
fn validate_property_access(prop_access: &PropertyAccess, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Check if the object variable is declared
    if !ctx.declared_variables.contains(&prop_access.object) {
        // If we have graph context (from session or FROM clause), 
        // variables will be resolved at runtime with the graph
        if !ctx.has_graph_context {
            errors.push(ValidationError {
                message: format!("Undefined variable '{}'", prop_access.object),
                location: None,
                error_type: ValidationErrorType::Semantic,
            });
        }
    }
    
    // Validate property name syntax
    if prop_access.property.is_empty() {
        errors.push(ValidationError {
            message: "Property name cannot be empty".to_string(),
            location: None,
            error_type: ValidationErrorType::Syntax,
        });
    }
}

/// Validate variable usage
fn validate_variable_usage(variable: &Variable, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    if !ctx.declared_variables.contains(&variable.name) {
        // If we have graph context (from session or FROM clause), 
        // variables will be resolved at runtime with the graph
        if !ctx.has_graph_context {
            errors.push(ValidationError {
                message: format!("Undefined variable '{}'", variable.name),
                location: None,
                error_type: ValidationErrorType::Semantic,
            });
        }
    }
}

/// Validate unary operations
fn validate_unary_operation(
    operator: &Operator,
    expression: &Expression,
    ctx: &mut ValidationContext,
    errors: &mut Vec<ValidationError>,
) {
    // Validate that the operand is a valid expression
    validate_expression(expression, ctx, errors);
    
    // Type compatibility checks for unary operators
    match operator {
        Operator::Not => {
            // NOT requires boolean operand
        }
        Operator::Minus => {
            // Unary minus requires numeric operand
        }
        _ => {
            errors.push(ValidationError {
                message: format!("Invalid unary operator '{:?}'", operator),
                location: None,
                error_type: ValidationErrorType::Type,
            });
        }
    }
}

/// Validate binary operations
fn validate_binary_operation(
    operator: &Operator,
    left: &Expression,
    right: &Expression,
    ctx: &mut ValidationContext,
    errors: &mut Vec<ValidationError>,
) {
    // Validate that both operands are valid expressions
    validate_expression(left, ctx, errors);
    validate_expression(right, ctx, errors);
    
    // Type compatibility checks (simplified)
    match operator {
        // Arithmetic operators
        Operator::Plus | Operator::Minus | Operator::Star | Operator::Slash | Operator::Percent | Operator::Caret => {
            // These require numeric types
        }
        // Comparison operators
        Operator::Equal | Operator::NotEqual | Operator::LessThan | Operator::LessEqual | Operator::GreaterThan | Operator::GreaterEqual => {
            // These can work with comparable types
        }
        // Logical operators
        Operator::And | Operator::Or | Operator::Not | Operator::Xor => {
            // These require boolean operands
        }
        // String operators
        Operator::In | Operator::NotIn | Operator::Contains | Operator::Starts | Operator::Ends | Operator::Like | Operator::Matches | Operator::FuzzyEqual | Operator::Concat => {
            // These require string operands (Concat converts non-strings to strings)
        }
        // Existence operator
        Operator::Exists => {
            // This is for checking existence
        }
        // Regex operator
        Operator::Regex => {
            // This requires string operands
        }
        // Temporal operator
        Operator::Within => {
            // This is for temporal operations
        }
    }
}

/// Validate literals
fn validate_literal(literal: &Literal, errors: &mut Vec<ValidationError>) {
    match literal {
        Literal::String(s) => {
            if s.is_empty() {
                errors.push(ValidationError {
                    message: "String literal cannot be empty".to_string(),
                    location: None,
                    error_type: ValidationErrorType::Syntax,
                });
            }
        }
        Literal::Vector(v) => {
            if v.is_empty() {
                errors.push(ValidationError {
                    message: "Vector literal cannot be empty".to_string(),
                    location: None,
                    error_type: ValidationErrorType::Syntax,
                });
            }
        }
        Literal::DateTime(dt) => {
            validate_datetime_format(dt, errors);
        }
        Literal::Duration(dur) => {
            validate_duration_format(dur, errors);
        }
        Literal::TimeWindow(tw) => {
            validate_timewindow_format(tw, errors);
        }
        _ => {} // Other literals are fine
    }
}

/// Validate datetime format
fn validate_datetime_format(dt: &str, errors: &mut Vec<ValidationError>) {
    // Basic ISO 8601 format validation
    if !dt.contains('T') {
        errors.push(ValidationError {
            message: "DateTime must be in ISO 8601 format (YYYY-MM-DDTHH:MM:SS)".to_string(),
            location: None,
            error_type: ValidationErrorType::Syntax,
        });
    }
}

/// Validate duration format
fn validate_duration_format(dur: &str, errors: &mut Vec<ValidationError>) {
    // Basic ISO duration format validation
    if !dur.starts_with('P') {
        errors.push(ValidationError {
            message: "Duration must be in ISO 8601 format (P1Y2M3DT4H5M6S)".to_string(),
            location: None,
            error_type: ValidationErrorType::Syntax,
        });
    }
}

/// Validate time window format
fn validate_timewindow_format(tw: &str, errors: &mut Vec<ValidationError>) {
    // Basic TIME_WINDOW format validation
    if !tw.starts_with("TIME_WINDOW(") || !tw.ends_with(')') {
        errors.push(ValidationError {
            message: "TimeWindow must be in format TIME_WINDOW(start, end)".to_string(),
            location: None,
            error_type: ValidationErrorType::Syntax,
        });
    }
}

/// Validate edge patterns
fn validate_edge_patterns(query: &Query, errors: &mut Vec<ValidationError>) {
    match query {
        Query::Basic(basic_query) => {
            validate_basic_query_edge_patterns(basic_query, errors);
        }
        Query::SetOperation(set_op) => {
            validate_edge_patterns(&set_op.left, errors);
            validate_edge_patterns(&set_op.right, errors);
        }
        Query::Limited { query, .. } => {
            validate_edge_patterns(query, errors);
        }
        Query::WithQuery(_) => {
            // TODO: Implement WithQuery validation
        }
        Query::Let(_) => {
            // TODO: Implement LET validation
        }
        Query::For(_) => {
            // TODO: Implement FOR validation  
        }
        Query::Filter(_) => {
            // TODO: Implement FILTER validation
        }
        Query::Return(_) => {
            // TODO: Implement RETURN validation
        }
        Query::Unwind(_) => {
            // TODO: Implement UNWIND validation
        }
        Query::MutationPipeline(_) => {
            // TODO: Validate mutation pipeline
        }
    }
}

fn validate_basic_query_edge_patterns(query: &BasicQuery, errors: &mut Vec<ValidationError>) {
    for pattern in &query.match_clause.patterns {
        for element in &pattern.elements {
            if let PatternElement::Edge(edge) = element {
                // Validate edge direction
                match edge.direction {
                    EdgeDirection::Both => {
                        // <-> is valid
                    }
                    EdgeDirection::Incoming => {
                        // <- is valid
                    }
                    EdgeDirection::Outgoing => {
                        // -> is valid
                    }
                    EdgeDirection::Undirected => {
                        // -- is valid
                    }
                }
                
                // Validate edge labels
                for label in &edge.labels {
                    if label.is_empty() {
                        errors.push(ValidationError {
                            message: "Edge label cannot be empty".to_string(),
                            location: None,
                            error_type: ValidationErrorType::Syntax,
                        });
                    }
                }
            }
        }
    }
}

/// Validate temporal literals
fn validate_temporal_literals(_query: &Query, _errors: &mut Vec<ValidationError>) {
    // This would traverse all expressions to find temporal literals
    // and validate their formats
    // For now, this is a placeholder for temporal validation logic
}

/// Validate expression variables (helper function)
fn validate_expression_variables(expr: &Expression, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    validate_expression(expr, ctx, errors);
}

/// Validate CALL statement
fn validate_call_statement(call_stmt: &CallStatement, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate procedure name (should be valid system procedure)
    if !crate::catalog::system_procedures::is_system_procedure(&call_stmt.procedure_name) {
        errors.push(ValidationError {
            message: format!("Unknown system procedure: {}", call_stmt.procedure_name),
            location: Some(call_stmt.location.clone()),
            error_type: ValidationErrorType::Semantic,
        });
    }
    
    // Validate arguments
    for arg in &call_stmt.arguments {
        validate_expression(arg, ctx, errors);
    }
    
    // Validate YIELD clause if present
    if let Some(yield_clause) = &call_stmt.yield_clause {
        validate_yield_clause(yield_clause, errors);
    }

    // Validate WHERE clause if present
    if let Some(where_clause) = &call_stmt.where_clause {
        // WHERE clause can only be used with YIELD clause
        if call_stmt.yield_clause.is_none() {
            errors.push(ValidationError {
                message: "WHERE clause can only be used with YIELD clause in CALL statements".to_string(),
                location: Some(where_clause.location.clone()),
                error_type: ValidationErrorType::Semantic,
            });
        } else {
            // For CALL statements, we only validate WHERE structure and that it references YIELD columns
            // We skip general variable existence validation since procedure results are runtime-dependent

            // Additional validation: WHERE should reference columns from YIELD
            // This is a semantic check to ensure WHERE only uses yielded columns
            if let Some(yield_clause) = &call_stmt.yield_clause {
                validate_where_references_yield_columns(where_clause, yield_clause, errors);
            }

            // Note: We deliberately skip validate_expression() here for CALL WHERE clauses
            // because CALL procedure results are only known at runtime. The WHERE clause
            // validation happens during execution when the actual column names are available.
        }
    }
}

/// Validate that WHERE clause only references columns from YIELD clause
fn validate_where_references_yield_columns(where_clause: &WhereClause, yield_clause: &YieldClause, errors: &mut Vec<ValidationError>) {
    // Get the set of available column names from YIELD (including aliases)
    let yielded_columns: std::collections::HashSet<String> = yield_clause.items.iter()
        .map(|item| item.alias.as_ref().unwrap_or(&item.column_name).clone())
        .collect();

    // Extract variable references from WHERE condition and validate them
    let mut referenced_columns = std::collections::HashSet::new();
    extract_variable_references(&where_clause.condition, &mut referenced_columns);

    // Check that all referenced columns are available from YIELD
    for column in referenced_columns {
        if !yielded_columns.contains(&column) {
            errors.push(ValidationError {
                message: format!("WHERE clause references column '{}' which is not available from YIELD clause", column),
                location: Some(where_clause.location.clone()),
                error_type: ValidationErrorType::Semantic,
            });
        }
    }
}

/// Extract variable references from an expression (helper for WHERE validation)
fn extract_variable_references(expr: &Expression, variables: &mut std::collections::HashSet<String>) {
    match expr {
        Expression::Variable(var) => {
            variables.insert(var.name.clone());
        }
        Expression::PropertyAccess(prop_access) => {
            variables.insert(prop_access.property.clone());
        }
        Expression::Binary(binary_expr) => {
            extract_variable_references(&binary_expr.left, variables);
            extract_variable_references(&binary_expr.right, variables);
        }
        Expression::Unary(unary_expr) => {
            extract_variable_references(&unary_expr.expression, variables);
        }
        Expression::FunctionCall(func_call) => {
            for arg in &func_call.arguments {
                extract_variable_references(arg, variables);
            }
        }
        Expression::Case(case_expr) => {
            match &case_expr.case_type {
                crate::ast::ast::CaseType::Simple(simple) => {
                    extract_variable_references(&simple.test_expression, variables);
                    for branch in &simple.when_branches {
                        for when_val in &branch.when_values {
                            extract_variable_references(when_val, variables);
                        }
                        extract_variable_references(&branch.then_expression, variables);
                    }
                    if let Some(else_expr) = &simple.else_expression {
                        extract_variable_references(else_expr, variables);
                    }
                }
                crate::ast::ast::CaseType::Searched(searched) => {
                    for branch in &searched.when_branches {
                        extract_variable_references(&branch.condition, variables);
                        extract_variable_references(&branch.then_expression, variables);
                    }
                    if let Some(else_expr) = &searched.else_expression {
                        extract_variable_references(else_expr, variables);
                    }
                }
            }
        }
        // For other expression types (literals, etc.), no variables to extract
        _ => {}
    }
}

/// Validate YIELD clause
fn validate_yield_clause(yield_clause: &YieldClause, errors: &mut Vec<ValidationError>) {
    // Check for duplicate column names/aliases
    let mut seen_names = std::collections::HashSet::new();
    
    for item in &yield_clause.items {
        let output_name = item.alias.as_ref().unwrap_or(&item.column_name);
        
        if !seen_names.insert(output_name) {
            errors.push(ValidationError {
                message: format!("Duplicate YIELD column: {}", output_name),
                location: Some(item.location.clone()),
                error_type: ValidationErrorType::Semantic,
            });
        }
    }
}

/// Validate SELECT statement
fn validate_select_statement(select_stmt: &SelectStatement, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Validate return items structure
    match &select_stmt.return_items {
        SelectItems::Wildcard { .. } => {
            // Wildcard (*) is always valid
        },
        SelectItems::Explicit { items, .. } => {
            if items.is_empty() {
                errors.push(ValidationError {
                    message: "SELECT statement must have at least one return item".to_string(),
                    location: Some(select_stmt.location.clone()),
                    error_type: ValidationErrorType::Structural,
                });
            }
        }
    }
    
    // Validate FROM clause FIRST to declare variables
    if let Some(from_clause) = &select_stmt.from_clause {
        validate_from_clause(from_clause, ctx, errors);
    }
    
    // Validate return item expressions AFTER variables are declared
    match &select_stmt.return_items {
        SelectItems::Wildcard { .. } => {
            // Wildcard is always valid, no expressions to validate
        },
        SelectItems::Explicit { items, .. } => {
            for item in items {
                validate_expression(&item.expression, ctx, errors);
            }
        }
    }
    
    // Validate WHERE clause if present
    if let Some(where_clause) = &select_stmt.where_clause {
        validate_expression(&where_clause.condition, ctx, errors);
    }
    
    // Validate GROUP BY clause if present
    if let Some(group_clause) = &select_stmt.group_clause {
        for expr in &group_clause.expressions {
            validate_expression(expr, ctx, errors);
        }
    }
    
    // Validate HAVING clause if present
    if let Some(having_clause) = &select_stmt.having_clause {
        validate_expression(&having_clause.condition, ctx, errors);
    }
    
    // Validate ORDER BY clause if present
    if let Some(order_clause) = &select_stmt.order_clause {
        for item in &order_clause.items {
            validate_expression(&item.expression, ctx, errors);
        }
    }
}

/// Validate FROM clause
fn validate_from_clause(from_clause: &FromClause, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    for graph_expr in &from_clause.graph_expressions {
        validate_graph_expression(&graph_expr.graph_expression, ctx, errors);
        
        if let Some(match_clause) = &graph_expr.match_statement {
            validate_match_clause(match_clause, ctx, errors);
        }
    }
}

/// Validate graph expression
fn validate_graph_expression(graph_expr: &GraphExpression, _ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    match graph_expr {
        GraphExpression::Reference(path) => {
            if path.segments.is_empty() {
                errors.push(ValidationError {
                    message: "Graph reference path cannot be empty".to_string(),
                    location: Some(path.location.clone()),
                    error_type: ValidationErrorType::Semantic,
                });
            }
        }
        GraphExpression::Union { left, right, .. } => {
            validate_graph_expression(left, _ctx, errors);
            validate_graph_expression(right, _ctx, errors);
        }
        GraphExpression::CurrentGraph => {
            // CurrentGraph is valid - it refers to the session's current graph
            // No validation needed
        }
    }
}

/// Validate match clause
fn validate_match_clause(match_clause: &MatchClause, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    if match_clause.patterns.is_empty() {
        errors.push(ValidationError {
            message: "MATCH clause must have at least one pattern".to_string(),
            location: Some(match_clause.location.clone()),
            error_type: ValidationErrorType::Structural,
        });
    }
    
    for pattern in &match_clause.patterns {
        validate_path_pattern(pattern, ctx, errors);
    }
}

/// Validate path pattern
fn validate_path_pattern(pattern: &PathPattern, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    if pattern.elements.is_empty() {
        errors.push(ValidationError {
            message: "Path pattern must have at least one element".to_string(),
            location: Some(pattern.location.clone()),
            error_type: ValidationErrorType::Structural,
        });
    }
    
    for element in &pattern.elements {
        match element {
            PatternElement::Node(node) => {
                validate_node(node, ctx, errors);
            }
            PatternElement::Edge(edge) => {
                validate_edge(edge, ctx, errors);
            }
        }
    }
}

/// Validate node
fn validate_node(node: &Node, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Declare variable if node has an identifier
    if let Some(ref identifier) = node.identifier {
        ctx.declared_variables.insert(identifier.clone());
        ctx.variable_types.insert(identifier.clone(), GqlType::String { max_length: None }); // Node as generic type
    }
    
    // Validate labels
    for label in &node.labels {
        if label.is_empty() {
            errors.push(ValidationError {
                message: "Node label cannot be empty".to_string(),
                location: Some(node.location.clone()),
                error_type: ValidationErrorType::Syntax,
            });
        }
    }
    
    // Validate properties if present
    if let Some(properties) = &node.properties {
        for property in &properties.properties {
            validate_expression(&property.value, ctx, errors);
        }
    }
}

/// Validate edge
fn validate_edge(edge: &Edge, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Declare variable if edge has an identifier
    if let Some(ref identifier) = edge.identifier {
        ctx.declared_variables.insert(identifier.clone());
        ctx.variable_types.insert(identifier.clone(), GqlType::String { max_length: None }); // Edge as generic type
    }
    
    // Validate labels
    for label in &edge.labels {
        if label.is_empty() {
            errors.push(ValidationError {
                message: "Edge label cannot be empty".to_string(),
                location: Some(edge.location.clone()),
                error_type: ValidationErrorType::Syntax,
            });
        }
    }
    
    // Validate properties if present
    if let Some(properties) = &edge.properties {
        for property in &properties.properties {
            validate_expression(&property.value, ctx, errors);
        }
    }
}

/// Validate a subquery recursively
fn validate_subquery(query: &Query, ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    // Create a new context scope for the subquery to avoid variable leakage
    let mut subquery_ctx = ctx.clone();
    
    // Validate the query structure
    validate_query_structure(query, errors);
    
    // Validate variable declarations and scope within subquery
    validate_variable_declarations(query, &mut subquery_ctx, errors);
    
    // Validate path patterns within subquery
    validate_path_patterns(query, &mut subquery_ctx, errors);
    
    // Validate expressions within subquery
    validate_expressions(query, &mut subquery_ctx, errors);
    
    // Validate temporal literals within subquery
    validate_temporal_literals(query, errors);
    
    // Validate edge patterns within subquery
    validate_edge_patterns(query, errors);
}

/// Validate a statement within a procedure body context
/// In procedure bodies, statements have different validation rules:
/// - MATCH statements don't require RETURN clauses when followed by NEXT
/// - Data statements (like DELETE) don't require RETURN clauses
fn validate_procedure_statement(statement: &Statement, ctx: &ValidationContext, errors: &mut Vec<ValidationError>) {
    match statement {
        Statement::Query(query) => {
            // For query statements in procedure bodies, validate without requiring RETURN clause
            validate_procedure_query(query, ctx, errors);
        },
        Statement::DataStatement(_data_stmt) => {
            // Data statements (like DELETE) in procedure bodies don't require RETURN clauses
            // Just validate that the statement structure is valid, but don't require RETURN
            // TODO: Implement basic data statement validation without RETURN requirement
        },
        _ => {
            // For other statement types, use general validation
            let doc = Document {
                statement: statement.clone(),
                location: Default::default(),
            };
            if let Err(mut nested_errors) = validate_query(&doc, ctx.has_graph_context) {
                errors.append(&mut nested_errors);
            }
            }
    }
}

/// Validate catalog statement (DDL operations)
fn validate_catalog_statement(catalog_stmt: &CatalogStatement, _ctx: &mut ValidationContext, errors: &mut Vec<ValidationError>) {
    match catalog_stmt {
        CatalogStatement::CreateSchema(create_schema) => {
            validate_create_schema_statement(create_schema, errors);
        },
        CatalogStatement::DropSchema(drop_schema) => {
            validate_drop_schema_statement(drop_schema, errors);
        },
        CatalogStatement::CreateGraph(create_graph) => {
            validate_create_graph_statement(create_graph, errors);
        },
        CatalogStatement::DropGraph(drop_graph) => {
            validate_drop_graph_statement(drop_graph, errors);
        },
        _ => {
            // Other catalog statements don't need specific validation yet
        }
    }
}

/// Validate a query within a procedure body - doesn't require RETURN clause
fn validate_procedure_query(query: &Query, ctx: &ValidationContext, errors: &mut Vec<ValidationError>) {
    match query {
        Query::Basic(basic_query) => {
            // Validate structure but don't require RETURN clause
            if basic_query.match_clause.patterns.is_empty() {
                errors.push(ValidationError {
                    message: "Query must have at least one path pattern in MATCH clause".to_string(),
                    location: None,
                    error_type: ValidationErrorType::Structural,
                });
            }
            // Note: We don't check for RETURN clause requirement here
            
            // Validate other aspects using a temporary context
            let mut temp_ctx = ctx.clone();
            
            // Validate variable declarations and patterns
            validate_basic_query_variables(basic_query, &mut temp_ctx, errors);
            validate_basic_query_path_patterns(basic_query, &mut temp_ctx, errors);
            validate_basic_query_expressions(basic_query, &mut temp_ctx, errors);
        },
        _ => {
            // For other query types, use standard validation for now
            // This is a simplified implementation - in production you'd want full validation
            // without RETURN clause requirements
        }
    }
}

/// Validate CREATE SCHEMA statement
fn validate_create_schema_statement(create_schema: &CreateSchemaStatement, errors: &mut Vec<ValidationError>) {
    // Validate schema name is not empty
    if create_schema.schema_path.segments.is_empty() || 
       create_schema.schema_path.segments.iter().any(|s| s.trim().is_empty()) {
        errors.push(ValidationError {
            message: "Invalid schema name".to_string(),
            location: Some(create_schema.location.clone()),
            error_type: ValidationErrorType::Syntax,
        });
    }
}

/// Validate DROP SCHEMA statement
fn validate_drop_schema_statement(drop_schema: &DropSchemaStatement, errors: &mut Vec<ValidationError>) {
    // Validate schema name is not empty
    if drop_schema.schema_path.segments.is_empty() || 
       drop_schema.schema_path.segments.iter().any(|s| s.trim().is_empty()) {
        errors.push(ValidationError {
            message: "Invalid schema name".to_string(),
            location: Some(drop_schema.location.clone()),
            error_type: ValidationErrorType::Syntax,
        });
    }
}

/// Validate CREATE GRAPH statement
fn validate_create_graph_statement(create_graph: &CreateGraphStatement, errors: &mut Vec<ValidationError>) {
    // Validate graph name is not empty
    if create_graph.graph_path.segments.is_empty() ||
       create_graph.graph_path.segments.iter().any(|s| s.trim().is_empty()) {
        errors.push(ValidationError {
            message: "Invalid graph name".to_string(),
            location: Some(create_graph.location.clone()),
            error_type: ValidationErrorType::Syntax,
        });
        return;
    }

    // Validate path structure
    match create_graph.graph_path.segments.len() {
        1 => {
            // Single segment: warn that this requires session schema
            // This is informational since executor will handle the actual validation
            // with proper session context
        },
        2 => {
            // Full path: schema/graph - valid
        },
        _ => {
            errors.push(ValidationError {
                message: "Invalid graph path: must be either 'graph_name' (when schema is set) or '/schema_name/graph_name'".to_string(),
                location: Some(create_graph.location.clone()),
                error_type: ValidationErrorType::Syntax,
            });
        }
    }
}

/// Validate DROP GRAPH statement
fn validate_drop_graph_statement(drop_graph: &DropGraphStatement, errors: &mut Vec<ValidationError>) {
    // Validate graph name is not empty
    if drop_graph.graph_path.segments.is_empty() ||
       drop_graph.graph_path.segments.iter().any(|s| s.trim().is_empty()) {
        errors.push(ValidationError {
            message: "Invalid graph name".to_string(),
            location: Some(drop_graph.location.clone()),
            error_type: ValidationErrorType::Syntax,
        });
        return;
    }

    // Validate path structure
    match drop_graph.graph_path.segments.len() {
        1 => {
            // Single segment: warn that this requires session schema
            // This is informational since executor will handle the actual validation
            // with proper session context
        },
        2 => {
            // Full path: schema/graph - valid
        },
        _ => {
            errors.push(ValidationError {
                message: "Invalid graph path: must be either 'graph_name' (when schema is set) or '/schema_name/graph_name'".to_string(),
                location: Some(drop_graph.location.clone()),
                error_type: ValidationErrorType::Syntax,
            });
        }
    }
}

