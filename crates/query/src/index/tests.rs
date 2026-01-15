use super::{BPlusTree, Index, IndexKey, IndexKeyType, IndexRange};
use crate::execution::operator::ExecutionResult;
use crate::execution::seq_scan::Rid;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use storage::{BufferPoolManager, DiskManager};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestContext {
    path: PathBuf,
}

impl TestContext {
    fn new(test_name: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("chronos_index_{}_{}.db", test_name, id));
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        Self { path }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn setup_bpm(test_name: &str, pool_size: usize) -> (TestContext, BufferPoolManager) {
    let ctx = TestContext::new(test_name);
    let disk_manager = DiskManager::open(ctx.path.to_str().unwrap()).unwrap();
    let bpm = BufferPoolManager::new(disk_manager, pool_size);
    (ctx, bpm)
}

fn rid_for(key: i64) -> Rid {
    Rid {
        page_id: 1,
        slot_id: key as u32,
    }
}

#[test]
fn btree_insert_ascending_lookup() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("btree_asc", 32);
    let index = BPlusTree::create(bpm, IndexKeyType::Integer, None, false)?;
    for key in 0..128 {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    for key in 0..128 {
        let rids = index.get(&IndexKey::Integer(key))?;
        assert_eq!(rids, vec![rid_for(key)]);
    }
    Ok(())
}

#[test]
fn btree_insert_descending_lookup() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("btree_desc", 32);
    let index = BPlusTree::create(bpm, IndexKeyType::Integer, None, false)?;
    for key in (0..128).rev() {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    for key in 0..128 {
        let rids = index.get(&IndexKey::Integer(key))?;
        assert_eq!(rids, vec![rid_for(key)]);
    }
    Ok(())
}

#[test]
fn btree_insert_random_lookup() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("btree_rand", 32);
    let index = BPlusTree::create(bpm, IndexKeyType::Integer, None, false)?;
    let keys = vec![42, 7, 19, 88, 3, 55, 24, 1, 90, 12];
    for key in &keys {
        index.insert(IndexKey::Integer(*key), rid_for(*key))?;
    }
    for key in &keys {
        let rids = index.get(&IndexKey::Integer(*key))?;
        assert_eq!(rids, vec![rid_for(*key)]);
    }
    Ok(())
}

#[test]
fn btree_leaf_link_is_ordered() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("btree_leaf_links", 32);
    let index = BPlusTree::create(bpm, IndexKeyType::Integer, None, false)?;
    for key in (0..200).rev() {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    let entries = index.iter_all()?;
    let keys: Vec<i64> = entries
        .iter()
        .map(|entry| match entry.key {
            IndexKey::Integer(value) => value,
            _ => 0,
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    Ok(())
}

#[test]
fn btree_range_scan_returns_sorted_slice() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("btree_range", 32);
    let index = BPlusTree::create(bpm, IndexKeyType::Integer, None, false)?;
    for key in 0..120 {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    let range = IndexRange {
        lower: Some((IndexKey::Integer(20), true)),
        upper: Some((IndexKey::Integer(30), false)),
    };
    let rids = index.range_scan(range)?;
    let expected: Vec<Rid> = (20..30).map(rid_for).collect();
    assert_eq!(rids, expected);
    Ok(())
}

#[test]
fn btree_splits_increase_height() -> ExecutionResult<()> {
    let (_ctx, bpm) = setup_bpm("btree_splits", 64);
    let index = BPlusTree::create(bpm, IndexKeyType::Integer, None, false)?;
    let max_leaf = index.max_leaf_entries();
    for key in 0..(max_leaf as i64 + 1) {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    assert_eq!(index.height()?, 2);

    let max_internal = index.max_internal_entries();
    let target = (max_leaf * (max_internal + 1)) + 5;
    for key in (max_leaf as i64 + 1)..(target as i64) {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    assert!(index.height()? >= 3);
    Ok(())
}
