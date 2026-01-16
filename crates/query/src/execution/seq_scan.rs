use crate::execution::operator::{ExecutionError, ExecutionResult, PhysicalOperator};
use crate::execution::tuple::{Tuple, Value};
use crate::schema::{DataType, Schema};
use std::any::Any;
use std::sync::{Arc, Mutex, MutexGuard};
use storage::{BufferPoolManager, FlushMode, Page, PageId, PAGE_LSN_SIZE, PAGE_SIZE};
use txn::{LockKey, LockMode, TxnId};

const HEADER_DATA_SIZE: usize = 16;
const HEADER_SIZE: usize = PAGE_LSN_SIZE + HEADER_DATA_SIZE;
const SLOT_SIZE: usize = 8;
const INVALID_PAGE_ID: PageId = 0;
const INLINE_BLOB_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rid {
    pub page_id: PageId,
    pub slot_id: u32,
}

#[derive(Debug, Clone, Copy)]
struct BlobPointer {
    first_page_id: PageId,
    length: u32,
}

#[derive(Clone)]
struct BlobStore {
    buffer_pool: BufferPoolManager,
}

impl BlobStore {
    fn new(buffer_pool: BufferPoolManager) -> Self {
        Self { buffer_pool }
    }

    fn write_blob(&self, bytes: &[u8]) -> ExecutionResult<BlobPointer> {
        if bytes.is_empty() {
            return Ok(BlobPointer {
                first_page_id: INVALID_PAGE_ID,
                length: 0,
            });
        }
        let total_len = u32::try_from(bytes.len())
            .map_err(|_| ExecutionError::Execution("blob too large".to_string()))?;
        let payload_capacity = PAGE_SIZE - blob_payload_offset();
        let mut page_ids = Vec::new();
        let mut remaining = bytes.len();
        while remaining > 0 {
            let page_id = self.buffer_pool.new_page()?.ok_or_else(|| {
                ExecutionError::Execution("buffer pool has no free frames".to_string())
            })?;
            page_ids.push(page_id);
            let chunk_len = remaining.min(payload_capacity);
            remaining -= chunk_len;
        }
        let mut offset = 0;
        for (index, page_id) in page_ids.iter().enumerate() {
            let next_page = page_ids.get(index + 1).copied().unwrap_or(INVALID_PAGE_ID);
            let remaining = bytes.len() - offset;
            let chunk_len = remaining.min(payload_capacity);
            let chunk = &bytes[offset..offset + chunk_len];
            offset += chunk_len;
            {
                let mut page_guard = self.fetch_page_exclusive(*page_id)?;
                write_blob_header(&mut page_guard, next_page, chunk_len as u32)?;
                let payload_offset = blob_payload_offset();
                if !page_guard.write_bytes(payload_offset, chunk) {
                    return Err(ExecutionError::Execution(
                        "failed to write blob payload".to_string(),
                    ));
                }
                page_guard.set_lsn(0);
            }
            self.buffer_pool.unpin_page(*page_id, true)?;
            self.buffer_pool
                .flush_page_with_mode(*page_id, FlushMode::Force)?;
        }
        Ok(BlobPointer {
            first_page_id: page_ids[0],
            length: total_len,
        })
    }

    fn read_blob(&self, pointer: BlobPointer) -> ExecutionResult<Vec<u8>> {
        if pointer.length == 0 {
            return Ok(Vec::new());
        }
        let mut output = Vec::with_capacity(pointer.length as usize);
        let payload_capacity = PAGE_SIZE - blob_payload_offset();
        let expected_pages = if pointer.length == 0 {
            0
        } else {
            (pointer.length as usize).div_ceil(payload_capacity)
        };
        let mut pages_seen = 0;
        let mut page_id = pointer.first_page_id;
        while page_id != INVALID_PAGE_ID {
            pages_seen += 1;
            if pages_seen > expected_pages {
                return Err(ExecutionError::Execution(
                    "blob page chain exceeds expected length".to_string(),
                ));
            }
            let (next_page, _payload_len, chunk) = {
                let page_guard = self.fetch_page_with_lock(page_id, LockMode::Shared)?;
                if page_guard.lsn() != 0 {
                    return Err(ExecutionError::Execution(
                        "blob page has unexpected WAL LSN".to_string(),
                    ));
                }
                let (next_page, payload_len) = read_blob_header(&page_guard)?;
                if payload_len == 0 {
                    return Err(ExecutionError::Execution(
                        "blob page payload length is zero".to_string(),
                    ));
                }
                if payload_len as usize > payload_capacity {
                    return Err(ExecutionError::Execution(
                        "blob payload length exceeds page capacity".to_string(),
                    ));
                }
                let payload_offset = blob_payload_offset();
                let data = page_guard
                    .read_bytes(payload_offset, payload_len as usize)
                    .ok_or_else(|| ExecutionError::Execution("blob payload truncated".to_string()))?
                    .to_vec();
                (next_page, payload_len, data)
            };
            self.buffer_pool.unpin_page(page_id, false)?;
            output.extend_from_slice(&chunk);
            if output.len() > pointer.length as usize {
                return Err(ExecutionError::Execution(
                    "blob payload exceeds expected length".to_string(),
                ));
            }
            page_id = next_page;
            if output.len() >= pointer.length as usize {
                break;
            }
        }
        if output.len() != pointer.length as usize {
            return Err(ExecutionError::Execution(
                "blob payload length mismatch".to_string(),
            ));
        }
        Ok(output)
    }

    fn fetch_page_with_lock(
        &self,
        page_id: PageId,
        mode: LockMode,
    ) -> ExecutionResult<storage::PageGuard<'_>> {
        if let (Some(lock_manager), Some(txn_id)) =
            (wal::current_lock_manager(), wal::current_txn_id())
        {
            let txn_id = TxnId(txn_id);
            match mode {
                LockMode::Shared => lock_manager
                    .lock_shared(txn_id, LockKey::Page(page_id))
                    .map_err(|err| ExecutionError::Execution(format!("lock error: {err:?}")))?,
                LockMode::Exclusive => lock_manager
                    .lock_exclusive(txn_id, LockKey::Page(page_id))
                    .map_err(|err| ExecutionError::Execution(format!("lock error: {err:?}")))?,
            }
        }
        self.buffer_pool.fetch_page(page_id)?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no available frame".to_string())
        })
    }

    fn fetch_page_exclusive(&self, page_id: PageId) -> ExecutionResult<storage::PageGuard<'_>> {
        self.fetch_page_with_lock(page_id, LockMode::Exclusive)
    }
}

#[derive(Clone)]
pub struct TableHeap {
    buffer_pool: BufferPoolManager,
    first_page_id: Arc<Mutex<Option<PageId>>>,
    blob_store: BlobStore,
}

impl TableHeap {
    pub fn new(buffer_pool: BufferPoolManager, first_page_id: Option<PageId>) -> Self {
        Self {
            blob_store: BlobStore::new(buffer_pool.clone()),
            buffer_pool,
            first_page_id: Arc::new(Mutex::new(first_page_id)),
        }
    }

    pub fn create(buffer_pool: BufferPoolManager) -> ExecutionResult<Self> {
        let heap = Self::new(buffer_pool, None);
        let page_id = heap.allocate_page()?;
        heap.set_first_page_id(Some(page_id))?;
        Ok(heap)
    }

    pub fn load(first_page_id: PageId, buffer_pool: BufferPoolManager) -> ExecutionResult<Self> {
        Ok(Self::new(buffer_pool, Some(first_page_id)))
    }

    pub fn buffer_pool(&self) -> &BufferPoolManager {
        &self.buffer_pool
    }

    pub fn first_page_id(&self) -> ExecutionResult<Option<PageId>> {
        Ok(*self.first_page_guard()?)
    }

    pub fn set_first_page_id(&self, page_id: Option<PageId>) -> ExecutionResult<()> {
        *self.first_page_guard()? = page_id;
        Ok(())
    }

    #[allow(dead_code)]
    fn fetch_page(&self, page_id: PageId) -> ExecutionResult<storage::PageGuard<'_>> {
        self.fetch_page_with_lock(page_id, LockMode::Shared)
    }

    fn fetch_page_exclusive(&self, page_id: PageId) -> ExecutionResult<storage::PageGuard<'_>> {
        self.fetch_page_with_lock(page_id, LockMode::Exclusive)
    }

    pub fn fetch_page_with_lock(
        &self,
        page_id: PageId,
        mode: LockMode,
    ) -> ExecutionResult<storage::PageGuard<'_>> {
        if let (Some(lock_manager), Some(txn_id)) =
            (wal::current_lock_manager(), wal::current_txn_id())
        {
            let txn_id = TxnId(txn_id);
            match mode {
                LockMode::Shared => lock_manager
                    .lock_shared(txn_id, LockKey::Page(page_id))
                    .map_err(|err| ExecutionError::Execution(format!("lock error: {err:?}")))?,
                LockMode::Exclusive => lock_manager
                    .lock_exclusive(txn_id, LockKey::Page(page_id))
                    .map_err(|err| ExecutionError::Execution(format!("lock error: {err:?}")))?,
            }
        }
        self.buffer_pool.fetch_page(page_id)?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no available frame".to_string())
        })
    }

    pub fn insert_tuple(&self, tuple: &Tuple, schema: &Schema) -> ExecutionResult<Rid> {
        let tuple_bytes = encode_tuple(tuple, schema, &self.blob_store)?;
        let mut current_page_id = self.first_page_id()?;
        if current_page_id.is_none() {
            let page_id = self.allocate_page()?;
            self.set_first_page_id(Some(page_id))?;
            current_page_id = Some(page_id);
        }

        loop {
            let page_id = current_page_id
                .ok_or_else(|| ExecutionError::Execution("table heap has no pages".to_string()))?;

            let mut inserted = false;
            let mut inserted_slot = None;
            let mut next_page_id = None;
            let mut page_dirty = false;
            let mut needs_new_page = false;

            {
                let mut page_guard = self.fetch_page_exclusive(page_id)?;
                let mut header = read_header(&page_guard)?;
                let slot_area = HEADER_SIZE + header.slot_count as usize * SLOT_SIZE;
                let free_space_offset = header.free_space_offset as usize;
                let available_space = free_space_offset.saturating_sub(slot_area);
                if available_space >= tuple_bytes.len() + SLOT_SIZE {
                    let tuple_offset =
                        (header.free_space_offset as usize - tuple_bytes.len()) as u32;
                    write_bytes_logged(&mut page_guard, tuple_offset as usize, &tuple_bytes)
                        .map_err(|_| {
                            ExecutionError::Execution("failed to write tuple bytes".to_string())
                        })?;
                    let slot_index = header.slot_count as usize;
                    write_slot(
                        &mut page_guard,
                        slot_index,
                        TableSlot {
                            offset: tuple_offset,
                            len: tuple_bytes.len() as u32,
                        },
                    )?;
                    header.slot_count += 1;
                    header.free_space_offset = tuple_offset;
                    write_header(&mut page_guard, &header)?;
                    inserted = true;
                    inserted_slot = Some(slot_index as u32);
                    page_dirty = true;
                } else if let Some(existing_next) = header.next_page_id {
                    next_page_id = Some(existing_next);
                } else {
                    needs_new_page = true;
                }
            }

            if inserted {
                self.buffer_pool.unpin_page(page_id, page_dirty)?;
                return Ok(Rid {
                    page_id,
                    slot_id: inserted_slot.expect("inserted slot missing"),
                });
            }

            self.buffer_pool.unpin_page(page_id, page_dirty)?;

            if needs_new_page {
                let new_page_id = self.allocate_page()?;
                let mut update_dirty = false;
                {
                    let mut page_guard = self.fetch_page_exclusive(page_id)?;
                    let mut header = read_header(&page_guard)?;
                    if header.next_page_id.is_none() {
                        header.next_page_id = Some(new_page_id);
                        write_header(&mut page_guard, &header)?;
                        update_dirty = true;
                    }
                }
                self.buffer_pool.unpin_page(page_id, update_dirty)?;
                current_page_id = Some(new_page_id);
            } else {
                current_page_id = next_page_id;
            }
        }
    }

    pub fn get_tuple(&self, rid: Rid, schema: &Schema) -> ExecutionResult<Option<Tuple>> {
        let result = {
            let page_guard = self.fetch_page_with_lock(rid.page_id, LockMode::Shared)?;
            let result: ExecutionResult<Option<Vec<u8>>> = (|| {
                let header = read_header(&page_guard)?;
                if rid.slot_id < header.slot_count {
                    if let Some(slot) = read_slot(&page_guard, rid.slot_id as usize)? {
                        return Ok(Some(read_tuple_bytes(&page_guard, &slot)?));
                    }
                }
                Ok(None)
            })();
            result
        };
        self.buffer_pool.unpin_page(rid.page_id, false)?;
        let tuple_bytes = result?;
        let tuple = match tuple_bytes {
            Some(bytes) => Some(decode_tuple(schema, &bytes, &self.blob_store)?),
            None => None,
        };
        Ok(tuple)
    }

    pub fn delete_tuple(&self, rid: Rid) -> ExecutionResult<bool> {
        let mut deleted = false;
        {
            let mut page_guard = self.fetch_page_exclusive(rid.page_id)?;
            let header = read_header(&page_guard)?;
            if rid.slot_id < header.slot_count {
                if let Some(mut slot) = read_slot(&page_guard, rid.slot_id as usize)? {
                    if slot.len != 0 {
                        slot.len = 0;
                        write_slot(&mut page_guard, rid.slot_id as usize, slot)?;
                        deleted = true;
                    }
                }
            }
        }
        self.buffer_pool.unpin_page(rid.page_id, deleted)?;
        Ok(deleted)
    }

    pub fn update_tuple(&self, rid: Rid, tuple: &Tuple, schema: &Schema) -> ExecutionResult<Rid> {
        let tuple_bytes = encode_tuple(tuple, schema, &self.blob_store)?;
        let mut updated = false;
        let needs_reinsert = {
            let mut page_guard = self.fetch_page_exclusive(rid.page_id)?;
            let header = read_header(&page_guard)?;
            if rid.slot_id >= header.slot_count {
                return Err(ExecutionError::Execution(
                    "update slot out of range".to_string(),
                ));
            }
            let slot = read_slot(&page_guard, rid.slot_id as usize)?
                .ok_or_else(|| ExecutionError::Execution("tuple slot is empty".to_string()))?;
            if slot.len == 0 {
                return Err(ExecutionError::Execution(
                    "tuple slot is deleted".to_string(),
                ));
            }
            let needs_reinsert = tuple_bytes.len() > slot.len as usize;
            if !needs_reinsert {
                write_bytes_logged(&mut page_guard, slot.offset as usize, &tuple_bytes).map_err(
                    |_| ExecutionError::Execution("failed to write updated tuple".to_string()),
                )?;
                let mut updated_slot = slot;
                updated_slot.len = tuple_bytes.len() as u32;
                write_slot(&mut page_guard, rid.slot_id as usize, updated_slot)?;
                updated = true;
            }
            needs_reinsert
        };
        self.buffer_pool.unpin_page(rid.page_id, updated)?;
        if updated {
            return Ok(rid);
        }
        if needs_reinsert {
            let _ = self.delete_tuple(rid)?;
            return self.insert_tuple(tuple, schema);
        }
        Ok(rid)
    }

    pub fn scan_tuples(&self, schema: &Schema) -> ExecutionResult<Vec<(Rid, Tuple)>> {
        let mut output = Vec::new();
        let mut current_page_id = self.first_page_id()?;
        while let Some(page_id) = current_page_id {
            let result = {
                let page_guard = self.fetch_page_with_lock(page_id, LockMode::Shared)?;
                let header = read_header(&page_guard)?;
                let mut tuples = Vec::new();
                for slot_index in 0..header.slot_count as usize {
                    if let Some(slot) = read_slot(&page_guard, slot_index)? {
                        let tuple_bytes = read_tuple_bytes(&page_guard, &slot)?;
                        tuples.push((
                            Rid {
                                page_id,
                                slot_id: slot_index as u32,
                            },
                            tuple_bytes,
                        ));
                    }
                }
                Ok::<_, ExecutionError>((header, tuples))
            };
            self.buffer_pool.unpin_page(page_id, false)?;
            let (header, tuple_bytes) = result?;
            for (rid, bytes) in tuple_bytes {
                let tuple = decode_tuple(schema, &bytes, &self.blob_store)?;
                output.push((rid, tuple));
            }
            current_page_id = header.next_page_id;
        }
        Ok(output)
    }

    fn allocate_page(&self) -> ExecutionResult<PageId> {
        let page_id = self.buffer_pool.new_page()?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no free frames".to_string())
        })?;
        {
            let mut page_guard = self.fetch_page_exclusive(page_id)?;
            initialize_page(&mut page_guard)?;
        }
        self.buffer_pool.unpin_page(page_id, true)?;
        Ok(page_id)
    }

    fn first_page_guard(&self) -> ExecutionResult<MutexGuard<'_, Option<PageId>>> {
        self.first_page_id
            .lock()
            .map_err(|_| ExecutionError::Execution("table heap lock poisoned".to_string()))
    }
}

pub struct SeqScan {
    table_heap: TableHeap,
    schema: Schema,
    current_page_id: Option<PageId>,
    current_slot: usize,
}

impl SeqScan {
    pub fn new(table_heap: TableHeap, schema: Schema) -> Self {
        Self {
            table_heap,
            schema,
            current_page_id: None,
            current_slot: 0,
        }
    }
}

impl PhysicalOperator for SeqScan {
    fn open(&mut self) -> ExecutionResult<()> {
        self.current_page_id = self.table_heap.first_page_id()?;
        self.current_slot = 0;
        Ok(())
    }

    fn next(&mut self) -> ExecutionResult<Option<Tuple>> {
        loop {
            let page_id = match self.current_page_id {
                Some(page_id) => page_id,
                None => return Ok(None),
            };

            let (header, tuple, advance_page) = {
                let page_guard = self
                    .table_heap
                    .fetch_page_with_lock(page_id, LockMode::Shared)?;
                let header = read_header(&page_guard)?;
                let mut tuple = None;
                let mut advance_page = false;
                if self.current_slot >= header.slot_count as usize {
                    advance_page = true;
                } else {
                    let slot_index = self.current_slot;
                    self.current_slot += 1;
                    if let Some(slot) = read_slot(&page_guard, slot_index)? {
                        let tuple_bytes = read_tuple_bytes(&page_guard, &slot)?;
                        tuple = Some(decode_tuple(
                            &self.schema,
                            &tuple_bytes,
                            &self.table_heap.blob_store,
                        )?);
                    }
                }
                (header, tuple, advance_page)
            };

            self.table_heap.buffer_pool.unpin_page(page_id, false)?;

            if advance_page {
                self.current_page_id = header.next_page_id;
                self.current_slot = 0;
                continue;
            }

            if let Some(tuple) = tuple {
                return Ok(Some(tuple));
            }
        }
    }

    fn close(&mut self) -> ExecutionResult<()> {
        self.current_page_id = None;
        self.current_slot = 0;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Copy)]
struct TableSlot {
    offset: u32,
    len: u32,
}

#[derive(Clone, Copy)]
struct TablePageHeader {
    next_page_id: Option<PageId>,
    slot_count: u32,
    free_space_offset: u32,
}

impl TablePageHeader {
    fn empty() -> Self {
        Self {
            next_page_id: None,
            slot_count: 0,
            free_space_offset: PAGE_SIZE as u32,
        }
    }
}

fn initialize_page(page: &mut Page) -> ExecutionResult<()> {
    page.set_lsn(0);
    let header = TablePageHeader::empty();
    write_header(page, &header)
}

fn read_header(page: &Page) -> ExecutionResult<TablePageHeader> {
    let header_bytes = page
        .read_bytes(PAGE_LSN_SIZE, HEADER_DATA_SIZE)
        .ok_or_else(|| ExecutionError::Execution("failed to read table page header".to_string()))?;
    let next_page_id = read_u64(&header_bytes[0..8]);
    let slot_count = read_u32(&header_bytes[8..12]);
    let free_space_offset = read_u32(&header_bytes[12..16]);
    Ok(TablePageHeader {
        next_page_id: if next_page_id == INVALID_PAGE_ID {
            None
        } else {
            Some(next_page_id)
        },
        slot_count,
        free_space_offset,
    })
}

fn write_bytes_logged(page: &mut Page, offset: usize, bytes: &[u8]) -> ExecutionResult<()> {
    if offset + bytes.len() > PAGE_SIZE {
        return Err(ExecutionError::Execution(
            "page write out of bounds".to_string(),
        ));
    }
    let before = page
        .read_bytes(offset, bytes.len())
        .ok_or_else(|| ExecutionError::Execution("page read out of bounds".to_string()))?
        .to_vec();
    let page_id = page
        .page_id()
        .ok_or_else(|| ExecutionError::Execution("page id missing".to_string()))?;
    let lsn = wal::log_page_update(page_id, offset as u32, before, bytes.to_vec())
        .map_err(|err| ExecutionError::Execution(format!("wal error: {}", err)))?;
    if !page.write_bytes(offset, bytes) {
        return Err(ExecutionError::Execution(
            "failed to write page bytes".to_string(),
        ));
    }
    if let Some(lsn) = lsn {
        let current_page_lsn = page.lsn();
        // DEBUG: This should help identify the LSN issue
        eprintln!(
            "DEBUG: page_id={}, write_lsn={}, current_page_lsn={}, lsn_comparison={}",
            page_id,
            lsn,
            current_page_lsn,
            if lsn > current_page_lsn {
                "GREATER"
            } else {
                "NOT_GREATER"
            }
        );
        if lsn > current_page_lsn {
            page.set_lsn(lsn);
        }
    }
    Ok(())
}

fn write_header(page: &mut Page, header: &TablePageHeader) -> ExecutionResult<()> {
    let mut bytes = [0u8; HEADER_DATA_SIZE];
    let next_page_id = header.next_page_id.unwrap_or(INVALID_PAGE_ID);
    bytes[0..8].copy_from_slice(&next_page_id.to_le_bytes());
    bytes[8..12].copy_from_slice(&header.slot_count.to_le_bytes());
    bytes[12..16].copy_from_slice(&header.free_space_offset.to_le_bytes());
    write_bytes_logged(page, PAGE_LSN_SIZE, &bytes)
}

fn read_slot(page: &Page, slot_index: usize) -> ExecutionResult<Option<TableSlot>> {
    let offset = HEADER_SIZE + slot_index * SLOT_SIZE;
    if offset + SLOT_SIZE > PAGE_SIZE {
        return Err(ExecutionError::Execution(
            "slot offset outside page".to_string(),
        ));
    }
    let slot_bytes = page
        .read_bytes(offset, SLOT_SIZE)
        .ok_or_else(|| ExecutionError::Execution("failed to read slot".to_string()))?;
    let tuple_offset = read_u32(&slot_bytes[0..4]);
    let tuple_len = read_u32(&slot_bytes[4..8]);
    if tuple_len == 0 {
        return Ok(None);
    }
    Ok(Some(TableSlot {
        offset: tuple_offset,
        len: tuple_len,
    }))
}

fn write_slot(page: &mut Page, slot_index: usize, slot: TableSlot) -> ExecutionResult<()> {
    let offset = HEADER_SIZE + slot_index * SLOT_SIZE;
    if offset + SLOT_SIZE > PAGE_SIZE {
        return Err(ExecutionError::Execution(
            "slot offset outside page".to_string(),
        ));
    }
    let mut bytes = [0u8; SLOT_SIZE];
    bytes[0..4].copy_from_slice(&slot.offset.to_le_bytes());
    bytes[4..8].copy_from_slice(&slot.len.to_le_bytes());
    write_bytes_logged(page, offset, &bytes)
        .map_err(|_| ExecutionError::Execution("failed to write slot".to_string()))
}

fn blob_payload_offset() -> usize {
    PAGE_LSN_SIZE + 12
}

fn read_blob_header(page: &Page) -> ExecutionResult<(PageId, u32)> {
    let header_bytes = page
        .read_bytes(PAGE_LSN_SIZE, 12)
        .ok_or_else(|| ExecutionError::Execution("failed to read blob page header".to_string()))?;
    let next_page = read_u64(&header_bytes[0..8]);
    let payload_len = read_u32(&header_bytes[8..12]);
    Ok((next_page, payload_len))
}

fn write_blob_header(page: &mut Page, next_page: PageId, payload_len: u32) -> ExecutionResult<()> {
    let mut header = [0u8; 12];
    header[0..8].copy_from_slice(&next_page.to_le_bytes());
    header[8..12].copy_from_slice(&payload_len.to_le_bytes());
    if !page.write_bytes(PAGE_LSN_SIZE, &header) {
        return Err(ExecutionError::Execution(
            "failed to write blob page header".to_string(),
        ));
    }
    page.set_lsn(0);
    Ok(())
}

fn read_tuple_bytes(page: &Page, slot: &TableSlot) -> ExecutionResult<Vec<u8>> {
    let offset = slot.offset as usize;
    let len = slot.len as usize;
    let data = page
        .read_bytes(offset, len)
        .ok_or_else(|| ExecutionError::Execution("failed to read tuple bytes".to_string()))?;
    Ok(data.to_vec())
}

fn encode_tuple(
    tuple: &Tuple,
    schema: &Schema,
    blob_store: &BlobStore,
) -> ExecutionResult<Vec<u8>> {
    if tuple.len() != schema.fields.len() {
        return Err(ExecutionError::Execution(
            "tuple length does not match schema".to_string(),
        ));
    }

    let mut buffer = Vec::new();
    for (field, value) in schema.fields.iter().zip(tuple.values()) {
        if value.is_null() {
            buffer.push(1);
            continue;
        }
        buffer.push(0);
        match (&field.data_type, value) {
            (DataType::Integer, Value::Integer(number)) => {
                buffer.extend_from_slice(
                    &(i32::try_from(*number).map_err(|_| {
                        ExecutionError::Execution("integer out of range".to_string())
                    })?)
                    .to_le_bytes(),
                );
            }
            (DataType::BigInt, Value::Integer(number))
            | (DataType::Timestamp, Value::Integer(number))
            | (DataType::Timestamp, Value::Timestamp(number)) => {
                buffer.extend_from_slice(&number.to_le_bytes());
            }
            (DataType::Real, Value::Float(number)) => {
                buffer.extend_from_slice(&number.to_le_bytes());
            }
            (DataType::Boolean, Value::Boolean(flag)) => {
                buffer.push(u8::from(*flag));
            }
            (DataType::Text, Value::String(text)) => {
                let text_bytes = text.as_bytes();
                let len = u32::try_from(text_bytes.len())
                    .map_err(|_| ExecutionError::Execution("text too large".to_string()))?;
                buffer.extend_from_slice(&len.to_le_bytes());
                buffer.extend_from_slice(text_bytes);
            }
            (DataType::Blob, Value::Blob(bytes)) => {
                if bytes.len() <= INLINE_BLOB_LIMIT {
                    buffer.push(0);
                    let len = u32::try_from(bytes.len())
                        .map_err(|_| ExecutionError::Execution("blob too large".to_string()))?;
                    buffer.extend_from_slice(&len.to_le_bytes());
                    buffer.extend_from_slice(bytes);
                } else {
                    buffer.push(1);
                    let pointer = blob_store.write_blob(bytes)?;
                    buffer.extend_from_slice(&pointer.first_page_id.to_le_bytes());
                    buffer.extend_from_slice(&pointer.length.to_le_bytes());
                }
            }
            (_, other) => {
                return Err(ExecutionError::Execution(format!(
                    "tuple value {:?} does not match schema",
                    other
                )));
            }
        }
    }

    Ok(buffer)
}

fn decode_tuple(schema: &Schema, data: &[u8], blob_store: &BlobStore) -> ExecutionResult<Tuple> {
    let mut cursor = 0usize;
    let mut values = Vec::with_capacity(schema.fields.len());

    for field in &schema.fields {
        if cursor >= data.len() {
            values.push(Value::Null);
            continue;
        }
        let is_null = data[cursor] == 1;
        cursor += 1;
        if is_null {
            values.push(Value::Null);
            continue;
        }

        match field.data_type {
            DataType::Integer => {
                let bytes = read_exact(data, cursor, 4)?;
                let number = i32::from_le_bytes(bytes.try_into().unwrap()) as i64;
                values.push(Value::Integer(number));
                cursor += 4;
            }
            DataType::BigInt => {
                let bytes = read_exact(data, cursor, 8)?;
                let number = i64::from_le_bytes(bytes.try_into().unwrap());
                values.push(Value::Integer(number));
                cursor += 8;
            }
            DataType::Real => {
                let bytes = read_exact(data, cursor, 8)?;
                let number = f64::from_le_bytes(bytes.try_into().unwrap());
                values.push(Value::Float(number));
                cursor += 8;
            }
            DataType::Boolean => {
                let flag = data.get(cursor).ok_or_else(|| {
                    ExecutionError::Execution("tuple bytes truncated".to_string())
                })?;
                values.push(Value::Boolean(*flag != 0));
                cursor += 1;
            }
            DataType::Text => {
                let length_bytes = read_exact(data, cursor, 4)?;
                let length = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
                cursor += 4;
                let text_bytes = read_exact(data, cursor, length)?;
                let text = String::from_utf8(text_bytes.to_vec())
                    .map_err(|_| ExecutionError::Execution("invalid utf8 string".to_string()))?;
                values.push(Value::String(text));
                cursor += length;
            }
            DataType::Timestamp => {
                let bytes = read_exact(data, cursor, 8)?;
                let number = i64::from_le_bytes(bytes.try_into().unwrap());
                values.push(Value::Timestamp(number));
                cursor += 8;
            }
            DataType::Blob => {
                let flag = data.get(cursor).ok_or_else(|| {
                    ExecutionError::Execution("tuple bytes truncated".to_string())
                })?;
                cursor += 1;
                match flag {
                    0 => {
                        let length_bytes = read_exact(data, cursor, 4)?;
                        let length = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
                        cursor += 4;
                        let blob_bytes = read_exact(data, cursor, length)?;
                        values.push(Value::Blob(blob_bytes.to_vec()));
                        cursor += length;
                    }
                    1 => {
                        let page_bytes = read_exact(data, cursor, 8)?;
                        let page_id = u64::from_le_bytes(page_bytes.try_into().unwrap());
                        cursor += 8;
                        let length_bytes = read_exact(data, cursor, 4)?;
                        let length = u32::from_le_bytes(length_bytes.try_into().unwrap());
                        cursor += 4;
                        let blob = blob_store.read_blob(BlobPointer {
                            first_page_id: page_id,
                            length,
                        })?;
                        values.push(Value::Blob(blob));
                    }
                    _ => {
                        return Err(ExecutionError::Execution(
                            "invalid blob storage flag".to_string(),
                        ));
                    }
                }
            }
        }
    }

    Ok(Tuple::new(values))
}

fn read_exact(data: &[u8], offset: usize, len: usize) -> ExecutionResult<&[u8]> {
    data.get(offset..offset + len)
        .ok_or_else(|| ExecutionError::Execution("tuple bytes truncated".to_string()))
}

fn read_u32(bytes: &[u8]) -> u32 {
    let mut array = [0u8; 4];
    array.copy_from_slice(bytes);
    u32::from_le_bytes(array)
}

fn read_u64(bytes: &[u8]) -> u64 {
    let mut array = [0u8; 8];
    array.copy_from_slice(bytes);
    u64::from_le_bytes(array)
}
