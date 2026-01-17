# Transaction Abort Bug Debug Report

## Bug Summary

**Bug**: Transaction abort was NOT rolling back changes - violating ACID Atomicity principle.

**Symptoms**:
- After calling `ABORT` on a transaction, modified data remained changed
- The `before` values were not being restored
- The WAL (Write-Ahead Log) was not being properly utilized during abort

**Impact**: Database integrity compromised - transactions were not truly atomic

---

## Root Cause Analysis

### Initial Investigation

The bug manifested when testing the transaction abort flow:

```bash
# Initial state
SELECT * FROM accounts WHERE id = 2;
# Returns: balance = 500

# Within transaction - update
UPDATE accounts SET balance = 600 WHERE id = 2;
# Returns: UPDATE 1

# Verify within transaction
SELECT * FROM accounts WHERE id = 2;
# Returns: balance = 600 ✓ (correct within transaction)

# ABORT the transaction
POST /api/tx/{tx_id}/abort
# Returns: {"message": "Transaction aborted"}

# After abort - SHOULD be 500, but was 600!
SELECT * FROM accounts WHERE id = 2;
# Returns: balance = 600 ✗ (BUG - should be 500)
```

### Key Discovery

The API's `abort_transaction` handler in `services/api/src/handlers.rs` was **only removing the transaction from a tracking map** without actually calling the recovery manager's `rollback_transaction` method.

**Original buggy code** (`services/api/src/handlers.rs:155-181`):
```rust
pub async fn abort_transaction(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let tx_id = path.into_inner();

    let mut transactions = match data.transactions.lock() { ... };

    // BUG: Only removes from map, never calls rollback!
    if transactions.remove(&tx_id).is_none() {
        return Ok(HttpResponse::NotFound()...);
    }

    Ok(HttpResponse::Ok().json(SuccessResponse {
        message: "Transaction aborted".to_string(),
    }))
}
```

### Secondary Issue: Missing Transaction Context

The API was storing `Arc<Mutex<Engine>>` instead of the actual `TransactionHandle` from the TransactionManager. This meant:

1. `begin_transaction()` was creating a UUID but not calling `engine.begin_transaction()`
2. SQL execution within transactions wasn't using the actual transaction context
3. No WAL records were being written for transaction operations

---

## Debug Steps

### Step 1: Reproduce the Bug

```bash
# Start fresh database
rm -rf /tmp/tx_test && mkdir -p /tmp/tx_test
DB_PATH=/tmp/tx_test/test.db PORT=9999 ./target/release/api &
sleep 3

# Test sequence
curl -X POST http://localhost:9999/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql": "CREATE TABLE accounts (id INT, balance INT);"}'

curl -X POST http://localhost:9999/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql": "INSERT INTO accounts VALUES (2, 500);"}'

# Get initial state
curl -X POST http://localhost:9999/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT * FROM accounts WHERE id = 2;"}'
# Expected: balance = 500
```

### Step 2: Test Transaction Abort

```bash
# Start transaction
TX_RESPONSE=$(curl -X POST http://localhost:9999/api/tx/begin)
TX_ID=$(echo $TX_RESPONSE | grep -o '"tx_id":"[^"]*"' | cut -d'"' -f4)

# Update within transaction
curl -X POST http://localhost:9999/api/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\": \"UPDATE accounts SET balance = 600 WHERE id = 2;\", \"tx_id\": \"$TX_ID\"}"

# Verify within transaction (should be 600)
curl -X POST http://localhost:9999/api/sql \
  -H "Content-Type: application/json" \
  -d "{\"sql\": \"SELECT * FROM accounts WHERE id = 2;\", \"tx_id\": \"$TX_ID\"}"

# ABORT
curl -X POST "http://localhost:9999/api/tx/$TX_ID/abort"

# Verify after abort (should be 500, but was 600 - BUG!)
curl -X POST http://localhost:9999/api/sql \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT * FROM accounts WHERE id = 2;"}'
```

### Step 3: Investigate WAL

```bash
# Check WAL file size (should have records if logging is working)
ls -la /tmp/tx_test/test.wal

# Expected: WAL file should grow with transaction records
# Actual: WAL file was EMPTY - no records being written!
```

### Step 4: Trace Transaction Context

Added debug output to `crates/wal/src/lib.rs:log_page_update`:

```rust
pub fn log_page_update(...) -> WalResult<Option<Lsn>> {
    CURRENT_TXN.with(|cell| {
        let context_opt = cell.borrow();
        let context = match context_opt.as_ref() {
            Some(ctx) => ctx,
            None => {
                eprintln!("DEBUG: log_page_update called with NO transaction context!");
                return Ok(None);  // Returns None - no logging!
            }
        };
        // ... rest of logging logic
    });
}
```

**Result**: Debug output showed `log_page_update called with NO transaction context!`

This confirmed the issue: the API wasn't setting up the transaction context for `with_transaction()`.

---

## Solution Implementation

### 1. Added Transaction Methods to Engine

**File**: `crates/db/src/engine.rs`

```rust
pub fn begin_transaction(&mut self) -> Result<wal::TransactionHandle> {
    self.txn_manager.begin().context("begin transaction")
}

pub fn execute_sql_in_transaction(
    &mut self,
    sql: &str,
    txn: &wal::TransactionHandle,
) -> Result<ReplOutput> {
    let plan = sql_to_logical_plan(sql)?;
    let txn_manager = self.txn_manager.clone();
    let result = txn_manager.with_transaction(txn, || self.execute_plan(plan));
    result
}

pub fn commit_transaction(&mut self, txn: &wal::TransactionHandle) -> Result<()> {
    self.txn_manager.commit(txn).context("commit transaction")?;
    Ok(())
}

pub fn abort_transaction(&mut self, txn: &wal::TransactionHandle) -> Result<()> {
    self.txn_manager.abort(txn).context("abort transaction")?;
    self.recovery
        .rollback_transaction(&self.buffer_pool, txn)
        .context("rollback transaction")?;
    Ok(())
}
```

### 2. Updated AppState to Track Real TransactionHandles

**File**: `services/api/src/main.rs`

```rust
#[derive(Clone)]
pub struct AppState {
    engine: Arc<Mutex<Engine>>,
    // Changed from: Arc<Mutex<Engine>>
    // To: Arc<Mutex<Transaction>>
    transactions: Arc<Mutex<HashMap<String, Arc<Mutex<wal::Transaction>>>>>,
}
```

### 3. Fixed API Handlers

**File**: `services/api/src/handlers.rs`

#### begin_transaction():
```rust
pub async fn begin_transaction(data: web::Data<AppState>) -> Result<HttpResponse> {
    let tx_id = uuid::Uuid::new_v4().to_string();

    // Now properly creates transaction via engine
    let mut engine = data.engine.lock();
    let txn = engine.begin_transaction()?;

    let mut transactions = data.transactions.lock();
    transactions.insert(tx_id.clone(), txn);

    Ok(HttpResponse::Ok().json(TransactionResponse { tx_id }))
}
```

#### execute_in_transaction():
```rust
async fn execute_in_transaction(data: &AppState, tx_id: &str, sql: &str) -> Result<HttpResponse> {
    let transactions = data.transactions.lock();

    let txn = match transactions.get(tx_id) {
        Some(txn) => txn,
        None => return Err(HttpResponse::NotFound()...),
    };

    let txn_clone = Arc::clone(txn);
    drop(transactions);

    let mut engine = data.engine.lock();
    // Uses the actual transaction handle
    match engine.execute_sql_in_transaction(sql, &txn_clone) { ... }
}
```

#### abort_transaction():
```rust
pub async fn abort_transaction(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let tx_id = path.into_inner();

    let mut transactions = data.transactions.lock();
    let txn = match transactions.remove(&tx_id) {
        Some(txn) => txn,
        None => return Err(HttpResponse::NotFound()...),
    };
    drop(transactions);

    let mut engine = data.engine.lock();
    // Now properly calls abort + rollback!
    engine.abort_transaction(&txn)?;

    Ok(HttpResponse::Ok().json(SuccessResponse {
        message: "Transaction aborted".to_string(),
    }))
}
```

### 4. Updated Dependencies

**File**: `services/api/Cargo.toml`

```toml
[dependencies]
# Added wal and parking_lot for proper mutex types
wal = { path = "../../crates/wal" }
parking_lot = "0.12"
```

---

## Verification

After the fix, the debug output confirmed proper operation:

```
DEBUG: Transaction begun: txn_id=1, lsn=57
DEBUG: log_page_update wrote lsn=58 for page_id=3
DEBUG: log_page_update wrote lsn=59 for page_id=3
DEBUG: Transaction abort: txn_id=1, last_lsn=59
DEBUG: Undo page_id=3 at offset=88, applying before value
DEBUG: Compensation record written: lsn=60
DEBUG: Undo complete, wrote END record at lsn=61
```

### Final Test Results

```
=== Test: Transaction ABORT should rollback ===
Initial state:
| id | val |
|----|-----|
| 1  | 100 |

In transaction - update to 200:
| id | val |
|----|-----|
| 1  | 200 |

Within transaction (should be 200):
| id | val |
|----|-----|
| 1  | 200 |

Abort transaction:
{"message":"Transaction aborted"}

After abort (should be 100):
| id | val |
|----|-----|
| 1  | 100  ✓ CORRECT!
```

---

## Files Modified

1. `crates/db/src/engine.rs` - Added transaction lifecycle methods
2. `services/api/src/main.rs` - Changed transaction storage type
3. `services/api/src/handlers.rs` - Fixed all transaction handlers
4. `services/api/Cargo.toml` - Added wal and parking_lot dependencies

---

## Lessons Learned

1. **ACID properties must be tested explicitly** - Just because commit works doesn't mean abort works
2. **Transaction context is critical** - Operations must run within the transaction context set by `with_transaction()`
3. **WAL emptiness is a red flag** - No WAL records means no recovery capability
4. **Debug output in key functions** - Adding eprintln in `log_page_update` quickly identified the missing context issue
