use crate::execution::operator::ExecutionError;
use crate::execution::seq_scan::TableHeap;
use crate::lock_api::*;
use crate::query::{Catalog, DataType, Field, Schema};
use crate::recovery::RecoveryManager;
use crate::storage::{BufferPoolManager, DiskManager, PageId};
use crate::txn::TransactionManager;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Core integration test suite for transaction rollback bug validation
///
/// This test suite validates that the transaction system correctly handles:
/// 1. Concurrent operations without corruption
/// 2. Proper rollback behavior  
/// 3. ACID properties
/// 4. Recovery after crashes
/// 5. Lock manager correctness
///
/// This serves as both a regression test suite and a way to validate fixes
/// to the transaction rollback bug.

pub struct TestEnvironment {
    pub buffer_pool: BufferPoolManager,
    pub txn_manager: TransactionManager,
    pub recovery_manager: RecoveryManager,
    pub temp_dir: TempDir,
}

impl TestEnvironment {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let disk_manager = DiskManager::open(temp_dir.path().join("test.db"))
            .expect("Failed to open test database");
        let buffer_pool = BufferPoolManager::new(disk_manager, 64);
        let log_manager = Arc::new(
            crate::wal::LogManager::open_with_buffer(temp_dir.path().join("test.wal"), 1024 * 1024)
                .expect("Failed to open WAL"),
        );
        let txn_manager = TransactionManager::with_lock_manager(
            log_manager.clone(),
            Arc::new(crate::lock_api::LockManager::new(
                crate::lock_api::DeadlockPolicy::Timeout(Duration::from_millis(100)),
            )),
        );
        let recovery_manager =
            RecoveryManager::new(log_manager.clone(), temp_dir.path().join("test.wal"));

        // Initialize empty database
        recovery_manager
            .recover(&buffer_pool)
            .expect("Failed to recover empty database");

        Self {
            buffer_pool,
            txn_manager,
            recovery_manager,
            temp_dir,
        }
    }

    pub fn create_test_table(&self) -> Result<(), ExecutionError> {
        let schema = Schema::new(vec![
            Field {
                name: "id".to_string(),
                table: Some("test_table".to_string()),
                data_type: DataType::Int,
                nullable: false,
                visible: true,
            },
            Field {
                name: "data".to_string(),
                table: Some("test_table".to_string()),
                data_type: DataType::Text,
                nullable: true,
                visible: true,
            },
        ]);

        let catalog = Catalog::new();
        catalog.create_table("test_table".to_string(), schema, &self.buffer_pool)
    }

    pub fn cleanup(&self) {
        // Drop all tables
        let catalog = Catalog::new();
        catalog
            .drop_table("test_table".to_string(), &self.buffer_pool)
            .expect("Failed to drop test_table");

        // Flush everything
        self.buffer_pool
            .flush_all_pages_with_mode(crate::storage::FlushMode::Force)
            .expect("Failed to flush pages");

        // Close WAL
        let log_manager = self.txn_manager.log_manager();
        log_manager
            .flush(log_manager.flushed_lsn())
            .expect("Failed to flush WAL");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1.1: Basic transaction commit/rollback functionality
    #[test]
    fn test_basic_transaction_commit() -> Result<(), ExecutionError> {
        let env = TestEnvironment::new();

        // Create table
        env.create_test_table()?;

        // Begin transaction and insert data
        let txn = env
            .txn_manager
            .begin()
            .expect("Failed to begin transaction");

        let result = env.txn_manager.with_transaction(&txn, || {
            // Insert data using table heap operations
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");

            // Create a tuple with values (id: 1, data: "test")
            let values = vec![
                crate::execution::tuple::Value::Int(1),
                crate::execution::tuple::Value::Text("test".to_string()),
            ];
            let tuple = crate::execution::tuple::Tuple::new(values);

            // Insert the tuple
            table.insert_tuple(&tuple)
        });

        // The insert should succeed
        assert!(result.is_ok(), "Insert should succeed: {:?}", result);

        // Commit the transaction
        env.txn_manager
            .commit(&txn)
            .expect("Failed to commit transaction");

        // Start new transaction and verify data exists
        let txn2 = env
            .txn_manager
            .begin()
            .expect("Failed to begin second transaction");
        let result2 = env.txn_manager.with_transaction(&txn2, || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");

            // Try to read the data
            let schema = table.schema.clone();
            let heap = TableHeap::new(table.clone(), &schema);
            heap.scan_tuples(&schema)
        });

        assert!(result2.is_ok(), "Read should succeed: {:?}", result2);
        env.txn_manager
            .commit(&txn2)
            .expect("Failed to commit second transaction");

        env.cleanup();
        Ok(())
    }

    /// Test 1.2: Transaction rollback functionality
    #[test]
    fn test_transaction_rollback() -> Result<(), ExecutionError> {
        let env = TestEnvironment::new();
        env.create_test_table()?;

        // Begin transaction
        let txn = env
            .txn_manager
            .begin()
            .expect("Failed to begin transaction");

        // Force a rollback by simulating an error during transaction
        let result = env.txn_manager.with_transaction(&txn, || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");

            // Insert valid data
            let values = vec![
                crate::execution::tuple::Value::Int(1),
                crate::execution::tuple::Value::Text("test".to_string()),
            ];
            let tuple = crate::execution::tuple::Tuple::new(values);
            let insert_result = table.insert_tuple(&tuple);

            // Simulate an error in the middle (e.g., constraint violation)
            if insert_result.is_ok() {
                // Try to insert same ID again to force error
                let duplicate_tuple = crate::execution::tuple::Tuple::new(values);
                let _ = table.insert_tuple(&duplicate_tuple);
                // This should cause some kind of error
                Err(crate::execution::operator::ExecutionError::Execution(
                    "Simulated error".to_string(),
                ))
            } else {
                Err(crate::execution::operator::ExecutionError::Execution(
                    "Initial insert failed".to_string(),
                ))
            }
        });

        // The transaction should fail
        assert!(
            result.is_err(),
            "Transaction should fail due to simulated error"
        );

        // Abort the transaction
        env.txn_manager
            .abort(&txn)
            .expect("Failed to abort transaction");

        // Verify data is rolled back (table should be empty)
        let txn2 = env
            .txn_manager
            .begin()
            .expect("Failed to begin verification transaction");
        let result2 = env.txn_manager.with_transaction(&txn2, || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");
            let heap = TableHeap::new(table.clone(), &table.schema);
            let tuples = heap
                .scan_tuples(&table.schema)
                .expect("Failed to scan tuples");

            // Table should be empty after rollback
            if tuples.is_empty() {
                Ok(())
            } else {
                Err(crate::execution::operator::ExecutionError::Execution(
                    "Data not rolled back".to_string(),
                ))
            }
        });

        assert!(
            result2.is_ok(),
            "Verification should succeed: {:?}",
            result2
        );
        env.txn_manager
            .commit(&txn2)
            .expect("Failed to commit verification transaction");

        env.cleanup();
        Ok(())
    }

    /// Test 2.1: Lost update prevention
    #[test]
    fn test_lost_update_prevention() -> Result<(), ExecutionError> {
        let env = TestEnvironment::new();
        env.create_test_table()?;

        // Insert initial data
        let initial_values = vec![
            crate::execution::tuple::Value::Int(1),
            crate::execution::tuple::Value::Int(100),
        ];
        let initial_tuple = crate::execution::tuple::Tuple::new(initial_values);

        let txn1 = env
            .txn_manager
            .begin()
            .expect("Failed to begin initial transaction");
        let result1 = env.txn_manager.with_transaction(&txn1, || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");
            table.insert_tuple(&initial_tuple)
        });

        assert!(result1.is_ok(), "Initial insert should succeed");
        env.txn_manager
            .commit(&txn1)
            .expect("Failed to commit initial transaction");

        // Read data in T2 (should see id=1, value=100)
        let txn2 = env
            .txn_manager
            .begin()
            .expect("Failed to begin read transaction");
        let read_result = env.txn_manager.with_transaction(&txn2, || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");
            let heap = TableHeap::new(table.clone(), &table.schema);

            // Find the tuple we just inserted
            heap.scan_tuples(&table.schema)
                .expect("Failed to scan tuples")
                .into_iter()
                .find(|tuple| match tuple.values().get(0) {
                    Some(crate::execution::tuple::Value::Int(id)) => *id == 1,
                    _ => false,
                })
        });

        assert!(read_result.is_ok(), "Read should succeed");
        env.txn_manager
            .commit(&txn2)
            .expect("Failed to commit read transaction");

        // Update to 200 in T3 (concurrent with T2)
        let barrier = Arc::new(Barrier::new(2));
        let updated_values = vec![
            crate::execution::tuple::Value::Int(1),
            crate::execution::tuple::Value::Int(200),
        ];
        let update_tuple = crate::execution::tuple::Tuple::new(updated_values);

        let txn3 = Arc::new(
            env.txn_manager
                .begin()
                .expect("Failed to begin update transaction"),
        );
        let txn3_clone = Arc::clone(&txn3);
        let barrier_clone = Arc::clone(&barrier);

        let handle3 = thread::spawn(move || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");

            let update_result = env
                .txn_manager
                .with_transaction(&txn3_clone.as_ref(), || table.insert_tuple(&update_tuple));

            barrier_clone.wait();
            update_result
        });

        let handle2 = thread::spawn(move || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");

            let read_result = env.txn_manager.with_transaction(&txn3.as_ref(), || {
                // Try to update to 150 (should be blocked by T3's exclusive lock)
                let conflict_values = vec![
                    crate::execution::tuple::Value::Int(1),
                    crate::execution::tuple::Value::Int(150),
                ];
                let conflict_tuple = crate::execution::tuple::Tuple::new(conflict_values);
                table.insert_tuple(&conflict_tuple)
            });

            barrier_clone.wait();
            read_result
        });

        // Wait for both to complete
        let update_result = handle3.join().expect("Update thread failed");
        let conflict_result = handle2.join().expect("Conflict thread failed");

        // Update should succeed, conflict should be blocked
        assert!(
            update_result.is_ok(),
            "Update should succeed: {:?}",
            update_result
        );
        assert!(
            conflict_result.is_err(),
            "Conflicting update should be blocked"
        );

        // Commit T3 and verify final state
        env.txn_manager
            .commit(&txn3.as_ref())
            .expect("Failed to commit update transaction");

        // Final verification - should see 200, not 150
        let txn4 = env
            .txn_manager
            .begin()
            .expect("Failed to begin final verification");
        let final_result = env.txn_manager.with_transaction(&txn4, || {
            let catalog = Catalog::new();
            let table = catalog.table("test_table").expect("Table not found");
            let heap = TableHeap::new(table.clone(), &table.schema);

            heap.scan_tuples(&table.schema)
                .expect("Failed to final scan")
                .into_iter()
                .find(|tuple| match tuple.values().get(0) {
                    Some(crate::execution::tuple::Value::Int(id)) => *id == 1,
                    _ => false,
                })
                .and_then(|tuple| tuple.values().get(1))
                .map(|value| match value {
                    crate::execution::tuple::Value::Int(val) => *val,
                    _ => panic!("Unexpected value type"),
                })
        });

        assert!(final_result.is_ok(), "Final read should succeed");

        match final_result {
            Some(crate::execution::tuple::Value::Int(200)) => {
                // Lost update was prevented - T3's update won
                println!("✅ Lost update prevention test PASSED");
            }
            Some(crate::execution::tuple::Value::Int(150)) => {
                panic!("❌ Lost update prevention test FAILED - conflicting write succeeded");
            }
            _ => panic!("❌ Lost update prevention test FAILED - unexpected result"),
        }

        env.txn_manager
            .commit(&txn4)
            .expect("Failed to commit final transaction");
        env.cleanup();
        Ok(())
    }
}
