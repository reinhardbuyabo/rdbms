use crate::execution::operator::{evaluate_predicate, ExecutionResult, PhysicalOperator};
use crate::execution::tuple::Tuple;
use crate::expr::Expr;
use crate::schema::Schema;
use std::any::Any;

pub struct NestedLoopJoin {
    left: Box<dyn PhysicalOperator>,
    right: Box<dyn PhysicalOperator>,
    predicate: Expr,
    combined_schema: Schema,
    current_left: Option<Tuple>,
    right_open: bool,
}

impl NestedLoopJoin {
    pub fn new(
        left: Box<dyn PhysicalOperator>,
        right: Box<dyn PhysicalOperator>,
        predicate: Expr,
        left_schema: Schema,
        right_schema: Schema,
    ) -> Self {
        let mut fields = left_schema.fields.clone();
        fields.extend(right_schema.fields.clone());
        let combined_schema = Schema::new(fields);
        Self {
            left,
            right,
            predicate,
            combined_schema,
            current_left: None,
            right_open: false,
        }
    }
}

impl PhysicalOperator for NestedLoopJoin {
    fn open(&mut self) -> ExecutionResult<()> {
        self.left.open()?;
        self.right.open()?;
        self.right_open = true;
        self.current_left = None;
        Ok(())
    }

    fn next(&mut self) -> ExecutionResult<Option<Tuple>> {
        loop {
            if self.current_left.is_none() {
                self.current_left = self.left.next()?;
                if self.current_left.is_none() {
                    return Ok(None);
                }
                if self.right_open {
                    self.right.close()?;
                }
                self.right.open()?;
                self.right_open = true;
            }

            match self.right.next()? {
                Some(right_tuple) => {
                    let joined = self.current_left.as_ref().unwrap().concat(&right_tuple);
                    if evaluate_predicate(&self.predicate, &joined, &self.combined_schema)? {
                        return Ok(Some(joined));
                    }
                }
                None => {
                    if self.right_open {
                        self.right.close()?;
                        self.right_open = false;
                    }
                    self.current_left = None;
                }
            }
        }
    }

    fn close(&mut self) -> ExecutionResult<()> {
        if self.right_open {
            self.right.close()?;
            self.right_open = false;
        }
        self.left.close()?;
        self.current_left = None;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
