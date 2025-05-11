use once_cell::sync::Lazy;
use sqlformat::{FormatOptions, Indent};
use sqlparser::{ast::{Delete, Distinct, Expr, Fetch, Function, FunctionArg, FunctionArgExpr, FunctionArgumentClause, FunctionArguments, GroupByExpr, Ident, Insert, Join, JoinConstraint, JsonTableColumnErrorHandling, NamedWindowExpr, Offset, OffsetRows, Query, ReplaceSelectItem, Select, SelectItem, SetExpr, Statement, TableFactor, TableVersion, Top, TopQuantity, Value, WildcardAdditionalOptions, WindowFrame, WindowFrameBound, WindowSpec}, dialect::{Dialect, GenericDialect, PostgreSqlDialect}, parser::Parser};
use std::collections::{HashMap, HashSet};


static COUNT_STMT: Lazy<SelectItem> = Lazy::new(|| get_sample_select_count());

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Mode {
    Select,
    Update,
    Delete,
    Insert
}

pub struct ValidateQueryResult {
    pub sql: String,
    pub params: Vec<String>
}



fn get_sample_select_count() -> SelectItem {
    let sql = "select COUNT(1) from t1";
    let dialect = GenericDialect {};
    let mut ast = Parser::parse_sql(&dialect, sql).unwrap();
    match ast.pop().unwrap() {
        Statement::Query(query) => {
            if let SetExpr::Select(select) = *query.body {
                let Select { mut projection, .. } = *select;
                return projection.pop().unwrap()
            }
        }
        _ => {}
    }
    panic!("Failed to get sample select count")
} 

pub fn convert_to_page_query(sql: &str, dialect: &dyn Dialect, params: &Vec<String>) -> Result<ValidateQueryResult, String> {
    let mut ast = Parser::parse_sql(dialect, sql).map_err(|x| format!("Parse SQL error. May be due to improperly syntax"))?;

    if ast.len() != 1 {
        return Err("Expected exactly one SQL statement".into());
    }

    match &mut ast[0] {
        Statement::Query(query) => {
            if let SetExpr::Select(select) = query.body.as_mut() {
                if query.offset.is_some() {
                    return Err("Query has OFFSET statement".into())
                }
                if query.limit.is_some() {
                    return Err("Query has LIMIT statement".into())
                }
                query.offset.replace(Offset { 
                    value: Expr::Value(Value::Placeholder(":offset".to_string())), 
                    rows: OffsetRows::None 
                });
                query.limit.replace(Expr::Value(Value::Placeholder(":limit".to_string())));
                let sql = ast[0].to_string();

                return validate_query(&sql, params, Some(Mode::Select), dialect)
            } else {
                Err("Unsupported query type".into())
            }
        },
        _ => Err("Expected a SELECT query".into()),
    }
}

pub fn convert_to_count_query(sql: &str, dialect: &dyn Dialect) -> Result<String, String> {
    let mut ast = Parser::parse_sql(dialect, sql).map_err(|x| format!("Parse SQL error. May be due to improperly syntax"))?;

    if ast.len() != 1 {
        return Err("Expected exactly one SQL statement".into());
    }

    match &mut ast[0] {
        Statement::Query(query) => {
            if let SetExpr::Select(select) = query.body.as_mut() {
                let Select { projection, .. } = &mut **select;
                projection.clear();
                projection.push(COUNT_STMT.clone());
                query.order_by.clear();
                Ok(ast[0].to_string())
            } else {
                Err("Unsupported query type".into())
            }
        },
        _ => Err("Expected a SELECT query".into()),
    }
}

fn validate_statement(statement: Statement, params: &Vec<String>, mode: Option<Mode>) -> Result<ValidateQueryResult, String> {
    let mut res = vec![];
    from_statement(&statement, &mut res)?; // Could do better by using trait and impl trait for every struct in sqlparser::ast
    for placeholer in res.as_slice() {
        if !params.contains(&placeholer[1..].to_string()) {
            return Err(format!("Holder {placeholer} is not found in param list"));
        }
    }
    let sql = statement.to_string();
    let (sql, params) = replace_placeholder(&sql, res);
    Ok(ValidateQueryResult { sql, params })
}

pub fn validate_multi_query(sql: &str, params: &Vec<String>, dialect: &dyn Dialect) -> Result<Vec<ValidateQueryResult>, String> {
    let mut statements = Parser::parse_sql(dialect, sql).map_err(|x| format!("Parse SQL error. May be due to improperly syntax"))?;
    statements.into_iter().map(|statement| validate_statement(statement, params, None)).collect()
}
pub fn validate_query(sql: &str, params: &Vec<String>, mode: Option<Mode>, dialect: &dyn Dialect) -> Result<ValidateQueryResult, String> {
    let mut statements = Parser::parse_sql(dialect, sql).map_err(|x| format!("Parse SQL error. May be due to improperly syntax"))?;
    if statements.len() != 1 {
        return Err("Only one statement is allowed".to_string())
    }
    let statement = statements.pop().unwrap();
    match statement {
        sqlparser::ast::Statement::Query(_) => if mode.is_some() && mode != Some(Mode::Select) {return Err("Select statement is not allowed here".to_string())},
        sqlparser::ast::Statement::Insert(_) => if mode.is_some() && mode != Some(Mode::Insert) {return Err("Insert statement is allowed here".to_string())},
        sqlparser::ast::Statement::Update { .. } => if mode.is_some() && mode != Some(Mode::Update) {return Err("Update statement is allowed here".to_string())},
        sqlparser::ast::Statement::Delete(_) => if mode.is_some() && mode != Some(Mode::Delete) {return Err("Delete statement is allowed here".to_string())},
        _ => if mode.is_some() {return Err("Only Select, Insert, Update, Delete statement is allowed".to_string())}
    }
    validate_statement(statement, params, mode)
}


fn replace_placeholder(s: &str, placeholder: Vec<String>) -> (String, Vec<String>) {
    let mut result = String::from(s);
    let mut keyword_order = Vec::new();
    let mut counter = 1;

    for keyword in placeholder {
        while let Some(pos) = result.find(&keyword) {
            let placeholder = format!("${}", counter);
            keyword_order.push(keyword.clone());
            result.replace_range(pos..pos + keyword.len(), &placeholder);
            counter += 1;
        }
    }

    (result, keyword_order)
}


pub fn get_value_place_holder(sql: &str, dialect: &dyn Dialect) -> Result<Vec<String>, String>{
    let mut statements = Parser::parse_sql(dialect, sql).map_err(|x| format!("Parse error. May be due to improperly syntax"))?;
    if statements.len() != 1 {
        return Err("Only one statement is allowed".to_string())
    }
    let statement = statements.pop().unwrap();

    let mut res = vec![];
    from_statement(&statement, &mut res)?; // Could do better by using trait and impl trait for every struct in sqlparser::ast
    println!("{}", statement.to_string());
    Ok(res)
}

#[inline]
fn from_value(val: &Value, res: &mut Vec<String>) -> Result<(), String> {
    match val {
        Value::Placeholder(s) => {
            let s = s.trim();
            if !s.starts_with(":") {
                return Err(format!("Holder '{s}' must be start with ':'"))
            }
            if !s.is_ascii() {
                return Err(format!("Holder must not contain non-ascii char: '{s}'"))
            }
            if s.chars().nth(1).expect("Holder name too short").is_digit(10) {
                return Err(format!("Holder must not start with number: '{s}'"))
            }
            res.push(s.to_string());
            Ok(())
        },
        _ => Ok(())
    }
}

fn from_statement(statement: &Statement, res: &mut Vec<String>) -> Result<(), String> {
    match statement {
        sqlparser::ast::Statement::Query(query) => from_query(query.as_ref(), res),
        sqlparser::ast::Statement::Insert(insert) => from_insert(insert, res),
        sqlparser::ast::Statement::Update { table, assignments, from, selection, returning } => {
            from_table_factor(&table.relation, res)?;
            for l1 in &table.joins {
                from_join(l1, res)?;
            }
            if let Some(x) = from {
                from_table_factor(&x.relation, res)?;
                for l1 in &x.joins {
                    from_join(l1, res)?;
                }
            }
            for l1 in assignments {
                from_expr(&l1.value, res)?;
            }
            if let Some(selection) = selection {
                from_expr(&selection, res)?;
            }
            if let Some(returning) = returning {
                for item in returning {
                    match item {
                        SelectItem::UnnamedExpr(x) => from_expr(x, res)?,
                        SelectItem::ExprWithAlias { expr, alias: _ } => from_expr(expr, res)?,
                        SelectItem::QualifiedWildcard(_, x) => from_wildcard_additional_options(x, res)?,
                        SelectItem::Wildcard(x) => from_wildcard_additional_options(x, res)?,
                    }
                }
            }
            Ok(())
        },
        sqlparser::ast::Statement::Delete(delete) => from_delete(delete, res),
        // _ => return Err("Only Query, Insert, Update, Delete statement is allowed".to_string())
        _ => Ok(())
        
    }

}

#[inline]
fn from_delete(delete: &Delete, res: &mut Vec<String>) -> Result<(), String> {
    if let Some(using) = &delete.using {
        for x in using {
            from_table_factor(&x.relation, res)?;
            for l1 in &x.joins {
                from_join(l1, res)?;
            }
        }
    }
    if let Some(selection) = &delete.selection {
        from_expr(&selection, res)?;
    }
    if let Some(returning) = &delete.returning {
        for item in returning {
            match item {
                SelectItem::UnnamedExpr(x) => from_expr(x, res)?,
                SelectItem::ExprWithAlias { expr, alias: _ } => from_expr(expr, res)?,
                SelectItem::QualifiedWildcard(_, x) => from_wildcard_additional_options(x, res)?,
                SelectItem::Wildcard(x) => from_wildcard_additional_options(x, res)?,
            }
        }
    }
    for x in &delete.order_by {
        from_expr(&x.expr, res)?;
    }
    if let Some(limit) = &delete.limit {
        from_expr(&limit, res)?;
    }

    Ok(())
}

#[inline]
fn from_insert(insert: &Insert, res: &mut Vec<String>) -> Result<(), String> {
    if let Some(source) = &insert.source {
        from_query(source.as_ref(), res)?;
    }
    if let Some(partitioned) = &insert.partitioned {
        for x in partitioned {
            from_expr(x, res)?;
        }
    }
    if let Some(on) = &insert.on {
        match on {
            sqlparser::ast::OnInsert::DuplicateKeyUpdate(x) => {
                for l1 in x {
                    from_expr(&l1.value, res)?;
                }
            },
            _ => {},
        }
    }
    if let Some(returning) = &insert.returning {
        for item in returning {
            match item {
                SelectItem::UnnamedExpr(x) => from_expr(x, res)?,
                SelectItem::ExprWithAlias { expr, alias: _ } => from_expr(expr, res)?,
                SelectItem::QualifiedWildcard(_, x) => from_wildcard_additional_options(x, res)?,
                SelectItem::Wildcard(x) => from_wildcard_additional_options(x, res)?,
            }
        }
    }
    Ok(())
}

#[inline]
fn from_join_contraint(join: &JoinConstraint, res: &mut Vec<String>) -> Result<(), String> {
    match &join {
        JoinConstraint::On(x) => from_expr(x, res),
        JoinConstraint::Using(_) => Ok(()),
        JoinConstraint::Natural => Ok(()),
        JoinConstraint::None => Ok(()),
    }
}

#[inline]
fn from_join(join: &Join, res: &mut Vec<String>) -> Result<(), String> {
    from_table_factor(&join.relation, res)?;
    match &join.join_operator {
        sqlparser::ast::JoinOperator::Inner(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::LeftOuter(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::RightOuter(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::FullOuter(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::CrossJoin => Ok(()),
        sqlparser::ast::JoinOperator::LeftSemi(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::RightSemi(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::LeftAnti(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::RightAnti(x) => from_join_contraint(x, res),
        sqlparser::ast::JoinOperator::CrossApply => Ok(()),
        sqlparser::ast::JoinOperator::OuterApply => Ok(()),
        sqlparser::ast::JoinOperator::AsOf { match_condition, constraint } => {
            from_expr(match_condition, res)?;
            from_join_contraint(constraint, res)?;
            Ok(())
        }
    }
}

#[inline]
fn from_window_frame_bound(window: &WindowFrameBound, res: &mut Vec<String>) -> Result<(), String> {
    match window {
        WindowFrameBound::CurrentRow => Ok(()),
        WindowFrameBound::Preceding(x) => {
            if let Some(x) = x {
                from_expr(x.as_ref(), res)?;
            }
            Ok(())
        },
        WindowFrameBound::Following(x) => {
            if let Some(x) = x {
                from_expr(x.as_ref(), res)?;
            }
            Ok(())
        },
    }
    
}

#[inline]
fn from_window_spec(window: &WindowSpec, res: &mut Vec<String>) -> Result<(), String> {
    for x in &window.partition_by {
        from_expr(x, res)?;
    }
    for x in &window.order_by {
        from_expr(&x.expr, res)?;
    }
    if let Some(WindowFrame {start_bound, end_bound, ..}) = &window.window_frame {
        from_window_frame_bound(start_bound, res)?;
        if let Some(end_bound) = &end_bound {
            from_window_frame_bound(end_bound, res)?;
        }
    }
    Ok(())
}

#[inline]
fn from_table_factor(factor: &TableFactor, res: &mut Vec<String>) -> Result<(), String> {
    match factor {
        TableFactor::Table { args, with_hints, version, .. } => {
            if let Some(args) = args {
                for x in args {
                    from_function_arg(x, res)?;
                }
            }
            for x in with_hints {
                from_expr(x, res)?;
            }
            if let Some(TableVersion::ForSystemTimeAsOf(expr)) = version {
                from_expr(expr, res)?;
            }
            Ok(())
        },
        TableFactor::Derived { lateral, subquery, alias } => from_query(subquery.as_ref(), res),
        TableFactor::TableFunction { expr, alias } => from_expr(&expr, res),
        TableFactor::Function {  args, .. } => {
            for x in args {
                from_function_arg(x, res)?;
            }
            Ok(())
        },
        TableFactor::UNNEST {  array_exprs, .. } => {
            for x in array_exprs {
                from_expr(x, res)?;
            }
            Ok(())
        },
        TableFactor::JsonTable { json_expr, json_path, columns, alias: _ } => {
            from_expr(json_expr, res)?;
            from_value(json_path, res)?;
            for x in columns {
                from_value(&x.path, res)?;
                if let Some(JsonTableColumnErrorHandling::Default(l1)) = &x.on_empty {
                    from_value(l1, res)?;
                }
                if let Some(JsonTableColumnErrorHandling::Default(l1)) = &x.on_error {
                    from_value(l1, res)?;
                }
            }
            Ok(())
        },
        TableFactor::NestedJoin { table_with_joins, alias: _ } => {
            from_table_factor(&table_with_joins.relation, res)?;
            for x in &table_with_joins.joins {
                from_join(x, res)?
            }
            Ok(())
        },
        TableFactor::Pivot { table, aggregate_functions, value_column: _, value_source, default_on_null, alias: _ } => {
            from_table_factor(table.as_ref(), res)?;
            for x in aggregate_functions {
                from_expr(&x.expr, res)?;
            }
            match value_source {
                sqlparser::ast::PivotValueSource::List(x) => {
                    for l1 in x {
                        from_expr(&l1.expr, res)?;
                    }
                },
                sqlparser::ast::PivotValueSource::Any(x) => {
                    for l1 in x {
                        from_expr(&l1.expr, res)?;
                    }
                },
                sqlparser::ast::PivotValueSource::Subquery(x) => {
                    from_query(x, res)?
                },
            }
            if let Some(x) = default_on_null {
                from_expr(x, res)?;
            }
            Ok(())
        },
        TableFactor::Unpivot { table,  .. } => from_table_factor(table.as_ref(), res),
        TableFactor::MatchRecognize { table, partition_by, order_by, measures, rows_per_match, after_match_skip, pattern, symbols, alias } => {
            from_table_factor(table.as_ref(), res)?;
            for x in partition_by {
                from_expr(x, res)?;
            }
            for x in order_by {
                from_expr(&x.expr, res)?;
            }
            for x in measures {
                from_expr(&x.expr, res)?;
            }
            for x in symbols {
                from_expr(&x.definition, res)?;
            }
            Ok(())
        },
    }
}

#[inline]
fn from_select(select: &Select, res: &mut Vec<String>) -> Result<(), String> {
    if let Some(Distinct::On(exprs)) = &select.distinct {
        for x in exprs {
            from_expr(x, res)?;
        }
    }
    if let Some(Top {quantity: Some(TopQuantity::Expr(x)), ..}) = &select.top {
        from_expr(x, res)?;
    }
    for projection in &select.projection {
        match projection {
            SelectItem::UnnamedExpr(x) => from_expr(x, res)?,
            SelectItem::ExprWithAlias { expr, .. } => from_expr(expr, res)?,
            SelectItem::QualifiedWildcard(_, x) => from_wildcard_additional_options(x, res)?,
            SelectItem::Wildcard(x) => from_wildcard_additional_options(x, res)?,
        }
    }

    for from in &select.from {
        from_table_factor(&from.relation, res)?;
        for join in &from.joins {
            from_join(join, res)?
        }
    }
    for view in &select.lateral_views {
        from_expr(&view.lateral_view, res)?;
    }
    if let Some(x) = &select.selection {
        from_expr(x, res)?;
    }
    if let GroupByExpr::Expressions(exprs) = &select.group_by {
        for x in exprs {
            from_expr(x, res)?;
        }
    }
    for x in &select.cluster_by {
        from_expr(x, res)?;
    }
    for x in &select.distribute_by {
        from_expr(x, res)?;
    }
    for x in &select.sort_by {
        from_expr(x, res)?;
    }
        for x in &select.sort_by {
        from_expr(x, res)?;
    }
    if let Some(x) = &select.having {
        from_expr(x, res)?;
    }

    for x in &select.named_window {
        if let NamedWindowExpr::WindowSpec(l1) = &x.1 {
            from_window_spec(l1, res)?;
        }
    }
    if let Some(x) = &select.qualify {
        from_expr(x, res)?;
    }
    if let Some(x) = &select.connect_by {
        from_expr(&x.condition, res)?;
        for l1 in &x.relationships {
            from_expr(&l1, res)?;
        }
    }

    Ok(())
}

#[inline]
fn from_wildcard_additional_options(option: &WildcardAdditionalOptions, res: &mut Vec<String>) -> Result<(), String> {
    if let Some(ReplaceSelectItem {items}) = &option.opt_replace {
        for x in items {
            from_expr(&x.expr, res)?;
        }
    }
    Ok(())
}

#[inline]
fn from_set_expr(set_expr: &SetExpr, res: &mut Vec<String>) -> Result<(), String> {
    match set_expr {
        SetExpr::Select(x) => from_select(x.as_ref(), res),
        SetExpr::Query(x) => from_query(x.as_ref(), res),
        SetExpr::SetOperation {  left, right , ..} => {
            from_set_expr(left.as_ref(), res)?;
            from_set_expr(right.as_ref(), res)?;
            Ok(())
        },
        SetExpr::Values(x) => {
            for l1 in &x.rows {
                for l2 in l1 {
                    from_expr(l2, res)?;
                }
            }
            Ok(())
        },
        SetExpr::Insert(x) => from_statement(x, res),
        SetExpr::Update(x) => from_statement(x, res),
        SetExpr::Table(_) => Ok(()),
    }
}

#[inline]
fn from_query(query: &Query, res: &mut Vec<String>) -> Result<(), String> {
    if let Some(with) = &query.with {
        for x in &with.cte_tables {
            from_query(x.query.as_ref(), res)?;
        }
    }
    from_set_expr(query.body.as_ref(), res)?;
    for x in &query.order_by {
        from_expr(&x.expr, res)?;
    }
    if let Some(limit) = &query.limit {
        from_expr(&limit, res)?;
    }
    for x in &query.limit_by {
        from_expr(x, res)?;
    }
    if let Some(offset) = &query.offset {
        from_expr(&offset.value, res)?;
    }
    if let Some(Fetch {quantity: Some(expr), ..}) = &query.fetch {
        from_expr(expr, res)?;
    }
    Ok(())
}

#[inline]
fn from_function_arg_clause(args: &FunctionArgumentClause, res: &mut Vec<String>) -> Result<(), String> {
    match args {
        FunctionArgumentClause::IgnoreOrRespectNulls(_) => Ok(()),
        FunctionArgumentClause::OrderBy(x) => {
            for l2 in x {
                from_expr(&l2.expr, res)?;
            }
            Ok(())
        },
        FunctionArgumentClause::Limit(x) => from_expr(x, res),
        FunctionArgumentClause::OnOverflow(x) => {
            match x {
                sqlparser::ast::ListAggOnOverflow::Truncate { filler: Some(x), with_count } => from_expr(x.as_ref(), res),
                _ => Ok(())
            }
        },
        FunctionArgumentClause::Having(x) => from_expr(&x.1, res),
        FunctionArgumentClause::Separator(x) => from_value(x, res),
    }
}

#[inline]
fn from_function_arg(args: &FunctionArg, res: &mut Vec<String>) -> Result<(), String> {
    match &args {
        FunctionArg::Named {  arg: FunctionArgExpr::Expr(expr), .. } => from_expr(expr, res),
        FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => from_expr(expr, res),
        _ => Ok(())
    }
}

#[inline]
fn from_function_arguments(args: &FunctionArguments, res: &mut Vec<String>) -> Result<(), String> {
    match &args {
        FunctionArguments::None => Ok(()),
        FunctionArguments::Subquery(query) => from_query(query.as_ref(), res),
        FunctionArguments::List(arg_list) => {
            for x in &arg_list.args {
                from_function_arg(x, res)?;
            }
            for x in &arg_list.clauses {
                from_function_arg_clause(x, res)?;
            }
            Ok(())
        }
    }
}

#[inline]
fn from_function(func: &Function, res: &mut Vec<String>) -> Result<(), String> {
    if let Some(x) = &func.filter {
        from_expr(x.as_ref(), res)?;
    }
    for x in &func.within_group {
        from_expr(&x.expr, res)?;
    }
    from_function_arguments(&func.args, res)?;
    Ok(())
}



#[inline]
fn from_expr(expr: &Expr, res: &mut Vec<String>) -> Result<(), String> {
    match expr {
        Expr::Identifier(x) => Ok(()),
        Expr::CompoundIdentifier(x) => Ok(()),
        Expr::JsonAccess { value, path } => from_expr(&**value, res),
        Expr::CompositeAccess { expr, key } => from_expr(&**expr, res),
        Expr::IsFalse(x) => from_expr(&**x, res),
        Expr::IsNotFalse(x) => from_expr(&**x, res),
        Expr::IsTrue(x) => from_expr(&**x, res),
        Expr::IsNotTrue(x) => from_expr(&**x, res),
        Expr::IsNull(x) => from_expr(&**x, res),
        Expr::IsNotNull(x) => from_expr(&**x, res),
        Expr::IsUnknown(x) => from_expr(&**x, res),
        Expr::IsNotUnknown(x) => from_expr(&**x, res),
        Expr::IsDistinctFrom(x, y) => {from_expr(&**x, res)?; from_expr(&**y, res)},
        Expr::IsNotDistinctFrom(x, y) => {from_expr(&**x, res)?; from_expr(&**y, res)},
        Expr::InList { expr, list, negated } => {
            from_expr(&**expr, res)?;
            for e in list {
                from_expr(e, res)?;
            }
            Ok(())
        },
        Expr::InSubquery { expr, subquery, negated } => {from_expr(&**expr, res)?; from_query(&**subquery, res)},
        Expr::InUnnest { expr, array_expr, negated } => {from_expr(&**expr, res)?; from_expr(&**array_expr, res)},
        Expr::Between { expr, negated, low, high } => {from_expr(&**expr, res)?; from_expr(&**low, res)?; from_expr(&**high, res)},
        Expr::BinaryOp { left, op, right } => {from_expr(&**left, res)?; from_expr(&**right, res)},
        Expr::Like { negated, expr, pattern, escape_char } => {from_expr(&**expr, res)?; from_expr(&**pattern, res)},
        Expr::ILike { negated, expr, pattern, escape_char } => {from_expr(&**expr, res)?; from_expr(&**pattern, res)},
        Expr::SimilarTo { negated, expr, pattern, escape_char } => {from_expr(&**expr, res)?; from_expr(&**pattern, res)},
        Expr::RLike { negated, expr, pattern, regexp } => {from_expr(&**expr, res)?; from_expr(&**pattern, res)},
        Expr::AnyOp { left, compare_op, right } => {from_expr(&**left, res)?; from_expr(&**right, res)},
        Expr::AllOp { left, compare_op, right } => {from_expr(&**left, res)?; from_expr(&**right, res)},
        Expr::UnaryOp { op, expr } => from_expr(&**expr, res),
        Expr::Convert { expr, data_type, charset, target_before_value, styles } => from_expr(&**expr, res),
        Expr::Cast { kind, expr, data_type, format } => from_expr(&**expr, res),
        Expr::AtTimeZone { timestamp, time_zone } => {from_expr(&**timestamp, res)?; from_expr(&**time_zone, res)},
        Expr::Extract { field, expr } => from_expr(&**expr, res),
        Expr::Ceil { expr, field } => from_expr(&**expr, res),
        Expr::Floor { expr, field } => from_expr(&**expr, res),
        Expr::Position { expr, r#in } => {from_expr(&**expr, res)?; from_expr(&**r#in, res)},
        Expr::Substring { expr, substring_from, substring_for, special } => {
            from_expr(&**expr, res)?;
            if let Some(expr) = substring_from {
                from_expr(&**expr, res)?;
            }
            if let Some(expr) = substring_for {
                from_expr(&**expr, res)?;
            }
            Ok(())
        },
        Expr::Trim { expr, trim_where, trim_what, trim_characters } => {
            from_expr(&**expr, res)?;
            if let Some(expr) = trim_what {
                from_expr(&**expr, res)?;
            }
            Ok(())
        },
        Expr::Overlay { expr, overlay_what, overlay_from, overlay_for } => {
            from_expr(&**expr, res)?;
            from_expr(&**overlay_what, res)?;
            from_expr(&**overlay_from, res)?;
            if let Some(expr) = overlay_for {
                from_expr(&**expr, res)?;
            }
            Ok(())
        },
        Expr::Collate { expr, collation } => from_expr(&**expr, res),
        Expr::Nested(x) => from_expr(&**x, res),
        Expr::Value(x) => from_value(x, res),
        Expr::IntroducedString { introducer, value } => from_value(value, res),
        Expr::TypedString { data_type, value } => Ok(()),
        Expr::MapAccess { column, keys } => from_expr(&**column, res),
        Expr::Function(x) => from_function(x, res),
        Expr::Case { operand, conditions, results, else_result } => {
            for l2 in conditions {
                from_expr(l2, res)?;
            }
            for l2 in results {
                from_expr(l2, res)?;
            }
            if let Some(l2) = operand {
                from_expr(&**l2, res)?;
            }
            Ok(())
        },
        Expr::Exists { subquery, negated } => from_query(&**subquery, res),
        Expr::Subquery(x) => from_query(&**x, res),
        Expr::GroupingSets(x) => {
            for l1 in x {
                for l2 in l1 {
                    from_expr(l2, res)?;
                }
            }
            Ok(())
        },
        Expr::Cube(x) => {
            for l1 in x {
                for l2 in l1 {
                    from_expr(l2, res)?;
                }
            }
            Ok(())
        },
        Expr::Rollup(x) => {
            for l1 in x {
                for l2 in l1 {
                    from_expr(l2, res)?;
                }
            }
            Ok(())
        },
        Expr::Tuple(x) => {
            for l1 in x {
                from_expr(l1, res)?;
            }
            Ok(())
        },
        Expr::Struct { values, fields } => {
            for l1 in values {
                from_expr(l1, res)?;
            }
            Ok(())
        },
        Expr::Named { expr, name } => from_expr(&**expr, res),
        Expr::Dictionary(x) => {
            for l1 in x {
                from_expr(&*l1.value, res)?;
            }
            Ok(())
        },
        Expr::Subscript { expr, subscript } => {
            from_expr(&**expr, res)?;
            match &**subscript {
                sqlparser::ast::Subscript::Index { index } => from_expr(index, res)?,
                sqlparser::ast::Subscript::Slice { lower_bound, upper_bound, stride } => {
                    if let Some(expr) = lower_bound {
                        from_expr(expr, res)?;
                    }
                    if let Some(expr) = upper_bound {
                        from_expr(expr, res)?;
                    }
                    if let Some(expr) = stride {
                        from_expr(expr, res)?;
                    }
                    return Ok(())
                },
            }
            Ok(())
        },
        Expr::Array(x) => {
            for l1 in &x.elem {
                from_expr(l1, res)?;
            }
            Ok(())
        },
        Expr::Interval(x) => from_expr(x.value.as_ref(), res),
        Expr::MatchAgainst { columns, match_value, opt_search_modifier } => from_value(match_value, res),
        Expr::Wildcard => Ok(()),
        Expr::QualifiedWildcard(x) => Ok(()),
        Expr::OuterJoin(x) => from_expr(&**x, res),
        Expr::Prior(x) => from_expr(&**x, res),
        Expr::Lambda(x) => from_expr(x.body.as_ref(), res),
    }
}

#[derive(Debug, Default)]
struct ColumnTableList {
    columns: HashSet<String>,
    tables: HashSet<String>,
}

impl ColumnTableList {
    fn add_columns(&mut self, column: &Ident) -> Result<(), String>{
        self.columns.insert(column.value.clone());
        Ok(())
    }

    fn add_tables(&mut self, ids: &Vec<Ident>) -> Result<(), String> {
        match ids.len() {
            0 => Ok(()),
            1 => self.add_columns(ids.get(0).unwrap()),
            2 => {
                let table = ids.get(0).unwrap();
                let column = ids.get(1).unwrap();
                self.columns.insert(column.value.clone());
                self.tables.insert(table.value.clone());
                Ok(())
            },
            _ => Err("Too much CompoundIdentifier".into())
        }
    }

    fn check_value(&self, val: &Value) -> Result<(), String> {
        match val {
            Value::Placeholder(_) => Err("Placeholder is not allowed".into()),
            _ => Ok(())
        }
    }
}

fn extract_columns_and_compound_ids(expr: &Expr, res: &mut ColumnTableList) -> Result<(), String> {
    match expr {
        Expr::Identifier(x) => Ok(res.add_columns(x)?),
        Expr::CompoundIdentifier(x) => Ok(res.add_tables(x)?),
        Expr::JsonAccess { value, path } => extract_columns_and_compound_ids(&**value, res),
        Expr::CompositeAccess { expr, key } => extract_columns_and_compound_ids(&**expr, res),
        Expr::IsFalse(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsNotFalse(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsTrue(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsNotTrue(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsNull(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsNotNull(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsUnknown(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsNotUnknown(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::IsDistinctFrom(x, y) => {extract_columns_and_compound_ids(&**x, res)?; extract_columns_and_compound_ids(&**y, res)},
        Expr::IsNotDistinctFrom(x, y) => {extract_columns_and_compound_ids(&**x, res)?; extract_columns_and_compound_ids(&**y, res)},
        Expr::InList { expr, list, negated } => {
            extract_columns_and_compound_ids(&**expr, res)?;
            for e in list {
                extract_columns_and_compound_ids(e, res)?;
            }
            Ok(())
        },
        Expr::InSubquery { expr, subquery, negated } => return Err("Subquery is not supported".into()),
        Expr::InUnnest { expr, array_expr, negated } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**array_expr, res)},
        Expr::Between { expr, negated, low, high } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**low, res)?; extract_columns_and_compound_ids(&**high, res)},
        Expr::BinaryOp { left, op, right } => {extract_columns_and_compound_ids(&**left, res)?; extract_columns_and_compound_ids(&**right, res)},
        Expr::Like { negated, expr, pattern, escape_char } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**pattern, res)},
        Expr::ILike { negated, expr, pattern, escape_char } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**pattern, res)},
        Expr::SimilarTo { negated, expr, pattern, escape_char } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**pattern, res)},
        Expr::RLike { negated, expr, pattern, regexp } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**pattern, res)},
        Expr::AnyOp { left, compare_op, right } => {extract_columns_and_compound_ids(&**left, res)?; extract_columns_and_compound_ids(&**right, res)},
        Expr::AllOp { left, compare_op, right } => {extract_columns_and_compound_ids(&**left, res)?; extract_columns_and_compound_ids(&**right, res)},
        Expr::UnaryOp { op, expr } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Convert { expr, data_type, charset, target_before_value, styles } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Cast { kind, expr, data_type, format } => extract_columns_and_compound_ids(&**expr, res),
        Expr::AtTimeZone { timestamp, time_zone } => {extract_columns_and_compound_ids(&**timestamp, res)?; extract_columns_and_compound_ids(&**time_zone, res)},
        Expr::Extract { field, expr } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Ceil { expr, field } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Floor { expr, field } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Position { expr, r#in } => {extract_columns_and_compound_ids(&**expr, res)?; extract_columns_and_compound_ids(&**r#in, res)},
        Expr::Substring { expr, substring_from, substring_for, special } => {
            extract_columns_and_compound_ids(&**expr, res)?;
            if let Some(expr) = substring_from {
                extract_columns_and_compound_ids(&**expr, res)?;
            }
            if let Some(expr) = substring_for {
                extract_columns_and_compound_ids(&**expr, res)?;
            }
            Ok(())
        },
        Expr::Trim { expr, trim_where, trim_what, trim_characters } => {
            extract_columns_and_compound_ids(&**expr, res)?;
            if let Some(expr) = trim_what {
                extract_columns_and_compound_ids(&**expr, res)?;
            }
            Ok(())
        },
        Expr::Overlay { expr, overlay_what, overlay_from, overlay_for } => {
            extract_columns_and_compound_ids(&**expr, res)?;
            extract_columns_and_compound_ids(&**overlay_what, res)?;
            extract_columns_and_compound_ids(&**overlay_from, res)?;
            if let Some(expr) = overlay_for {
                extract_columns_and_compound_ids(&**expr, res)?;
            }
            Ok(())
        },
        Expr::Collate { expr, collation } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Nested(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::Value(x) => res.check_value(x),
        Expr::IntroducedString { introducer, value } => res.check_value(value),
        Expr::TypedString { data_type, value } => return Ok(()),
        Expr::MapAccess { column, keys } => extract_columns_and_compound_ids(&**column, res),
        Expr::Function(x) => return Err("Function is not supported".into()),
        Expr::Case { operand, conditions, results, else_result } => {
            for l2 in conditions {
                extract_columns_and_compound_ids(l2, res)?;
            }
            for l2 in results {
                extract_columns_and_compound_ids(l2, res)?;
            }
            if let Some(l2) = operand {
                extract_columns_and_compound_ids(&**l2, res)?;
            }
            Ok(())
        },
        Expr::Exists { subquery, negated } => return Err("Exists is not supported".into()),
        Expr::Subquery(x) => return Err("Subquery is not supported".into()),
        Expr::GroupingSets(x) => {
            for l1 in x {
                for l2 in l1 {
                    extract_columns_and_compound_ids(l2, res)?;
                }
            }
            Ok(())
        },
        Expr::Cube(x) => {
            for l1 in x {
                for l2 in l1 {
                    extract_columns_and_compound_ids(l2, res)?;
                }
            }
            Ok(())
        },
        Expr::Rollup(x) => {
            for l1 in x {
                for l2 in l1 {
                    extract_columns_and_compound_ids(l2, res)?;
                }
            }
            Ok(())
        },
        Expr::Tuple(x) => {
            for l1 in x {
                extract_columns_and_compound_ids(l1, res)?;
            }
            Ok(())
        },
        Expr::Struct { values, fields } => {
            for l1 in values {
                extract_columns_and_compound_ids(l1, res)?;
            }
            Ok(())
        },
        Expr::Named { expr, name } => extract_columns_and_compound_ids(&**expr, res),
        Expr::Dictionary(x) => {
            for l1 in x {
                extract_columns_and_compound_ids(&*l1.value, res)?;
            }
            Ok(())
        },
        Expr::Subscript { expr, subscript } => {
            extract_columns_and_compound_ids(&**expr, res)?;
            match &**subscript {
                sqlparser::ast::Subscript::Index { index } => extract_columns_and_compound_ids(index, res)?,
                sqlparser::ast::Subscript::Slice { lower_bound, upper_bound, stride } => {
                    if let Some(expr) = lower_bound {
                        extract_columns_and_compound_ids(expr, res)?;
                    }
                    if let Some(expr) = upper_bound {
                        extract_columns_and_compound_ids(expr, res)?;
                    }
                    if let Some(expr) = stride {
                        extract_columns_and_compound_ids(expr, res)?;
                    }
                    return Ok(())
                },
            }
            Ok(())
        },
        Expr::Array(x) => {
            for l1 in &x.elem {
                extract_columns_and_compound_ids(l1, res)?;
            }
            Ok(())
        },
        Expr::Interval(x) => extract_columns_and_compound_ids(x.value.as_ref(), res),
        Expr::MatchAgainst { columns, match_value, opt_search_modifier } => return Ok(()),
        Expr::Wildcard => Ok(()),
        Expr::QualifiedWildcard(x) => Ok(()),
        Expr::OuterJoin(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::Prior(x) => extract_columns_and_compound_ids(&**x, res),
        Expr::Lambda(x) => extract_columns_and_compound_ids(x.body.as_ref(), res),
    }
}

pub fn get_columns_and_compound_ids(sql: &str, dialect: Box<dyn Dialect>) -> Result<(HashSet<String>, HashSet<String>), String> {
    
    let mut p = Parser::new(dialect.as_ref())
        .try_with_sql(sql)
        .map_err(|e| format!("Parse SQL error: {e}"))?;
    let expr = p.parse_expr().map_err(|e| format!("Parse Expr error: {e}"))?;
    let mut res = ColumnTableList::default();
    extract_columns_and_compound_ids(&expr, &mut res)?;
    Ok((res.columns, res.tables))
}

#[test]
fn test() {
    let sql = "
        select c1 ->> :column from t1 t where t.user = :user and t.age = :a1 and timestamp(t.full -> :a1 ) like '%:name%'
        ";
        let dialect = PostgreSqlDialect {}; 
        // let ast = Parser::parse_sql(&dialect, sql);
        let a = get_value_place_holder(sql, &dialect);
        let sql = sqlformat::format(sql, &sqlformat::QueryParams::None, FormatOptions {
            indent: Indent::Spaces(2),
            uppercase: true,
            lines_between_queries: 0,
        });
        let (new_sql, ordered_param) = replace_placeholder(&sql, a.expect("Failed to get value placeholder"));
        dbg!(new_sql);
        dbg!(ordered_param);

        // println!("{sql}");
        // dbg!(a);

        let s = "SELECT c ->> :name, d ->> 'age' ->> :age FROM t1 WHERE user = :name and age = :age";
        let keywords = vec![":name".to_string(), ":age".to_string()];
    
        let (new_s, keyword_order) = replace_placeholder(s, keywords);
    
        println!("Chuỗi đầu ra: {}", new_s);
        println!("Thứ tự keywords: {:?}", keyword_order);
}


#[test]
fn test_query() {
    let sql = "
        select t.cali, t.baque, t.xola from t1 t where t.user = :user  offset 0
        ";
        let dialect = PostgreSqlDialect {}; 
    let ast = Parser::parse_sql(&dialect, sql).unwrap();
    dbg!(ast);
    
}

#[test]
fn test_expr() {
    let sql = "
         t.user = :user and EXCLUDED.id > tt.id
        ";
    let dialect = PostgreSqlDialect {}; 
    let res = get_columns_and_compound_ids(sql, Box::new(dialect));
    dbg!(&res);
    assert!(res.is_err());
    
    let sql = "
    t.user = 'user' and EXCLUDED.id > tt.id
    ";
    let dialect = PostgreSqlDialect {}; 
    let res = get_columns_and_compound_ids(sql, Box::new(dialect));
    dbg!(&res);
    assert!(res.is_ok());
    let (cols, tables) = res.unwrap();
    assert!(cols.contains("user"));
    assert!(cols.contains("id"));
    assert!(tables.contains("t"));
    assert!(tables.contains("EXCLUDED"));
    
}

// #[test]
// fn test_page_query() {
//     let sql = "
//         select t.cali, t.baque, t.xola from t1 t where t.user = :user and t.age = :a1 and timestamp(t.full -> :a1 ) like '%:name%' limit :name
//         ";
//         let dialect = PostgreSqlDialect {}; 
//     // let query = convert_to_page_query(sql, &PostgreSqlDialect {});
//     dbg!(query);
    
// }

#[test]
fn test_count_query() {
    let sql = "
        select t.cali, t.baque, t.xola from t1 t where t.user = :user and t.age = :a1 and timestamp(t.full -> :a1 ) like '%:name%'
        ";
        let dialect = PostgreSqlDialect {}; 
    let count_query = convert_to_count_query(sql, &PostgreSqlDialect {});
    dbg!(count_query);
    
}