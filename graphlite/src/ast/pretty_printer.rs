// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Pretty printer for AST nodes with debug logging

use log::debug;

use crate::ast::*;

/// Pretty print an AST Document with indented tree structure and debug logging
pub fn pretty_print_ast(document: &Document) {
    debug!("Starting AST pretty printing");
    debug!("Document");
    print_statement(&document.statement, 1);
    debug!("AST pretty printing completed");
}

fn print_statement(statement: &Statement, indent: usize) {
    match statement {
        Statement::Query(query) => {
            debug!("{}{}", get_indent(indent), "Query Statement");
            print_query(query, indent + 1);
        }
        Statement::Select(select_stmt) => {
            debug!("{}{}", get_indent(indent), "Select Statement");
            print_select_statement(select_stmt, indent + 1);
        }
        Statement::Call(call_stmt) => {
            debug!("{}{}", get_indent(indent), "Call Statement");
            print_call_statement(call_stmt, indent + 1);
        }
        Statement::CatalogStatement(_catalog_stmt) => {
            debug!("{}{}", get_indent(indent), "Catalog Statement");
        }
        Statement::DataStatement(_data_stmt) => {
            debug!("{}{}", get_indent(indent), "Data Statement");
        }
        Statement::SessionStatement(_session_stmt) => {
            debug!("{}{}", get_indent(indent), "Session Statement");
        }
        Statement::Declare(declare_stmt) => {
            debug!("{}{}", get_indent(indent), "Declare Statement");
            debug!(
                "{}Variables: {}",
                get_indent(indent + 1),
                declare_stmt.variable_declarations.len()
            );
        }
        Statement::Next(next_stmt) => {
            debug!("{}{}", get_indent(indent), "Next Statement");
            debug!(
                "{}Has target: {}",
                get_indent(indent + 1),
                next_stmt.target_statement.is_some()
            );
        }
        Statement::AtLocation(at_stmt) => {
            debug!("{}{}", get_indent(indent), "At Location Statement");
            debug!(
                "{}Path: {}",
                get_indent(indent + 1),
                at_stmt.location_path
            );
            debug!(
                "{}Statements: {}",
                get_indent(indent + 1),
                at_stmt.statements.len()
            );
        }
        Statement::TransactionStatement(transaction_stmt) => {
            debug!("{}{}", get_indent(indent), "Transaction Statement");
            print_transaction_statement(transaction_stmt, indent + 1);
        }
        Statement::ProcedureBody(procedure_body) => {
            debug!("{}{}", get_indent(indent), "Procedure Body Statement");
            debug!(
                "{}Variable definitions: {}",
                get_indent(indent + 1),
                procedure_body.variable_definitions.len()
            );
            debug!("{}Initial statement:", get_indent(indent + 1));
            print_statement(&procedure_body.initial_statement, indent + 2);
            debug!(
                "{}Chained statements: {}",
                get_indent(indent + 1),
                procedure_body.chained_statements.len()
            );
            for (i, chained) in procedure_body.chained_statements.iter().enumerate() {
                debug!("{}NEXT statement {}:", get_indent(indent + 2), i + 1);
                print_statement(&chained.statement, indent + 3);
            }
        }
        Statement::IndexStatement(index_stmt) => {
            debug!("{}{}", get_indent(indent), "Index Statement");
            match index_stmt {
                crate::ast::IndexStatement::Create(create_idx) => {
                    debug!(
                        "{}CREATE INDEX: {}",
                        get_indent(indent + 1),
                        create_idx.name
                    );
                }
                crate::ast::IndexStatement::Drop(drop_idx) => {
                    debug!("{}DROP INDEX: {}", get_indent(indent + 1), drop_idx.name);
                }
                crate::ast::IndexStatement::Alter(alter_idx) => {
                    debug!("{}ALTER INDEX: {}", get_indent(indent + 1), alter_idx.name);
                }
                crate::ast::IndexStatement::Optimize(optimize_idx) => {
                    debug!(
                        "{}OPTIMIZE INDEX: {}",
                        get_indent(indent + 1),
                        optimize_idx.name
                    );
                }
                crate::ast::IndexStatement::Reindex(reindex) => {
                    debug!("{}REINDEX: {}", get_indent(indent + 1), reindex.name);
                }
            }
        }
        Statement::Let(let_stmt) => {
            debug!("{}{}", get_indent(indent), "LET Statement");
            for (i, var_def) in let_stmt.variable_definitions.iter().enumerate() {
                debug!(
                    "{}Variable {}: {} = [expression]",
                    get_indent(indent + 1),
                    i,
                    var_def.variable_name
                );
            }
        }
    }
}

fn print_query(query: &Query, indent: usize) {
    match query {
        Query::Basic(basic_query) => {
            debug!("{}{}", get_indent(indent), "Basic Query");
            print_basic_query(basic_query, indent + 1);
        }
        Query::SetOperation(set_op) => {
            debug!("{}{}", get_indent(indent), "Set Operation Query");
            print_set_operation(set_op, indent + 1);
        }
        Query::Limited {
            query,
            order_clause,
            limit_clause,
        } => {
            debug!("{}{}", get_indent(indent), "Limited Query");
            print_query(query, indent + 1);
            if order_clause.is_some() {
                debug!("{}ORDER BY clause", get_indent(indent + 1));
            }
            if limit_clause.is_some() {
                debug!("{}LIMIT clause", get_indent(indent + 1));
            }
        }
        Query::WithQuery(with_query) => {
            debug!("{}{}", get_indent(indent), "WITH Query");
            debug!(
                "{}WITH query with {} segments",
                get_indent(indent + 1),
                with_query.segments.len()
            );
            // TODO: Implement full WITH query pretty printing
        }
        Query::Let(let_stmt) => {
            debug!("{}{}", get_indent(indent), "LET Statement");
            debug!(
                "{}LET statement with {} variable definitions",
                get_indent(indent + 1),
                let_stmt.variable_definitions.len()
            );
        }
        Query::For(for_stmt) => {
            debug!("{}{}", get_indent(indent), "FOR Statement");
            debug!(
                "{}FOR statement with variable '{}'",
                get_indent(indent + 1),
                for_stmt.variable
            );
        }
        Query::Filter(stmt) => {
            debug!("{}{}", get_indent(indent), "FILTER Statement");
            debug!("{}Where Clause", get_indent(indent + 1));
            print_where_clause(&stmt.where_clause, indent + 2);
        }
        Query::Return(return_query) => {
            debug!("{}{}", get_indent(indent), "RETURN Query");
            print_return_query(return_query, indent + 1);
        }
        Query::Unwind(unwind_stmt) => {
            debug!("{}{}", get_indent(indent), "UNWIND Statement");
            debug!(
                "{}UNWIND expression AS '{}'",
                get_indent(indent + 1),
                unwind_stmt.variable
            );
        }
        Query::MutationPipeline(pipeline) => {
            debug!("{}{}", get_indent(indent), "Mutation Pipeline");
            debug!(
                "{}Pipeline with {} segments",
                get_indent(indent + 1),
                pipeline.segments.len()
            );
            debug!(
                "{}Final mutation action (REMOVE/SET/DELETE)",
                get_indent(indent + 1)
            );
        }
    }
}

fn print_basic_query(query: &BasicQuery, indent: usize) {
    debug!("{}{}", get_indent(indent), "MatchClause");
    print_match_clause(&query.match_clause, indent + 1);

    if let Some(where_clause) = &query.where_clause {
        debug!("{}{}", get_indent(indent), "WhereClause");
        print_where_clause(where_clause, indent + 1);
    }

    debug!("{}{}", get_indent(indent), "ReturnClause");
    print_return_clause(&query.return_clause, indent + 1);
}

fn print_return_query(query: &ReturnQuery, indent: usize) {
    debug!("{}{}", get_indent(indent), "ReturnClause");
    print_return_clause(&query.return_clause, indent + 1);

    if query.group_clause.is_some() {
        debug!("{}{}", get_indent(indent), "GROUP BY clause");
    }
    if query.having_clause.is_some() {
        debug!("{}{}", get_indent(indent), "HAVING clause");
    }
    if query.order_clause.is_some() {
        debug!("{}{}", get_indent(indent), "ORDER BY clause");
    }
    if query.limit_clause.is_some() {
        debug!("{}{}", get_indent(indent), "LIMIT clause");
    }
}

fn print_set_operation(set_op: &SetOperation, indent: usize) {
    debug!("{}{:?}", get_indent(indent), set_op.operation);
    debug!("{}Left Query:", get_indent(indent));
    print_query(&set_op.left, indent + 1);
    debug!("{}Right Query:", get_indent(indent));
    print_query(&set_op.right, indent + 1);
}

fn print_call_statement(call_stmt: &CallStatement, indent: usize) {
    debug!(
        "{}Procedure: {}",
        get_indent(indent),
        call_stmt.procedure_name
    );

    if !call_stmt.arguments.is_empty() {
        debug!(
            "{}Arguments ({})",
            get_indent(indent),
            call_stmt.arguments.len()
        );
        for (i, arg) in call_stmt.arguments.iter().enumerate() {
            debug!("{}Argument {}", get_indent(indent + 1), i);
            print_expression(arg, indent + 2);
        }
    }

    if let Some(yield_clause) = &call_stmt.yield_clause {
        debug!("{}YieldClause", get_indent(indent));
        print_yield_clause(yield_clause, indent + 1);
    }

    if let Some(where_clause) = &call_stmt.where_clause {
        debug!("{}WhereClause", get_indent(indent));
        print_where_clause(where_clause, indent + 1);
    }
}

fn print_yield_clause(yield_clause: &YieldClause, indent: usize) {
    debug!(
        "{}Yield Items ({})",
        get_indent(indent),
        yield_clause.items.len()
    );

    for (i, item) in yield_clause.items.iter().enumerate() {
        debug!(
            "{}Item {}: {}{}",
            get_indent(indent + 1),
            i,
            item.column_name,
            if let Some(alias) = &item.alias {
                format!(" AS {}", alias)
            } else {
                String::new()
            }
        );
    }
}

fn print_match_clause(match_clause: &MatchClause, indent: usize) {
    debug!(
        "{}Patterns ({})",
        get_indent(indent),
        match_clause.patterns.len()
    );

    for (i, pattern) in match_clause.patterns.iter().enumerate() {
        debug!("{}PathPattern {}", get_indent(indent + 1), i);
        print_path_pattern(pattern, indent + 2);
    }
}

fn print_path_pattern(path_pattern: &PathPattern, indent: usize) {
    debug!(
        "{}Elements ({})",
        get_indent(indent),
        path_pattern.elements.len()
    );

    for (i, element) in path_pattern.elements.iter().enumerate() {
        match element {
            PatternElement::Node(node) => {
                debug!("{}Node {}", get_indent(indent + 1), i);
                print_node(node, indent + 2);
            }
            PatternElement::Edge(edge) => {
                debug!("{}Edge {}", get_indent(indent + 1), i);
                print_edge(edge, indent + 2);
            }
        }
    }
}

fn print_node(node: &Node, indent: usize) {
    if let Some(identifier) = &node.identifier {
        debug!("{}Identifier: {}", get_indent(indent), identifier);
    }

    if !node.labels.is_empty() {
        debug!("{}Labels: [{}]", get_indent(indent), node.labels.join(", "));
    }

    if let Some(properties) = &node.properties {
        debug!("{}Properties", get_indent(indent));
        print_property_map(properties, indent + 1);
    }
}

fn print_edge(edge: &Edge, indent: usize) {
    if let Some(identifier) = &edge.identifier {
        debug!("{}Identifier: {}", get_indent(indent), identifier);
    }

    if !edge.labels.is_empty() {
        debug!("{}Labels: [{}]", get_indent(indent), edge.labels.join(", "));
    }

    debug!("{}Direction: {:?}", get_indent(indent), edge.direction);

    if let Some(properties) = &edge.properties {
        debug!("{}Properties", get_indent(indent));
        print_property_map(properties, indent + 1);
    }
}

fn print_property_map(prop_map: &PropertyMap, indent: usize) {
    for property in &prop_map.properties {
        debug!("{}Property: {}", get_indent(indent), property.key);
        print_expression(&property.value, indent + 1);
    }
}

fn print_where_clause(where_clause: &WhereClause, indent: usize) {
    debug!("{}Condition", get_indent(indent));
    print_expression(&where_clause.condition, indent + 1);
}

fn print_return_clause(return_clause: &ReturnClause, indent: usize) {
    debug!(
        "{}Items ({})",
        get_indent(indent),
        return_clause.items.len()
    );

    for (i, item) in return_clause.items.iter().enumerate() {
        debug!("{}ReturnItem {}", get_indent(indent + 1), i);
        print_return_item(item, indent + 2);
    }
}

fn print_return_item(item: &ReturnItem, indent: usize) {
    debug!("{}Expression", get_indent(indent));
    print_expression(&item.expression, indent + 1);

    if let Some(alias) = &item.alias {
        debug!("{}Alias: {}", get_indent(indent), alias);
    }
}

fn print_expression(expr: &Expression, indent: usize) {
    match expr {
        Expression::Binary(binary) => {
            debug!(
                "{}BinaryExpression ({:?})",
                get_indent(indent),
                binary.operator
            );
            debug!("{}Left", get_indent(indent + 1));
            print_expression(&binary.left, indent + 2);
            debug!("{}Right", get_indent(indent + 1));
            print_expression(&binary.right, indent + 2);
        }
        Expression::Unary(unary) => {
            debug!(
                "{}UnaryExpression ({:?})",
                get_indent(indent),
                unary.operator
            );
            print_expression(&unary.expression, indent + 1);
        }
        Expression::FunctionCall(func) => {
            debug!("{}FunctionCall: {}", get_indent(indent), func.name);
            debug!(
                "{}Arguments ({})",
                get_indent(indent + 1),
                func.arguments.len()
            );
            for (i, arg) in func.arguments.iter().enumerate() {
                debug!("{}Arg {}", get_indent(indent + 2), i);
                print_expression(arg, indent + 3);
            }
        }
        Expression::PropertyAccess(prop) => {
            debug!(
                "{}PropertyAccess: {}.{}",
                get_indent(indent),
                prop.object,
                prop.property
            );
        }
        Expression::Variable(var) => {
            debug!("{}Variable: ${}", get_indent(indent), var.name);
        }
        Expression::Literal(literal) => {
            let literal_str = match literal {
                Literal::String(s) => format!("Literal: \"{}\"", s),
                Literal::Integer(i) => format!("Literal: {}", i),
                Literal::Float(f) => format!("Literal: {}", f),
                Literal::Boolean(b) => format!("Literal: {}", b),
                Literal::Null => "Literal: null".to_string(),
                Literal::DateTime(dt) => format!("Literal: DateTime({})", dt),
                Literal::Duration(dur) => format!("Literal: Duration({})", dur),
                Literal::TimeWindow(tw) => format!("Literal: TimeWindow({})", tw),
                Literal::Vector(vec) => format!("Literal: Vector({:?})", vec),
                Literal::List(list) => format!("Literal: List({:?})", list),
            };
            debug!("{}{}", get_indent(indent), literal_str);
        }
        Expression::Case(case_expr) => {
            debug!("{}CaseExpression", get_indent(indent));
            match &case_expr.case_type {
                crate::ast::CaseType::Simple(simple) => {
                    debug!("{}Simple CASE", get_indent(indent + 1));
                    debug!("{}Test Expression:", get_indent(indent + 2));
                    print_expression(&simple.test_expression, indent + 3);
                    debug!(
                        "{}WHEN branches: {}",
                        get_indent(indent + 2),
                        simple.when_branches.len()
                    );
                    for (i, branch) in simple.when_branches.iter().enumerate() {
                        debug!("{}Branch {}", get_indent(indent + 3), i);
                        debug!(
                            "{}WHEN values: {}",
                            get_indent(indent + 4),
                            branch.when_values.len()
                        );
                        for (j, val) in branch.when_values.iter().enumerate() {
                            debug!("{}Value {}", get_indent(indent + 5), j);
                            print_expression(val, indent + 6);
                        }
                        debug!("{}THEN:", get_indent(indent + 4));
                        print_expression(&branch.then_expression, indent + 5);
                    }
                    if let Some(else_expr) = &simple.else_expression {
                        debug!("{}ELSE:", get_indent(indent + 2));
                        print_expression(else_expr, indent + 3);
                    }
                }
                crate::ast::CaseType::Searched(searched) => {
                    debug!("{}Searched CASE", get_indent(indent + 1));
                    debug!(
                        "{}WHEN branches: {}",
                        get_indent(indent + 2),
                        searched.when_branches.len()
                    );
                    for (i, branch) in searched.when_branches.iter().enumerate() {
                        debug!("{}Branch {}", get_indent(indent + 3), i);
                        debug!("{}WHEN condition:", get_indent(indent + 4));
                        print_expression(&branch.condition, indent + 5);
                        debug!("{}THEN:", get_indent(indent + 4));
                        print_expression(&branch.then_expression, indent + 5);
                    }
                    if let Some(else_expr) = &searched.else_expression {
                        debug!("{}ELSE:", get_indent(indent + 2));
                        print_expression(else_expr, indent + 3);
                    }
                }
            }
        }
        Expression::PathConstructor(path_constructor) => {
            debug!("{}PathConstructor", get_indent(indent));
            debug!("{}Elements:", get_indent(indent + 1));
            for (i, element) in path_constructor.elements.iter().enumerate() {
                debug!("{}Element {}:", get_indent(indent + 2), i);
                print_expression(element, indent + 3);
            }
        }
        Expression::Cast(cast_expr) => {
            debug!("{}CastExpression", get_indent(indent));
            debug!("{}Expression:", get_indent(indent + 1));
            print_expression(&cast_expr.expression, indent + 2);
            debug!(
                "{}Target Type: {:?}",
                get_indent(indent + 1),
                cast_expr.target_type
            );
        }
        Expression::Subquery(subquery_expr) => {
            debug!("{}SubqueryExpression", get_indent(indent));
            debug!("{}Query:", get_indent(indent + 1));
            print_query(&subquery_expr.query, indent + 2);
        }
        Expression::ExistsSubquery(subquery_expr) => {
            debug!("{}ExistsSubqueryExpression", get_indent(indent));
            debug!("{}Query:", get_indent(indent + 1));
            print_query(&subquery_expr.query, indent + 2);
        }
        Expression::NotExistsSubquery(subquery_expr) => {
            debug!("{}NotExistsSubqueryExpression", get_indent(indent));
            debug!("{}Query:", get_indent(indent + 1));
            print_query(&subquery_expr.query, indent + 2);
        }
        Expression::InSubquery(subquery_expr) => {
            debug!("{}InSubqueryExpression", get_indent(indent));
            debug!("{}Expression:", get_indent(indent + 1));
            print_expression(&subquery_expr.expression, indent + 2);
            debug!("{}Subquery:", get_indent(indent + 1));
            print_query(&subquery_expr.query, indent + 2);
        }
        Expression::NotInSubquery(subquery_expr) => {
            debug!("{}NotInSubqueryExpression", get_indent(indent));
            debug!("{}Expression:", get_indent(indent + 1));
            print_expression(&subquery_expr.expression, indent + 2);
            debug!("{}Subquery:", get_indent(indent + 1));
            print_query(&subquery_expr.query, indent + 2);
        }
        Expression::QuantifiedComparison(quantified_expr) => {
            debug!(
                "{}QuantifiedComparison ({:?})",
                get_indent(indent),
                quantified_expr.quantifier
            );
            debug!("{}Left:", get_indent(indent + 1));
            print_expression(&quantified_expr.left, indent + 2);
            debug!(
                "{}Operator: {:?}",
                get_indent(indent + 1),
                quantified_expr.operator
            );
            debug!("{}Subquery:", get_indent(indent + 1));
            print_expression(&quantified_expr.subquery, indent + 2);
        }
        Expression::IsPredicate(is_predicate) => {
            debug!("{}IsPredicateExpression", get_indent(indent));
            debug!("{}Subject:", get_indent(indent + 1));
            print_expression(&is_predicate.subject, indent + 2);
            debug!(
                "{}Predicate Type: {:?}",
                get_indent(indent + 1),
                is_predicate.predicate_type
            );
            debug!(
                "{}Negated: {}",
                get_indent(indent + 1),
                is_predicate.negated
            );
            if let Some(ref target) = is_predicate.target {
                debug!("{}Target:", get_indent(indent + 1));
                print_expression(target, indent + 2);
            }
            if let Some(ref type_spec) = is_predicate.type_spec {
                debug!("{}Type Spec: {:?}", get_indent(indent + 1), type_spec);
            }
        }
        Expression::ArrayIndex(array_index) => {
            debug!("{}ArrayIndexExpression", get_indent(indent));
            debug!("{}Array:", get_indent(indent + 1));
            print_expression(&array_index.array, indent + 2);
            debug!("{}Index:", get_indent(indent + 1));
            print_expression(&array_index.index, indent + 2);
        }
        Expression::Parameter(parameter) => {
            debug!("{}Parameter: ${}", get_indent(indent), parameter.name);
        }
        Expression::Pattern(pattern_expr) => {
            debug!("{}PatternExpression", get_indent(indent));
            print_path_pattern(&pattern_expr.pattern, indent + 1);
        }
    }
}

fn print_select_statement(select_stmt: &SelectStatement, indent: usize) {
    debug!("{}Distinct: {:?}", get_indent(indent), select_stmt.distinct);

    match &select_stmt.return_items {
        SelectItems::Wildcard { .. } => {
            debug!("{}Return Items: * (wildcard)", get_indent(indent));
        }
        SelectItems::Explicit { items, .. } => {
            debug!("{}Return Items ({})", get_indent(indent), items.len());
            for (i, item) in items.iter().enumerate() {
                debug!("{}ReturnItem {}", get_indent(indent + 1), i);
                print_return_item(item, indent + 2);
            }
        }
    }

    if let Some(from_clause) = &select_stmt.from_clause {
        debug!("{}From Clause", get_indent(indent));
        print_from_clause(from_clause, indent + 1);
    }

    if let Some(where_clause) = &select_stmt.where_clause {
        debug!("{}Where Clause", get_indent(indent));
        print_where_clause(where_clause, indent + 1);
    }

    if let Some(group_clause) = &select_stmt.group_clause {
        debug!("{}Group By Clause", get_indent(indent));
        print_group_clause(group_clause, indent + 1);
    }

    if let Some(having_clause) = &select_stmt.having_clause {
        debug!("{}Having Clause", get_indent(indent));
        print_having_clause(having_clause, indent + 1);
    }

    if let Some(order_clause) = &select_stmt.order_clause {
        debug!("{}Order By Clause", get_indent(indent));
        print_order_clause(order_clause, indent + 1);
    }

    if let Some(limit_clause) = &select_stmt.limit_clause {
        debug!("{}Limit Clause", get_indent(indent));
        print_limit_clause(limit_clause, indent + 1);
    }
}

fn print_from_clause(from_clause: &FromClause, indent: usize) {
    debug!(
        "{}Graph Expressions ({})",
        get_indent(indent),
        from_clause.graph_expressions.len()
    );
    for (i, graph_expr) in from_clause.graph_expressions.iter().enumerate() {
        debug!("{}GraphExpression {}", get_indent(indent + 1), i);
        print_from_graph_expression(graph_expr, indent + 2);
    }
}

fn print_from_graph_expression(graph_expr: &FromGraphExpression, indent: usize) {
    debug!("{}Graph Expression", get_indent(indent));
    print_graph_expression(&graph_expr.graph_expression, indent + 1);

    if let Some(match_clause) = &graph_expr.match_statement {
        debug!("{}Match Clause", get_indent(indent));
        print_match_clause(match_clause, indent + 1);
    }
}

fn print_graph_expression(graph_expr: &GraphExpression, indent: usize) {
    match graph_expr {
        GraphExpression::Reference(path) => {
            debug!("{}Reference: {}", get_indent(indent), path);
        }
        GraphExpression::Union { left, right, all } => {
            debug!("{}Union (ALL: {})", get_indent(indent), all);
            debug!("{}Left", get_indent(indent + 1));
            print_graph_expression(left, indent + 2);
            debug!("{}Right", get_indent(indent + 1));
            print_graph_expression(right, indent + 2);
        }
        GraphExpression::CurrentGraph => {
            debug!("{}CurrentGraph (session graph)", get_indent(indent));
        }
    }
}

fn print_group_clause(group_clause: &GroupClause, indent: usize) {
    debug!(
        "{}Expressions ({})",
        get_indent(indent),
        group_clause.expressions.len()
    );
    for (i, expr) in group_clause.expressions.iter().enumerate() {
        debug!("{}Expression {}", get_indent(indent + 1), i);
        print_expression(expr, indent + 2);
    }
}

fn print_having_clause(having_clause: &HavingClause, indent: usize) {
    debug!("{}Condition", get_indent(indent));
    print_expression(&having_clause.condition, indent + 1);
}

fn print_order_clause(order_clause: &OrderClause, indent: usize) {
    debug!("{}Items ({})", get_indent(indent), order_clause.items.len());
    for (i, item) in order_clause.items.iter().enumerate() {
        debug!("{}OrderItem {}", get_indent(indent + 1), i);
        print_order_item(item, indent + 2);
    }
}

fn print_order_item(order_item: &OrderItem, indent: usize) {
    debug!("{}Expression", get_indent(indent));
    print_expression(&order_item.expression, indent + 1);
    debug!(
        "{}Direction: {:?}",
        get_indent(indent),
        order_item.direction
    );
    if let Some(nulls_ordering) = &order_item.nulls_ordering {
        debug!("{}Nulls Ordering: {:?}", get_indent(indent), nulls_ordering);
    }
}

fn print_limit_clause(limit_clause: &LimitClause, indent: usize) {
    debug!("{}Count: {}", get_indent(indent), limit_clause.count);
    if let Some(offset) = limit_clause.offset {
        debug!("{}Offset: {}", get_indent(indent), offset);
    }
}

fn print_transaction_statement(transaction_stmt: &TransactionStatement, indent: usize) {
    match transaction_stmt {
        TransactionStatement::StartTransaction(start_stmt) => {
            debug!("{}{}", get_indent(indent), "Start Transaction Statement");
            if let Some(ref characteristics) = start_stmt.characteristics {
                debug!("{}Characteristics:", get_indent(indent + 1));
                if let Some(ref isolation_level) = characteristics.isolation_level {
                    debug!(
                        "{}Isolation Level: {}",
                        get_indent(indent + 2),
                        isolation_level.as_str()
                    );
                }
                if let Some(ref access_mode) = characteristics.access_mode {
                    debug!(
                        "{}Access Mode: {}",
                        get_indent(indent + 2),
                        access_mode.as_str()
                    );
                }
            }
        }
        TransactionStatement::Commit(commit_stmt) => {
            debug!("{}{}", get_indent(indent), "Commit Statement");
            if commit_stmt.work {
                debug!("{}Work: true", get_indent(indent + 1));
            }
        }
        TransactionStatement::Rollback(rollback_stmt) => {
            debug!("{}{}", get_indent(indent), "Rollback Statement");
            if rollback_stmt.work {
                debug!("{}Work: true", get_indent(indent + 1));
            }
        }
        TransactionStatement::SetTransactionCharacteristics(set_stmt) => {
            debug!(
                "{}{}",
                get_indent(indent),
                "Set Transaction Characteristics Statement"
            );
            debug!("{}Characteristics:", get_indent(indent + 1));
            if let Some(ref isolation_level) = set_stmt.characteristics.isolation_level {
                debug!(
                    "{}Isolation Level: {}",
                    get_indent(indent + 2),
                    isolation_level.as_str()
                );
            }
            if let Some(ref access_mode) = set_stmt.characteristics.access_mode {
                debug!(
                    "{}Access Mode: {}",
                    get_indent(indent + 2),
                    access_mode.as_str()
                );
            }
        }
    }
}

fn get_indent(level: usize) -> String {
    "  ".repeat(level)
}
