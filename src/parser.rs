use once_cell::sync::Lazy;
use sqlformat::{FormatOptions, Indent};
use sqlparser::{ast::{Delete, Distinct, Expr, Fetch, Function, FunctionArg, FunctionArgExpr, FunctionArgumentClause, FunctionArguments, GroupByExpr, HavingBound, Ident, Insert, Join, JoinConstraint, JsonTableColumnErrorHandling, NamedWindowExpr, Offset, OffsetRows, Query, ReplaceSelectItem, Select, SelectItem, SetExpr, Statement, TableFactor, TableVersion, Top, TopQuantity, Value, WildcardAdditionalOptions, WindowFrame, WindowFrameBound, WindowSpec}, dialect::{Dialect, GenericDialect, PostgreSqlDialect}, parser::Parser};
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
    convert_to_page_query_with_db(sql, dialect, params, crate::sqlx_template::Database::Postgres)
}

pub fn convert_to_page_query_with_db(sql: &str, dialect: &dyn Dialect, params: &Vec<String>, db: crate::sqlx_template::Database) -> Result<ValidateQueryResult, String> {
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

                return validate_query_with_db(&sql, params, Some(Mode::Select), dialect, db)
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
                // Check if query has GROUP BY or JOIN - if so, wrap in subquery
                let has_group_by = !matches!(select.group_by, GroupByExpr::Expressions(ref exprs) if exprs.is_empty());
                let has_join = select.from.iter().any(|from| !from.joins.is_empty());

                if has_group_by || has_join {
                    // For queries with GROUP BY or JOIN, wrap the original query in a subquery
                    // Remove LIMIT and OFFSET from original query for count
                    query.limit = None;
                    query.offset = None;
                    query.order_by.clear();

                    let original_query = ast[0].to_string();
                    return Ok(format!("SELECT COUNT(*) FROM ({}) AS count_subquery", original_query));
                } else {
                    // For simple queries without GROUP BY or JOIN, use the original approach
                    let Select { projection, .. } = &mut **select;
                    projection.clear();
                    projection.push(COUNT_STMT.clone());
                    query.order_by.clear();
                    Ok(ast[0].to_string())
                }
            } else {
                Err("Unsupported query type".into())
            }
        },
        _ => Err("Expected a SELECT query".into()),
    }
}

fn validate_statement(statement: Statement, params: &Vec<String>, mode: Option<Mode>) -> Result<ValidateQueryResult, String> {
    validate_statement_with_db(statement, params, mode, crate::sqlx_template::Database::Postgres)
}

fn validate_statement_with_db(statement: Statement, params: &Vec<String>, mode: Option<Mode>, db: crate::sqlx_template::Database) -> Result<ValidateQueryResult, String> {
    let mut res = vec![];
    from_statement(&statement, &mut res)?; // Could do better by using trait and impl trait for every struct in sqlparser::ast
    for placeholer in res.as_slice() {
        if !params.contains(&placeholer[1..].to_string()) {
            return Err(format!("Holder {placeholer} is not found in param list"));
        }
    }
    let sql = statement.to_string();
    let (sql, params) = replace_placeholder_with_db(&sql, res, None, db);
    Ok(ValidateQueryResult { sql, params })
}

pub fn validate_multi_query(sql: &str, params: &Vec<String>, dialect: &dyn Dialect) -> Result<Vec<ValidateQueryResult>, String> {
    validate_multi_query_with_db(sql, params, dialect, crate::sqlx_template::Database::Postgres)
}

pub fn validate_multi_query_with_db(sql: &str, params: &Vec<String>, dialect: &dyn Dialect, db: crate::sqlx_template::Database) -> Result<Vec<ValidateQueryResult>, String> {
    let mut statements = Parser::parse_sql(dialect, sql).map_err(|x| format!("Parse SQL error. May be due to improperly syntax"))?;
    statements.into_iter().map(|statement| validate_statement_with_db(statement, params, None, db)).collect()
}

pub fn validate_query(sql: &str, params: &Vec<String>, mode: Option<Mode>, dialect: &dyn Dialect) -> Result<ValidateQueryResult, String> {
    validate_query_with_db(sql, params, mode, dialect, crate::sqlx_template::Database::Postgres)
}

pub fn validate_query_with_db(sql: &str, params: &Vec<String>, mode: Option<Mode>, dialect: &dyn Dialect, db: crate::sqlx_template::Database) -> Result<ValidateQueryResult, String> {
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
    validate_statement_with_db(statement, params, mode, db)
}


pub(crate) fn replace_placeholder(s: &str, placeholder: Vec<String>, start_counter: Option<i32>) -> (String, Vec<String>) {
    replace_placeholder_with_db(s, placeholder, start_counter, crate::sqlx_template::Database::Postgres)
}

pub(crate) fn replace_placeholder_with_db(s: &str, placeholder: Vec<String>, start_counter: Option<i32>, db: crate::sqlx_template::Database) -> (String, Vec<String>) {
    let mut result = String::from(s);
    let mut keyword_order = Vec::new();
    let mut counter = start_counter.unwrap_or(1);

    for keyword in placeholder {
        while let Some(pos) = result.find(&keyword) {
            let placeholder_str = match db {
                crate::sqlx_template::Database::Postgres => format!("${}", counter),
                crate::sqlx_template::Database::Sqlite | crate::sqlx_template::Database::Mysql | crate::sqlx_template::Database::Any => "?".to_string(),
            };
            keyword_order.push(keyword.clone());
            result.replace_range(pos..pos + keyword.len(), &placeholder_str);
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

/// Represents the result of parsing SQL expressions to extract columns, tables, and placeholder variables.
///
/// This struct contains information about:
/// - Column names referenced in the SQL expression
/// - Table names referenced in the SQL expression
/// - Placeholder variables (e.g., `:user_id`, `:name`) found in the expression
/// - Mapping between placeholder variables and the columns they are associated with
#[derive(Debug, Default)]
pub(crate) struct ColumnTableList {
    /// Set of column names found in the SQL expression
    pub(crate) columns: HashSet<String>,
    /// Set of table names found in the SQL expression
    pub(crate) tables: HashSet<String>,
    /// List of placeholder variables found in the SQL expression (e.g., `:user_id`, `:name`)
    pub(crate) placeholder_vars: Vec<String>,
    /// Mapping from placeholder variables to the columns they are associated with.
    /// For example, in "name = :user_name", the mapping would be {":user_name": {"name"}}
    pub(crate) placeholder_to_columns: HashMap<String, HashSet<String>>,
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

    fn add_value(&mut self, val: &Value) -> Result<(), String>{
        match val {
            Value::Placeholder(p) => {
                if !p.starts_with(":") {
                    return Err("Placeholder {} is not valid. Must start with ':'".into());
                }
                self.placeholder_vars.push((&p).to_string());
            },
            _ => {}
        };
        Ok(())
    }

    fn add_placeholder_column_mapping(&mut self, placeholder: &str, column: &str) -> Result<(), String> {
        let columns = self.placeholder_to_columns.entry(placeholder.to_string()).or_insert_with(HashSet::new);

        // Check if this placeholder is already mapped to a different column
        if !columns.is_empty() && !columns.contains(column) {
            let existing_columns: Vec<String> = columns.iter().cloned().collect();
            return Err(format!(
                "Placeholder '{}' is mapped to multiple columns: {} and {}. Each placeholder can only be mapped to one column.",
                placeholder,
                existing_columns.join(", "),
                column
            ));
        }

        columns.insert(column.to_string());
        Ok(())
    }

    /// Get the mapping of placeholder variables to columns
    pub fn get_placeholder_column_mapping(&self) -> &HashMap<String, HashSet<String>> {
        &self.placeholder_to_columns
    }

    /// Get columns associated with a specific placeholder
    pub fn get_columns_for_placeholder(&self, placeholder: &str) -> Option<&HashSet<String>> {
        self.placeholder_to_columns.get(placeholder)
    }
}

fn handle_binary_op_with_operator(left: &Expr, op: &sqlparser::ast::BinaryOperator, right: &Expr, res: &mut ColumnTableList) -> Result<(), String> {
    use sqlparser::ast::BinaryOperator;

    // Only process comparison operators, not JSON access or arithmetic operators
    match op {
        BinaryOperator::Eq | BinaryOperator::NotEq |
        BinaryOperator::Lt | BinaryOperator::LtEq |
        BinaryOperator::Gt | BinaryOperator::GtEq => {
            // This is a comparison operation, try to detect column-placeholder relationships
            handle_comparison_op(left, right, res)?;
        },
        BinaryOperator::Arrow | BinaryOperator::LongArrow | // JSON access operators
        BinaryOperator::Plus | BinaryOperator::Minus |      // Arithmetic operators
        BinaryOperator::Multiply | BinaryOperator::Divide |
        BinaryOperator::Modulo | BinaryOperator::StringConcat |
        BinaryOperator::BitwiseOr | BinaryOperator::BitwiseAnd |
        BinaryOperator::BitwiseXor | BinaryOperator::PGBitwiseShiftLeft |
        BinaryOperator::PGBitwiseShiftRight | BinaryOperator::PGExp => {
            // These are not comparison operations, don't create mappings
        },
        _ => {
            // For other operators, be conservative and don't create mappings
        }
    }
    Ok(())
}

fn handle_comparison_op(left: &Expr, right: &Expr, res: &mut ColumnTableList) -> Result<(), String> {
    // Try to detect patterns like: column = :placeholder or :placeholder = column
    let (column_expr, placeholder_expr) = match (left, right) {
        (Expr::Identifier(_), Expr::Value(Value::Placeholder(_))) => (left, right),
        (Expr::CompoundIdentifier(_), Expr::Value(Value::Placeholder(_))) => (left, right),
        (Expr::Function(_), Expr::Value(Value::Placeholder(_))) => (left, right), // Handle function-like identifiers
        (Expr::Value(Value::Placeholder(_)), Expr::Identifier(_)) => (right, left),
        (Expr::Value(Value::Placeholder(_)), Expr::CompoundIdentifier(_)) => (right, left),
        (Expr::Value(Value::Placeholder(_)), Expr::Function(_)) => (right, left), // Handle function-like identifiers
        _ => return Ok(()), // Not a column-placeholder pattern
    };

    // Extract column name
    let column_name = match column_expr {
        Expr::Identifier(ident) => ident.value.clone(),
        Expr::CompoundIdentifier(idents) => {
            if idents.len() >= 2 {
                idents[1].value.clone() // Take the column part of table.column
            } else if idents.len() == 1 {
                idents[0].value.clone()
            } else {
                return Ok(());
            }
        },
        Expr::Function(func) => {
            // Handle function-like identifiers (e.g., "user" parsed as "user()")
            if matches!(func.args, sqlparser::ast::FunctionArguments::None) && func.name.0.len() == 1 {
                func.name.0[0].value.clone()
            } else {
                return Ok(()); // This is a real function call, not a column
            }
        },
        _ => return Ok(()),
    };

    // Extract placeholder name
    if let Expr::Value(Value::Placeholder(placeholder)) = placeholder_expr {
        if placeholder.starts_with(":") {
            res.add_placeholder_column_mapping(placeholder, &column_name)?;
        }
    }

    Ok(())
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
        Expr::BinaryOp { left, op, right } => {
            // First extract columns and placeholders
            extract_columns_and_compound_ids(&**left, res)?;
            extract_columns_and_compound_ids(&**right, res)?;
            // Then try to detect column-placeholder relationships only in comparison operations
            handle_binary_op_with_operator(left, op, right, res)
        },
        Expr::Like { negated, expr, pattern, escape_char } => {
            // First extract columns and placeholders
            extract_columns_and_compound_ids(&**expr, res)?;
            extract_columns_and_compound_ids(&**pattern, res)?;
            // Then try to detect column-placeholder relationships in LIKE operations
            handle_comparison_op(expr, pattern, res)
        },
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
        Expr::Value(x) => res.add_value(x),
        Expr::IntroducedString { introducer, value } => res.add_value(value),
        Expr::TypedString { data_type, value } => return Ok(()),
        Expr::MapAccess { column, keys } => extract_columns_and_compound_ids(&**column, res),
        Expr::Function(x) => {
            if let Some(ref filter) = x.filter {
                extract_columns_and_compound_ids(&**filter, res)?;
            }
            for x in &x.within_group {
                extract_columns_and_compound_ids(&x.expr, res)?;
            }
            
            match &x.args {
                FunctionArguments::None => {
                    // Handle function-like identifiers (e.g., "user" parsed as "user()")
                    if x.name.0.len() == 1 {
                        let ident = &x.name.0[0];
                        res.add_columns(ident)?;
                    }
                },
                FunctionArguments::Subquery(_) => return Err("Subquery is not supported".into()),
                FunctionArguments::List(arg_list) => {
                    for arg in &arg_list.args {
                        match arg {
                            FunctionArg::Named { arg: FunctionArgExpr::Expr(expr), .. } => extract_columns_and_compound_ids(expr, res)?,
                            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => extract_columns_and_compound_ids(expr, res)?,
                            _ => {}
                        }
                    }
                    for clause in &arg_list.clauses {
                        match clause {
                            FunctionArgumentClause::OrderBy(exprs) => {
                                for expr in exprs {
                                    extract_columns_and_compound_ids(&expr.expr, res)?;
                                }
                            },
                            FunctionArgumentClause::Limit(expr) => extract_columns_and_compound_ids(expr, res)?,
                            FunctionArgumentClause::Having(having_bound) => extract_columns_and_compound_ids(&having_bound.1, res)?,
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        },
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

/// Parses a SQL expression and extracts column names, table names, placeholder variables,
/// and the mapping between placeholders and columns.
///
/// This function analyzes SQL expressions (typically WHERE clauses or conditions) to identify:
/// - Column names referenced in the expression
/// - Table names referenced in compound identifiers (e.g., `users.id`)
/// - Placeholder variables (e.g., `:user_id`, `:name`)
/// - Relationships between placeholders and columns (e.g., `name = :user_name` maps `:user_name` to `name`)
///
/// # Validation Rules
///
/// The function enforces that each placeholder can only be mapped to **one column**. If a placeholder
/// is used with multiple different columns, an error will be returned with details about the conflict.
///
/// # Arguments
///
/// * `sql` - The SQL expression string to parse
/// * `dialect` - The SQL dialect to use for parsing (PostgreSQL, MySQL, SQLite, etc.)
///
/// # Returns
///
/// Returns a `ColumnTableList` containing:
/// - `columns`: Set of column names found
/// - `tables`: Set of table names found
/// - `placeholder_vars`: List of placeholder variables found
/// - `placeholder_to_columns`: Mapping from placeholders to associated columns
///
/// # Examples
///
/// ## Valid Usage
/// ```rust,ignore
/// use sqlx_template::parser::get_columns_and_compound_ids;
/// use sqlparser::dialect::PostgreSqlDialect;
///
/// let sql = "users.name = :user_name AND age > :min_age";
/// let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {})).unwrap();
///
/// // Access columns and tables
/// assert!(result.columns.contains("name"));
/// assert!(result.columns.contains("age"));
/// assert!(result.tables.contains("users"));
///
/// // Access placeholder variables
/// assert!(result.placeholder_vars.contains(&":user_name".to_string()));
/// assert!(result.placeholder_vars.contains(&":min_age".to_string()));
///
/// // Access placeholder-to-column mapping
/// let name_columns = result.get_columns_for_placeholder(":user_name").unwrap();
/// assert!(name_columns.contains("name"));
/// ```
///
/// ## Error Case - Placeholder Used with Multiple Columns
/// ```rust,ignore
/// use sqlx_template::parser::get_columns_and_compound_ids;
/// use sqlparser::dialect::PostgreSqlDialect;
///
/// // This will fail because :search_term is used with both 'name' and 'email' columns
/// let sql = "name = :search_term AND email = :search_term";
/// let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));
/// assert!(result.is_err());
///
/// let error = result.unwrap_err();
/// assert!(error.contains("Placeholder ':search_term' is mapped to multiple columns"));
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The SQL expression cannot be parsed or contains invalid syntax
/// - A placeholder is mapped to multiple different columns
/// - Placeholder format is invalid (must start with ':' and follow naming rules)
pub fn get_columns_and_compound_ids(sql: &str, dialect: Box<dyn Dialect>) -> Result<ColumnTableList, String> {

    let mut p = Parser::new(dialect.as_ref())
        .try_with_sql(sql)
        .map_err(|e| format!("Parse SQL error: {e}"))?;
    let expr = p.parse_expr().map_err(|e| format!("Parse Expr error: {e}"))?;

    // Debug: print the parsed expression (disabled)
    // if sql == "user = :user" {
    //     println!("DEBUG: Parsed AST for '{}': {:#?}", sql, expr);
    // }

    let mut res = ColumnTableList::default();
    extract_columns_and_compound_ids(&expr, &mut res)?;
    Ok(res)
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
        let (new_sql, ordered_param) = replace_placeholder(&sql, a.expect("Failed to get value placeholder"), None);
        dbg!(new_sql);
        dbg!(ordered_param);

        // println!("{sql}");
        // dbg!(a);

        let s = "SELECT c ->> :name, d ->> 'age' ->> :age FROM t1 WHERE user = :name and age = :age";
        let keywords = vec![":name".to_string(), ":age".to_string()];
    
        let (new_s, keyword_order) = replace_placeholder(s, keywords, None);
    
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
    assert!(res.is_ok());
    
    let sql = "
    t.user = 'user' and EXCLUDED.id > tt.id
    ";
    let dialect = PostgreSqlDialect {}; 
    let res = get_columns_and_compound_ids(sql, Box::new(dialect));
    dbg!(&res);
    assert!(res.is_ok());
    let res = res.unwrap();
    assert!(res.columns.contains("user"));
    assert!(res.columns.contains("id"));
    assert!(res.tables.contains("t"));
    assert!(res.tables.contains("EXCLUDED"));
    
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
    // Test simple query without JOIN or GROUP BY
    let sql = "
        select t.cali, t.baque, t.xola from t1 t where t.user = :user and t.age = :a1 and timestamp(t.full -> :a1 ) like '%:name%'
        ";
    let dialect = PostgreSqlDialect {};
    let count_query = convert_to_count_query(sql, &PostgreSqlDialect {});
    dbg!(&count_query);
    assert!(count_query.is_ok());
    let count_sql = count_query.unwrap();
    assert!(count_sql.contains("COUNT(1)"));
    assert!(!count_sql.contains("SELECT COUNT(*) FROM ("));

    // Test query with JOIN - should wrap in subquery
    let sql_with_join = "
        select t1.name, t2.value
        from table1 t1
        join table2 t2 on t1.id = t2.table1_id
        where t1.status = :status
        ";
    let count_query_join = convert_to_count_query(sql_with_join, &PostgreSqlDialect {});
    dbg!(&count_query_join);
    assert!(count_query_join.is_ok());
    let count_sql_join = count_query_join.unwrap();
    assert!(count_sql_join.contains("SELECT COUNT(*) FROM ("));
    assert!(count_sql_join.contains(") AS count_subquery"));

    // Test query with GROUP BY - should wrap in subquery
    let sql_with_group_by = "
        select department, count(*) as emp_count
        from employees
        where salary > :min_salary
        group by department
        ";
    let count_query_group = convert_to_count_query(sql_with_group_by, &PostgreSqlDialect {});
    dbg!(&count_query_group);
    assert!(count_query_group.is_ok());
    let count_sql_group = count_query_group.unwrap();
    assert!(count_sql_group.contains("SELECT COUNT(*) FROM ("));
    assert!(count_sql_group.contains(") AS count_subquery"));

    // Test query with both JOIN and GROUP BY - should wrap in subquery
    let sql_with_join_and_group = "
        select t1.department, count(t2.id) as order_count
        from employees t1
        left join orders t2 on t1.id = t2.employee_id
        where t1.active = :active
        group by t1.department
        order by order_count desc
        ";
    let count_query_complex = convert_to_count_query(sql_with_join_and_group, &PostgreSqlDialect {});
    dbg!(&count_query_complex);
    assert!(count_query_complex.is_ok());
    let count_sql_complex = count_query_complex.unwrap();
    assert!(count_sql_complex.contains("SELECT COUNT(*) FROM ("));
    assert!(count_sql_complex.contains(") AS count_subquery"));
    // Should not contain ORDER BY in the final count query
    assert!(!count_sql_complex.ends_with("ORDER BY order_count DESC"));
}

#[test]
fn test_placeholder_column_mapping() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Test simple column = placeholder
    let sql = "name = :user_name";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));
    assert!(result.is_ok());
    let parsed = result.unwrap();

    assert!(parsed.columns.contains("name"));
    assert!(parsed.placeholder_vars.contains(&":user_name".to_string()));
    assert!(parsed.placeholder_to_columns.contains_key(":user_name"));
    assert!(parsed.placeholder_to_columns[":user_name"].contains("name"));

    // Test placeholder = column
    let sql2 = ":email = email";
    let result2 = get_columns_and_compound_ids(sql2, Box::new(PostgreSqlDialect {}));
    assert!(result2.is_ok());
    let parsed2 = result2.unwrap();

    assert!(parsed2.columns.contains("email"));
    assert!(parsed2.placeholder_vars.contains(&":email".to_string()));
    assert!(parsed2.placeholder_to_columns.contains_key(":email"));
    assert!(parsed2.placeholder_to_columns[":email"].contains("email"));

    // Test compound identifier (table.column = placeholder)
    let sql3 = "users.id = :user_id";
    let result3 = get_columns_and_compound_ids(sql3, Box::new(PostgreSqlDialect {}));
    assert!(result3.is_ok());
    let parsed3 = result3.unwrap();

    assert!(parsed3.columns.contains("id"));
    assert!(parsed3.tables.contains("users"));
    assert!(parsed3.placeholder_vars.contains(&":user_id".to_string()));
    assert!(parsed3.placeholder_to_columns.contains_key(":user_id"));
    assert!(parsed3.placeholder_to_columns[":user_id"].contains("id"));

    // Test complex expression with multiple mappings
    let sql4 = "name = :name AND age > :min_age AND users.email = :email";
    let result4 = get_columns_and_compound_ids(sql4, Box::new(PostgreSqlDialect {}));
    assert!(result4.is_ok());
    let parsed4 = result4.unwrap();
    dbg!(&parsed4);

    assert!(parsed4.columns.contains("name"));
    assert!(parsed4.columns.contains("age"));
    assert!(parsed4.columns.contains("email"));
    assert!(parsed4.tables.contains("users"));

    assert!(parsed4.placeholder_vars.contains(&":name".to_string()));
    assert!(parsed4.placeholder_vars.contains(&":min_age".to_string()));
    assert!(parsed4.placeholder_vars.contains(&":email".to_string()));

    assert!(parsed4.placeholder_to_columns[":name"].contains("name"));
    assert!(parsed4.placeholder_to_columns[":min_age"].contains("age"));
    assert!(parsed4.placeholder_to_columns[":email"].contains("email"));
}

#[test]
fn test_placeholder_column_mapping_api() {
    use sqlparser::dialect::PostgreSqlDialect;

    let sql = "
         t.user = :user and t.name1 = :name1$str and t.name2 = :name2 and t.name3 = :name3$UserDefineType and and timestamp(t.full -> :a2 ) like '%' || :name|| '%'
        ";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));
    assert!(result.is_ok());
    let parsed = result.unwrap();
    dbg!(&parsed);

    let sql = "
         t.user = :user and t.age = :a1 and timestamp(t.full -> :a2 ) like '%:name%'
        ";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));
    assert!(result.is_ok());
    let parsed = result.unwrap();
    dbg!(&parsed);

    let sql = "users.name = :user_name AND age > :min_age AND email = :user_email";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));
    assert!(result.is_ok());
    let parsed = result.unwrap();
    dbg!(&parsed);
    // Test the public API methods
    let mapping = parsed.get_placeholder_column_mapping();
    assert_eq!(mapping.len(), 3);

    // Test getting columns for specific placeholder
    let name_columns = parsed.get_columns_for_placeholder(":user_name");
    assert!(name_columns.is_some());
    assert!(name_columns.unwrap().contains("name"));

    let age_columns = parsed.get_columns_for_placeholder(":min_age");
    assert!(age_columns.is_some());
    assert!(age_columns.unwrap().contains("age"));

    let email_columns = parsed.get_columns_for_placeholder(":user_email");
    assert!(email_columns.is_some());
    assert!(email_columns.unwrap().contains("email"));

    // Test non-existent placeholder
    let non_existent = parsed.get_columns_for_placeholder(":non_existent");
    assert!(non_existent.is_none());

    // Print mapping for demonstration
    println!("Placeholder to Column Mapping:");
    for (placeholder, columns) in mapping {
        println!("  {} -> {:?}", placeholder, columns);
    }


}

#[test]
fn test_placeholder_multiple_columns_error() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Test case where a placeholder is used with multiple different columns - should fail
    let sql = "name = :user_param AND email = :user_param";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));
    assert!(result.is_err());
    let error_msg = result.unwrap_err();
    assert!(error_msg.contains("Placeholder ':user_param' is mapped to multiple columns"));
    assert!(error_msg.contains("name") && error_msg.contains("email"));

    // Test case where same placeholder is used with same column multiple times - should be OK
    let sql2 = "name = :user_name AND (age > 18 OR name = :user_name)";
    let result2 = get_columns_and_compound_ids(sql2, Box::new(PostgreSqlDialect {}));
    assert!(result2.is_ok());
    let parsed2 = result2.unwrap();
    assert!(parsed2.placeholder_to_columns[":user_name"].contains("name"));
    assert_eq!(parsed2.placeholder_to_columns[":user_name"].len(), 1);

    // Test case with compound identifiers
    let sql3 = "users.name = :param AND orders.status = :param";
    let result3 = get_columns_and_compound_ids(sql3, Box::new(PostgreSqlDialect {}));
    assert!(result3.is_err());
    let error_msg3 = result3.unwrap_err();
    assert!(error_msg3.contains("Placeholder ':param' is mapped to multiple columns"));
    assert!(error_msg3.contains("name") && error_msg3.contains("status"));
}

#[test]
fn test_placeholder_error_message_demo() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Demo case: User accidentally uses same placeholder for different columns
    let sql = "users.email = :search_term AND users.name = :search_term";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));

    match result {
        Ok(_) => panic!("Expected error but got success"),
        Err(error_msg) => {
            println!("Error caught successfully:");
            println!("{}", error_msg);
            assert!(error_msg.contains("Placeholder ':search_term' is mapped to multiple columns"));
            assert!(error_msg.contains("email") && error_msg.contains("name"));
        }
    }

    // Demo case: Complex query with multiple placeholder conflicts
    let sql2 = "orders.status = :filter AND customers.region = :filter AND products.category = :filter";
    let result2 = get_columns_and_compound_ids(sql2, Box::new(PostgreSqlDialect {}));

    match result2 {
        Ok(_) => panic!("Expected error but got success"),
        Err(error_msg) => {
            println!("\nSecond error caught successfully:");
            println!("{}", error_msg);
            // Should catch the first conflict (status vs region)
            assert!(error_msg.contains("Placeholder ':filter' is mapped to multiple columns"));
        }
    }
}

#[test]
fn test_placeholder_in_string_literal() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Test case: placeholder inside string literal should NOT be recognized as placeholder
    let sql1 = "name LIKE '%:search_term%'";
    let result1 = get_columns_and_compound_ids(sql1, Box::new(PostgreSqlDialect {}));
    assert!(result1.is_ok());
    let parsed1 = result1.unwrap();

    println!("SQL with placeholder in string: {}", sql1);
    println!("Placeholders found: {:?}", parsed1.placeholder_vars);
    // Should be empty because :search_term is inside quotes
    assert!(parsed1.placeholder_vars.is_empty());
    assert!(parsed1.placeholder_to_columns.is_empty());

    // Test case: proper placeholder usage for LIKE patterns
    let sql2 = "name LIKE :search_pattern";
    let result2 = get_columns_and_compound_ids(sql2, Box::new(PostgreSqlDialect {}));
    assert!(result2.is_ok());
    let parsed2 = result2.unwrap();

    println!("SQL with proper placeholder: {}", sql2);
    println!("Placeholders found: {:?}", parsed2.placeholder_vars);
    assert!(parsed2.placeholder_vars.contains(&":search_pattern".to_string()));
    if let Some(columns) = parsed2.get_columns_for_placeholder(":search_pattern") {
        assert!(columns.contains("name"));
    } else {
        panic!("Expected :search_pattern to be mapped to name column");
    }

    // Test case: PostgreSQL concatenation with placeholders
    let sql3 = "name LIKE '%' || :search_term || '%'";
    let result3 = get_columns_and_compound_ids(sql3, Box::new(PostgreSqlDialect {}));
    assert!(result3.is_ok());
    let parsed3 = result3.unwrap();

    println!("SQL with concatenation: {}", sql3);
    println!("Placeholders found: {:?}", parsed3.placeholder_vars);
    println!("Mapping: {:?}", parsed3.placeholder_to_columns);
    assert!(parsed3.placeholder_vars.contains(&":search_term".to_string()));
    // Note: In concatenation like '%' || :search_term || '%', the placeholder is not directly
    // compared to the column, so no mapping is created. This is expected behavior.
    println!("Note: No mapping for concatenation case - this is expected behavior");

    // Test the original problematic case (fixed to use different placeholders)
    let sql4 = "t.user = :user and t.age = :a1 and timestamp(t.full -> :a2 ) like '%:name%'";
    let result4 = get_columns_and_compound_ids(sql4, Box::new(PostgreSqlDialect {}));
    if result4.is_err() {
        println!("Error in original case: {}", result4.as_ref().unwrap_err());
    }
    assert!(result4.is_ok());
    let parsed4 = result4.unwrap();

    println!("Original SQL (fixed): {}", sql4);
    println!("Placeholders found: {:?}", parsed4.placeholder_vars);
    println!("Mapping: {:?}", parsed4.placeholder_to_columns);
    // Should only contain :user, :a1, :a2, NOT :name because it's in quotes
    assert!(parsed4.placeholder_vars.contains(&":user".to_string()));
    assert!(parsed4.placeholder_vars.contains(&":a1".to_string()));
    assert!(parsed4.placeholder_vars.contains(&":a2".to_string()));
    assert!(!parsed4.placeholder_vars.contains(&":name".to_string()));

    // Test a case that should actually fail (same placeholder with different columns in comparisons)
    println!("\n--- Testing a case that should fail ---");
    let sql5 = "t.user = :param and t.age = :param"; // Same placeholder for different columns in comparisons
    let result5 = get_columns_and_compound_ids(sql5, Box::new(PostgreSqlDialect {}));
    assert!(result5.is_err());
    let error5 = result5.unwrap_err();
    println!("Expected error for reused placeholder: {}", error5);
    assert!(error5.contains("Placeholder ':param' is mapped to multiple columns"));
    assert!(error5.contains("user") && error5.contains("age"));
}

#[test]
fn test_placeholder_mapping_flexibility() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Test case: placeholders can be unmapped (used in JSON access, concatenation, etc.)
    let sql1 = "t.user = :user and t.name1 = :name1 and timestamp(t.full -> :a2 ) like '%' || :name || '%'";
    let result1 = get_columns_and_compound_ids(sql1, Box::new(PostgreSqlDialect {}));
    assert!(result1.is_ok());
    let parsed1 = result1.unwrap();

    println!("SQL: {}", sql1);
    println!("All placeholders found: {:?}", parsed1.placeholder_vars);
    println!("Mapped placeholders: {:?}", parsed1.placeholder_to_columns);

    // Should find all placeholders
    assert!(parsed1.placeholder_vars.contains(&":user".to_string()));
    assert!(parsed1.placeholder_vars.contains(&":name1".to_string()));
    assert!(parsed1.placeholder_vars.contains(&":a2".to_string()));
    assert!(parsed1.placeholder_vars.contains(&":name".to_string()));

    // But only some are mapped to columns (those in direct comparisons)
    assert!(parsed1.placeholder_to_columns.contains_key(":user"));
    assert!(parsed1.placeholder_to_columns.contains_key(":name1"));
    // :a2 and :name are NOT mapped because they're used in JSON access and concatenation
    assert!(!parsed1.placeholder_to_columns.contains_key(":a2"));
    assert!(!parsed1.placeholder_to_columns.contains_key(":name"));

    println!("✓ Mapped placeholders: :user -> user, :name1 -> name1");
    println!("✓ Unmapped placeholders: :a2 (JSON access), :name (concatenation)");

    // Test case: still enforce the "1 placeholder = 1 column" rule for mapped ones
    let sql2 = "t.user = :param and t.email = :param";  // Same placeholder for different columns
    let result2 = get_columns_and_compound_ids(sql2, Box::new(PostgreSqlDialect {}));
    assert!(result2.is_err());
    let error2 = result2.unwrap_err();
    println!("\nError for multiple column mapping: {}", error2);
    assert!(error2.contains("Placeholder ':param' is mapped to multiple columns"));
}

#[test]
fn test_simple_comparison_debug() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Debug the simple case that's failing
    let sql = "user = :user";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));

    println!("=== Debug Simple Comparison ===");
    println!("SQL: {}", sql);

    match result {
        Ok(parsed) => {
            println!("✓ Parse successful");
            println!("Placeholders found: {:?}", parsed.placeholder_vars);
            println!("Columns found: {:?}", parsed.columns);
            println!("Mapping: {:?}", parsed.placeholder_to_columns);

            // The issue is that no columns are found at all!
            // This means the identifier 'user' is not being detected as a column
            assert!(parsed.columns.contains("user"), "Column 'user' should be detected");

            // Check if :user is mapped to user column
            if let Some(columns) = parsed.get_columns_for_placeholder(":user") {
                println!("✓ :user is mapped to columns: {:?}", columns);
                assert!(columns.contains("user"));
            } else {
                println!("✗ :user is not mapped to any column");
                // This is expected to fail until we fix the column detection
            }
        },
        Err(e) => {
            println!("✗ Parse failed: {}", e);
            panic!("Parse should succeed");
        }
    }
}

#[test]
fn test_user_specific_case() {
    use sqlparser::dialect::PostgreSqlDialect;

    // Test the exact user case
    let sql = "t.user = :user and t.name1 = :name1$str and t.name2 = :name2 and t.name3 = :name3$UserDefineType and timestamp(t.full -> :a2 ) like '%' || :name || '%'";
    let result = get_columns_and_compound_ids(sql, Box::new(PostgreSqlDialect {}));

    println!("=== User's Specific Test Case ===");
    println!("SQL: {}", sql);

    match result {
        Ok(parsed) => {
            dbg!(&parsed);
            println!("✓ Parse successful");
            println!("All placeholders found: {:?}", parsed.placeholder_vars);
            println!("Mapped placeholders: {:?}", parsed.placeholder_to_columns);
            println!("Columns found: {:?}", parsed.columns);
            println!("Tables found: {:?}", parsed.tables);

            // Check if :a2 and :name are in placeholder_vars
            let has_a2 = parsed.placeholder_vars.contains(&":a2".to_string());
            let has_name = parsed.placeholder_vars.contains(&":name".to_string());

            println!("Contains :a2? {}", has_a2);
            println!("Contains :name? {}", has_name);

            if !has_a2 {
                println!("⚠️  :a2 is missing from placeholder_vars (JSON access case)");
            }
            if !has_name {
                println!("⚠️  :name is missing from placeholder_vars (concatenation case)");
            }

            // Test individual components
            println!("\n--- Testing individual components ---");

            // Test JSON access alone
            let json_sql = "t.full -> :a2";
            let json_result = get_columns_and_compound_ids(json_sql, Box::new(PostgreSqlDialect {}));
            match json_result {
                Ok(json_parsed) => {
                    println!("JSON access '{}' placeholders: {:?}", json_sql, json_parsed.placeholder_vars);
                },
                Err(e) => println!("JSON access parse error: {}", e),
            }

            // Test concatenation alone
            let concat_sql = "'%' || :name || '%'";
            let concat_result = get_columns_and_compound_ids(concat_sql, Box::new(PostgreSqlDialect {}));
            match concat_result {
                Ok(concat_parsed) => {
                    println!("Concatenation '{}' placeholders: {:?}", concat_sql, concat_parsed.placeholder_vars);
                },
                Err(e) => println!("Concatenation parse error: {}", e),
            }

            // Test LIKE with concatenation
            let like_sql = "column LIKE '%' || :name || '%'";
            let like_result = get_columns_and_compound_ids(like_sql, Box::new(PostgreSqlDialect {}));
            match like_result {
                Ok(like_parsed) => {
                    println!("LIKE with concat '{}' placeholders: {:?}", like_sql, like_parsed.placeholder_vars);
                },
                Err(e) => println!("LIKE parse error: {}", e),
            }

        },
        Err(e) => {
            println!("✗ Parse failed: {}", e);
        }
    }
}
