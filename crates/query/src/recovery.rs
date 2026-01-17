use crate::execution::operator::{ExecutionError, ExecutionResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use storage::BufferPoolManager;
use wal::{
    LogManager, LogPayload, LogReader, LogRecord, LogRecordType, Transaction, TransactionHandle,
    log_compensation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionStatus {
    Running,
    Committed,
    Aborted,
}

struct TransactionState {
    status: TransactionStatus,
    last_lsn: Option<wal::Lsn>,
}

type AnalysisResult = (
    Vec<LogRecord>,
    HashMap<wal::TxnId, TransactionState>,
    HashMap<wal::PageId, wal::Lsn>,
);

pub struct RecoveryManager {
    log_manager: Arc<LogManager>,
    log_path: PathBuf,
}

impl RecoveryManager {
    pub fn new(log_manager: Arc<LogManager>, log_path: impl AsRef<Path>) -> Self {
        Self {
            log_manager,
            log_path: log_path.as_ref().to_path_buf(),
        }
    }

    pub fn recover(&self, buffer_pool: &BufferPoolManager) -> ExecutionResult<()> {
        let (records, txn_table, dirty_pages) = self.analyze()?;
        self.redo(buffer_pool, &records, &dirty_pages)?;
        self.undo(buffer_pool, &records, &txn_table)?;
        buffer_pool
            .flush_all_pages_with_mode(storage::FlushMode::Force)
            .map_err(|err| ExecutionError::Execution(format!("flush error: {err}")))?;
        Ok(())
    }

    pub fn rollback_transaction(
        &self,
        buffer_pool: &BufferPoolManager,
        txn: &TransactionHandle,
    ) -> ExecutionResult<()> {
        let txn_guard = txn.lock();
        let last_lsn = txn_guard.last_lsn;
        let txn_id = txn_guard.txn_id;
        drop(txn_guard);

        // Flush WAL to ensure all records are on disk before reading
        if let Some(lsn) = last_lsn {
            self.log_manager.flush(lsn).map_err(map_wal_error)?;
        }

        let records = self.load_records()?;
        let record_map = build_record_map(&records);
        self.undo_single(buffer_pool, &record_map, txn_id, last_lsn, txn)?;
        let end_lsn = self
            .log_manager
            .append(LogRecord::end(0, txn_id, txn.lock().last_lsn))
            .map_err(map_wal_error)?;
        txn.lock().last_lsn = Some(end_lsn);
        self.log_manager.flush(end_lsn).map_err(map_wal_error)?;
        Ok(())
    }

    fn analyze(&self) -> ExecutionResult<AnalysisResult> {
        let records = self.load_records()?;
        let mut txn_table: HashMap<wal::TxnId, TransactionState> = HashMap::new();
        let mut dirty_pages: HashMap<wal::PageId, wal::Lsn> = HashMap::new();
        for record in &records {
            let entry = txn_table.entry(record.txn_id).or_insert(TransactionState {
                status: TransactionStatus::Running,
                last_lsn: None,
            });
            entry.last_lsn = Some(record.lsn);
            match record.record_type {
                LogRecordType::Begin => {
                    entry.status = TransactionStatus::Running;
                }
                LogRecordType::Commit => {
                    entry.status = TransactionStatus::Committed;
                }
                LogRecordType::Abort => {
                    entry.status = TransactionStatus::Aborted;
                }
                LogRecordType::End => {
                    txn_table.remove(&record.txn_id);
                }
                LogRecordType::Checkpoint => {}
                LogRecordType::PageUpdate | LogRecordType::Compensation => {
                    if let Some(page_id) = record_page_id(record) {
                        dirty_pages.entry(page_id).or_insert(record.lsn);
                    }
                }
            }
        }
        Ok((records, txn_table, dirty_pages))
    }

    fn redo(
        &self,
        buffer_pool: &BufferPoolManager,
        records: &[LogRecord],
        dirty_pages: &HashMap<wal::PageId, wal::Lsn>,
    ) -> ExecutionResult<()> {
        let Some(start_lsn) = dirty_pages.values().min().copied() else {
            return Ok(());
        };
        for record in records.iter().filter(|record| record.lsn >= start_lsn) {
            match &record.payload {
                LogPayload::PageUpdate {
                    page_id,
                    offset,
                    after,
                    ..
                }
                | LogPayload::Compensation {
                    page_id,
                    offset,
                    after,
                    ..
                } => {
                    if *page_id == 0 {
                        continue;
                    }
                    self.apply_redo(buffer_pool, *page_id, record.lsn, *offset, after)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn undo(
        &self,
        buffer_pool: &BufferPoolManager,
        records: &[LogRecord],
        txn_table: &HashMap<wal::TxnId, TransactionState>,
    ) -> ExecutionResult<()> {
        let record_map = build_record_map(records);
        for (txn_id, state) in txn_table {
            if state.status == TransactionStatus::Committed {
                continue;
            }
            let txn_handle = Arc::new(parking_lot::Mutex::new(Transaction {
                txn_id: *txn_id,
                last_lsn: state.last_lsn,
            }));
            self.undo_single(
                buffer_pool,
                &record_map,
                *txn_id,
                state.last_lsn,
                &txn_handle,
            )?;
            let end_lsn = self
                .log_manager
                .append(LogRecord::end(0, *txn_id, txn_handle.lock().last_lsn))
                .map_err(map_wal_error)?;
            txn_handle.lock().last_lsn = Some(end_lsn);
            self.log_manager.flush(end_lsn).map_err(map_wal_error)?;
        }
        Ok(())
    }

    fn undo_single(
        &self,
        buffer_pool: &BufferPoolManager,
        records: &HashMap<wal::Lsn, LogRecord>,
        txn_id: wal::TxnId,
        start_lsn: Option<wal::Lsn>,
        txn_handle: &TransactionHandle,
    ) -> ExecutionResult<()> {
        let mut current_lsn = start_lsn;
        while let Some(lsn) = current_lsn {
            let record = match records.get(&lsn) {
                Some(rec) => rec.clone(),
                None => {
                    eprintln!("WARN: missing log record at lsn={}, skipping undo", lsn);
                    break;
                }
            };
            current_lsn = match &record.payload {
                LogPayload::PageUpdate {
                    page_id,
                    offset,
                    before,
                    ..
                } => {
                    if *page_id == 0 {
                        record.prev_lsn
                    } else {
                        let clr_lsn = log_compensation(
                            txn_handle,
                            &self.log_manager,
                            *page_id,
                            *offset,
                            before.clone(),
                            record.prev_lsn,
                        )
                        .map_err(map_wal_error)?;
                        self.apply_undo(buffer_pool, *page_id, clr_lsn, *offset, before)?;
                        record.prev_lsn
                    }
                }
                LogPayload::Compensation { undo_next_lsn, .. } => *undo_next_lsn,
                _ => record.prev_lsn,
            };
        }
        let _ = txn_id;
        Ok(())
    }

    fn apply_redo(
        &self,
        buffer_pool: &BufferPoolManager,
        page_id: wal::PageId,
        lsn: wal::Lsn,
        offset: u32,
        after: &[u8],
    ) -> ExecutionResult<()> {
        let mut page_guard = buffer_pool.fetch_page(page_id)?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no available frame".to_string())
        })?;
        if lsn <= page_guard.lsn() {
            drop(page_guard);
            buffer_pool.unpin_page(page_id, false)?;
            return Ok(());
        }
        let result = (|| {
            if !page_guard.write_bytes(offset as usize, after) {
                return Err(ExecutionError::Execution(
                    "failed to apply redo update".to_string(),
                ));
            }
            if lsn > page_guard.lsn() {
                page_guard.set_lsn(lsn);
            }
            Ok(())
        })();
        drop(page_guard);
        buffer_pool.unpin_page(page_id, result.is_ok())?;
        result
    }

    fn apply_undo(
        &self,
        buffer_pool: &BufferPoolManager,
        page_id: wal::PageId,
        lsn: wal::Lsn,
        offset: u32,
        before: &[u8],
    ) -> ExecutionResult<()> {
        let mut page_guard = buffer_pool.fetch_page(page_id)?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no available frame".to_string())
        })?;
        let result = (|| {
            if !page_guard.write_bytes(offset as usize, before) {
                return Err(ExecutionError::Execution(
                    "failed to apply undo update".to_string(),
                ));
            }
            if lsn > page_guard.lsn() {
                page_guard.set_lsn(lsn);
            }
            Ok(())
        })();
        drop(page_guard);
        buffer_pool.unpin_page(page_id, result.is_ok())?;
        result
    }

    fn load_records(&self) -> ExecutionResult<Vec<LogRecord>> {
        let mut reader = LogReader::open(&self.log_path).map_err(map_wal_error)?;
        let mut records = Vec::new();
        let mut count = 0;
        while let Some(record) = reader.next_record().map_err(map_wal_error)? {
            records.push(record);
            count += 1;
        }
        eprintln!("DEBUG load_records: loaded {} records from WAL", count);
        Ok(records)
    }
}

fn build_record_map(records: &[LogRecord]) -> HashMap<wal::Lsn, LogRecord> {
    let mut map = HashMap::new();
    for record in records {
        map.insert(record.lsn, record.clone());
    }
    map
}

fn record_page_id(record: &LogRecord) -> Option<wal::PageId> {
    match &record.payload {
        LogPayload::PageUpdate { page_id, .. } => Some(*page_id),
        LogPayload::Compensation { page_id, .. } => Some(*page_id),
        _ => None,
    }
}

fn map_wal_error(err: wal::WalError) -> ExecutionError {
    ExecutionError::Execution(format!("wal error: {}", err))
}
