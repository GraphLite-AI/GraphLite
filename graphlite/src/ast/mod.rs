// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! AST subsystem: Lexer, parser, AST nodes, and query validation for GQL

#[allow(clippy::module_inception)]
mod ast;
pub use ast::*;
pub mod lexer;
pub mod parser;
pub mod pretty_printer;
pub mod validator;
