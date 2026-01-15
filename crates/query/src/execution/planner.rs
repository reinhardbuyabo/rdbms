use crate::execution::filter::Filter;
use crate::execution::nested_loop_join::NestedLoopJoin;
use crate::execution::operator::{ExecutionError, ExecutionResult, PhysicalOperator};
use crate::execution::projection::Projection;
use crate::execution::seq_scan::{SeqScan, TableHeap};
use crate::expr::Expr;
use crate::logical_plan::{JoinType, LogicalPlan};
use crate::schema::{DataType, Field, Schema};
use std::collections::HashMap;

pub struct TableInfo {
    pub schema: Schema,
    pub heap: TableHeap,
}

pub struct Catalog {
    tables: HashMap<String, TableInfo>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    pub fn register_table(
        &mut self,
        table_name: impl Into<String>,
        schema: Schema,
        heap: TableHeap,
    ) {
        let table_name = table_name.into();
        let name = normalize_name(&table_name);
        self.tables.insert(name, TableInfo { schema, heap });
    }

    pub fn table(&self, table_name: &str) -> Option<&TableInfo> {
        let name = normalize_name(table_name);
        self.tables.get(&name)
    }
}

pub struct PhysicalPlanner<'a> {
    catalog: &'a Catalog,
}

impl<'a> PhysicalPlanner<'a> {
    pub fn new(catalog: &'a Catalog) -> Self {
        Self { catalog }
    }

    pub fn plan(&self, plan: &LogicalPlan) -> ExecutionResult<Box<dyn PhysicalOperator>> {
        Ok(self.plan_node(plan)?.operator)
    }

    fn plan_node(&self, plan: &LogicalPlan) -> ExecutionResult<PlannedOperator> {
        match plan {
            LogicalPlan::Scan {
                table_name, alias, ..
            } => {
                let table = self
                    .catalog
                    .table(table_name)
                    .ok_or_else(|| ExecutionError::TableNotFound(table_name.clone()))?;
                let schema = apply_alias(&table.schema, alias.as_deref());
                let operator = Box::new(SeqScan::new(table.heap.clone(), schema.clone()));
                Ok(PlannedOperator { operator, schema })
            }
            LogicalPlan::Filter { input, predicate } => {
                let input_planned = self.plan_node(input)?;
                let schema = input_planned.schema.clone();
                let operator = Box::new(Filter::new(
                    input_planned.operator,
                    predicate.clone(),
                    schema.clone(),
                ));
                Ok(PlannedOperator { operator, schema })
            }
            LogicalPlan::Project {
                input,
                expressions,
                aliases,
            } => {
                let input_planned = self.plan_node(input)?;
                let output_schema =
                    build_projection_schema(expressions, aliases.as_ref(), &input_planned.schema)?;
                let operator = Box::new(Projection::new(
                    input_planned.operator,
                    expressions.clone(),
                    input_planned.schema,
                ));
                Ok(PlannedOperator {
                    operator,
                    schema: output_schema,
                })
            }
            LogicalPlan::Join {
                left,
                right,
                join_type,
                condition,
            } => {
                if *join_type != JoinType::Inner {
                    return Err(ExecutionError::UnsupportedPlan(format!(
                        "only inner joins are supported, found {}",
                        join_type
                    )));
                }
                let predicate = condition.clone().ok_or_else(|| {
                    ExecutionError::UnsupportedPlan("inner join requires condition".to_string())
                })?;
                let left_planned = self.plan_node(left)?;
                let right_planned = self.plan_node(right)?;
                let mut fields = left_planned.schema.fields.clone();
                fields.extend(right_planned.schema.fields.clone());
                let output_schema = Schema::new(fields);
                let operator = Box::new(NestedLoopJoin::new(
                    left_planned.operator,
                    right_planned.operator,
                    predicate,
                    left_planned.schema,
                    right_planned.schema,
                ));
                Ok(PlannedOperator {
                    operator,
                    schema: output_schema,
                })
            }
            _ => Err(ExecutionError::UnsupportedPlan(format!(
                "logical plan {:?} is not supported in execution",
                plan
            ))),
        }
    }
}

struct PlannedOperator {
    operator: Box<dyn PhysicalOperator>,
    schema: Schema,
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase()
}

fn apply_alias(schema: &Schema, alias: Option<&str>) -> Schema {
    if let Some(alias_name) = alias {
        Schema::new(
            schema
                .fields
                .iter()
                .map(|field| Field {
                    name: format!("{}.{}", alias_name, field.name),
                    table: Some(alias_name.to_string()),
                    data_type: field.data_type.clone(),
                    nullable: field.nullable,
                })
                .collect(),
        )
    } else {
        schema.clone()
    }
}

fn build_projection_schema(
    expressions: &[Expr],
    aliases: Option<&Vec<String>>,
    input_schema: &Schema,
) -> ExecutionResult<Schema> {
    let mut fields = Vec::new();
    for (index, expr) in expressions.iter().enumerate() {
        match expr {
            Expr::Wildcard => fields.extend(input_schema.fields.clone()),
            Expr::QualifiedWildcard { table } => {
                let mut matched = false;
                for field in &input_schema.fields {
                    let table_matches = field
                        .table
                        .as_ref()
                        .map(|field_table| field_table.eq_ignore_ascii_case(table))
                        .unwrap_or(false)
                        || field
                            .name
                            .split('.')
                            .next()
                            .map(|prefix| prefix.eq_ignore_ascii_case(table))
                            .unwrap_or(false);
                    if table_matches {
                        matched = true;
                        fields.push(field.clone());
                    }
                }
                if !matched {
                    return Err(ExecutionError::Schema(format!(
                        "qualified wildcard {} did not match any columns",
                        table
                    )));
                }
            }
            _ => {
                let name = aliases
                    .and_then(|aliases| aliases.get(index).cloned())
                    .unwrap_or_else(|| expr.to_string());
                fields.push(Field {
                    name,
                    table: None,
                    data_type: DataType::Text,
                    nullable: true,
                });
            }
        }
    }
    Ok(Schema::new(fields))
}
