use crate::execution::operator::{ExecutionError, ExecutionResult};
use crate::execution::seq_scan::Rid;
use crate::execution::tuple::Value;
use std::cmp::Ordering;
use storage::{BufferPoolManager, Page, PageId, PAGE_SIZE};

const INVALID_PAGE_ID: PageId = 0;
const PAGE_TYPE_HEADER: u8 = 1;
const PAGE_TYPE_INTERNAL: u8 = 2;
const PAGE_TYPE_LEAF: u8 = 3;

const PAGE_TYPE_OFFSET: usize = 0;
const KEY_COUNT_OFFSET: usize = 1;
const PARENT_OFFSET: usize = 8;
const SPECIAL_OFFSET: usize = 16;

const LEAF_HEADER_SIZE: usize = 24;
const INTERNAL_HEADER_SIZE: usize = 24;
const RID_SIZE: usize = 12;
const DEFAULT_TEXT_KEY_SIZE: usize = 128;

const HEADER_ROOT_OFFSET: usize = 8;
const HEADER_KEY_TYPE_OFFSET: usize = 16;
const HEADER_KEY_SIZE_OFFSET: usize = 17;
const HEADER_UNIQUE_OFFSET: usize = 19;
const HEADER_COMPOSITE_COUNT_OFFSET: usize = 20;
const HEADER_TEXT_KEY_SIZE_OFFSET: usize = 21;
const HEADER_COMPOSITE_TYPES_OFFSET: usize = 23;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKeyType {
    Integer,
    Text,
    Composite,
}

impl IndexKeyType {
    fn to_byte(self) -> u8 {
        match self {
            IndexKeyType::Integer => 1,
            IndexKeyType::Text => 2,
            IndexKeyType::Composite => 3,
        }
    }

    fn from_byte(value: u8) -> ExecutionResult<Self> {
        match value {
            1 => Ok(IndexKeyType::Integer),
            2 => Ok(IndexKeyType::Text),
            3 => Ok(IndexKeyType::Composite),
            _ => Err(ExecutionError::Execution(format!(
                "unknown index key type {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexKey {
    Integer(i64),
    Text(String),
    Composite(Vec<IndexKey>),
}

impl IndexKey {
    pub fn from_value(value: &Value, key_type: IndexKeyType) -> ExecutionResult<Self> {
        match (value, key_type) {
            (_, IndexKeyType::Composite) => Err(ExecutionError::Execution(
                "composite index keys require multiple values".to_string(),
            )),
            (Value::Integer(number), IndexKeyType::Integer)
            | (Value::Timestamp(number), IndexKeyType::Integer) => Ok(IndexKey::Integer(*number)),
            (Value::String(text), IndexKeyType::Text) => Ok(IndexKey::Text(text.clone())),
            (Value::Null, _) => Err(ExecutionError::Execution(
                "cannot use NULL value as index key".to_string(),
            )),
            (other, _) => Err(ExecutionError::Execution(format!(
                "value {:?} cannot be used as index key",
                other
            ))),
        }
    }

    pub fn from_values(values: &[Value], key_types: &[IndexKeyType]) -> ExecutionResult<Self> {
        if key_types.is_empty() {
            return Err(ExecutionError::Execution(
                "index key types cannot be empty".to_string(),
            ));
        }
        if key_types.len() == 1 {
            let value = values
                .first()
                .ok_or_else(|| ExecutionError::Execution("missing index key value".to_string()))?;
            return Self::from_value(value, key_types[0]);
        }
        if values.len() != key_types.len() {
            return Err(ExecutionError::Execution(
                "composite key value count mismatch".to_string(),
            ));
        }
        let mut keys = Vec::with_capacity(key_types.len());
        for (value, key_type) in values.iter().zip(key_types.iter()) {
            if *key_type == IndexKeyType::Composite {
                return Err(ExecutionError::Execution(
                    "composite key type cannot be nested".to_string(),
                ));
            }
            keys.push(Self::from_value(value, *key_type)?);
        }
        Ok(IndexKey::Composite(keys))
    }

    fn encode(&self, key_types: &[IndexKeyType], text_key_size: usize) -> ExecutionResult<Vec<u8>> {
        if key_types.is_empty() {
            return Err(ExecutionError::Execution(
                "index key types cannot be empty".to_string(),
            ));
        }
        if key_types.len() == 1 {
            let encoded = encode_component(self, key_types[0], text_key_size)?;
            let expected = total_key_size(key_types, text_key_size)?;
            if encoded.len() != expected {
                return Err(ExecutionError::Execution(
                    "index key size mismatch".to_string(),
                ));
            }
            return Ok(encoded);
        }
        let components = match self {
            IndexKey::Composite(keys) => keys,
            _ => {
                return Err(ExecutionError::Execution(
                    "composite index key mismatch".to_string(),
                ));
            }
        };
        if components.len() != key_types.len() {
            return Err(ExecutionError::Execution(
                "composite index key length mismatch".to_string(),
            ));
        }
        let mut buffer = Vec::new();
        for (component, key_type) in components.iter().zip(key_types.iter()) {
            let encoded = encode_component(component, *key_type, text_key_size)?;
            buffer.extend_from_slice(&encoded);
        }
        let expected = total_key_size(key_types, text_key_size)?;
        if buffer.len() != expected {
            return Err(ExecutionError::Execution(
                "index key size mismatch".to_string(),
            ));
        }
        Ok(buffer)
    }

    fn decode(
        bytes: &[u8],
        key_types: &[IndexKeyType],
        text_key_size: usize,
    ) -> ExecutionResult<Self> {
        if key_types.is_empty() {
            return Err(ExecutionError::Execution(
                "index key types cannot be empty".to_string(),
            ));
        }
        if key_types.len() == 1 {
            return decode_component(bytes, key_types[0], text_key_size);
        }
        let mut offset = 0;
        let mut components = Vec::with_capacity(key_types.len());
        for key_type in key_types.iter().copied() {
            let size = component_size(key_type, text_key_size)?;
            let end = offset + size;
            if end > bytes.len() {
                return Err(ExecutionError::Execution(
                    "composite index key bytes truncated".to_string(),
                ));
            }
            components.push(decode_component(
                &bytes[offset..end],
                key_type,
                text_key_size,
            )?);
            offset = end;
        }
        if offset != bytes.len() {
            return Err(ExecutionError::Execution(
                "composite index key bytes size mismatch".to_string(),
            ));
        }
        Ok(IndexKey::Composite(components))
    }

    pub fn display(&self) -> String {
        match self {
            IndexKey::Integer(number) => number.to_string(),
            IndexKey::Text(text) => text.clone(),
            IndexKey::Composite(keys) => {
                let inner = keys
                    .iter()
                    .map(|key| key.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
        }
    }
}

fn encode_component(
    key: &IndexKey,
    key_type: IndexKeyType,
    text_key_size: usize,
) -> ExecutionResult<Vec<u8>> {
    match (key, key_type) {
        (_, IndexKeyType::Composite) => Err(ExecutionError::Execution(
            "composite key component type is invalid".to_string(),
        )),
        (IndexKey::Integer(number), IndexKeyType::Integer) => Ok(number.to_le_bytes().to_vec()),
        (IndexKey::Text(text), IndexKeyType::Text) => {
            if text_key_size < 2 {
                return Err(ExecutionError::Execution(
                    "text index key size must be at least 2".to_string(),
                ));
            }
            let bytes = text.as_bytes();
            let max_len = text_key_size - 2;
            if bytes.len() > max_len {
                return Err(ExecutionError::Execution(format!(
                    "text index key length {} exceeds max {}",
                    bytes.len(),
                    max_len
                )));
            }
            let mut buffer = vec![0u8; text_key_size];
            let len = u16::try_from(bytes.len())
                .map_err(|_| ExecutionError::Execution("text index key too long".to_string()))?;
            buffer[0..2].copy_from_slice(&len.to_le_bytes());
            buffer[2..2 + bytes.len()].copy_from_slice(bytes);
            Ok(buffer)
        }
        _ => Err(ExecutionError::Execution(
            "index key type mismatch".to_string(),
        )),
    }
}

fn decode_component(
    bytes: &[u8],
    key_type: IndexKeyType,
    text_key_size: usize,
) -> ExecutionResult<IndexKey> {
    match key_type {
        IndexKeyType::Composite => Err(ExecutionError::Execution(
            "composite key component type is invalid".to_string(),
        )),
        IndexKeyType::Integer => {
            if bytes.len() != 8 {
                return Err(ExecutionError::Execution(
                    "invalid integer key bytes".to_string(),
                ));
            }
            let mut array = [0u8; 8];
            array.copy_from_slice(bytes);
            Ok(IndexKey::Integer(i64::from_le_bytes(array)))
        }
        IndexKeyType::Text => {
            if bytes.len() != text_key_size || text_key_size < 2 {
                return Err(ExecutionError::Execution(
                    "invalid text key bytes".to_string(),
                ));
            }
            let len = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
            let end = 2 + len;
            if end > bytes.len() {
                return Err(ExecutionError::Execution(
                    "text key length exceeds payload".to_string(),
                ));
            }
            let text = String::from_utf8(bytes[2..end].to_vec())
                .map_err(|_| ExecutionError::Execution("invalid utf8 key".to_string()))?;
            Ok(IndexKey::Text(text))
        }
    }
}

fn component_size(key_type: IndexKeyType, text_key_size: usize) -> ExecutionResult<usize> {
    match key_type {
        IndexKeyType::Integer => Ok(8),
        IndexKeyType::Text => {
            if text_key_size < 2 {
                return Err(ExecutionError::Execution(
                    "text index key size must be at least 2".to_string(),
                ));
            }
            Ok(text_key_size)
        }
        IndexKeyType::Composite => Err(ExecutionError::Execution(
            "composite key component type is invalid".to_string(),
        )),
    }
}

fn resolve_text_key_size(
    key_type: IndexKeyType,
    key_size: Option<usize>,
) -> ExecutionResult<usize> {
    match key_type {
        IndexKeyType::Composite => Err(ExecutionError::Execution(
            "composite key type requires component metadata".to_string(),
        )),
        IndexKeyType::Text => {
            let resolved = key_size.unwrap_or(DEFAULT_TEXT_KEY_SIZE);
            if resolved < 2 {
                return Err(ExecutionError::Execution(
                    "text index key size must be at least 2".to_string(),
                ));
            }
            Ok(resolved)
        }
        IndexKeyType::Integer => Ok(DEFAULT_TEXT_KEY_SIZE),
    }
}

fn total_key_size(key_types: &[IndexKeyType], text_key_size: usize) -> ExecutionResult<usize> {
    let mut total = 0;
    for key_type in key_types.iter().copied() {
        total += component_size(key_type, text_key_size)?;
    }
    Ok(total)
}

impl Ord for IndexKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (IndexKey::Integer(left), IndexKey::Integer(right)) => left.cmp(right),
            (IndexKey::Text(left), IndexKey::Text(right)) => left.cmp(right),
            (IndexKey::Composite(left), IndexKey::Composite(right)) => {
                for (left_key, right_key) in left.iter().zip(right.iter()) {
                    let cmp = left_key.cmp(right_key);
                    if cmp != Ordering::Equal {
                        return cmp;
                    }
                }
                left.len().cmp(&right.len())
            }
            (IndexKey::Composite(_), _) => Ordering::Greater,
            (_, IndexKey::Composite(_)) => Ordering::Less,
            (IndexKey::Integer(_), IndexKey::Text(_)) => Ordering::Less,
            (IndexKey::Text(_), IndexKey::Integer(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for IndexKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub key: IndexKey,
    pub rid: Rid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRange {
    pub lower: Option<(IndexKey, bool)>,
    pub upper: Option<(IndexKey, bool)>,
}

impl IndexRange {
    pub fn full() -> Self {
        Self {
            lower: None,
            upper: None,
        }
    }

    pub fn equality(key: IndexKey) -> Self {
        Self {
            lower: Some((key.clone(), true)),
            upper: Some((key, true)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageType {
    Header,
    Internal,
    Leaf,
}

impl PageType {
    fn as_byte(self) -> u8 {
        match self {
            PageType::Header => PAGE_TYPE_HEADER,
            PageType::Internal => PAGE_TYPE_INTERNAL,
            PageType::Leaf => PAGE_TYPE_LEAF,
        }
    }

    fn from_byte(value: u8) -> ExecutionResult<Self> {
        match value {
            PAGE_TYPE_HEADER => Ok(PageType::Header),
            PAGE_TYPE_INTERNAL => Ok(PageType::Internal),
            PAGE_TYPE_LEAF => Ok(PageType::Leaf),
            _ => Err(ExecutionError::Execution(format!(
                "unknown b+tree page type {}",
                value
            ))),
        }
    }
}

#[derive(Clone)]
pub struct BPlusTree {
    buffer_pool: BufferPoolManager,
    header_page_id: PageId,
    key_types: Vec<IndexKeyType>,
    key_size: usize,
    text_key_size: usize,
    unique: bool,
}

impl BPlusTree {
    pub fn create(
        buffer_pool: BufferPoolManager,
        key_type: IndexKeyType,
        key_size: Option<usize>,
        unique: bool,
    ) -> ExecutionResult<Self> {
        let text_key_size = resolve_text_key_size(key_type, key_size)?;
        Self::create_with_types(buffer_pool, vec![key_type], text_key_size, unique)
    }

    pub fn create_composite(
        buffer_pool: BufferPoolManager,
        key_types: Vec<IndexKeyType>,
        text_key_size: Option<usize>,
        unique: bool,
    ) -> ExecutionResult<Self> {
        if key_types.len() < 2 {
            return Err(ExecutionError::Execution(
                "composite index must include at least two columns".to_string(),
            ));
        }
        let resolved_text_key_size = text_key_size.unwrap_or(DEFAULT_TEXT_KEY_SIZE);
        if resolved_text_key_size < 2 {
            return Err(ExecutionError::Execution(
                "text index key size must be at least 2".to_string(),
            ));
        }
        Self::create_with_types(buffer_pool, key_types, resolved_text_key_size, unique)
    }

    fn create_with_types(
        buffer_pool: BufferPoolManager,
        key_types: Vec<IndexKeyType>,
        text_key_size: usize,
        unique: bool,
    ) -> ExecutionResult<Self> {
        if key_types.is_empty() {
            return Err(ExecutionError::Execution(
                "index key types cannot be empty".to_string(),
            ));
        }
        if key_types.contains(&IndexKeyType::Composite) {
            return Err(ExecutionError::Execution(
                "composite key type cannot be nested".to_string(),
            ));
        }
        let key_size = total_key_size(&key_types, text_key_size)?;
        let header_key_type = if key_types.len() > 1 {
            IndexKeyType::Composite
        } else {
            key_types[0]
        };
        let header_page_id = allocate_page(&buffer_pool)?;
        let root_page_id = allocate_page(&buffer_pool)?;

        {
            let mut header_guard = fetch_page(&buffer_pool, header_page_id)?;
            init_header_page(
                &mut header_guard,
                root_page_id,
                header_key_type,
                key_size,
                unique,
                &key_types,
                text_key_size,
            )?;
        }
        buffer_pool.unpin_page(header_page_id, true)?;

        {
            let mut root_guard = fetch_page(&buffer_pool, root_page_id)?;
            init_leaf_page(&mut root_guard, None, None)?;
        }
        buffer_pool.unpin_page(root_page_id, true)?;

        Ok(Self {
            buffer_pool,
            header_page_id,
            key_types,
            key_size,
            text_key_size,
            unique,
        })
    }

    pub fn open(buffer_pool: BufferPoolManager, header_page_id: PageId) -> ExecutionResult<Self> {
        let (key_types, key_size, text_key_size, unique) = {
            let header_guard = fetch_page(&buffer_pool, header_page_id)?;
            let key_type =
                IndexKeyType::from_byte(read_u8(&header_guard, HEADER_KEY_TYPE_OFFSET)?)?;
            let key_size = read_u16(&header_guard, HEADER_KEY_SIZE_OFFSET)? as usize;
            let unique = read_u8(&header_guard, HEADER_UNIQUE_OFFSET)? != 0;
            let composite_count = read_u8(&header_guard, HEADER_COMPOSITE_COUNT_OFFSET)? as usize;
            let text_key_size = read_u16(&header_guard, HEADER_TEXT_KEY_SIZE_OFFSET)? as usize;
            let text_key_size = if text_key_size == 0 {
                DEFAULT_TEXT_KEY_SIZE
            } else {
                text_key_size
            };
            let key_types = if composite_count > 0 {
                let mut types = Vec::with_capacity(composite_count);
                for index in 0..composite_count {
                    let offset = HEADER_COMPOSITE_TYPES_OFFSET + index;
                    let value = read_u8(&header_guard, offset)?;
                    let key_type = IndexKeyType::from_byte(value)?;
                    if key_type == IndexKeyType::Composite {
                        return Err(ExecutionError::Execution(
                            "composite key component type is invalid".to_string(),
                        ));
                    }
                    types.push(key_type);
                }
                types
            } else if key_type == IndexKeyType::Composite {
                return Err(ExecutionError::Execution(
                    "missing composite key type metadata".to_string(),
                ));
            } else {
                vec![key_type]
            };
            if key_type == IndexKeyType::Composite && key_types.len() < 2 {
                return Err(ExecutionError::Execution(
                    "composite index must have at least two columns".to_string(),
                ));
            }
            let expected_size = total_key_size(&key_types, text_key_size)?;
            if expected_size != key_size {
                return Err(ExecutionError::Execution(
                    "index key size metadata mismatch".to_string(),
                ));
            }
            (key_types, key_size, text_key_size, unique)
        };
        buffer_pool.unpin_page(header_page_id, false)?;
        Ok(Self {
            buffer_pool,
            header_page_id,
            key_types,
            key_size,
            text_key_size,
            unique,
        })
    }

    pub fn header_page_id(&self) -> PageId {
        self.header_page_id
    }

    pub fn key_type(&self) -> IndexKeyType {
        if self.key_types.len() > 1 {
            IndexKeyType::Composite
        } else {
            self.key_types[0]
        }
    }

    pub fn key_types(&self) -> &[IndexKeyType] {
        &self.key_types
    }

    pub fn key_size(&self) -> usize {
        self.key_size
    }

    pub fn text_key_size(&self) -> usize {
        self.text_key_size
    }

    pub fn unique(&self) -> bool {
        self.unique
    }

    pub fn root_is_leaf(&self) -> ExecutionResult<bool> {
        let root_page_id = self.root_page_id()?;
        let page_type = {
            let root_guard = fetch_page(&self.buffer_pool, root_page_id)?;
            read_page_type(&root_guard)?
        };
        self.buffer_pool.unpin_page(root_page_id, false)?;
        Ok(page_type == PageType::Leaf)
    }

    pub fn height(&self) -> ExecutionResult<usize> {
        let mut height = 0;
        let mut page_id = self.root_page_id()?;
        loop {
            let (page_type, left_child) = {
                let page_guard = fetch_page(&self.buffer_pool, page_id)?;
                let page_type = read_page_type(&page_guard)?;
                let left_child = match page_type {
                    PageType::Internal => {
                        let internal = read_internal_page(
                            &page_guard,
                            &self.key_types,
                            self.text_key_size,
                            self.key_size,
                        )?;
                        Some(internal.children[0])
                    }
                    _ => None,
                };
                (page_type, left_child)
            };
            self.buffer_pool.unpin_page(page_id, false)?;
            height += 1;
            if page_type == PageType::Leaf {
                break;
            }
            page_id = left_child.ok_or_else(|| {
                ExecutionError::Execution("missing internal child pointer".to_string())
            })?;
        }
        Ok(height)
    }

    pub fn max_leaf_entries(&self) -> usize {
        let entry_size = self.key_size + RID_SIZE;
        (PAGE_SIZE - LEAF_HEADER_SIZE) / entry_size
    }

    pub fn max_internal_entries(&self) -> usize {
        let entry_size = self.key_size + 8;
        (PAGE_SIZE - INTERNAL_HEADER_SIZE) / entry_size
    }

    fn root_page_id(&self) -> ExecutionResult<PageId> {
        let root = {
            let header_guard = fetch_page(&self.buffer_pool, self.header_page_id)?;
            read_u64(&header_guard, HEADER_ROOT_OFFSET)?
        };
        self.buffer_pool.unpin_page(self.header_page_id, false)?;
        Ok(root)
    }

    fn set_root_page_id(&self, root_page_id: PageId) -> ExecutionResult<()> {
        {
            let mut header_guard = fetch_page(&self.buffer_pool, self.header_page_id)?;
            write_u64(&mut header_guard, HEADER_ROOT_OFFSET, root_page_id)?;
        }
        self.buffer_pool.unpin_page(self.header_page_id, true)?;
        Ok(())
    }

    fn find_leaf_page(
        &self,
        key: Option<&IndexKey>,
        use_upper_bound: bool,
    ) -> ExecutionResult<PageId> {
        let mut page_id = self.root_page_id()?;
        loop {
            let (page_type, next_id) = {
                let page_guard = fetch_page(&self.buffer_pool, page_id)?;
                let page_type = read_page_type(&page_guard)?;
                let next_id = match page_type {
                    PageType::Leaf => None,
                    PageType::Internal => {
                        let internal = read_internal_page(
                            &page_guard,
                            &self.key_types,
                            self.text_key_size,
                            self.key_size,
                        )?;
                        let child_index = if let Some(key) = key {
                            pick_child_index(&internal.keys, key, use_upper_bound)
                        } else {
                            0
                        };
                        Some(internal.children[child_index])
                    }
                    PageType::Header => {
                        return Err(ExecutionError::Execution(
                            "unexpected header page while searching".to_string(),
                        ));
                    }
                };
                (page_type, next_id)
            };
            self.buffer_pool.unpin_page(page_id, false)?;
            if page_type == PageType::Leaf {
                return Ok(page_id);
            }
            page_id = next_id
                .ok_or_else(|| ExecutionError::Execution("missing child pointer".to_string()))?;
        }
    }

    fn read_leaf_entries(&self, page_id: PageId) -> ExecutionResult<(LeafPage, Vec<IndexEntry>)> {
        let (leaf_page, entries) = {
            let page_guard = fetch_page(&self.buffer_pool, page_id)?;
            let leaf_page = read_leaf_page(&page_guard)?;
            let entries = read_leaf_entries(
                &page_guard,
                &self.key_types,
                self.text_key_size,
                self.key_size,
            )?;
            (leaf_page, entries)
        };
        self.buffer_pool.unpin_page(page_id, false)?;
        Ok((leaf_page, entries))
    }

    fn write_leaf_entries(
        &self,
        page_id: PageId,
        leaf_page: &LeafPage,
        entries: &[IndexEntry],
    ) -> ExecutionResult<()> {
        {
            let mut page_guard = fetch_page(&self.buffer_pool, page_id)?;
            write_leaf_page(
                &mut page_guard,
                leaf_page,
                entries,
                &self.key_types,
                self.text_key_size,
                self.key_size,
            )?;
        }
        self.buffer_pool.unpin_page(page_id, true)?;
        Ok(())
    }

    fn read_internal_page(&self, page_id: PageId) -> ExecutionResult<InternalPage> {
        let internal = {
            let page_guard = fetch_page(&self.buffer_pool, page_id)?;
            read_internal_page(
                &page_guard,
                &self.key_types,
                self.text_key_size,
                self.key_size,
            )?
        };
        self.buffer_pool.unpin_page(page_id, false)?;
        Ok(internal)
    }

    fn write_internal_page(&self, page_id: PageId, internal: &InternalPage) -> ExecutionResult<()> {
        {
            let mut page_guard = fetch_page(&self.buffer_pool, page_id)?;
            write_internal_page(
                &mut page_guard,
                internal,
                &self.key_types,
                self.text_key_size,
                self.key_size,
            )?;
        }
        self.buffer_pool.unpin_page(page_id, true)?;
        Ok(())
    }

    fn set_parent(&self, page_id: PageId, parent: Option<PageId>) -> ExecutionResult<()> {
        {
            let mut page_guard = fetch_page(&self.buffer_pool, page_id)?;
            write_parent_page_id(&mut page_guard, parent)?;
        }
        self.buffer_pool.unpin_page(page_id, true)?;
        Ok(())
    }

    fn insert_into_leaf(&self, page_id: PageId, key: IndexKey, rid: Rid) -> ExecutionResult<()> {
        let (mut leaf_page, mut entries) = self.read_leaf_entries(page_id)?;
        let insert_position = entries
            .iter()
            .position(|entry| entry.key > key)
            .unwrap_or(entries.len());
        entries.insert(
            insert_position,
            IndexEntry {
                key: key.clone(),
                rid,
            },
        );

        if entries.len() <= self.max_leaf_entries() {
            self.write_leaf_entries(page_id, &leaf_page, &entries)?;
            return Ok(());
        }

        let split_index = entries.len() / 2;
        let right_entries = entries.split_off(split_index);
        let separator_key = right_entries
            .first()
            .ok_or_else(|| ExecutionError::Execution("empty split".to_string()))?
            .key
            .clone();
        let new_page_id = allocate_page(&self.buffer_pool)?;
        let new_leaf = LeafPage {
            parent: leaf_page.parent,
            next: leaf_page.next,
        };
        leaf_page.next = Some(new_page_id);
        self.write_leaf_entries(page_id, &leaf_page, &entries)?;
        self.write_leaf_entries(new_page_id, &new_leaf, &right_entries)?;
        self.insert_into_parent(page_id, separator_key, new_page_id)
    }

    fn insert_into_parent(
        &self,
        left_page_id: PageId,
        separator_key: IndexKey,
        right_page_id: PageId,
    ) -> ExecutionResult<()> {
        let parent_id = {
            let left_guard = fetch_page(&self.buffer_pool, left_page_id)?;
            read_parent_page_id(&left_guard)?
        };
        self.buffer_pool.unpin_page(left_page_id, false)?;

        match parent_id {
            None => {
                let new_root_id = allocate_page(&self.buffer_pool)?;
                let root = InternalPage {
                    parent: None,
                    keys: vec![separator_key],
                    children: vec![left_page_id, right_page_id],
                };
                self.write_internal_page(new_root_id, &root)?;
                self.set_root_page_id(new_root_id)?;
                self.set_parent(left_page_id, Some(new_root_id))?;
                self.set_parent(right_page_id, Some(new_root_id))?;
                Ok(())
            }
            Some(parent_page_id) => {
                let mut parent = self.read_internal_page(parent_page_id)?;
                let child_index = parent
                    .children
                    .iter()
                    .position(|&child| child == left_page_id)
                    .ok_or_else(|| {
                        ExecutionError::Execution("missing parent child pointer".to_string())
                    })?;
                parent.keys.insert(child_index, separator_key);
                parent.children.insert(child_index + 1, right_page_id);

                if parent.keys.len() <= self.max_internal_entries() {
                    self.write_internal_page(parent_page_id, &parent)?;
                    self.set_parent(right_page_id, Some(parent_page_id))?;
                    return Ok(());
                }

                self.split_internal(parent_page_id, parent)
            }
        }
    }

    fn split_internal(&self, page_id: PageId, mut page: InternalPage) -> ExecutionResult<()> {
        let split_index = page.keys.len() / 2;
        let separator_key = page.keys[split_index].clone();
        let right_keys = page.keys.split_off(split_index + 1);
        let right_children = page.children.split_off(split_index + 1);
        page.keys.truncate(split_index);

        let right_page_id = allocate_page(&self.buffer_pool)?;
        let right_page = InternalPage {
            parent: page.parent,
            keys: right_keys,
            children: right_children,
        };
        self.write_internal_page(page_id, &page)?;
        self.write_internal_page(right_page_id, &right_page)?;

        for child_id in &right_page.children {
            self.set_parent(*child_id, Some(right_page_id))?;
        }

        self.insert_into_parent(page_id, separator_key, right_page_id)
    }

    fn scan_entries(&self, range: IndexRange) -> ExecutionResult<Vec<IndexEntry>> {
        let mut results = Vec::new();
        let mut page_id = if let Some((ref lower_key, _)) = range.lower {
            self.find_leaf_page(Some(lower_key), false)?
        } else {
            self.find_leaf_page(None, false)?
        };

        loop {
            let (leaf_page, entries) = self.read_leaf_entries(page_id)?;
            for entry in entries {
                if !matches_lower_bound(&entry.key, &range) {
                    continue;
                }
                if matches_upper_stop(&entry.key, &range) {
                    return Ok(results);
                }
                results.push(entry);
            }
            match leaf_page.next {
                Some(next_page) => page_id = next_page,
                None => return Ok(results),
            }
        }
    }
}

impl crate::index::Index for BPlusTree {
    fn insert(&self, key: IndexKey, rid: Rid) -> ExecutionResult<()> {
        if self.unique {
            let existing = self.get(&key)?;
            if !existing.is_empty() {
                return Err(ExecutionError::Execution("duplicate index key".to_string()));
            }
        }
        let leaf_page_id = self.find_leaf_page(Some(&key), true)?;
        self.insert_into_leaf(leaf_page_id, key, rid)
    }

    fn delete(&self, key: &IndexKey, rid: Rid) -> ExecutionResult<bool> {
        let mut page_id = self.find_leaf_page(Some(key), false)?;
        loop {
            let (leaf_page, mut entries) = self.read_leaf_entries(page_id)?;
            if let Some(position) = entries
                .iter()
                .position(|entry| entry.key == *key && entry.rid == rid)
            {
                entries.remove(position);
                self.write_leaf_entries(page_id, &leaf_page, &entries)?;
                return Ok(true);
            }
            let should_advance = match entries.last() {
                Some(entry) => entry.key <= *key,
                None => true,
            };
            match (should_advance, leaf_page.next) {
                (true, Some(next_page)) => page_id = next_page,
                _ => return Ok(false),
            }
        }
    }

    fn get(&self, key: &IndexKey) -> ExecutionResult<Vec<Rid>> {
        let range = IndexRange::equality(key.clone());
        let entries = self.scan_entries(range)?;
        Ok(entries.into_iter().map(|entry| entry.rid).collect())
    }

    fn range_scan(&self, range: IndexRange) -> ExecutionResult<Vec<Rid>> {
        let entries = self.scan_entries(range)?;
        Ok(entries.into_iter().map(|entry| entry.rid).collect())
    }

    fn iter_all(&self) -> ExecutionResult<Vec<IndexEntry>> {
        self.scan_entries(IndexRange::full())
    }
}

#[derive(Debug, Clone)]
struct LeafPage {
    parent: Option<PageId>,
    next: Option<PageId>,
}

#[derive(Debug, Clone)]
struct InternalPage {
    parent: Option<PageId>,
    keys: Vec<IndexKey>,
    children: Vec<PageId>,
}

fn allocate_page(buffer_pool: &BufferPoolManager) -> ExecutionResult<PageId> {
    let page_id = buffer_pool
        .new_page()?
        .ok_or_else(|| ExecutionError::Execution("buffer pool has no free frames".to_string()))?;
    buffer_pool.unpin_page(page_id, false)?;
    Ok(page_id)
}

fn fetch_page<'a>(
    buffer_pool: &'a BufferPoolManager,
    page_id: PageId,
) -> ExecutionResult<storage::PageGuard<'a>> {
    buffer_pool
        .fetch_page(page_id)?
        .ok_or_else(|| ExecutionError::Execution("buffer pool has no available frame".to_string()))
}

fn init_header_page(
    page: &mut Page,
    root_page_id: PageId,
    key_type: IndexKeyType,
    key_size: usize,
    unique: bool,
    key_types: &[IndexKeyType],
    text_key_size: usize,
) -> ExecutionResult<()> {
    if key_types.len() > u8::MAX as usize {
        return Err(ExecutionError::Execution(
            "too many composite key columns".to_string(),
        ));
    }
    write_u8(page, PAGE_TYPE_OFFSET, PageType::Header.as_byte())?;
    write_u64(page, HEADER_ROOT_OFFSET, root_page_id)?;
    write_u8(page, HEADER_KEY_TYPE_OFFSET, key_type.to_byte())?;
    write_u16(page, HEADER_KEY_SIZE_OFFSET, key_size as u16)?;
    write_u8(page, HEADER_UNIQUE_OFFSET, if unique { 1 } else { 0 })?;
    write_u8(page, HEADER_COMPOSITE_COUNT_OFFSET, key_types.len() as u8)?;
    write_u16(page, HEADER_TEXT_KEY_SIZE_OFFSET, text_key_size as u16)?;
    for (index, key_type) in key_types.iter().enumerate() {
        let offset = HEADER_COMPOSITE_TYPES_OFFSET + index;
        write_u8(page, offset, key_type.to_byte())?;
    }
    Ok(())
}

fn init_leaf_page(
    page: &mut Page,
    parent: Option<PageId>,
    next: Option<PageId>,
) -> ExecutionResult<()> {
    write_u8(page, PAGE_TYPE_OFFSET, PageType::Leaf.as_byte())?;
    write_u16(page, KEY_COUNT_OFFSET, 0)?;
    write_parent_page_id(page, parent)?;
    write_special_page_id(page, next)?;
    Ok(())
}

fn read_page_type(page: &Page) -> ExecutionResult<PageType> {
    let value = read_u8(page, PAGE_TYPE_OFFSET)?;
    PageType::from_byte(value)
}

fn read_key_count(page: &Page) -> ExecutionResult<u16> {
    read_u16(page, KEY_COUNT_OFFSET)
}

fn write_key_count(page: &mut Page, count: u16) -> ExecutionResult<()> {
    write_u16(page, KEY_COUNT_OFFSET, count)
}

fn read_parent_page_id(page: &Page) -> ExecutionResult<Option<PageId>> {
    let parent = read_u64(page, PARENT_OFFSET)?;
    Ok(if parent == INVALID_PAGE_ID {
        None
    } else {
        Some(parent)
    })
}

fn write_parent_page_id(page: &mut Page, parent: Option<PageId>) -> ExecutionResult<()> {
    let value = parent.unwrap_or(INVALID_PAGE_ID);
    write_u64(page, PARENT_OFFSET, value)
}

fn read_special_page_id(page: &Page) -> ExecutionResult<Option<PageId>> {
    let id = read_u64(page, SPECIAL_OFFSET)?;
    Ok(if id == INVALID_PAGE_ID {
        None
    } else {
        Some(id)
    })
}

fn write_special_page_id(page: &mut Page, id: Option<PageId>) -> ExecutionResult<()> {
    let value = id.unwrap_or(INVALID_PAGE_ID);
    write_u64(page, SPECIAL_OFFSET, value)
}

fn read_leaf_page(page: &Page) -> ExecutionResult<LeafPage> {
    let parent = read_parent_page_id(page)?;
    let next = read_special_page_id(page)?;
    Ok(LeafPage { parent, next })
}

fn read_internal_page(
    page: &Page,
    key_types: &[IndexKeyType],
    text_key_size: usize,
    key_size: usize,
) -> ExecutionResult<InternalPage> {
    let key_count = read_key_count(page)? as usize;
    let parent = read_parent_page_id(page)?;
    let left_child = read_special_page_id(page)?
        .ok_or_else(|| ExecutionError::Execution("internal page missing left child".to_string()))?;
    let mut keys = Vec::with_capacity(key_count);
    let mut children = Vec::with_capacity(key_count + 1);
    children.push(left_child);
    for index in 0..key_count {
        let offset = INTERNAL_HEADER_SIZE + index * (key_size + 8);
        let key_bytes = read_bytes(page, offset, key_size)?;
        let key = IndexKey::decode(key_bytes, key_types, text_key_size)?;
        let child = read_u64(page, offset + key_size)?;
        keys.push(key);
        children.push(child);
    }
    Ok(InternalPage {
        parent,
        keys,
        children,
    })
}

fn write_leaf_page(
    page: &mut Page,
    leaf: &LeafPage,
    entries: &[IndexEntry],
    key_types: &[IndexKeyType],
    text_key_size: usize,
    key_size: usize,
) -> ExecutionResult<()> {
    write_u8(page, PAGE_TYPE_OFFSET, PageType::Leaf.as_byte())?;
    write_key_count(page, entries.len() as u16)?;
    write_parent_page_id(page, leaf.parent)?;
    write_special_page_id(page, leaf.next)?;
    for (index, entry) in entries.iter().enumerate() {
        let offset = LEAF_HEADER_SIZE + index * (key_size + RID_SIZE);
        let key_bytes = entry.key.encode(key_types, text_key_size)?;
        write_bytes(page, offset, &key_bytes)?;
        write_rid(page, offset + key_size, entry.rid)?;
    }
    Ok(())
}

fn write_internal_page(
    page: &mut Page,
    internal: &InternalPage,
    key_types: &[IndexKeyType],
    text_key_size: usize,
    key_size: usize,
) -> ExecutionResult<()> {
    if internal.children.len() != internal.keys.len() + 1 {
        return Err(ExecutionError::Execution(
            "internal page children count mismatch".to_string(),
        ));
    }
    write_u8(page, PAGE_TYPE_OFFSET, PageType::Internal.as_byte())?;
    write_key_count(page, internal.keys.len() as u16)?;
    write_parent_page_id(page, internal.parent)?;
    write_special_page_id(page, Some(internal.children[0]))?;
    for (index, key) in internal.keys.iter().enumerate() {
        let offset = INTERNAL_HEADER_SIZE + index * (key_size + 8);
        let key_bytes = key.encode(key_types, text_key_size)?;
        write_bytes(page, offset, &key_bytes)?;
        write_u64(page, offset + key_size, internal.children[index + 1])?;
    }
    Ok(())
}

fn read_leaf_entries(
    page: &Page,
    key_types: &[IndexKeyType],
    text_key_size: usize,
    key_size: usize,
) -> ExecutionResult<Vec<IndexEntry>> {
    let key_count = read_key_count(page)? as usize;
    let mut entries = Vec::with_capacity(key_count);
    for index in 0..key_count {
        let offset = LEAF_HEADER_SIZE + index * (key_size + RID_SIZE);
        let key_bytes = read_bytes(page, offset, key_size)?;
        let key = IndexKey::decode(key_bytes, key_types, text_key_size)?;
        let rid = read_rid(page, offset + key_size)?;
        entries.push(IndexEntry { key, rid });
    }
    Ok(entries)
}

fn read_rid(page: &Page, offset: usize) -> ExecutionResult<Rid> {
    let page_id = read_u64(page, offset)?;
    let slot_id = read_u32(page, offset + 8)?;
    Ok(Rid { page_id, slot_id })
}

fn write_rid(page: &mut Page, offset: usize, rid: Rid) -> ExecutionResult<()> {
    write_u64(page, offset, rid.page_id)?;
    write_u32(page, offset + 8, rid.slot_id)
}

fn read_u8(page: &Page, offset: usize) -> ExecutionResult<u8> {
    let bytes = read_bytes(page, offset, 1)?;
    Ok(bytes[0])
}

fn write_u8(page: &mut Page, offset: usize, value: u8) -> ExecutionResult<()> {
    write_bytes(page, offset, &[value])
}

fn read_u16(page: &Page, offset: usize) -> ExecutionResult<u16> {
    let bytes = read_bytes(page, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn write_u16(page: &mut Page, offset: usize, value: u16) -> ExecutionResult<()> {
    write_bytes(page, offset, &value.to_le_bytes())
}

fn read_u32(page: &Page, offset: usize) -> ExecutionResult<u32> {
    let bytes = read_bytes(page, offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn write_u32(page: &mut Page, offset: usize, value: u32) -> ExecutionResult<()> {
    write_bytes(page, offset, &value.to_le_bytes())
}

fn read_u64(page: &Page, offset: usize) -> ExecutionResult<u64> {
    let bytes = read_bytes(page, offset, 8)?;
    let mut array = [0u8; 8];
    array.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(array))
}

fn write_u64(page: &mut Page, offset: usize, value: u64) -> ExecutionResult<()> {
    write_bytes(page, offset, &value.to_le_bytes())
}

fn read_bytes(page: &Page, offset: usize, len: usize) -> ExecutionResult<&[u8]> {
    page.read_bytes(offset, len)
        .ok_or_else(|| ExecutionError::Execution("page read out of bounds".to_string()))
}

fn write_bytes(page: &mut Page, offset: usize, bytes: &[u8]) -> ExecutionResult<()> {
    if page.write_bytes(offset, bytes) {
        Ok(())
    } else {
        Err(ExecutionError::Execution(
            "page write out of bounds".to_string(),
        ))
    }
}

fn pick_child_index(keys: &[IndexKey], key: &IndexKey, use_upper: bool) -> usize {
    let mut index = 0;
    for existing in keys {
        let cmp = existing.cmp(key);
        if cmp == Ordering::Less {
            index += 1;
            continue;
        }
        if use_upper && cmp == Ordering::Equal {
            index += 1;
            continue;
        }
        break;
    }
    index
}

fn matches_lower_bound(key: &IndexKey, range: &IndexRange) -> bool {
    match &range.lower {
        Some((lower, inclusive)) => match key.cmp(lower) {
            Ordering::Less => false,
            Ordering::Equal => *inclusive,
            Ordering::Greater => true,
        },
        None => true,
    }
}

fn matches_upper_stop(key: &IndexKey, range: &IndexRange) -> bool {
    match &range.upper {
        Some((upper, inclusive)) => match key.cmp(upper) {
            Ordering::Greater => true,
            Ordering::Equal => !*inclusive,
            Ordering::Less => false,
        },
        None => false,
    }
}
