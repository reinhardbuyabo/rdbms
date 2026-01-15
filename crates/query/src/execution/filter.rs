use crate::execution::operator::{evaluate_predicate, ExecutionResult, PhysicalOperator};
use crate::execution::tuple::Tuple;
use crate::expr::Expr;
use crate::schema::Schema;

pub struct Filter {
    child: Box<dyn PhysicalOperator>,
    predicate: Expr,
    schema: Schema,
}

impl Filter {
    pub fn new(child: Box<dyn PhysicalOperator>, predicate: Expr, schema: Schema) -> Self {
        Self {
            child,
            predicate,
            schema,
        }
    }
}

impl PhysicalOperator for Filter {
    fn open(&mut self) -> ExecutionResult<()> {
        self.child.open()
    }

    fn next(&mut self) -> ExecutionResult<Option<Tuple>> {
        loop {
            let tuple = match self.child.next()? {
                Some(tuple) => tuple,
                None => return Ok(None),
            };
            if evaluate_predicate(&self.predicate, &tuple, &self.schema)? {
                return Ok(Some(tuple));
            }
        }
    }

    fn close(&mut self) -> ExecutionResult<()> {
        self.child.close()
    }
}
