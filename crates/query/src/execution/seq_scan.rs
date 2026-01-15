use crate::execution::operator::{ExecutionError, ExecutionResult, PhysicalOperator};
use crate::execution::tuple::{Tuple, Value};
use crate::schema::{DataType, Schema};
use std::sync::{Arc, Mutex, MutexGuard};
use storage::{BufferPoolManager, PAGE_SIZE, Page, PageId};

const HEADER_SIZE: usize = 16;
const SLOT_SIZE: usize = 8;
const INVALID_PAGE_ID: PageId = 0;

#[derive(Clone)]
pub struct TableHeap {
    buffer_pool: BufferPoolManager,
    first_page_id: Arc<Mutex<Option<PageId>>>,
}

impl TableHeap {
    pub fn new(buffer_pool: BufferPoolManager, first_page_id: Option<PageId>) -> Self {
        Self {
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

    pub fn insert_tuple(&self, tuple: &Tuple, schema: &Schema) -> ExecutionResult<()> {
        let tuple_bytes = encode_tuple(tuple, schema)?;
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
            let mut next_page_id = None;
            let mut page_dirty = false;

            {
                let mut page_guard = self.fetch_page(page_id)?;
                let mut header = read_header(&page_guard)?;
                let available_space = header.free_space_offset as usize
                    - (HEADER_SIZE + header.slot_count as usize * SLOT_SIZE);
                if available_space >= tuple_bytes.len() + SLOT_SIZE {
                    let tuple_offset =
                        (header.free_space_offset as usize - tuple_bytes.len()) as u32;
                    if !page_guard.write_bytes(tuple_offset as usize, &tuple_bytes) {
                        return Err(ExecutionError::Execution(
                            "failed to write tuple bytes".to_string(),
                        ));
                    }
                    write_slot(
                        &mut page_guard,
                        header.slot_count as usize,
                        TableSlot {
                            offset: tuple_offset,
                            len: tuple_bytes.len() as u32,
                        },
                    )?;
                    header.slot_count += 1;
                    header.free_space_offset = tuple_offset;
                    write_header(&mut page_guard, &header)?;
                    inserted = true;
                    page_dirty = true;
                } else {
                    next_page_id = header.next_page_id;
                    if next_page_id.is_none() {
                        let new_page_id = self.allocate_page()?;
                        header.next_page_id = Some(new_page_id);
                        write_header(&mut page_guard, &header)?;
                        next_page_id = Some(new_page_id);
                        page_dirty = true;
                    }
                }
            }

            if inserted {
                self.buffer_pool.unpin_page(page_id, page_dirty)?;
                return Ok(());
            }

            self.buffer_pool.unpin_page(page_id, page_dirty)?;
            current_page_id = next_page_id;
        }
    }

    fn allocate_page(&self) -> ExecutionResult<PageId> {
        let page_id = self.buffer_pool.new_page()?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no free frames".to_string())
        })?;
        {
            let mut page_guard = self.fetch_page(page_id)?;
            initialize_page(&mut page_guard)?;
        }
        self.buffer_pool.unpin_page(page_id, true)?;
        self.buffer_pool.unpin_page(page_id, false)?;
        Ok(page_id)
    }

    fn fetch_page(&self, page_id: PageId) -> ExecutionResult<storage::PageGuard<'_>> {
        self.buffer_pool.fetch_page(page_id)?.ok_or_else(|| {
            ExecutionError::Execution("buffer pool has no available frame".to_string())
        })
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
                let page_guard = self.table_heap.fetch_page(page_id)?;
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
                        tuple = Some(decode_tuple(&self.schema, &tuple_bytes)?);
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
    let header = TablePageHeader::empty();
    write_header(page, &header)
}

fn read_header(page: &Page) -> ExecutionResult<TablePageHeader> {
    let header_bytes = page
        .read_bytes(0, HEADER_SIZE)
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

fn write_header(page: &mut Page, header: &TablePageHeader) -> ExecutionResult<()> {
    let mut bytes = [0u8; HEADER_SIZE];
    let next_page_id = header.next_page_id.unwrap_or(INVALID_PAGE_ID);
    bytes[0..8].copy_from_slice(&next_page_id.to_le_bytes());
    bytes[8..12].copy_from_slice(&header.slot_count.to_le_bytes());
    bytes[12..16].copy_from_slice(&header.free_space_offset.to_le_bytes());
    if page.write_bytes(0, &bytes) {
        Ok(())
    } else {
        Err(ExecutionError::Execution(
            "failed to write table page header".to_string(),
        ))
    }
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
    if page.write_bytes(offset, &bytes) {
        Ok(())
    } else {
        Err(ExecutionError::Execution(
            "failed to write slot".to_string(),
        ))
    }
}

fn read_tuple_bytes(page: &Page, slot: &TableSlot) -> ExecutionResult<Vec<u8>> {
    let offset = slot.offset as usize;
    let len = slot.len as usize;
    let data = page
        .read_bytes(offset, len)
        .ok_or_else(|| ExecutionError::Execution("failed to read tuple bytes".to_string()))?;
    Ok(data.to_vec())
}

fn encode_tuple(tuple: &Tuple, schema: &Schema) -> ExecutionResult<Vec<u8>> {
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

fn decode_tuple(schema: &Schema, data: &[u8]) -> ExecutionResult<Tuple> {
    let mut cursor = 0usize;
    let mut values = Vec::with_capacity(schema.fields.len());

    for field in &schema.fields {
        if cursor >= data.len() {
            return Err(ExecutionError::Execution(
                "tuple bytes truncated".to_string(),
            ));
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
