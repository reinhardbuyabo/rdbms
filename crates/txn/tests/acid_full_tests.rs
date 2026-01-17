use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use txn::{DeadlockPolicy, LockKey, LockManager, TxnId};
use wal::{LogManager, LogRecord, LogRecordType, TransactionManager};

#[cfg(test)]
mod atomicity_tests {
    use super::*;

    #[test]
    fn test_atomicity_wal_begin_commit_recorded() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_manager = Arc::new(
            LogManager::open_with_buffer(temp_dir.path().join("atomicity_test.wal"), 1024 * 1024)
                .expect("Failed to open WAL"),
        );
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let txn_manager = TransactionManager::with_lock_manager(log_manager.clone(), lock_manager);

        let txn = txn_manager.begin().expect("Failed to begin transaction");
        txn_manager.commit(&txn).expect("Commit failed");

        let lsn = log_manager.flushed_lsn() + 100;
        log_manager.flush(lsn).expect("Flush should succeed");

        let mut reader = wal::LogReader::open(temp_dir.path().join("atomicity_test.wal"))
            .expect("Failed to open log reader");
        let mut records = Vec::new();
        while let Some(record) = reader.next_record().expect("Read error") {
            records.push(record.record_type);
        }

        assert!(
            records.contains(&LogRecordType::Begin),
            "WAL should contain BEGIN record"
        );
        assert!(
            records.contains(&LogRecordType::Commit),
            "WAL should contain COMMIT record"
        );
    }

    #[test]
    fn test_atomicity_wal_begin_abort_recorded() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_manager = Arc::new(
            LogManager::open_with_buffer(temp_dir.path().join("abort_test.wal"), 1024 * 1024)
                .expect("Failed to open WAL"),
        );
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let txn_manager = TransactionManager::with_lock_manager(log_manager.clone(), lock_manager);

        let txn = txn_manager.begin().expect("Failed to begin transaction");
        txn_manager.abort(&txn).expect("Abort failed");

        let lsn = log_manager.flushed_lsn() + 100;
        log_manager.flush(lsn).expect("Flush should succeed");

        let mut reader = wal::LogReader::open(temp_dir.path().join("abort_test.wal"))
            .expect("Failed to open log reader");
        let mut records = Vec::new();
        while let Some(record) = reader.next_record().expect("Read error") {
            records.push(record.record_type);
        }

        assert!(
            records.contains(&LogRecordType::Begin),
            "WAL should contain BEGIN record"
        );
        assert!(
            records.contains(&LogRecordType::Abort),
            "WAL should contain ABORT record"
        );
    }

    #[test]
    fn test_atomicity_lock_released_on_abort() {
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let key = LockKey::Page(42);
        let txn_id = TxnId(1);

        lock_manager
            .lock_exclusive(txn_id, key.clone())
            .expect("Lock should succeed");

        assert!(
            !lock_manager.held_keys_for(txn_id).is_empty(),
            "Lock should be held"
        );

        lock_manager.unlock_all(txn_id);

        assert!(
            lock_manager.held_keys_for(txn_id).is_empty(),
            "All locks should be released after unlock_all"
        );
    }

    #[test]
    fn test_atomicity_multiple_txns_isolated() {
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let key = LockKey::Page(1);
        let key_for_thread2 = key.clone();

        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_clone = started.clone();

        let lock_manager1 = Arc::clone(&lock_manager);
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let _ = started_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            let result = lock_manager1.lock_exclusive(TxnId(1), key.clone());
            result
        });

        barrier.wait();
        let result1 = lock_manager.lock_exclusive(TxnId(2), key_for_thread2);

        assert!(
            result1.is_err(),
            "Second lock attempt should fail (first transaction holds lock)"
        );

        assert!(
            started.load(std::sync::atomic::Ordering::SeqCst),
            "Second transaction should have attempted lock"
        );
        lock_manager.unlock_all(TxnId(1));

        let result2 = handle.join().unwrap();
        assert!(
            result2.is_ok(),
            "First lock should be released, second transaction can acquire"
        );

        lock_manager.unlock_all(TxnId(1));
        lock_manager.unlock_all(TxnId(2));
    }
}

#[cfg(test)]
mod consistency_tests {
    use super::*;

    #[test]
    fn test_consistency_lock_manager_state_integrity() {
        let manager = LockManager::new(DeadlockPolicy::Timeout(Duration::from_millis(500)));
        let key1 = LockKey::Page(1);
        let key2 = LockKey::Page(2);
        let txn_id = TxnId(1);

        manager
            .lock_shared(txn_id, key1.clone())
            .expect("First lock should succeed");
        manager
            .lock_exclusive(txn_id, key2.clone())
            .expect("Second lock should succeed");

        let held_keys = manager.held_keys_for(txn_id);
        assert_eq!(held_keys.len(), 2, "Should hold 2 locks");
        assert!(held_keys.contains(&key1), "Should hold key1");
        assert!(held_keys.contains(&key2), "Should hold key2");

        manager.unlock_all(txn_id);
        assert!(
            manager.held_keys_for(txn_id).is_empty(),
            "Should release all locks"
        );
    }

    #[test]
    fn test_consistency_wal_record_sequence() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_manager = Arc::new(
            LogManager::open_with_buffer(temp_dir.path().join("seq_test.wal"), 1024 * 1024)
                .expect("Failed to open WAL"),
        );
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let _txn_manager = TransactionManager::with_lock_manager(log_manager.clone(), lock_manager);

        let lsn1 = log_manager
            .append(LogRecord::begin(0, 1, None))
            .expect("First log should succeed");
        assert_eq!(lsn1, 0, "First LSN should be 0");

        let lsn2 = log_manager
            .append(LogRecord::commit(0, 1, Some(lsn1)))
            .expect("Second log should succeed");
        assert!(lsn2 > lsn1, "Second LSN should be greater");

        let lsn3 = log_manager
            .append(LogRecord::end(0, 1, Some(lsn2)))
            .expect("Third log should succeed");
        assert!(lsn3 > lsn2, "Third LSN should be greater");

        log_manager.flush(lsn3).expect("Flush should succeed");

        let flushed = log_manager.flushed_lsn();
        assert!(
            flushed >= lsn3,
            "Flushed LSN should be at least the last record"
        );
    }
}

#[cfg(test)]
mod isolation_tests {
    use super::*;

    #[test]
    fn test_isolation_shared_locks_compatible() {
        let manager = LockManager::new(DeadlockPolicy::Timeout(Duration::from_millis(500)));
        let key = LockKey::Page(1);

        let result1 = manager.lock_shared(TxnId(1), key.clone());
        assert!(result1.is_ok(), "First shared lock should succeed");

        let result2 = manager.lock_shared(TxnId(2), key.clone());
        assert!(
            result2.is_ok(),
            "Second shared lock should succeed (compatible)"
        );

        let held1 = manager.held_keys_for(TxnId(1));
        let held2 = manager.held_keys_for(TxnId(2));
        assert!(held1.contains(&key), "Txn1 should hold the lock");
        assert!(held2.contains(&key), "Txn2 should hold the lock");

        manager.unlock_all(TxnId(1));
        manager.unlock_all(TxnId(2));
    }

    #[test]
    fn test_isolation_exclusive_blocks_shared() {
        let manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let key = LockKey::Page(1);

        manager
            .lock_exclusive(TxnId(1), key.clone())
            .expect("Exclusive lock should succeed");

        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || manager_clone.lock_shared(TxnId(2), key.clone()));

        let result = handle.join().unwrap();
        assert!(
            result.is_err(),
            "Shared lock should timeout/block (exclusive blocks shared)"
        );

        manager.unlock_all(TxnId(1));
    }

    #[test]
    fn test_isolation_exclusive_blocks_exclusive() {
        let manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(100),
        )));
        let key = LockKey::Page(1);

        manager
            .lock_exclusive(TxnId(1), key.clone())
            .expect("First exclusive lock should succeed");

        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || manager_clone.lock_exclusive(TxnId(2), key.clone()));

        let result = handle.join().unwrap();
        assert!(
            result.is_err(),
            "Second exclusive lock should timeout (deadlock detection)"
        );

        manager.unlock_all(TxnId(1));
    }

    #[test]
    fn test_isolation_concurrent_txns_no_interference() {
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(1000),
        )));
        let key1 = LockKey::Page(1);
        let key2 = LockKey::Page(2);

        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));

        let ls = Arc::clone(&lock_manager);
        let b = Arc::clone(&barrier);
        let r = Arc::clone(&results);
        let k1 = key1.clone();
        let k2 = key2.clone();
        let handle1 = thread::spawn(move || {
            b.wait();
            let r1 = ls.lock_exclusive(TxnId(1), k1);
            let r2 = ls.lock_shared(TxnId(1), k2);
            let mut results = r.lock().unwrap();
            results.push(("txn1", r1.is_ok(), r2.is_ok()));
            if r1.is_ok() {
                ls.unlock_all(TxnId(1));
            }
            if r2.is_ok() {
                ls.unlock_all(TxnId(1));
            }
        });

        let ls = Arc::clone(&lock_manager);
        let b = Arc::clone(&barrier);
        let r = Arc::clone(&results);
        let k1 = key1.clone();
        let k2 = key2.clone();
        let handle2 = thread::spawn(move || {
            b.wait();
            let r1 = ls.lock_shared(TxnId(2), k1);
            let r2 = ls.lock_exclusive(TxnId(2), k2);
            let mut results = r.lock().unwrap();
            results.push(("txn2", r1.is_ok(), r2.is_ok()));
            if r1.is_ok() {
                ls.unlock_all(TxnId(2));
            }
            if r2.is_ok() {
                ls.unlock_all(TxnId(2));
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        let results = results.lock().unwrap();
        assert!(
            results.iter().all(|(_, a, b)| *a && *b),
            "Both transactions should complete successfully"
        );
    }
}

#[cfg(test)]
mod durability_tests {
    use super::*;

    #[test]
    fn test_durability_wal_persists_records() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let wal_path = temp_dir.path().join("durability_test.wal");

        {
            let log_manager = Arc::new(
                LogManager::open_with_buffer(&wal_path, 1024 * 1024).expect("Failed to open WAL"),
            );
            log_manager
                .append(LogRecord::begin(0, 1, None))
                .expect("Log append should succeed");
            log_manager
                .append(LogRecord::commit(0, 1, Some(0)))
                .expect("Log append should succeed");
            log_manager.flush(1).expect("Flush should succeed");
        }

        {
            let mut reader = wal::LogReader::open(&wal_path).expect("Failed to open log reader");
            let mut records = Vec::new();
            while let Some(record) = reader.next_record().expect("Read error") {
                records.push(record);
            }
            assert_eq!(records.len(), 2, "Should have 2 records after persistence");
        }
    }

    #[test]
    fn test_durability_log_record_serialization() {
        let begin_record = LogRecord::begin(100, 42, Some(50));
        let bytes = begin_record.to_bytes();
        let recovered = LogRecord::from_bytes(&bytes).expect("Deserialization should succeed");

        assert_eq!(recovered.lsn, begin_record.lsn);
        assert_eq!(recovered.txn_id, begin_record.txn_id);
        assert_eq!(recovered.prev_lsn, begin_record.prev_lsn);
        assert_eq!(recovered.record_type, begin_record.record_type);
        assert!(matches!(recovered.payload, wal::LogPayload::None));
    }

    #[test]
    fn test_durability_multiple_flushes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_manager = Arc::new(
            LogManager::open_with_buffer(temp_dir.path().join("flush_test.wal"), 256)
                .expect("Failed to open WAL"),
        );

        for i in 0..3 {
            log_manager
                .append(LogRecord::begin(0, i, None))
                .expect("Log append should succeed");
        }

        let lsn = log_manager.flushed_lsn() + 200;
        log_manager.flush(lsn).expect("Flush should succeed");

        let mut reader = wal::LogReader::open(temp_dir.path().join("flush_test.wal"))
            .expect("Failed to open log reader");
        let mut count = 0;
        while let Some(_) = reader.next_record().expect("Read error") {
            count += 1;
        }
        assert!(count >= 1, "At least 1 record should be durable");
    }
}
