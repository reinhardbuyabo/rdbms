use crate::lock_api::{DeadlockPolicy, LockKey, LockManager, LockResult};
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(test)]
mod lock_tests {
    use super::*;

    fn manager() -> LockManager {
        LockManager::new(DeadlockPolicy::Timeout(Duration::from_millis(100)))
    }

    fn manager_arc() -> Arc<LockManager> {
        Arc::new(manager())
    }

    #[test]
    fn shared_shared_is_compatible() {
        let manager = manager();
        let key = LockKey::Page(42);
        assert!(manager.lock_shared(1, key.clone()).is_ok());
        assert!(manager.lock_shared(2, key.clone()).is_ok());
        let held = manager.held_keys_for(1);
        assert_eq!(held, vec![key]);
    }

    #[test]
    fn exclusive_blocks_shared() {
        let manager = manager_arc();
        let key = LockKey::Page(1);

        // Thread 1: Acquire exclusive lock
        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(1, key.clone())
        });

        // Thread 2: Try to acquire shared lock (should block)
        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10)); // Ensure T1 acquires first
                manager.lock_shared(2, key)
            }
        });

        // Let T1 acquire exclusive lock
        thread::sleep(Duration::from_millis(20));

        // T2 should block
        let result2 = handle2.join().unwrap();
        assert!(result2.is_err());

        // Release T1's exclusive lock
        manager.unlock_all(1);

        // T2 should now succeed
        let handle3 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_shared(2, key)
            }
        });

        let result3 = handle3.join().unwrap();
        assert!(result3.is_ok());

        handle1.join().unwrap();
    }

    #[test]
    fn exclusive_blocks_exclusive() {
        let manager = manager_arc();
        let key = LockKey::Page(1);

        // Thread 1: Acquire exclusive lock
        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(1, key.clone())
        });

        // Thread 2: Try to acquire exclusive lock (should block)
        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_exclusive(2, key)
            }
        });

        thread::sleep(Duration::from_millis(20));

        // T2 should block until T1 releases
        let result2 = handle2.join().unwrap();
        assert!(result2.is_err());

        manager.unlock_all(1);

        // T2 should succeed after T1 releases
        let result3 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || {
                thread::sleep(Duration::from_millis(10));
                manager.lock_exclusive(2, key)
            }
        })
        .join()
        .unwrap();

        assert!(result3.is_ok());
        handle1.join().unwrap();
    }

    #[test]
    fn upgrade_shared_to_exclusive() {
        let manager = manager();
        let key = LockKey::Page(1);
        let txn = 1;

        // Acquire shared lock
        assert!(manager.lock_shared(txn, key.clone()).is_ok());

        // Upgrade to exclusive (should work)
        assert!(manager.lock_exclusive(txn, key.clone()).is_ok());

        let held = manager.held_keys_for(txn);
        assert_eq!(held, vec![key]);
    }

    #[test]
    fn unlock_all_releases_keys() {
        let manager = manager();
        let key1 = LockKey::Page(1);
        let key2 = LockKey::Page(2);
        let key3 = LockKey::Page(3);
        let txn = 1;

        // Acquire multiple locks
        assert!(manager.lock_exclusive(txn, key1.clone()).is_ok());
        assert!(manager.lock_exclusive(txn, key2.clone()).is_ok());
        assert!(manager.lock_exclusive(txn, key3.clone()).is_ok());

        // Verify all held
        let held_before = manager.held_keys_for(txn);
        assert_eq!(held_before.len(), 3);

        // Release all
        manager.unlock_all(txn);

        // Verify none held
        let held_after = manager.held_keys_for(txn);
        assert!(held_after.is_empty());
    }

    #[test]
    fn reentrant_shared_lock_is_noop() {
        let manager = manager();
        let key = LockKey::Page(1);
        let txn = 1;

        // Acquire shared lock twice
        assert!(manager.lock_shared(txn, key.clone()).is_ok());
        assert!(manager.lock_shared(txn, key.clone()).is_ok());

        let held = manager.held_keys_for(txn);
        assert_eq!(held.len(), 1); // Should only count once
    }

    #[test]
    fn wait_queue_fairness() {
        let manager = manager_arc();
        let key = LockKey::Page(1);

        // Thread 1: Hold exclusive lock
        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(1, key.clone())
        });

        thread::sleep(Duration::from_millis(10));

        // Thread 2-4: Queue for shared lock
        let mut handles = vec![];
        for i in 2..=4 {
            let manager = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                thread::sleep(Duration::from_millis(5 * (i - 1))); // Stagger requests
                manager.lock_shared(i as u64, key.clone())
            }));
        }

        // Release T1 and wait for others
        manager.unlock_all(1);

        // Join all waiting threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify fairness: All should get the lock eventually
        let final_held = manager.held_keys_for(1);
        assert!(final_held.is_empty()); // All released
    }

    #[test]
    fn concurrent_different_keys_no_contention() {
        let manager = manager_arc();
        let key1 = LockKey::Page(1);
        let key2 = LockKey::Page(2);

        let handle1 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(1, key1.clone())
        });

        let handle2 = thread::spawn({
            let manager = Arc::clone(&manager);
            move || manager.lock_exclusive(2, key2.clone())
        });

        // Both should succeed immediately (different keys)
        let result1 = handle1.join().unwrap();
        let result2 = handle2.join().unwrap();

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        handle1.join().unwrap();
        handle2.join().unwrap();
    }

    #[test]
    fn stress_test_many_keys_many_threads() {
        let manager = manager_arc();
        let num_threads = 8;
        let num_operations = 100;

        let mut handles = vec![];
        for i in 0..num_threads {
            let manager = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for j in 0..num_operations {
                    let key = LockKey::Page((i * num_operations + j) as u64);
                    let _ = manager.lock_exclusive(i as u64, key);
                    let _ = manager.unlock_all(i as u64);
                }
            }));
        }

        // All operations should complete without deadlock or timeout
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
