mod common;

use common::temp_buffer_pool;
use query::execution::{ExecutionError, ExecutionResult, Rid};
use query::index::{BPlusTree, Index, IndexKey, IndexKeyType, IndexRange};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{BTreeMap, HashSet};

fn rid_for(key: i64) -> Rid {
    Rid {
        page_id: 1,
        slot_id: key as u32,
    }
}

#[test]
fn empty_get_empty() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, false)?;
    let rids = index.get(&IndexKey::Integer(1))?;
    assert!(rids.is_empty());
    Ok(())
}

#[test]
fn insert_lookup() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, false)?;
    index.insert(IndexKey::Integer(42), rid_for(42))?;
    assert_eq!(index.get(&IndexKey::Integer(42))?, vec![rid_for(42)]);
    Ok(())
}

#[test]
fn ascending_inserts_force_splits_scan_sorted() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, false)?;
    let count = index.max_leaf_entries() + 20;
    for key in 0..count as i64 {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    let keys: Vec<i64> = index
        .iter_all()?
        .into_iter()
        .map(|entry| match entry.key {
            IndexKey::Integer(value) => value,
            _ => 0,
        })
        .collect();
    let mut expected = keys.clone();
    expected.sort();
    assert_eq!(keys, expected);
    Ok(())
}

#[test]
fn descending_inserts_scan_sorted() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, false)?;
    let count = index.max_leaf_entries() + 20;
    for key in (0..count as i64).rev() {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    let keys: Vec<i64> = index
        .iter_all()?
        .into_iter()
        .map(|entry| match entry.key {
            IndexKey::Integer(value) => value,
            _ => 0,
        })
        .collect();
    let mut expected = keys.clone();
    expected.sort();
    assert_eq!(keys, expected);
    Ok(())
}

#[test]
fn random_inserts_match_btreemap_reference() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, false)?;
    let mut rng = StdRng::seed_from_u64(1234);
    let mut seen = HashSet::new();
    let mut reference = BTreeMap::new();

    while seen.len() < 200 {
        let value = rng.gen_range(0..5000) as i64;
        if seen.insert(value) {
            let rid = rid_for(value);
            index.insert(IndexKey::Integer(value), rid)?;
            reference.insert(value, rid);
        }
    }

    for (key, rid) in &reference {
        assert_eq!(index.get(&IndexKey::Integer(*key))?, vec![*rid]);
    }

    let keys: Vec<i64> = index
        .iter_all()?
        .into_iter()
        .map(|entry| match entry.key {
            IndexKey::Integer(value) => value,
            _ => 0,
        })
        .collect();
    let expected: Vec<i64> = reference.keys().cloned().collect();
    assert_eq!(keys, expected);
    Ok(())
}

#[test]
fn range_scan_correct() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, false)?;
    for key in 0..200 {
        index.insert(IndexKey::Integer(key), rid_for(key))?;
    }
    let range = IndexRange {
        lower: Some((IndexKey::Integer(50), true)),
        upper: Some((IndexKey::Integer(75), false)),
    };
    let rids = index.range_scan(range)?;
    let expected: Vec<Rid> = (50..75).map(rid_for).collect();
    assert_eq!(rids, expected);
    Ok(())
}

#[test]
fn unique_behavior_rejects_duplicates() -> ExecutionResult<()> {
    let index = BPlusTree::create(temp_buffer_pool(), IndexKeyType::Integer, None, true)?;
    index.insert(IndexKey::Integer(9), rid_for(9))?;
    let result = index.insert(IndexKey::Integer(9), rid_for(9));
    assert!(matches!(result, Err(ExecutionError::Execution(_))));
    Ok(())
}

#[test]
fn composite_key_ordering_is_lexicographic() -> ExecutionResult<()> {
    let index = BPlusTree::create_composite(
        temp_buffer_pool(),
        vec![IndexKeyType::Text, IndexKeyType::Text],
        None,
        false,
    )?;
    let entries = vec![
        ("Smith", "Bob"),
        ("Smith", "Alice"),
        ("Adams", "Zoe"),
        ("Adams", "Aaron"),
    ];
    for (offset, (last, first)) in entries.iter().enumerate() {
        let key = IndexKey::Composite(vec![
            IndexKey::Text((*last).to_string()),
            IndexKey::Text((*first).to_string()),
        ]);
        index.insert(key, rid_for(offset as i64))?;
    }

    let keys: Vec<IndexKey> = index
        .iter_all()?
        .into_iter()
        .map(|entry| entry.key)
        .collect();

    let expected = vec![
        IndexKey::Composite(vec![
            IndexKey::Text("Adams".to_string()),
            IndexKey::Text("Aaron".to_string()),
        ]),
        IndexKey::Composite(vec![
            IndexKey::Text("Adams".to_string()),
            IndexKey::Text("Zoe".to_string()),
        ]),
        IndexKey::Composite(vec![
            IndexKey::Text("Smith".to_string()),
            IndexKey::Text("Alice".to_string()),
        ]),
        IndexKey::Composite(vec![
            IndexKey::Text("Smith".to_string()),
            IndexKey::Text("Bob".to_string()),
        ]),
    ];
    assert_eq!(keys, expected);
    Ok(())
}
