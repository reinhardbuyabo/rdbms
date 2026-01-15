use crate::expr::Expr;
use crate::schema::{ColumnDef, DataType, Field, Schema};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LogicalPlan {
    Scan {
        table_name: String,
        alias: Option<String>,
        schema: Option<Schema>,
    },
    Filter {
        input: Box<LogicalPlan>,
        predicate: Expr,
    },
    Project {
        input: Box<LogicalPlan>,
        expressions: Vec<Expr>,
        aliases: Option<Vec<String>>,
    },
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        join_type: JoinType,
        condition: Option<Expr>,
    },
    Sort {
        input: Box<LogicalPlan>,
        sort_exprs: Vec<SortExpr>,
    },
    Limit {
        input: Box<LogicalPlan>,
        offset: Option<usize>,
        limit: Option<usize>,
    },
    Aggregate {
        input: Box<LogicalPlan>,
        group_by: Vec<Expr>,
        aggregates: Vec<AggregateExpr>,
    },
    Insert {
        table_name: String,
        columns: Option<Vec<String>>,
        values: Vec<Vec<Expr>>,
        schema: Option<Schema>,
    },
    Update {
        table_name: String,
        assignments: Vec<Assignment>,
        filter: Option<Expr>,
        schema: Option<Schema>,
    },
    Delete {
        table_name: String,
        filter: Option<Expr>,
        schema: Option<Schema>,
    },
    CreateTable {
        table_name: String,
        columns: Vec<ColumnDef>,
        if_not_exists: bool,
    },
    DropTable {
        table_name: String,
        if_exists: bool,
    },
    AlterTableRename {
        table_name: String,
        new_table_name: String,
    },
    AlterTableRenameColumn {
        table_name: String,
        old_column_name: String,
        new_column_name: String,
    },
    AlterTableAddColumn {
        table_name: String,
        column_def: ColumnDef,
    },
    AlterTableDropColumn {
        table_name: String,
        column_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

impl fmt::Display for JoinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JoinType::Inner => write!(f, "INNER"),
            JoinType::Left => write!(f, "LEFT"),
            JoinType::Right => write!(f, "RIGHT"),
            JoinType::Full => write!(f, "FULL"),
            JoinType::Cross => write!(f, "CROSS"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortExpr {
    pub expr: Expr,
    pub asc: bool,
    pub nulls_first: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregateExpr {
    pub func: AggregateFunction,
    pub args: Vec<Expr>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

impl std::fmt::Display for AggregateFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregateFunction::Count => write!(f, "COUNT"),
            AggregateFunction::Sum => write!(f, "SUM"),
            AggregateFunction::Avg => write!(f, "AVG"),
            AggregateFunction::Min => write!(f, "MIN"),
            AggregateFunction::Max => write!(f, "MAX"),
        }
    }
}

impl LogicalPlan {
    pub fn schema(&self) -> Schema {
        match self {
            LogicalPlan::Scan {
                schema,
                table_name: _,
                alias,
                ..
            } => {
                if let Some(s) = schema {
                    if let Some(a) = alias {
                        Schema::new(
                            s.fields
                                .iter()
                                .map(|f| Field {
                                    name: format!("{}.{}", a, f.name),
                                    table: Some(a.clone()),
                                    data_type: f.data_type.clone(),
                                    nullable: f.nullable,
                                    visible: f.visible,
                                })
                                .collect(),
                        )
                    } else {
                        s.clone()
                    }
                } else {
                    Schema::empty()
                }
            }
            LogicalPlan::Filter { input, .. } => input.schema(),
            LogicalPlan::Project {
                expressions,
                aliases,
                input: _,
            } => {
                Schema::new(
                    expressions
                        .iter()
                        .enumerate()
                        .map(|(i, expr)| {
                            let name = if let Some(aliases) = aliases {
                                aliases[i].clone()
                            } else {
                                format!("{}", expr)
                            };
                            Field {
                                name,
                                table: None,
                                data_type: DataType::Text, // Simplified
                                nullable: true,
                                visible: true,
                            }
                        })
                        .collect(),
                )
            }
            LogicalPlan::Join { left, right, .. } => {
                let left_schema = left.schema();
                let right_schema = right.schema();
                let mut fields = left_schema.fields.clone();
                fields.extend(right_schema.fields);
                Schema::new(fields)
            }
            LogicalPlan::Sort { input, .. } => input.schema(),
            LogicalPlan::Limit { input, .. } => input.schema(),
            LogicalPlan::Aggregate { .. } => Schema::empty(),
            LogicalPlan::Insert { schema: _, .. }
            | LogicalPlan::Update { schema: _, .. }
            | LogicalPlan::Delete { schema: _, .. } => Schema::new(vec![Field {
                name: "rows_affected".to_string(),
                table: None,
                data_type: DataType::Integer,
                nullable: false,
                visible: true,
            }]),
            LogicalPlan::CreateTable { .. }
            | LogicalPlan::DropTable { .. }
            | LogicalPlan::AlterTableRename { .. }
            | LogicalPlan::AlterTableRenameColumn { .. }
            | LogicalPlan::AlterTableAddColumn { .. }
            | LogicalPlan::AlterTableDropColumn { .. } => Schema::new(vec![Field {
                name: "status".to_string(),
                table: None,
                data_type: DataType::Text,
                nullable: false,
                visible: true,
            }]),
        }
    }

    pub fn explain(&self) -> String {
        self.explain_with_indent(0)
    }
    fn explain_with_indent(&self, indent: usize) -> String {
        let prefix = "  ".repeat(indent);
        let child_indent = indent + 1;

        match self {
            LogicalPlan::Scan {
                table_name, alias, ..
            } => {
                if let Some(alias_name) = alias {
                    format!("{}Scan: {} (alias: {})", prefix, table_name, alias_name)
                } else {
                    format!("{}Scan: {}", prefix, table_name)
                }
            }
            LogicalPlan::Filter { input, predicate } => {
                format!(
                    "{}Filter: {}\n{}",
                    prefix,
                    predicate,
                    input.explain_with_indent(child_indent)
                )
            }
            LogicalPlan::Project {
                input,
                expressions,
                aliases,
            } => {
                let expr_str = if let Some(a) = aliases {
                    expressions
                        .iter()
                        .zip(a.iter())
                        .map(|(e, a)| format!("{} AS {}", e, a))
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    expressions
                        .iter()
                        .map(|e| format!("{}", e))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!(
                    "{}Project: [{}]\n{}",
                    prefix,
                    expr_str,
                    input.explain_with_indent(child_indent)
                )
            }
            LogicalPlan::Join {
                left,
                right,
                join_type,
                condition,
            } => {
                let cond_str = condition
                    .as_ref()
                    .map(|c| format!(" ON {}", c))
                    .unwrap_or_default();
                format!(
                    "{}Join [{}]{}\n{}\n{}",
                    prefix,
                    join_type,
                    cond_str,
                    left.explain_with_indent(child_indent),
                    right.explain_with_indent(child_indent)
                )
            }
            LogicalPlan::Sort { input, sort_exprs } => {
                let sort_str = sort_exprs
                    .iter()
                    .map(|s| format!("{} {}", s.expr, if s.asc { "ASC" } else { "DESC" }))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{}Sort: [{}]\n{}",
                    prefix,
                    sort_str,
                    input.explain_with_indent(child_indent)
                )
            }
            LogicalPlan::Limit {
                input,
                offset,
                limit,
            } => {
                let limit_str = match (offset, limit) {
                    (Some(o), Some(l)) => format!("OFFSET {} LIMIT {}", o, l),
                    (Some(o), None) => format!("OFFSET {}", o),
                    (None, Some(l)) => format!("LIMIT {}", l),
                    (None, None) => "".to_string(),
                };
                format!(
                    "{}Limit: {}\n{}",
                    prefix,
                    limit_str,
                    input.explain_with_indent(child_indent)
                )
            }
            LogicalPlan::Aggregate {
                input,
                group_by,
                aggregates,
            } => {
                let group_str = if group_by.is_empty() {
                    "[]".to_string()
                } else {
                    format!(
                        "[{}]",
                        group_by
                            .iter()
                            .map(|e| format!("{}", e))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let agg_str = aggregates
                    .iter()
                    .map(|a| format!("{:?}({:?})", a.func, a.args))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{}Aggregate: group_by={}, aggs=[{}]\n{}",
                    prefix,
                    group_str,
                    agg_str,
                    input.explain_with_indent(child_indent)
                )
            }
            LogicalPlan::Insert {
                table_name,
                columns,
                values,
                ..
            } => {
                let col_str = columns
                    .as_ref()
                    .map(|c| format!("({})", c.join(", ")))
                    .unwrap_or_else(|| "(all columns)".to_string());
                format!(
                    "{}Insert into {}{}: {} rows",
                    prefix,
                    table_name,
                    col_str,
                    values.len()
                )
            }
            LogicalPlan::Update {
                table_name,
                assignments,
                filter,
                ..
            } => {
                let assign_str = assignments
                    .iter()
                    .map(|a| format!("{} = {}", a.column, a.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                let filter_str = filter
                    .as_ref()
                    .map(|f| format!(" WHERE {}", f))
                    .unwrap_or_default();
                format!(
                    "{}Update {}: SET {}{}",
                    prefix, table_name, assign_str, filter_str
                )
            }
            LogicalPlan::Delete {
                table_name, filter, ..
            } => {
                let filter_str = filter
                    .as_ref()
                    .map(|f| format!(" WHERE {}", f))
                    .unwrap_or_default();
                format!("{}Delete from {}{}", prefix, table_name, filter_str)
            }
            LogicalPlan::CreateTable {
                table_name,
                columns,
                if_not_exists,
            } => {
                let ine = if *if_not_exists { " IF NOT EXISTS" } else { "" };
                format!(
                    "{}CreateTable{} {}: {} columns",
                    prefix,
                    ine,
                    table_name,
                    columns.len()
                )
            }
            LogicalPlan::DropTable {
                table_name,
                if_exists,
            } => {
                let ie = if *if_exists { " IF EXISTS" } else { "" };
                format!("{}DropTable{} {}", prefix, ie, table_name)
            }
            LogicalPlan::AlterTableRename {
                table_name,
                new_table_name,
            } => format!(
                "{}AlterTable {} RENAME TO {}",
                prefix, table_name, new_table_name
            ),
            LogicalPlan::AlterTableRenameColumn {
                table_name,
                old_column_name,
                new_column_name,
            } => format!(
                "{}AlterTable {} RENAME COLUMN {} TO {}",
                prefix, table_name, old_column_name, new_column_name
            ),
            LogicalPlan::AlterTableAddColumn {
                table_name,
                column_def,
            } => format!(
                "{}AlterTable {} ADD COLUMN {}",
                prefix, table_name, column_def.name
            ),
            LogicalPlan::AlterTableDropColumn {
                table_name,
                column_name,
            } => format!(
                "{}AlterTable {} DROP COLUMN {}",
                prefix, table_name, column_name
            ),
        }
    }

    pub fn to_dot(&self) -> String {
        let mut output = String::new();
        output.push_str("digraph LogicalPlan {\n");
        output.push_str("  rankdir=TB;\n");
        output
            .push_str("  node [shape=box, style=\"rounded,filled\", fontname=\"Helvetica\"];\n\n");
        self.write_dot_node(1, &mut output);
        output.push_str("}\n");
        output
    }

    fn write_dot_node(&self, node_id: usize, output: &mut String) -> usize {
        let (label, children) = self.get_dot_info();
        output.push_str(&format!(
            "  n{} [label=\"{}\", fillcolor=lightblue];\n",
            node_id, label
        ));

        let mut next_id = node_id + 1;
        for child in children {
            let child_id = next_id;
            next_id = child.write_dot_node(child_id, output);
            output.push_str(&format!("  n{} -> n{};\n", node_id, child_id));
        }
        next_id
    }

    fn get_dot_info(&self) -> (String, Vec<&LogicalPlan>) {
        match self {
            LogicalPlan::Scan {
                table_name, alias, ..
            } => {
                let label = format!(
                    "Scan: {}{}",
                    table_name,
                    alias
                        .as_ref()
                        .map(|a| format!(" AS {}", a))
                        .unwrap_or_default()
                );
                (label, vec![])
            }
            LogicalPlan::Filter { predicate, input } => {
                let label = format!("Filter: {}", predicate);
                (label, vec![input])
            }
            LogicalPlan::Project {
                expressions,
                aliases,
                input,
            } => {
                let exprs = expressions
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        if let Some(aliases) = aliases {
                            format!("{} AS {}", e, aliases[i])
                        } else {
                            format!("{}", e)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let label = format!("Project: [{}]", exprs);
                (label, vec![input])
            }
            LogicalPlan::Join {
                left,
                right,
                join_type,
                condition,
            } => {
                let cond = condition
                    .as_ref()
                    .map(|c| format!(" ON {}", c))
                    .unwrap_or_default();
                let label = format!("{:?}Join{}", join_type, cond);
                (label, vec![left, right])
            }
            LogicalPlan::Sort { sort_exprs, input } => {
                let sort_str = sort_exprs
                    .iter()
                    .map(|s| format!("{} {}", s.expr, if s.asc { "ASC" } else { "DESC" }))
                    .collect::<Vec<_>>()
                    .join(", ");
                let label = format!("Sort: [{}]", sort_str);
                (label, vec![input])
            }
            LogicalPlan::Limit {
                offset,
                limit,
                input,
            } => {
                let limit_str = match (offset, limit) {
                    (Some(o), Some(l)) => format!("OFFSET {} LIMIT {}", o, l),
                    (Some(o), None) => format!("OFFSET {}", o),
                    (None, Some(l)) => format!("LIMIT {}", l),
                    (None, None) => "".to_string(),
                };
                let label = format!("Limit: {}", limit_str);
                (label, vec![input])
            }
            LogicalPlan::Aggregate {
                group_by,
                aggregates,
                input,
            } => {
                let group_str = if group_by.is_empty() {
                    "".to_string()
                } else {
                    format!(
                        "GROUP BY {}",
                        group_by
                            .iter()
                            .map(|g| format!("{}", g))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let agg_str = aggregates
                    .iter()
                    .map(|a| {
                        format!(
                            "{}({})",
                            a.func,
                            a.args
                                .iter()
                                .map(|arg| format!("{}", arg))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let label = format!(
                    "Aggregate\n{}{}",
                    group_str,
                    if !agg_str.is_empty() {
                        format!("\nAggregates: {}", agg_str)
                    } else {
                        "".to_string()
                    }
                );
                (label, vec![input])
            }
            LogicalPlan::Insert {
                table_name,
                columns,
                values,
                ..
            } => {
                let col_str = columns
                    .as_ref()
                    .map(|c| format!("({})", c.join(", ")))
                    .unwrap_or_default();
                let val_count = values.first().map(|v| v.len()).unwrap_or(0);
                let label = format!(
                    "Insert into {}{}\n{} rows, {} values each",
                    table_name,
                    col_str,
                    values.len(),
                    val_count
                );
                (label, vec![])
            }
            LogicalPlan::Update {
                table_name,
                assignments,
                filter,
                ..
            } => {
                let assign_str = assignments
                    .iter()
                    .map(|a| format!("{} = {}", a.column, a.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                let filter_str = filter
                    .as_ref()
                    .map(|f| format!(" WHERE {}", f))
                    .unwrap_or_default();
                let label = format!("Update {}\nSet: {}{}", table_name, assign_str, filter_str);
                (label, vec![])
            }
            LogicalPlan::Delete {
                table_name, filter, ..
            } => {
                let filter_str = filter
                    .as_ref()
                    .map(|f| format!(" WHERE {}", f))
                    .unwrap_or_default();
                let label = format!("Delete from {}{}", table_name, filter_str);
                (label, vec![])
            }
            LogicalPlan::CreateTable {
                table_name,
                columns,
                ..
            } => {
                let col_str = columns
                    .iter()
                    .map(|c| format!("{}: {:?}", c.name, c.data_type))
                    .collect::<Vec<_>>()
                    .join(", ");
                let label = format!("CreateTable: {}\n{}", table_name, col_str);
                (label, vec![])
            }
            LogicalPlan::DropTable { table_name, .. } => {
                let label = format!("DropTable: {}", table_name);
                (label, vec![])
            }
            LogicalPlan::AlterTableRename {
                table_name,
                new_table_name,
            } => {
                let label = format!("AlterTable Rename {} -> {}", table_name, new_table_name);
                (label, vec![])
            }
            LogicalPlan::AlterTableRenameColumn {
                table_name,
                old_column_name,
                new_column_name,
            } => {
                let label = format!(
                    "AlterTable {} Rename Column {} -> {}",
                    table_name, old_column_name, new_column_name
                );
                (label, vec![])
            }
            LogicalPlan::AlterTableAddColumn {
                table_name,
                column_def,
            } => {
                let label = format!("AlterTable {} Add Column {}", table_name, column_def.name);
                (label, vec![])
            }
            LogicalPlan::AlterTableDropColumn {
                table_name,
                column_name,
            } => {
                let label = format!("AlterTable {} Drop Column {}", table_name, column_name);
                (label, vec![])
            }
        }
    }
}
