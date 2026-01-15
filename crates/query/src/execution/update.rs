use crate::execution::operator::{ExecutionResult, PhysicalOperator};
use crate::execution::planner::TableInfo;
use crate::execution::tuple::Tuple;
use crate::expr::Expr;
use crate::logical_plan::Assignment;
use std::any::Any;

pub struct Update {
    table: TableInfo,
    assignments: Vec<Assignment>,
    filter: Option<Expr>,
    updated: Vec<Tuple>,
    position: usize,
}

impl Update {
    pub fn new(table: TableInfo, assignments: Vec<Assignment>, filter: Option<Expr>) -> Self {
        Self {
            table,
            assignments,
            filter,
            updated: Vec::new(),
            position: 0,
        }
    }
}

impl PhysicalOperator for Update {
    fn open(&mut self) -> ExecutionResult<()> {
        self.updated = self
            .table
            .update_tuples(&self.assignments, self.filter.as_ref())?;
        self.position = 0;
        Ok(())
    }

    fn next(&mut self) -> ExecutionResult<Option<Tuple>> {
        if self.position >= self.updated.len() {
            return Ok(None);
        }
        let tuple = self.updated[self.position].clone();
        self.position += 1;
        Ok(Some(tuple))
    }

    fn close(&mut self) -> ExecutionResult<()> {
        self.updated.clear();
        self.position = 0;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
