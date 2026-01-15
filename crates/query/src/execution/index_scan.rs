use crate::execution::operator::{ExecutionResult, PhysicalOperator};
use crate::execution::seq_scan::{Rid, TableHeap};
use crate::execution::tuple::Tuple;
use crate::index::{BPlusTree, Index, IndexKey, IndexRange};
use crate::schema::Schema;
use std::any::Any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexPredicate {
    pub lower: Option<(IndexKey, bool)>,
    pub upper: Option<(IndexKey, bool)>,
}

impl IndexPredicate {
    pub fn equality(key: IndexKey) -> Self {
        Self {
            lower: Some((key.clone(), true)),
            upper: Some((key, true)),
        }
    }

    pub fn to_range(&self) -> IndexRange {
        IndexRange {
            lower: self.lower.clone(),
            upper: self.upper.clone(),
        }
    }
}

pub struct IndexScan {
    table_heap: TableHeap,
    schema: Schema,
    index: BPlusTree,
    predicate: IndexPredicate,
    rids: Vec<Rid>,
    position: usize,
}

impl IndexScan {
    pub fn new(
        table_heap: TableHeap,
        schema: Schema,
        index: BPlusTree,
        predicate: IndexPredicate,
    ) -> Self {
        Self {
            table_heap,
            schema,
            index,
            predicate,
            rids: Vec::new(),
            position: 0,
        }
    }
}

impl PhysicalOperator for IndexScan {
    fn open(&mut self) -> ExecutionResult<()> {
        let range = self.predicate.to_range();
        self.rids = self.index.range_scan(range)?;
        self.position = 0;
        Ok(())
    }

    fn next(&mut self) -> ExecutionResult<Option<Tuple>> {
        while self.position < self.rids.len() {
            let rid = self.rids[self.position];
            self.position += 1;
            if let Some(tuple) = self.table_heap.get_tuple(rid, &self.schema)? {
                return Ok(Some(tuple));
            }
        }
        Ok(None)
    }

    fn close(&mut self) -> ExecutionResult<()> {
        self.rids.clear();
        self.position = 0;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
