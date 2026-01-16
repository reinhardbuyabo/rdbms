# Transaction Management Refactoring Plan

## Executive Summary

Based on the comprehensive test suite provided, we will **completely refactor** the transaction management system using a test-driven approach. Instead of debugging the existing buggy code, we'll:

1. **Create the test suite first** to validate all functionality
2. **Refactor core components** with clean implementations
3. **Validate against the test suite** to ensure correctness
4. **Build up features** based on the minimal must-pass set

## Core Problem Analysis

The current transaction rollback bug stems from:
- **Incorrect LSN management** in page updates
- **Transaction context issues** in thread-local storage  
- **Recovery process interfering** with normal operations
- **Missing proper abort/rollback mechanisms**

## Refactoring Strategy

### Phase 1: Lock Manager & Core Primitives (Must Pass Set)

#### 1.1 Lock Manager Tests
```rust
// test_lock_compatibility()
test_shared_shared_compatibility()
test_exclusive_blocks_shared()  
test_shared_blocks_exclusive()
test_exclusive_blocks_exclusive()
test_reentrant_locking()
test_unlock_all_correctness()
```

#### 1.2 Deadlock Handling Tests
```rust
test_classic_two_key_deadlock()
test_three_txn_deadlock_cycle()
test_no_false_deadlock_under_normal_waiting()
test_victim_cleanup_correctness()
```

### Phase 2: Transaction Lifecycle Tests

#### 2.1 Basic ACID Tests  
```rust
test_single_txn_commit_persistence()
test_single_txn_rollback()
test_concurrent_insert_no_corruption()
test_concurrent_update_no_lost_updates()
```

#### 2.2 Isolation Level Tests
```rust
test_dirty_read_prevention()
test_non_repeatable_read_prevention()
test_lost_update_prevention()
test_phantom_prevention() // Expected limitation
```

### Phase 3: Runtime Abort Tests

```rust
test_abort_undoes_single_page_update()
test_abort_undoes_multi_page_changes()
test_abort_releases_locks_unblocks_others()
test_abort_is_idempotent()
```

### Phase 4: DDL & Catalog Tests

```rust  
test_ddl_rollback_add_column()
test_ddl_rollback_rename_table()
test_ddl_rollback_rename_column()
test_catalog_invariants_after_abort()
```

## Minimal "Must Pass" Set (Priority Order)

### Priority 1: Lock Manager Correctness
1. Lock compatibility (S/S, S/X, X/X)
2. unlock_all releases all locks
3. No deadlock under normal contention

### Priority 2: Basic ACID Properties  
4. Single transaction commit works
5. Single transaction rollback works
6. No lost updates under concurrency

### Priority 3: Recovery & Abort
7. Runtime abort undoes changes correctly
8. Abort releases locks properly
9. No corruption after abort

### Priority 4: DDL Safety
10. DDL rollback works correctly

## Implementation Plan

### Step 1: Create Comprehensive Test Suite
Create `tests/transaction_core_tests.rs` with all test cases organized by priority.

### Step 2: Implement Clean Lock Manager
Refactor `LockManager` with:
- Proper lock compatibility matrix
- Fair wait queue
- Correct unlock_all behavior
- Deadlock detection/timeout

### Step 3: Implement Clean Transaction Manager  
Refactor `TransactionManager` with:
- Clean transaction lifecycle (begin, commit, abort)
- Proper WAL integration
- Correct LSN management
- Working recovery process

### Step 4: Implement Proper Recovery
Refactor `RecoveryManager` with:
- Correct redo/undo logic
- Clean page LSN management
- No interference with normal operations

### Step 5: Integration Testing
Run comprehensive test suite to validate all functionality.

## Test Execution Strategy

### Single-Thread Mode (Always Pass)
Run all tests in single-threaded mode first to validate basic functionality.

### Multi-Thread Mode (Catch Concurrency Bugs)
Run each test 50-200 times with 2-8 threads to catch timing-dependent bugs.

### Validation Criteria
Each test must assert:
- ✅ Final logical state is correct
- ✅ No panics or unexpected deadlocks
- ✅ No resource leaks  
- ✅ Internal invariants hold (lock table empty after txns)

## Success Metrics

### Week 1: Foundation
- [ ] All Priority 1 tests passing (lock manager)
- [ ] Lock compatibility matrix fully implemented
- [ ] No deadlocks under normal operations

### Week 2: Core Functionality  
- [ ] All Priority 2 tests passing (basic ACID)
- [ ] Commit/rollback working correctly
- [ ] Lost update prevention verified

### Week 3: Advanced Features
- [ ] All Priority 3 tests passing (abort/rollback)
- [ ] Multi-page changes rollback correctly
- [ ] Lock release working properly

### Week 4: DDL & Integration
- [ ] All Priority 4 tests passing (DDL)
- [ ] Catalog invariants maintained
- [ ] Full integration test suite passing

## Files to Create/Modify

### New Files
- `tests/transaction_core_tests.rs` - Comprehensive test suite
- `docs/refactoring_plan.md` - This plan
- `src/txn/refactored_lock_manager.rs` - Clean lock manager (if needed)

### Modified Files  
- `src/txn/src/lib.rs` - Updated transaction management
- `src/query/src/recovery.rs` - Fixed recovery logic
- `src/query/src/execution/seq_scan.rs` - Fixed LSN management
- `src/db/src/engine.rs` - Fixed transaction orchestration

## Expected Outcomes

### Before Refactoring
- ❌ All write operations fail with "rollback transaction"
- ❌ Lock manager may have compatibility issues
- ❌ Recovery process interferes with normal operations

### After Refactoring
- ✅ All write operations succeed (INSERT/UPDATE/DELETE)
- ✅ Lock manager handles all compatibility cases correctly
- ✅ Recovery only runs on startup, not during normal operations
- ✅ Proper ACID properties maintained
- ✅ All test cases passing

## Risk Mitigation

### Risk: Test Complexity
**Mitigation**: Start with minimal must-pass set, add complexity gradually

### Risk: Performance Impact
**Mitigation**: Benchmark existing vs refactored, optimize hot paths

### Risk: Integration Issues  
**Mitigation**: Comprehensive integration tests, gradual rollout

## Next Steps

1. **Create the comprehensive test suite** with all test cases
2. **Implement Priority 1 tests** (lock manager)
3. **Refactor lock manager** to pass tests
4. **Implement Priority 2 tests** (basic ACID)
5. **Refactor transaction manager** to pass tests
6. **Continue through all priorities** until full test suite passes

This approach ensures we have a **solid foundation** before building up features, preventing the same bugs from reoccurring.

---

*Refactoring Plan v1.0 - Test-Driven Transaction System Overhaul*