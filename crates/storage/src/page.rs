use crate::{PAGE_SIZE, PageId};

/// In-memory page container with metadata for buffer management.
#[derive(Debug, Clone)]
pub struct Page {
    pub(crate) data: [u8; PAGE_SIZE],
    pub(crate) page_id: Option<PageId>,
    pub(crate) is_dirty: bool,
    pub(crate) pin_count: u32,
}

pub const PAGE_LSN_SIZE: usize = 8;

impl Page {
    /// Creates a zeroed page with no identity.
    pub fn new() -> Self {
        Self {
            data: [0u8; PAGE_SIZE],
            page_id: None,
            is_dirty: false,
            pin_count: 0,
        }
    }

    /// Returns the page identifier, if assigned.
    pub fn page_id(&self) -> Option<PageId> {
        self.page_id
    }

    /// Returns the page LSN stored in the header.
    pub fn lsn(&self) -> u64 {
        let mut bytes = [0u8; PAGE_LSN_SIZE];
        bytes.copy_from_slice(&self.data[..PAGE_LSN_SIZE]);
        u64::from_le_bytes(bytes)
    }

    /// Updates the page LSN in the header.
    pub fn set_lsn(&mut self, lsn: u64) {
        self.data[..PAGE_LSN_SIZE].copy_from_slice(&lsn.to_le_bytes());
    }

    /// Returns whether the page has been modified.
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Returns the current pin count.
    pub fn pin_count(&self) -> u32 {
        self.pin_count
    }

    /// Returns the entire page data.
    pub fn data(&self) -> &[u8; PAGE_SIZE] {
        &self.data
    }

    /// Returns a mutable reference to the entire page data.
    pub fn data_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut self.data
    }

    /// Reads a slice of bytes from the page.
    pub fn read_bytes(&self, offset: usize, len: usize) -> Option<&[u8]> {
        if offset.checked_add(len)? > PAGE_SIZE {
            return None;
        }
        Some(&self.data[offset..offset + len])
    }

    /// Writes bytes into the page at the given offset.
    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> bool {
        if offset + bytes.len() > PAGE_SIZE {
            return false;
        }
        self.data[offset..offset + bytes.len()].copy_from_slice(bytes);
        true
    }

    /// Resets all data and metadata to defaults.
    pub fn reset_memory(&mut self) {
        self.data.fill(0);
        self.page_id = None;
        self.is_dirty = false;
        self.pin_count = 0;
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::new()
    }
}
