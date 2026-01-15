use crate::execution::operator::{
    evaluate_expr, ExecutionError, ExecutionResult, PhysicalOperator,
};
use crate::execution::tuple::Tuple;
use crate::expr::Expr;
use crate::schema::Schema;
use std::any::Any;

pub struct Projection {
    child: Box<dyn PhysicalOperator>,
    expressions: Vec<Expr>,
    input_schema: Schema,
    resolved_items: Vec<ProjectionItem>,
}

impl Projection {
    pub fn new(
        child: Box<dyn PhysicalOperator>,
        expressions: Vec<Expr>,
        input_schema: Schema,
    ) -> Self {
        Self {
            child,
            expressions,
            input_schema,
            resolved_items: Vec::new(),
        }
    }

    pub fn child(&self) -> &dyn PhysicalOperator {
        &*self.child
    }
}

impl PhysicalOperator for Projection {
    fn open(&mut self) -> ExecutionResult<()> {
        self.child.open()?;
        self.resolved_items = resolve_projection_items(&self.expressions, &self.input_schema)?;
        Ok(())
    }

    fn next(&mut self) -> ExecutionResult<Option<Tuple>> {
        let tuple = match self.child.next()? {
            Some(tuple) => tuple,
            None => return Ok(None),
        };

        let mut values = Vec::with_capacity(self.resolved_items.len());
        for item in &self.resolved_items {
            match item {
                ProjectionItem::FieldIndex(index) => {
                    let value = tuple.get(*index).ok_or_else(|| {
                        ExecutionError::Schema(format!("projection index {} out of range", index))
                    })?;
                    values.push(value.clone());
                }
                ProjectionItem::Expression(expr) => {
                    values.push(evaluate_expr(expr, &tuple, &self.input_schema)?);
                }
            }
        }

        Ok(Some(Tuple::new(values)))
    }

    fn close(&mut self) -> ExecutionResult<()> {
        self.resolved_items.clear();
        self.child.close()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
enum ProjectionItem {
    FieldIndex(usize),
    Expression(Expr),
}

fn resolve_projection_items(
    expressions: &[Expr],
    input_schema: &Schema,
) -> ExecutionResult<Vec<ProjectionItem>> {
    let mut items = Vec::new();
    for expr in expressions {
        match expr {
            Expr::Wildcard => {
                for index in 0..input_schema.fields.len() {
                    items.push(ProjectionItem::FieldIndex(index));
                }
            }
            Expr::QualifiedWildcard { table } => {
                let mut matched = false;
                for (index, field) in input_schema.fields.iter().enumerate() {
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
                        items.push(ProjectionItem::FieldIndex(index));
                    }
                }
                if !matched {
                    return Err(ExecutionError::Schema(format!(
                        "qualified wildcard {} did not match any columns",
                        table
                    )));
                }
            }
            _ => items.push(ProjectionItem::Expression(expr.clone())),
        }
    }
    Ok(items)
}
