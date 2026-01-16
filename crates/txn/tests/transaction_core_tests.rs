// Transaction Core Test Suite - Lock Manager Tests Only
// Priority 1: Lock Manager Correctness
//
// These tests validate the fundamental lock manager functionality needed
// to fix the transaction rollback bug.

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use txn::{DeadlockPolicy, LockError, LockKey, LockManager, TxnId};

#[cfg(test)]
mod lock_manager_tests {
    use super::*;

    fn create_lock_manager() -> LockManager {
        LockManager::new(DeadlockPolicy::Timeout(Duration::from_millis(200)))
    }

    fn create_lock_manager_arc() -> Arc<LockManager> {
        Arc::new(create_lock_manager())
    }

    #[test]
    fn test_shared_shared_compatibility() {
        let manager = create_lock_manager();
        let key = LockKey::Page(42);
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);

        // Both should acquire shared locks immediately
        assert!(manager.lock_shared(txn1, key.clone()).is_ok());
        assert!(manager.lock_shared(txn2, key.clone()).is_ok());

        // Verify both hold the lock
        let held_by_txn1 = manager.held_keys_for(txn1);
        let held_by_txn2 = manager.held_keys_for(txn2);

        assert_eq!(held_by_txn1.len(), 1);
        assert_eq!(held_by_txn2.len(), 1);
        assert_eq!(held_by_txn1[0], key);
        assert_eq!(held_by_txn2[0], key);
    }

    #[test]
    fn test_exclusive_blocks_shared() {
        let manager: Arc<LockManager> = create_lock_manager_arc();
        let key = LockKey::Page(1);
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);

        // Thread 1 acquires exclusive lock
        let key1 = key.clone();
        let key2 = key.clone();
        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(txn1, key1)
        });

        // Thread 2 tries to acquire shared lock (should block)
        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_shared(txn2, key2)
            }
        });

        // Give T1 time to acquire
        thread::sleep(Duration::from_millis(20));

        // T2 should be blocked (timeout)
        let result2 = handle2.join().unwrap();
        assert!(matches!(result2, Err(LockError::DeadlockTimeout)));

        // Release T1's lock
        manager.unlock_all(txn1);

        // Now T2 should succeed
        let handle3 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_shared(txn2, key)
            }
        });

        let result3 = handle3.join().unwrap();
        assert!(result3.is_ok());

        handle1.join().unwrap();
    }

    #[test]
    fn test_exclusive_blocks_exclusive() {
        let manager: Arc<LockManager> = create_lock_manager_arc();
        let key = LockKey::Page(1);
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);

        let key1 = key.clone();
        let key2 = key.clone();
        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(txn1, key1)
        });

        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_exclusive(txn2, key2)
            }
        });

        thread::sleep(Duration::from_millis(20));

        // T2 should timeout waiting
        let result2 = handle2.join().unwrap();
        assert!(matches!(result2, Err(LockError::DeadlockTimeout)));

        // Release T1, T2 should succeed
        manager.unlock_all(txn1);

        let handle3 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_exclusive(txn2, key)
            }
        });

        let result3 = handle3.join().unwrap();
        assert!(result3.is_ok());

        handle1.join().unwrap();
    }

    #[test]
    fn test_reentrant_shared_lock() {
        let manager = create_lock_manager();
        let key = LockKey::Page(1);
        let txn = TxnId(1);

        // Acquire same shared lock twice
        assert!(manager.lock_shared(txn, key.clone()).is_ok());
        assert!(manager.lock_shared(txn, key.clone()).is_ok()); // Should be no-op

        // Should only appear once in held keys
        let held = manager.held_keys_for(txn);
        assert_eq!(held.len(), 1); // Not duplicated
    }

    #[test]
    fn test_unlock_all_releases_everything() {
        let manager = create_lock_manager();
        let txn = TxnId(1);
        let keys: Vec<LockKey> = (1..=5).map(|i| LockKey::Page(i)).collect();

        // Acquire multiple locks
        for key in &keys {
            assert!(manager.lock_exclusive(txn, key.clone()).is_ok());
        }

        // Verify all held
        let held_before = manager.held_keys_for(txn);
        assert_eq!(held_before.len(), 5);

        // Release all
        manager.unlock_all(txn);

        // Verify none held
        let held_after = manager.held_keys_for(txn);
        assert!(held_after.is_empty());
    }

    #[test]
    fn test_multiple_keys_independent_progress() {
        let manager: Arc<LockManager> = create_lock_manager_arc();
        let key1 = LockKey::Page(1);
        let key2 = LockKey::Page(2);
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);

        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(txn1, key1)
        });

        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(txn2, key2)
        });

        // Both should succeed (different keys)
        let result1 = handle1.join().unwrap();
        let result2 = handle2.join().unwrap();

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Clean up
        manager.unlock_all(txn1);
        manager.unlock_all(txn2);
    }

    #[test]
    fn test_wait_queue_fairness() {
        let manager: Arc<LockManager> = create_lock_manager_arc();
        let key = LockKey::Page(1);
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);
        let txn3 = TxnId(3);

        let key1 = key.clone();
        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(txn1, key1)
        });

        thread::sleep(Duration::from_millis(10));

        // T2 requests shared, T3 requests shared (after T2)
        let barrier = Arc::new(Barrier::new(3));
        let barrier1 = Arc::clone(&barrier);
        let barrier2 = Arc::clone(&barrier);

        let key2 = key.clone();
        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            let barrier = barrier1;
            move || {
                barrier.wait();
                manager.lock_shared(txn2, key2)
            }
        });

        let key3 = key.clone();
        let handle3 = thread::spawn({
            let manager = Arc::clone(&manager);
            let barrier = barrier2;
            move || {
                barrier.wait();
                manager.lock_shared(txn3, key3)
            }
        });

        // Release T1
        manager.unlock_all(txn1);
        barrier.wait();

        // Both should eventually get the lock
        let result2 = handle2.join().unwrap();
        let result3 = handle3.join().unwrap();

        assert!(result2.is_ok());
        assert!(result3.is_ok());

        handle1.join().unwrap();
    }

    #[test]
    fn test_classic_deadlock_resolution() {
        let manager: Arc<LockManager> = create_lock_manager_arc();
        let key1 = LockKey::Page(1);
        let key2 = LockKey::Page(2);
        let key1_clone1 = key1.clone();
        let key1_clone2 = key1.clone();
        let key2_clone1 = key2.clone();
        let key2_clone2 = key2.clone();
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);

        // Create deadlock: T1 holds K1 waits K2, T2 holds K2 waits K1
        let barrier = Arc::new(Barrier::new(2));
        let barrier1 = Arc::clone(&barrier);
        let barrier2 = Arc::clone(&barrier);

        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            let barrier = barrier1;
            move || {
                barrier.wait();
                let _ = manager.lock_exclusive(txn1, key1_clone1);
                thread::sleep(Duration::from_millis(5));
                manager.lock_exclusive(txn1, key2_clone1)
            }
        });

        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            let barrier = barrier2;
            move || {
                barrier.wait();
                let _ = manager.lock_exclusive(txn2, key2_clone2);
                thread::sleep(Duration::from_millis(5));
                manager.lock_exclusive(txn2, key1_clone2)
            }
        });

        // One should timeout (victim), other should succeed
        let result1 = handle1.join().unwrap();
        let result2 = handle2.join().unwrap();

        // At least one should timeout (deadlock detected)
        let victim_count = [result1.clone(), result2.clone()]
            .iter()
            .filter(|r| matches!(r, Err(LockError::DeadlockTimeout)))
            .count();

        assert!(
            victim_count >= 1,
            "Expected at least one deadlock victim, got: {:?}, {:?}",
            result1,
            result2
        );

        // Clean up any remaining locks
        let _ = manager.unlock_all(txn1);
        let _ = manager.unlock_all(txn2);
    }
}

#[cfg(test)]
mod transaction_integration_tests {
    use super::*;

    #[test]
    fn test_transaction_abort_releases_locks() {
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(100),
        )));
        let key = LockKey::Page(1);
        let txn = TxnId(1);

        // Acquire lock
        assert!(lock_manager.lock_exclusive(txn, key.clone()).is_ok());

        // Verify lock is held
        let held_before = lock_manager.held_keys_for(txn);
        assert!(!held_before.is_empty(), "Lock should be held");

        // Simulate abort (unlock_all)
        lock_manager.unlock_all(txn);

        // Verify lock is released
        let held_after = lock_manager.held_keys_for(txn);
        assert!(held_after.is_empty(), "Lock should be released after abort");
    }

    #[test]
    fn test_transaction_abort_unblocks_waiters() {
        let lock_manager = Arc::new(LockManager::new(DeadlockPolicy::Timeout(
            Duration::from_millis(500),
        )));
        let key = LockKey::Page(1);
        let txn1 = TxnId(1);
        let txn2 = TxnId(2);

        // T1 holds exclusive lock
        let key1 = key.clone();
        let handle1 = thread::spawn({
            let lock_manager = lock_manager.clone();
            move || {
                lock_manager.lock_exclusive(txn1, key1).ok();
                thread::sleep(Duration::from_millis(50));
            }
        });

        thread::sleep(Duration::from_millis(10));

        // T2 waits for lock
        let key2 = key.clone();
        let handle2 = thread::spawn({
            let lock_manager = lock_manager.clone();
            move || {
                thread::sleep(Duration::from_millis(5));
                lock_manager.lock_exclusive(txn2, key2)
            }
        });

        thread::sleep(Duration::from_millis(30));

        // T2 should be blocked (timeout waiting)
        let result2 = handle2.join().unwrap();
        assert!(
            matches!(result2, Err(LockError::DeadlockTimeout)),
            "T2 should timeout waiting"
        );

        // Release T1
        lock_manager.unlock_all(txn1);

        // Now T2 should be able to acquire (in new transaction)
        let handle3 = thread::spawn({
            let lock_manager = lock_manager.clone();
            move || {
                thread::sleep(Duration::from_millis(10));
                lock_manager.lock_exclusive(txn2, key)
            }
        });

        let result3 = handle3.join().unwrap();
        assert!(result3.is_ok(), "T2 should succeed after T1 releases");

        handle1.join().unwrap();
    }
}
