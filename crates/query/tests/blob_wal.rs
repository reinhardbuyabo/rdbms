use std::sync::Arc;

use query::{DataType, Field, RecoveryManager, Schema, TableHeap, Tuple, Value};
use storage::{BufferPoolManager, DiskManager};
use tempfile::TempDir;
use wal::{LogManager, TransactionManager};

fn setup_wal_env() -> (
    TempDir,
    std::path::PathBuf,
    Arc<LogManager>,
    BufferPoolManager,
    TransactionManager,
) {
    let dir = TempDir::new().expect("temp dir");
    let data_path = dir.path().join("db");
    let wal_path = dir.path().join("db.wal");
    let disk_manager = DiskManager::open(&data_path).expect("open db");
    let log_manager = Arc::new(LogManager::open(&wal_path).expect("open wal"));
    let buffer_pool =
        BufferPoolManager::new_with_log(disk_manager, 32, Some(Arc::clone(&log_manager)));
    let txn_manager = TransactionManager::new(Arc::clone(&log_manager));
    (dir, wal_path, log_manager, buffer_pool, txn_manager)
}

fn blob_schema(table: &str) -> Schema {
    Schema::new(vec![
        Field {
            name: "id".to_string(),
            table: Some(table.to_string()),
            data_type: DataType::Integer,
            nullable: false,
            visible: true,
        },
        Field {
            name: "payload".to_string(),
            table: Some(table.to_string()),
            data_type: DataType::Blob,
            nullable: true,
            visible: true,
        },
    ])
}

#[test]
fn wal_recovery_replays_committed_blob() {
    let (dir, wal_path, log_manager, buffer_pool, txn_manager) = setup_wal_env();
    let schema = blob_schema("files");
    let heap = TableHeap::create(buffer_pool.clone()).expect("heap");
    let first_page = heap.first_page_id().unwrap().expect("first page");

    let blob = vec![0x5A; 8_192];
    let tuple = Tuple::new(vec![Value::Integer(1), Value::Blob(blob.clone())]);
    let txn = txn_manager.begin().expect("begin");
    txn_manager.with_transaction(&txn, || {
        heap.insert_tuple(&tuple, &schema).expect("insert");
    });
    txn_manager.commit(&txn).expect("commit");
    buffer_pool
        .flush_all_pages_with_mode(storage::FlushMode::Force)
        .expect("flush pages");

    drop(heap);
    drop(buffer_pool);
    drop(log_manager);

    let disk_manager = DiskManager::open(dir.path().join("db")).expect("reopen db");
    let log_manager = Arc::new(LogManager::open(&wal_path).expect("reopen wal"));
    let buffer_pool =
        BufferPoolManager::new_with_log(disk_manager, 32, Some(Arc::clone(&log_manager)));
    let recovery = RecoveryManager::new(Arc::clone(&log_manager), &wal_path);
    recovery.recover(&buffer_pool).expect("recover");

    let heap = TableHeap::new(buffer_pool.clone(), Some(first_page));
    let rows = heap.scan_tuples(&schema).expect("scan");
    assert_eq!(rows.len(), 1, "rows: {rows:?}");
    assert_eq!(rows[0].1.values()[1], Value::Blob(blob));
}

#[test]
fn wal_recovery_undoes_uncommitted_blob() {
    let (dir, wal_path, log_manager, buffer_pool, txn_manager) = setup_wal_env();
    let schema = blob_schema("files");
    let heap = TableHeap::create(buffer_pool.clone()).expect("heap");
    let first_page = heap.first_page_id().unwrap().expect("first page");

    let blob = vec![0x1A; 2_048];
    let tuple = Tuple::new(vec![Value::Integer(1), Value::Blob(blob)]);
    let txn = txn_manager.begin().expect("begin");
    txn_manager.with_transaction(&txn, || {
        heap.insert_tuple(&tuple, &schema).expect("insert");
    });
    buffer_pool
        .flush_all_pages_with_mode(storage::FlushMode::Force)
        .expect("flush pages");

    drop(heap);
    drop(buffer_pool);
    drop(log_manager);

    let disk_manager = DiskManager::open(dir.path().join("db")).expect("reopen db");
    let log_manager = Arc::new(LogManager::open(&wal_path).expect("reopen wal"));
    let buffer_pool =
        BufferPoolManager::new_with_log(disk_manager, 32, Some(Arc::clone(&log_manager)));
    let recovery = RecoveryManager::new(Arc::clone(&log_manager), &wal_path);
    recovery.recover(&buffer_pool).expect("recover");

    let heap = TableHeap::new(buffer_pool.clone(), Some(first_page));
    let rows = heap.scan_tuples(&schema).expect("scan");
    assert!(rows.is_empty());
}
