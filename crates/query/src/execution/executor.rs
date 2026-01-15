use crate::execution::operator::{ExecutionResult, PhysicalOperator};
use crate::execution::tuple::Tuple;

pub struct Executor {
    root: Box<dyn PhysicalOperator>,
}

impl Executor {
    pub fn new(root: Box<dyn PhysicalOperator>) -> Self {
        Self { root }
    }

    pub fn execute(&mut self) -> ExecutionResult<Vec<Tuple>> {
        self.root.open()?;
        let mut output = Vec::new();
        let result = loop {
            match self.root.next() {
                Ok(Some(tuple)) => output.push(tuple),
                Ok(None) => break Ok(output),
                Err(error) => break Err(error),
            }
        };
        let close_result = self.root.close();
        match (result, close_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(output), Ok(())) => Ok(output),
        }
    }
}
