// Fixed planner.rs with correct sqlparser 0.52 API

use crate::expr::{
    BinaryOperator as LocalBinaryOperator, Expr as LocalExpr, LiteralValue,
    UnaryOperator as LocalUnaryOperator,
};
use crate::logical_plan::{
    AggregateExpr, AggregateFunction, Assignment, JoinType, LogicalPlan, SortExpr,
};
use crate::schema::{ColumnDef, DataType as LocalDataType, DefaultValue};
use anyhow::{bail, Context, Result};
use sqlparser::ast::{
    AlterTableOperation, AssignmentTarget, BinaryOperator as SqlBinaryOp, ColumnOption,
    CreateTable, DataType as SqlDataType, Delete, Expr as SqlExpr, FromTable, FunctionArg,
    FunctionArgExpr, FunctionArguments, GroupByExpr, Insert, JoinConstraint, JoinOperator,
    ObjectName, OrderByExpr, Query, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins,
    UnaryOperator as SqlUnaryOp, Value,
};
use std::collections::HashMap;

pub struct LogicalPlanner {
    table_aliases: HashMap<String, String>,
}

impl LogicalPlanner {
    pub fn new() -> Self {
        Self {
            table_aliases: HashMap::new(),
        }
    }

    pub fn plan_statement(&mut self, stmt: Statement) -> Result<LogicalPlan> {
        self.table_aliases.clear();
        match stmt {
            Statement::Query(query) => self.plan_query(*query),
            Statement::Insert(insert) => self.plan_insert(insert),
            Statement::Update {
                table,
                assignments,
                selection,
                ..
            } => {
                // table is TableWithJoins, need to extract the first table
                let tf = match &table {
                    TableWithJoins { relation, joins } if joins.is_empty() => relation,
                    _ => bail!("UPDATE only supports simple table references"),
                };
                self.plan_update(tf, assignments, selection)
            }
            Statement::Delete(delete) => self.plan_delete(delete),
            Statement::CreateTable(ct) => self.plan_create_table(ct),
            Statement::CreateIndex(create_index) => self.plan_create_index(
                create_index.name,
                create_index.table_name,
                create_index.columns,
                create_index.if_not_exists,
                create_index.unique,
            ),
            Statement::Drop {
                object_type,
                if_exists,
                names,
                ..
            } => self.plan_drop(object_type, names, if_exists),
            Statement::AlterTable {
                name, operations, ..
            } => self.plan_alter_table(name, operations),
            _ => bail!("Unsupported statement type: {:?}", stmt),
        }
    }

    fn plan_query(&mut self, query: Query) -> Result<LogicalPlan> {
        let order_by = query.order_by;
        let limit = query.limit.map(|e| self.parse_limit_expr(e)).transpose()?;
        let offset = query
            .offset
            .map(|o| self.parse_limit_expr(o.value))
            .transpose()?;
        let select = match *query.body {
            SetExpr::Select(s) => s,
            SetExpr::Query(q) => return self.plan_query(*q),
            _ => bail!("UNION/INTERSECT/EXCEPT not yet supported"),
        };
        let mut plan = self.plan_from_clause(&select.from)?;
        if let Some(selection) = select.selection {
            let predicate = self.plan_expr(selection)?;
            self.validate_filter_predicate(&predicate, &plan)?;
            plan = LogicalPlan::Filter {
                input: Box::new(plan),
                predicate,
            };
        }
        match &select.group_by {
            GroupByExpr::Expressions(exprs, _) if !exprs.is_empty() => {
                let group_by_exprs: Result<Vec<_>> =
                    exprs.iter().map(|e| self.plan_expr(e.clone())).collect();
                let group_by_exprs = group_by_exprs?;
                let aggregates = self.extract_aggregates(&select.projection)?;
                plan = LogicalPlan::Aggregate {
                    input: Box::new(plan),
                    group_by: group_by_exprs,
                    aggregates,
                };
            }
            _ => {}
        }
        if let Some(having) = select.having {
            let predicate = self.plan_expr(having)?;
            plan = LogicalPlan::Filter {
                input: Box::new(plan),
                predicate,
            };
        }
        let (expressions, aliases) = self.plan_select_items(&select.projection)?;
        plan = LogicalPlan::Project {
            input: Box::new(plan),
            expressions,
            aliases: if aliases.is_empty() {
                None
            } else {
                Some(aliases)
            },
        };
        if let Some(order) = order_by {
            let sort_exprs: Result<Vec<_>> = order
                .exprs
                .iter()
                .map(|o| self.plan_order_by_expr(o))
                .collect();
            let sort_exprs = sort_exprs?;
            plan = LogicalPlan::Sort {
                input: Box::new(plan),
                sort_exprs,
            };
        }
        if limit.is_some() || offset.is_some() {
            plan = LogicalPlan::Limit {
                input: Box::new(plan),
                offset,
                limit,
            };
        }
        Ok(plan)
    }

    fn plan_from_clause(&mut self, from: &[TableWithJoins]) -> Result<LogicalPlan> {
        if from.is_empty() {
            bail!("SELECT without FROM clause not supported");
        }
        if from.len() > 1 {
            bail!("Multiple comma-separated tables not supported [use JOIN]");
        }
        let twj = &from[0];
        let mut plan = self.plan_table_factor(&twj.relation)?;
        for join in &twj.joins {
            let right = self.plan_table_factor(&join.relation)?;
            let (join_type, condition) = match &join.join_operator {
                JoinOperator::Inner(constraint) => {
                    (JoinType::Inner, self.plan_join_constraint(constraint)?)
                }
                JoinOperator::LeftOuter(constraint) => {
                    (JoinType::Left, self.plan_join_constraint(constraint)?)
                }
                JoinOperator::RightOuter(constraint) => {
                    (JoinType::Right, self.plan_join_constraint(constraint)?)
                }
                JoinOperator::FullOuter(constraint) => {
                    (JoinType::Full, self.plan_join_constraint(constraint)?)
                }
                JoinOperator::CrossJoin => (JoinType::Cross, None),
                _ => bail!("Unsupported join type: {:?}", join.join_operator),
            };
            if let Some(ref cond) = condition {
                self.validate_join_condition(cond, &plan, &right)?;
            }
            plan = LogicalPlan::Join {
                left: Box::new(plan),
                right: Box::new(right),
                join_type,
                condition,
            };
        }
        Ok(plan)
    }

    fn plan_table_factor(&mut self, tf: &TableFactor) -> Result<LogicalPlan> {
        match tf {
            TableFactor::Table { name, alias, .. } => {
                let tbl = object_name_to_string(name);
                let alias_name = alias.as_ref().map(|a| a.name.value.clone());
                if let Some(ref a) = alias_name {
                    self.table_aliases.insert(a.clone(), tbl.clone());
                }
                Ok(LogicalPlan::Scan {
                    table_name: tbl,
                    alias: alias_name,
                    schema: None,
                })
            }
            TableFactor::Derived {
                subquery, alias, ..
            } => {
                let subplan = self.plan_query(*subquery.clone())?;
                if let Some(a) = alias {
                    let alias_name = a.name.value.clone();
                    self.table_aliases
                        .insert(alias_name.clone(), "subquery".to_string());
                }
                Ok(subplan)
            }
            _ => bail!("Only simple table references and subqueries supported in FROM"),
        }
    }

    fn plan_join_constraint(&mut self, constraint: &JoinConstraint) -> Result<Option<LocalExpr>> {
        match constraint {
            JoinConstraint::On(expr) => Ok(Some(self.plan_expr(expr.clone())?)),
            JoinConstraint::None => Ok(None),
            JoinConstraint::Using(_) => bail!("USING clause not yet supported"),
            _ => bail!("Unsupported join constraint: {:?}", constraint),
        }
    }

    fn plan_select_items(&mut self, items: &[SelectItem]) -> Result<(Vec<LocalExpr>, Vec<String>)> {
        let mut expressions = Vec::new();
        let mut aliases = Vec::new();
        for item in items {
            match item {
                SelectItem::UnnamedExpr(expr) => {
                    let planned = self.plan_expr(expr.clone())?;
                    let alias = format!("{}", planned);
                    expressions.push(planned);
                    aliases.push(alias);
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    expressions.push(self.plan_expr(expr.clone())?);
                    aliases.push(alias.value.clone());
                }
                SelectItem::Wildcard(_) => {
                    expressions.push(LocalExpr::Wildcard);
                    aliases.push("*".to_string());
                }
                SelectItem::QualifiedWildcard(obj_name, _) => {
                    let table = object_name_to_string(obj_name);
                    expressions.push(LocalExpr::QualifiedWildcard {
                        table: table.clone(),
                    });
                    aliases.push(format!("{}.*", table));
                }
            }
        }
        Ok((expressions, aliases))
    }

    fn plan_expr(&mut self, expr: SqlExpr) -> Result<LocalExpr> {
        match expr {
            SqlExpr::Identifier(ident) => Ok(LocalExpr::Column {
                table: None,
                name: ident.value,
            }),
            SqlExpr::CompoundIdentifier(idents) => {
                if idents.len() == 2 {
                    Ok(LocalExpr::Column {
                        table: Some(idents[0].value.clone()),
                        name: idents[1].value.clone(),
                    })
                } else if idents.len() == 3 {
                    Ok(LocalExpr::Column {
                        table: Some(idents[1].value.clone()),
                        name: idents[2].value.clone(),
                    })
                } else {
                    bail!(
                        "Unsupported compound identifier with {} parts",
                        idents.len()
                    );
                }
            }
            SqlExpr::Value(value) => Ok(LocalExpr::Literal(self.plan_value(value)?)),
            SqlExpr::BinaryOp { left, op, right } => Ok(LocalExpr::BinaryOp {
                left: Box::new(self.plan_expr(*left)?),
                op: self.convert_binary_op(op)?,
                right: Box::new(self.plan_expr(*right)?),
            }),
            SqlExpr::UnaryOp { op, expr } => Ok(LocalExpr::UnaryOp {
                op: self.convert_unary_op(op)?,
                expr: Box::new(self.plan_expr(*expr)?),
            }),
            SqlExpr::Cast {
                expr, data_type, ..
            } => Ok(LocalExpr::Cast {
                expr: Box::new(self.plan_expr(*expr)?),
                target_type: self.convert_data_type(&data_type)?,
            }),
            SqlExpr::IsNull(expr) => Ok(LocalExpr::IsNull {
                expr: Box::new(self.plan_expr(*expr)?),
                negated: false,
            }),
            SqlExpr::IsNotNull(expr) => Ok(LocalExpr::IsNull {
                expr: Box::new(self.plan_expr(*expr)?),
                negated: true,
            }),
            SqlExpr::Between {
                expr,
                low,
                high,
                negated,
            } => Ok(LocalExpr::Between {
                expr: Box::new(self.plan_expr(*expr)?),
                low: Box::new(self.plan_expr(*low)?),
                high: Box::new(self.plan_expr(*high)?),
                negated,
            }),
            SqlExpr::InList {
                expr,
                list,
                negated,
            } => {
                let planned_list: Result<Vec<_>> =
                    list.into_iter().map(|e| self.plan_expr(e)).collect();
                Ok(LocalExpr::In {
                    expr: Box::new(self.plan_expr(*expr)?),
                    list: planned_list?,
                    negated,
                })
            }
            SqlExpr::Function(func) => {
                let name = object_name_to_string(&func.name);
                let args = match &func.args {
                    FunctionArguments::List(args) => args
                        .args
                        .iter()
                        .map(|arg| self.plan_function_arg(arg))
                        .collect::<Result<Vec<_>>>()?,
                    FunctionArguments::None => Vec::new(),
                    FunctionArguments::Subquery(_) => {
                        bail!("Subquery function arguments not supported")
                    }
                };
                Ok(LocalExpr::Function {
                    name: name.to_uppercase(),
                    args,
                })
            }
            SqlExpr::Nested(expr) => self.plan_expr(*expr),
            _ => bail!("Unsupported expression type: {:?}", expr),
        }
    }

    fn plan_function_arg(&mut self, arg: &FunctionArg) -> Result<LocalExpr> {
        match arg {
            FunctionArg::Unnamed(expr_or_wildcard) => match expr_or_wildcard {
                FunctionArgExpr::Expr(e) => self.plan_expr(e.clone()),
                FunctionArgExpr::Wildcard => Ok(LocalExpr::Wildcard),
                FunctionArgExpr::QualifiedWildcard(name) => {
                    let table = object_name_to_string(name);
                    Ok(LocalExpr::QualifiedWildcard { table })
                }
            },
            FunctionArg::Named { name: _, arg, .. } => match arg {
                FunctionArgExpr::Expr(e) => self.plan_expr(e.clone()),
                FunctionArgExpr::Wildcard => Ok(LocalExpr::Wildcard),
                FunctionArgExpr::QualifiedWildcard(name) => {
                    let table = object_name_to_string(name);
                    Ok(LocalExpr::QualifiedWildcard { table })
                }
            },
        }
    }

    fn plan_value(&self, value: Value) -> Result<LiteralValue> {
        match value {
            Value::Number(s, _) => {
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    Ok(LiteralValue::Float(s.parse().context("Invalid float")?))
                } else {
                    Ok(LiteralValue::Integer(s.parse().context("Invalid integer")?))
                }
            }
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
                Ok(LiteralValue::String(s))
            }
            Value::Boolean(b) => Ok(LiteralValue::Boolean(b)),
            Value::Null => Ok(LiteralValue::Null),
            Value::HexStringLiteral(hex) => Ok(LiteralValue::Blob(self.parse_hex_literal(&hex)?)),
            Value::SingleQuotedByteStringLiteral(bytes)
            | Value::DoubleQuotedByteStringLiteral(bytes)
            | Value::TripleSingleQuotedByteStringLiteral(bytes)
            | Value::TripleDoubleQuotedByteStringLiteral(bytes) => {
                Ok(LiteralValue::Blob(bytes.into_bytes()))
            }
            _ => bail!("Unsupported literal value: {:?}", value),
        }
    }

    fn convert_binary_op(&self, op: SqlBinaryOp) -> Result<LocalBinaryOperator> {
        Ok(match op {
            SqlBinaryOp::Plus => LocalBinaryOperator::Plus,
            SqlBinaryOp::Minus => LocalBinaryOperator::Minus,
            SqlBinaryOp::Multiply => LocalBinaryOperator::Multiply,
            SqlBinaryOp::Divide => LocalBinaryOperator::Divide,
            SqlBinaryOp::Modulo => LocalBinaryOperator::Modulo,
            SqlBinaryOp::Eq => LocalBinaryOperator::Eq,
            SqlBinaryOp::NotEq => LocalBinaryOperator::NotEq,
            SqlBinaryOp::Lt => LocalBinaryOperator::Lt,
            SqlBinaryOp::LtEq => LocalBinaryOperator::LtEq,
            SqlBinaryOp::Gt => LocalBinaryOperator::Gt,
            SqlBinaryOp::GtEq => LocalBinaryOperator::GtEq,
            SqlBinaryOp::And => LocalBinaryOperator::And,
            SqlBinaryOp::Or => LocalBinaryOperator::Or,
            SqlBinaryOp::StringConcat => LocalBinaryOperator::Concat,
            _ => bail!("Unsupported binary operator: {:?}", op),
        })
    }

    fn convert_unary_op(&self, op: SqlUnaryOp) -> Result<LocalUnaryOperator> {
        Ok(match op {
            SqlUnaryOp::Not => LocalUnaryOperator::Not,
            SqlUnaryOp::Minus => LocalUnaryOperator::Minus,
            SqlUnaryOp::Plus => LocalUnaryOperator::Plus,
            _ => bail!("Unsupported unary operator: {:?}", op),
        })
    }

    fn parse_hex_literal(&self, value: &str) -> Result<Vec<u8>> {
        let normalized = value.trim();
        if !normalized.len().is_multiple_of(2) {
            bail!("Invalid hex literal length");
        }
        let mut bytes = Vec::with_capacity(normalized.len() / 2);
        let mut chars = normalized.chars();
        while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
            let hex = [high, low].iter().collect::<String>();
            let byte = u8::from_str_radix(&hex, 16).context("Invalid hex literal")?;
            bytes.push(byte);
        }
        Ok(bytes)
    }

    fn convert_data_type(&self, dt: &SqlDataType) -> Result<LocalDataType> {
        Ok(match dt {
            SqlDataType::Int(_) | SqlDataType::Integer(_) => LocalDataType::Integer,
            SqlDataType::BigInt(_) => LocalDataType::BigInt,
            SqlDataType::Real | SqlDataType::Float(_) | SqlDataType::Double => LocalDataType::Real,
            SqlDataType::Varchar(_) | SqlDataType::Text | SqlDataType::String(_) => {
                LocalDataType::Text
            }
            SqlDataType::Boolean => LocalDataType::Boolean,
            SqlDataType::Timestamp(_, _) => LocalDataType::Timestamp,
            SqlDataType::Blob(_) | SqlDataType::Bytes(_) | SqlDataType::Bytea => {
                LocalDataType::Blob
            }
            _ => bail!("Unsupported data type: {:?}", dt),
        })
    }

    fn plan_insert(&mut self, ins: Insert) -> Result<LogicalPlan> {
        let table = object_name_to_string(&ins.table_name);
        let column_names = if ins.columns.is_empty() {
            None
        } else {
            Some(ins.columns.into_iter().map(|c| c.value).collect())
        };
        if let Some(query) = ins.source {
            if let SetExpr::Values(values) = *query.body {
                let rows: Result<Vec<Vec<LocalExpr>>> = values
                    .rows
                    .into_iter()
                    .map(|row| row.into_iter().map(|e| self.plan_expr(e)).collect())
                    .collect();
                return Ok(LogicalPlan::Insert {
                    table_name: table,
                    columns: column_names,
                    values: rows?,
                    schema: None,
                });
            }
            bail!("INSERT ... SELECT not yet supported");
        }
        bail!("INSERT requires VALUES clause");
    }

    fn plan_update(
        &mut self,
        table: &TableFactor,
        assignments: Vec<sqlparser::ast::Assignment>,
        selection: Option<SqlExpr>,
    ) -> Result<LogicalPlan> {
        let table_name = match table {
            TableFactor::Table { name, .. } => object_name_to_string(name),
            _ => bail!("UPDATE only supports simple table references"),
        };
        let planned_assignments: Result<Vec<_>> = assignments
            .into_iter()
            .map(|a| {
                let col = match &a.target {
                    AssignmentTarget::ColumnName(name) => object_name_to_string(name),
                    _ => bail!("Only column assignments supported"),
                };
                let value = self.plan_expr(a.value)?;
                Ok(Assignment { column: col, value })
            })
            .collect();
        let filter = selection.map(|e| self.plan_expr(e)).transpose()?;
        Ok(LogicalPlan::Update {
            table_name,
            assignments: planned_assignments?,
            filter,
            schema: None,
        })
    }

    fn plan_delete(&mut self, del: Delete) -> Result<LogicalPlan> {
        let tables = match &del.from {
            FromTable::WithFromKeyword(tables) => tables,
            FromTable::WithoutKeyword(tables) => tables,
        };
        if tables.len() != 1 {
            bail!("DELETE only supports single table");
        }
        let table_name = match &tables[0] {
            TableWithJoins {
                relation: TableFactor::Table { name, .. },
                joins,
            } if joins.is_empty() => object_name_to_string(name),
            _ => bail!("DELETE only supports simple table references"),
        };
        let filter = del.selection.map(|e| self.plan_expr(e)).transpose()?;
        Ok(LogicalPlan::Delete {
            table_name,
            filter,
            schema: None,
        })
    }

    fn plan_create_table(&mut self, ct: CreateTable) -> Result<LogicalPlan> {
        let table_name = object_name_to_string(&ct.name);
        let column_defs: Result<Vec<_>> = ct
            .columns
            .into_iter()
            .map(|col| self.plan_column_def(col))
            .collect();
        let column_defs = column_defs?;
        Ok(LogicalPlan::CreateTable {
            table_name,
            columns: column_defs,
            if_not_exists: ct.if_not_exists,
        })
    }

    fn plan_create_index(
        &mut self,
        index_name: Option<ObjectName>,
        table_name: ObjectName,
        columns: Vec<OrderByExpr>,
        if_not_exists: bool,
        unique: bool,
    ) -> Result<LogicalPlan> {
        let table_name = object_name_to_string(&table_name);
        let index_name = index_name
            .map(|name| object_name_to_string(&name))
            .unwrap_or_else(|| format!("idx_{}", table_name));

        if columns.is_empty() {
            bail!("CREATE INDEX requires at least one column");
        }

        let column = match &columns[0].expr {
            SqlExpr::Identifier(ident) => ident.value.clone(),
            _ => bail!("CREATE INDEX only supports simple column references"),
        };

        Ok(LogicalPlan::CreateIndex {
            table_name,
            index_name,
            column_name: column,
            if_not_exists,
            unique,
        })
    }

    fn plan_column_def(&mut self, col: sqlparser::ast::ColumnDef) -> Result<ColumnDef> {
        let data_type = self.convert_data_type(&col.data_type)?;
        let mut nullable = true;
        let mut primary_key = false;
        let mut unique = false;
        let mut default_value = None;
        let mut auto_increment = false;
        for option in col.options {
            match option.option {
                ColumnOption::Null => nullable = true,
                ColumnOption::NotNull => nullable = false,
                ColumnOption::Unique { is_primary, .. } => {
                    unique = true;
                    if is_primary {
                        primary_key = true;
                        nullable = false;
                    }
                }
                ColumnOption::Default(expr) => {
                    if data_type == LocalDataType::Blob {
                        bail!("BLOB columns cannot have DEFAULT values");
                    }
                    default_value = Some(self.plan_expr_to_default(expr)?);
                }
                ColumnOption::DialectSpecific(tokens) => {
                    let token_str = tokens.iter().map(|t| t.to_string()).collect::<String>();
                    let token_upper = token_str.to_uppercase();
                    if token_upper.contains("AUTOINCREMENT")
                        || token_upper.contains("AUTO_INCREMENT")
                    {
                        auto_increment = true;
                        nullable = false;
                    }
                }

                _ => {}
            }
        }
        if data_type == LocalDataType::Blob {
            if primary_key || unique {
                bail!("BLOB columns cannot be PRIMARY KEY or UNIQUE");
            }
            if default_value.is_some() {
                bail!("BLOB columns cannot have DEFAULT values");
            }
        }
        Ok(ColumnDef {
            name: col.name.value,
            data_type,
            nullable,
            primary_key,
            unique,
            default_value,
            auto_increment,
        })
    }

    fn plan_drop(
        &mut self,
        object_type: sqlparser::ast::ObjectType,
        names: Vec<ObjectName>,
        if_exists: bool,
    ) -> Result<LogicalPlan> {
        match object_type {
            sqlparser::ast::ObjectType::Table => {
                if names.len() != 1 {
                    bail!("DROP TABLE only supports single table");
                }
                Ok(LogicalPlan::DropTable {
                    table_name: object_name_to_string(&names[0]),
                    if_exists,
                })
            }
            _ => bail!("Only DROP TABLE supported"),
        }
    }

    fn plan_alter_table(
        &mut self,
        name: ObjectName,
        operations: Vec<AlterTableOperation>,
    ) -> Result<LogicalPlan> {
        let table_name = object_name_to_string(&name);
        if operations.len() != 1 {
            bail!("ALTER TABLE only supports a single operation per statement");
        }
        let operation = operations
            .into_iter()
            .next()
            .expect("single alter operation");
        match operation {
            AlterTableOperation::RenameTable {
                table_name: new_name,
            } => Ok(LogicalPlan::AlterTableRename {
                table_name,
                new_table_name: object_name_to_string(&new_name),
            }),
            AlterTableOperation::RenameColumn {
                old_column_name,
                new_column_name,
            } => Ok(LogicalPlan::AlterTableRenameColumn {
                table_name,
                old_column_name: old_column_name.value,
                new_column_name: new_column_name.value,
            }),
            AlterTableOperation::AddColumn { column_def, .. } => {
                let column_def = self.plan_column_def(column_def)?;
                Ok(LogicalPlan::AlterTableAddColumn {
                    table_name,
                    column_def,
                })
            }
            AlterTableOperation::DropColumn { column_name, .. } => {
                Ok(LogicalPlan::AlterTableDropColumn {
                    table_name,
                    column_name: column_name.value,
                })
            }
            _ => bail!("Unsupported ALTER TABLE operation: {:?}", operation),
        }
    }

    fn plan_expr_to_default(&mut self, expr: SqlExpr) -> Result<DefaultValue> {
        match expr {
            SqlExpr::Value(Value::Null) => Ok(DefaultValue::Null),
            SqlExpr::Value(Value::Number(s, _)) => {
                if s.contains('.') {
                    Ok(DefaultValue::Real(s.parse()?))
                } else {
                    Ok(DefaultValue::Integer(s.parse()?))
                }
            }
            SqlExpr::Value(Value::SingleQuotedString(s)) => Ok(DefaultValue::Text(s)),
            SqlExpr::Value(Value::Boolean(b)) => Ok(DefaultValue::Boolean(b)),
            SqlExpr::Function(f)
                if object_name_to_string(&f.name).to_uppercase() == "CURRENT_TIMESTAMP" =>
            {
                Ok(DefaultValue::CurrentTimestamp)
            }
            _ => bail!("Unsupported default value expression: {:?}", expr),
        }
    }

    fn parse_limit_expr(&mut self, expr: SqlExpr) -> Result<usize> {
        match expr {
            SqlExpr::Value(Value::Number(s, _)) => {
                Ok(s.parse().context("Invalid LIMIT/OFFSET value")?)
            }
            _ => bail!("LIMIT/OFFSET must be a number"),
        }
    }

    fn plan_order_by_expr(&mut self, order: &OrderByExpr) -> Result<SortExpr> {
        Ok(SortExpr {
            expr: self.plan_expr(order.expr.clone())?,
            asc: order.asc.unwrap_or(true),
            nulls_first: order.nulls_first.unwrap_or(false),
        })
    }

    fn extract_aggregates(&mut self, items: &[SelectItem]) -> Result<Vec<AggregateExpr>> {
        let mut aggregates = Vec::new();
        for item in items {
            match item {
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                    if let Some(agg) = self.extract_aggregate_from_expr(expr)? {
                        aggregates.push(agg);
                    }
                }
                _ => {}
            }
        }
        Ok(aggregates)
    }

    fn extract_aggregate_from_expr(&mut self, expr: &SqlExpr) -> Result<Option<AggregateExpr>> {
        match expr {
            SqlExpr::Function(func) => {
                let name = object_name_to_string(&func.name).to_uppercase();
                let agg_func = match name.as_str() {
                    "COUNT" => AggregateFunction::Count,
                    "SUM" => AggregateFunction::Sum,
                    "AVG" => AggregateFunction::Avg,
                    "MIN" => AggregateFunction::Min,
                    "MAX" => AggregateFunction::Max,
                    _ => return Ok(None),
                };
                let args = match &func.args {
                    FunctionArguments::List(args) => args
                        .args
                        .iter()
                        .map(|arg| self.plan_function_arg(arg))
                        .collect::<Result<Vec<_>>>()?,
                    FunctionArguments::None => Vec::new(),
                    FunctionArguments::Subquery(_) => bail!("Subquery in aggregate not supported"),
                };
                Ok(Some(AggregateExpr {
                    func: agg_func,
                    args,
                    alias: None,
                }))
            }
            _ => Ok(None),
        }
    }

    fn validate_filter_predicate(&self, predicate: &LocalExpr, _input: &LogicalPlan) -> Result<()> {
        self.validate_expr_well_formed(predicate)
    }

    fn validate_join_condition(
        &self,
        condition: &LocalExpr,
        _left: &LogicalPlan,
        _right: &LogicalPlan,
    ) -> Result<()> {
        self.validate_expr_well_formed(condition)
    }

    fn validate_expr_well_formed(&self, expr: &LocalExpr) -> Result<()> {
        match expr {
            LocalExpr::Column { .. }
            | LocalExpr::Literal(_)
            | LocalExpr::Wildcard
            | LocalExpr::QualifiedWildcard { .. } => Ok(()),
            LocalExpr::BinaryOp { left, right, .. } => {
                self.validate_expr_well_formed(left)?;
                self.validate_expr_well_formed(right)
            }
            LocalExpr::UnaryOp { expr, .. } => self.validate_expr_well_formed(expr),
            LocalExpr::Function { args, .. } => {
                for arg in args {
                    self.validate_expr_well_formed(arg)?;
                }
                Ok(())
            }
            LocalExpr::Cast { expr, .. } => self.validate_expr_well_formed(expr),
            LocalExpr::IsNull { expr, .. } => self.validate_expr_well_formed(expr),
            LocalExpr::Between {
                expr, low, high, ..
            } => {
                self.validate_expr_well_formed(expr)?;
                self.validate_expr_well_formed(low)?;
                self.validate_expr_well_formed(high)
            }
            LocalExpr::In { expr, list, .. } => {
                self.validate_expr_well_formed(expr)?;
                for item in list {
                    self.validate_expr_well_formed(item)?;
                }
                Ok(())
            }
        }
    }
}

impl Default for LogicalPlanner {
    fn default() -> Self {
        Self::new()
    }
}

fn object_name_to_string(name: &ObjectName) -> String {
    name.0
        .iter()
        .map(|ident| ident.value.clone())
        .collect::<Vec<_>>()
        .join(".")
}
