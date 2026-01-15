mod btree;

pub use btree::{BPlusTree, IndexEntry, IndexKey, IndexKeyType, IndexRange};

use crate::execution::operator::ExecutionResult;
use crate::execution::seq_scan::Rid;

pub trait Index {
    fn insert(&self, key: IndexKey, rid: Rid) -> ExecutionResult<()>;
    fn delete(&self, key: &IndexKey, rid: Rid) -> ExecutionResult<bool>;
    fn get(&self, key: &IndexKey) -> ExecutionResult<Vec<Rid>>;
    fn range_scan(&self, range: IndexRange) -> ExecutionResult<Vec<Rid>>;
    fn iter_all(&self) -> ExecutionResult<Vec<IndexEntry>>;
}

#[cfg(test)]
mod tests;
