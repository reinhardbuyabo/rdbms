# Transaction Rollback Bug Analysis

## Current Status: CRITICAL BUG IDENTIFIED

All write operations (INSERT/UPDATE) fail with "rollback transaction" errors despite the API implementation being correct.

## Root Cause Investigation Progress

### ✅ What We've Confirmed:
1. **API Implementation**: Complete and correct
2. **Transaction Flow**: Engine begins transaction → executes plan → commits/aborts
3. **WAL Logging**: Page updates are being logged correctly  
4. **Recovery Process**: Not the root cause (disabled for testing)

### 🔍 Suspected Root Causes:

#### Primary Hypothesis: **LSN Management Issue**
The issue appears to be in how LSNs (Log Sequence Numbers) are managed during page updates within a single transaction.

**Critical Code Path**: `seq_scan.rs:write_bytes_logged()` (lines 606-617)

```rust
let lsn = wal::log_page_update(page_id, offset as u32, before, bytes.to_vec())
    .map_err(|err| ExecutionError::Execution(format!("wal error: {}", err)))?;
if !page.write_bytes(offset, bytes) {
    return Err(ExecutionError::Execution("failed to write page bytes".to_string()));
}
if let Some(lsn) = lsn {
    if lsn > page.lsn() {  // ← This logic might be wrong
        page.set_lsn(lsn);
    }
}
```

#### Secondary Hypothesis: **Transaction Context Issue**
The `CURRENT_TXN` thread-local storage might not be properly maintained during engine execution, causing WAL operations to fail or return invalid LSNs.

#### Tertiary Hypothesis: **Page LSN Initialization Issue**
New pages might be initialized with incorrect LSN values, causing all subsequent LSN comparisons to fail.

## Next Steps to Debug:

1. **Add Logging**: Insert debug logging to trace exact LSN values
2. **Isolate Components**: Test each component individually
3. **Minimal Reproduction**: Create the simplest possible failing case
4. **Transaction Context Debug**: Verify CURRENT_TXN is properly set

## Current Working Theory:

The bug is most likely in the **LSN comparison logic** or **transaction context management**. The fix will require carefully examining how page LSNs are initialized, updated, and compared during the redo phase of recovery.

## Files Being Analyzed:

- `crates/db/src/engine.rs` - Main transaction orchestration
- `crates/query/src/recovery.rs` - LSN management and page updates
- `crates/query/src/execution/seq_scan.rs` - Page write operations with WAL logging
- `crates/wal/src/lib.rs` - Transaction context and WAL logging
- `crates/storage/src/page.rs` - Page LSN operations

## Impact: 
- Complete data persistence failure
- All write operations non-functional
- API and REPL both equally affected
- Production blocker status

---

*Documenting investigation progress for targeted bug fix.*