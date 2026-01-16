use parking_lot::{Condvar, Mutex};
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

use thiserror::Error;
use txn::LockManager;

pub type Lsn = u64;
pub type TxnId = u64;
pub type PageId = u64;

const INVALID_LSN: Lsn = u64::MAX;
const DEFAULT_LOG_BUFFER_SIZE: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum WalError {
    #[error("wal io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("wal corruption: {0}")]
    Corrupt(String),
    #[error("wal channel closed")]
    ChannelClosed,
}

pub type WalResult<T> = Result<T, WalError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRecordType {
    Begin,
    Commit,
    Abort,
    End,
    PageUpdate,
    Compensation,
}

impl LogRecordType {
    fn to_byte(self) -> u8 {
        match self {
            LogRecordType::Begin => 1,
            LogRecordType::Commit => 2,
            LogRecordType::Abort => 3,
            LogRecordType::End => 4,
            LogRecordType::PageUpdate => 5,
            LogRecordType::Compensation => 6,
        }
    }

    fn from_byte(value: u8) -> WalResult<Self> {
        match value {
            1 => Ok(LogRecordType::Begin),
            2 => Ok(LogRecordType::Commit),
            3 => Ok(LogRecordType::Abort),
            4 => Ok(LogRecordType::End),
            5 => Ok(LogRecordType::PageUpdate),
            6 => Ok(LogRecordType::Compensation),
            _ => Err(WalError::Corrupt(format!(
                "invalid log record type {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LogPayload {
    None,
    PageUpdate {
        page_id: PageId,
        offset: u32,
        before: Vec<u8>,
        after: Vec<u8>,
    },
    Compensation {
        page_id: PageId,
        offset: u32,
        after: Vec<u8>,
        undo_next_lsn: Option<Lsn>,
    },
}

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub lsn: Lsn,
    pub txn_id: TxnId,
    pub prev_lsn: Option<Lsn>,
    pub record_type: LogRecordType,
    pub payload: LogPayload,
}

impl LogRecord {
    pub fn begin(lsn: Lsn, txn_id: TxnId, prev_lsn: Option<Lsn>) -> Self {
        Self {
            lsn,
            txn_id,
            prev_lsn,
            record_type: LogRecordType::Begin,
            payload: LogPayload::None,
        }
    }

    pub fn commit(lsn: Lsn, txn_id: TxnId, prev_lsn: Option<Lsn>) -> Self {
        Self {
            lsn,
            txn_id,
            prev_lsn,
            record_type: LogRecordType::Commit,
            payload: LogPayload::None,
        }
    }

    pub fn abort(lsn: Lsn, txn_id: TxnId, prev_lsn: Option<Lsn>) -> Self {
        Self {
            lsn,
            txn_id,
            prev_lsn,
            record_type: LogRecordType::Abort,
            payload: LogPayload::None,
        }
    }

    pub fn end(lsn: Lsn, txn_id: TxnId, prev_lsn: Option<Lsn>) -> Self {
        Self {
            lsn,
            txn_id,
            prev_lsn,
            record_type: LogRecordType::End,
            payload: LogPayload::None,
        }
    }

    pub fn page_update(
        lsn: Lsn,
        txn_id: TxnId,
        prev_lsn: Option<Lsn>,
        page_id: PageId,
        offset: u32,
        before: Vec<u8>,
        after: Vec<u8>,
    ) -> Self {
        Self {
            lsn,
            txn_id,
            prev_lsn,
            record_type: LogRecordType::PageUpdate,
            payload: LogPayload::PageUpdate {
                page_id,
                offset,
                before,
                after,
            },
        }
    }

    pub fn compensation(
        lsn: Lsn,
        txn_id: TxnId,
        prev_lsn: Option<Lsn>,
        page_id: PageId,
        offset: u32,
        after: Vec<u8>,
        undo_next_lsn: Option<Lsn>,
    ) -> Self {
        Self {
            lsn,
            txn_id,
            prev_lsn,
            record_type: LogRecordType::Compensation,
            payload: LogPayload::Compensation {
                page_id,
                offset,
                after,
                undo_next_lsn,
            },
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.push(self.record_type.to_byte());
        buffer.extend_from_slice(&self.lsn.to_le_bytes());
        buffer.extend_from_slice(&self.txn_id.to_le_bytes());
        let prev = self.prev_lsn.unwrap_or(INVALID_LSN);
        buffer.extend_from_slice(&prev.to_le_bytes());
        match &self.payload {
            LogPayload::None => {}
            LogPayload::PageUpdate {
                page_id,
                offset,
                before,
                after,
            } => {
                buffer.extend_from_slice(&page_id.to_le_bytes());
                buffer.extend_from_slice(&offset.to_le_bytes());
                buffer.extend_from_slice(&(before.len() as u32).to_le_bytes());
                buffer.extend_from_slice(&(after.len() as u32).to_le_bytes());
                buffer.extend_from_slice(before);
                buffer.extend_from_slice(after);
            }
            LogPayload::Compensation {
                page_id,
                offset,
                after,
                undo_next_lsn,
            } => {
                buffer.extend_from_slice(&page_id.to_le_bytes());
                buffer.extend_from_slice(&offset.to_le_bytes());
                buffer.extend_from_slice(&(after.len() as u32).to_le_bytes());
                let undo_next = undo_next_lsn.unwrap_or(INVALID_LSN);
                buffer.extend_from_slice(&undo_next.to_le_bytes());
                buffer.extend_from_slice(after);
            }
        }
        let len = buffer.len() as u32;
        buffer[0..4].copy_from_slice(&len.to_le_bytes());
        buffer
    }

    pub fn from_bytes(bytes: &[u8]) -> WalResult<Self> {
        if bytes.len() < 1 + 8 + 8 + 8 {
            return Err(WalError::Corrupt("log record too small".to_string()));
        }
        let record_type = LogRecordType::from_byte(bytes[0])?;
        let lsn = read_u64(&bytes[1..9]);
        let txn_id = read_u64(&bytes[9..17]);
        let prev_raw = read_u64(&bytes[17..25]);
        let prev_lsn = if prev_raw == INVALID_LSN {
            None
        } else {
            Some(prev_raw)
        };
        let mut offset = 25;
        let payload = match record_type {
            LogRecordType::PageUpdate => {
                if bytes.len() < offset + 8 + 4 + 4 + 4 {
                    return Err(WalError::Corrupt(
                        "page update record truncated".to_string(),
                    ));
                }
                let page_id = read_u64(&bytes[offset..offset + 8]);
                offset += 8;
                let write_offset = read_u32(&bytes[offset..offset + 4]);
                offset += 4;
                let before_len = read_u32(&bytes[offset..offset + 4]) as usize;
                offset += 4;
                let after_len = read_u32(&bytes[offset..offset + 4]) as usize;
                offset += 4;
                if bytes.len() < offset + before_len + after_len {
                    return Err(WalError::Corrupt("page update bytes truncated".to_string()));
                }
                let before = bytes[offset..offset + before_len].to_vec();
                offset += before_len;
                let after = bytes[offset..offset + after_len].to_vec();
                LogPayload::PageUpdate {
                    page_id,
                    offset: write_offset,
                    before,
                    after,
                }
            }
            LogRecordType::Compensation => {
                if bytes.len() < offset + 8 + 4 + 4 + 8 {
                    return Err(WalError::Corrupt(
                        "compensation record truncated".to_string(),
                    ));
                }
                let page_id = read_u64(&bytes[offset..offset + 8]);
                offset += 8;
                let write_offset = read_u32(&bytes[offset..offset + 4]);
                offset += 4;
                let after_len = read_u32(&bytes[offset..offset + 4]) as usize;
                offset += 4;
                let undo_next_raw = read_u64(&bytes[offset..offset + 8]);
                offset += 8;
                if bytes.len() < offset + after_len {
                    return Err(WalError::Corrupt(
                        "compensation bytes truncated".to_string(),
                    ));
                }
                let after = bytes[offset..offset + after_len].to_vec();
                let undo_next_lsn = if undo_next_raw == INVALID_LSN {
                    None
                } else {
                    Some(undo_next_raw)
                };
                LogPayload::Compensation {
                    page_id,
                    offset: write_offset,
                    after,
                    undo_next_lsn,
                }
            }
            _ => LogPayload::None,
        };
        Ok(LogRecord {
            lsn,
            txn_id,
            prev_lsn,
            record_type,
            payload,
        })
    }
}

#[derive(Debug)]
pub struct Transaction {
    pub txn_id: TxnId,
    pub last_lsn: Option<Lsn>,
}

pub type TransactionHandle = Arc<Mutex<Transaction>>;

#[derive(Clone)]
pub struct TransactionManager {
    log_manager: Arc<LogManager>,
    lock_manager: Option<Arc<LockManager>>,
    next_txn_id: Arc<AtomicU64>,
}

impl TransactionManager {
    pub fn new(log_manager: Arc<LogManager>) -> Self {
        Self {
            log_manager,
            lock_manager: None,
            next_txn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn with_lock_manager(log_manager: Arc<LogManager>, lock_manager: Arc<LockManager>) -> Self {
        Self {
            log_manager,
            lock_manager: Some(lock_manager),
            next_txn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn log_manager(&self) -> Arc<LogManager> {
        Arc::clone(&self.log_manager)
    }

    pub fn lock_manager(&self) -> Option<Arc<LockManager>> {
        self.lock_manager.clone()
    }

    pub fn begin(&self) -> WalResult<TransactionHandle> {
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::SeqCst);
        let mut txn = Transaction {
            txn_id,
            last_lsn: None,
        };
        let lsn = self.log_manager.append(LogRecord::begin(0, txn_id, None))?;
        txn.last_lsn = Some(lsn);
        Ok(Arc::new(Mutex::new(txn)))
    }

    pub fn commit(&self, txn: &TransactionHandle) -> WalResult<()> {
        let txn_id = txn.lock().txn_id;
        let mut guard = txn.lock();
        let lsn = self
            .log_manager
            .append(LogRecord::commit(0, guard.txn_id, guard.last_lsn))?;
        guard.last_lsn = Some(lsn);
        drop(guard);
        self.log_manager.flush(lsn)?;
        let mut guard = txn.lock();
        let end_lsn = self
            .log_manager
            .append(LogRecord::end(0, guard.txn_id, guard.last_lsn))?;
        guard.last_lsn = Some(end_lsn);
        self.log_manager.flush(end_lsn)?;
        drop(guard);
        if let Some(lock_manager) = &self.lock_manager {
            lock_manager.unlock_all(txn::TxnId(txn_id));
        }
        Ok(())
    }

    pub fn abort(&self, txn: &TransactionHandle) -> WalResult<()> {
        let txn_id = txn.lock().txn_id;
        let mut guard = txn.lock();
        let lsn = self
            .log_manager
            .append(LogRecord::abort(0, guard.txn_id, guard.last_lsn))?;
        guard.last_lsn = Some(lsn);
        drop(guard);
        if let Some(lock_manager) = &self.lock_manager {
            lock_manager.unlock_all(txn::TxnId(txn_id));
        }
        Ok(())
    }

    pub fn end(&self, txn: &TransactionHandle) -> WalResult<()> {
        let txn_id = txn.lock().txn_id;
        let mut guard = txn.lock();
        let lsn = self
            .log_manager
            .append(LogRecord::end(0, guard.txn_id, guard.last_lsn))?;
        guard.last_lsn = Some(lsn);
        drop(guard);
        if let Some(lock_manager) = &self.lock_manager {
            lock_manager.unlock_all(txn::TxnId(txn_id));
        }
        Ok(())
    }

    pub fn with_transaction<F, R>(&self, txn: &TransactionHandle, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let guard =
            set_transaction_context(self.log_manager(), Arc::clone(txn), self.lock_manager());
        let result = f();
        drop(guard);
        result
    }
}

pub struct TransactionGuard {
    previous: Option<TransactionContext>,
}

#[derive(Clone)]
struct TransactionContext {
    log_manager: Arc<LogManager>,
    transaction: TransactionHandle,
    lock_manager: Option<Arc<LockManager>>,
}

thread_local! {
    static CURRENT_TXN: RefCell<Option<TransactionContext>> = const { RefCell::new(None) };
}

fn set_transaction_context(
    log_manager: Arc<LogManager>,
    transaction: TransactionHandle,
    lock_manager: Option<Arc<LockManager>>,
) -> TransactionGuard {
    let previous = CURRENT_TXN.with(|cell| {
        cell.replace(Some(TransactionContext {
            log_manager,
            transaction,
            lock_manager,
        }))
    });
    TransactionGuard { previous }
}

pub fn current_txn_id() -> Option<TxnId> {
    CURRENT_TXN.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|ctx| ctx.transaction.lock().txn_id)
    })
}

pub fn current_txn_handle() -> Option<TransactionHandle> {
    CURRENT_TXN.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|ctx| Arc::clone(&ctx.transaction))
    })
}

pub fn current_lock_manager() -> Option<Arc<LockManager>> {
    CURRENT_TXN.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|ctx| ctx.lock_manager.clone())
    })
}

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        let previous = self.previous.clone();
        CURRENT_TXN.with(|cell| {
            *cell.borrow_mut() = previous;
        });
    }
}

pub fn log_page_update(
    page_id: PageId,
    offset: u32,
    before: Vec<u8>,
    after: Vec<u8>,
) -> WalResult<Option<Lsn>> {
    CURRENT_TXN.with(|cell| {
        let mut context = cell.borrow_mut();
        let Some(context) = context.as_mut() else {
            return Ok(None);
        };
        let mut txn_guard = context.transaction.lock();
        let record = LogRecord::page_update(
            0,
            txn_guard.txn_id,
            txn_guard.last_lsn,
            page_id,
            offset,
            before,
            after,
        );
        let lsn = context.log_manager.append(record)?;
        txn_guard.last_lsn = Some(lsn);
        Ok(Some(lsn))
    })
}

pub fn log_compensation(
    txn: &TransactionHandle,
    log_manager: &LogManager,
    page_id: PageId,
    offset: u32,
    after: Vec<u8>,
    undo_next_lsn: Option<Lsn>,
) -> WalResult<Lsn> {
    let mut guard = txn.lock();
    let record = LogRecord::compensation(
        0,
        guard.txn_id,
        guard.last_lsn,
        page_id,
        offset,
        after,
        undo_next_lsn,
    );
    let lsn = log_manager.append(record)?;
    guard.last_lsn = Some(lsn);
    Ok(lsn)
}

#[derive(Clone)]
pub struct LogManager {
    state: Arc<Mutex<LogState>>,
    condvar: Arc<Condvar>,
    sender: mpsc::Sender<FlushRequest>,
}

struct FlushRequest {
    start_lsn: Lsn,
    end_lsn: Lsn,
    bytes: Vec<u8>,
}

struct LogState {
    active: Vec<u8>,
    flushing: Vec<u8>,
    active_start_lsn: Lsn,
    next_lsn: Lsn,
    flushed_lsn: Lsn,
    flushing_in_progress: bool,
    buffer_size: usize,
    last_error: Option<WalError>,
}

impl LogManager {
    pub fn open(path: impl AsRef<Path>) -> WalResult<Self> {
        Self::open_with_buffer(path, DEFAULT_LOG_BUFFER_SIZE)
    }

    pub fn open_with_buffer(path: impl AsRef<Path>, buffer_size: usize) -> WalResult<Self> {
        let path_ref = path.as_ref();
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path_ref)?;
        let len = file.metadata()?.len();
        file.seek(SeekFrom::End(0))?;
        let state = Arc::new(Mutex::new(LogState {
            active: Vec::with_capacity(buffer_size),
            flushing: Vec::with_capacity(buffer_size),
            active_start_lsn: len,
            next_lsn: len,
            flushed_lsn: len,
            flushing_in_progress: false,
            buffer_size,
            last_error: None,
        }));
        let condvar = Arc::new(Condvar::new());
        let (sender, receiver) = mpsc::channel();
        let state_clone = Arc::clone(&state);
        let condvar_clone = Arc::clone(&condvar);
        std::thread::spawn(move || {
            for request in receiver {
                let result = write_flush_request(&mut file, &request);
                let mut state = state_clone.lock();
                if let Err(error) = result {
                    state.last_error = Some(error);
                } else {
                    state.flushed_lsn = state.flushed_lsn.max(request.end_lsn);
                }
                state.flushing.clear();
                state.flushing_in_progress = false;
                condvar_clone.notify_all();
            }
        });
        Ok(Self {
            state,
            condvar,
            sender,
        })
    }

    pub fn append(&self, mut record: LogRecord) -> WalResult<Lsn> {
        let mut state = self.state.lock();
        state.ensure_ok()?;
        record.lsn = state.next_lsn;
        let bytes = record.to_bytes();
        if state.active.len() + bytes.len() > state.buffer_size {
            self.flush_active_locked(&mut state)?;
        }
        let lsn = record.lsn;
        state.active.extend_from_slice(&bytes);
        state.next_lsn += bytes.len() as u64;
        Ok(lsn)
    }

    pub fn flush(&self, lsn: Lsn) -> WalResult<()> {
        let mut state = self.state.lock();
        state.ensure_ok()?;
        if lsn <= state.flushed_lsn {
            return Ok(());
        }
        if lsn >= state.active_start_lsn {
            self.flush_active_locked(&mut state)?;
        }
        while state.flushed_lsn < lsn {
            self.condvar.wait(&mut state);
            state.ensure_ok()?;
        }
        Ok(())
    }

    pub fn flushed_lsn(&self) -> Lsn {
        self.state.lock().flushed_lsn
    }

    fn flush_active_locked(
        &self,
        state: &mut parking_lot::MutexGuard<'_, LogState>,
    ) -> WalResult<()> {
        if state.active.is_empty() {
            return Ok(());
        }
        while state.flushing_in_progress {
            self.condvar.wait(state);
            state.ensure_ok()?;
        }
        let start_lsn = state.active_start_lsn;
        let mut flush_buffer = std::mem::take(&mut state.active);
        std::mem::swap(&mut flush_buffer, &mut state.flushing);
        flush_buffer.clear();
        state.active = flush_buffer;
        let end_lsn = start_lsn + state.flushing.len() as u64;

        state.active_start_lsn = end_lsn;
        state.flushing_in_progress = true;
        let mut bytes = Vec::with_capacity(state.flushing.len());
        bytes.extend_from_slice(&state.flushing);
        self.sender
            .send(FlushRequest {
                start_lsn,
                end_lsn,
                bytes,
            })
            .map_err(|_| WalError::ChannelClosed)?;
        Ok(())
    }
}

impl LogState {
    fn ensure_ok(&self) -> WalResult<()> {
        if let Some(error) = &self.last_error {
            return Err(WalError::Corrupt(error.to_string()));
        }
        Ok(())
    }
}

pub struct LogReader {
    file: File,
    offset: u64,
}

impl LogReader {
    pub fn open(path: impl AsRef<Path>) -> WalResult<Self> {
        let file = OpenOptions::new().read(true).open(path)?;
        Ok(Self { file, offset: 0 })
    }

    pub fn seek(&mut self, lsn: Lsn) -> WalResult<()> {
        self.offset = lsn;
        self.file.seek(SeekFrom::Start(lsn))?;
        Ok(())
    }

    pub fn next_record(&mut self) -> WalResult<Option<LogRecord>> {
        let mut len_bytes = [0u8; 4];
        let bytes_read = self.file.read(&mut len_bytes)?;
        if bytes_read == 0 {
            return Ok(None);
        }
        if bytes_read < 4 {
            return Err(WalError::Corrupt("log record length truncated".to_string()));
        }
        let len = u32::from_le_bytes(len_bytes) as usize;
        if len < 4 {
            return Err(WalError::Corrupt("invalid log record length".to_string()));
        }
        let mut payload = vec![0u8; len - 4];
        self.file.read_exact(&mut payload)?;
        self.offset += len as u64;
        let record = LogRecord::from_bytes(&payload)?;
        Ok(Some(record))
    }
}

fn write_flush_request(file: &mut File, request: &FlushRequest) -> WalResult<()> {
    file.seek(SeekFrom::Start(request.start_lsn))?;
    file.write_all(&request.bytes)?;
    file.sync_data()?;
    Ok(())
}

fn read_u64(bytes: &[u8]) -> u64 {
    let mut array = [0u8; 8];
    array.copy_from_slice(bytes);
    u64::from_le_bytes(array)
}

fn read_u32(bytes: &[u8]) -> u32 {
    let mut array = [0u8; 4];
    array.copy_from_slice(bytes);
    u32::from_le_bytes(array)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn log_round_trip() {
        let path = std::env::temp_dir().join("wal_round_trip.log");
        let _ = fs::remove_file(&path);
        let manager = LogManager::open_with_buffer(&path, 128).unwrap();
        let txn_manager = TransactionManager::new(Arc::new(manager));
        let txn = txn_manager.begin().unwrap();
        txn_manager.with_transaction(&txn, || {
            let lsn = log_page_update(42, 12, vec![1, 2], vec![3, 4]).unwrap();
            assert!(lsn.is_some());
        });
        txn_manager.commit(&txn).unwrap();
        let mut reader = LogReader::open(&path).unwrap();
        let mut seen = Vec::new();
        while let Some(record) = reader.next_record().unwrap() {
            seen.push(record.record_type);
        }
        assert!(seen.contains(&LogRecordType::Begin));
        assert!(seen.contains(&LogRecordType::PageUpdate));
        assert!(seen.contains(&LogRecordType::Commit));
        let _ = fs::remove_file(&path);
    }
}
