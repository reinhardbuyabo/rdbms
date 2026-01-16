use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use thiserror::Error;

use crate::page::Page;
use crate::replacer::{FrameId, LRUReplacer, Replacer};
use crate::{DiskManager, PageId};
use wal::LogManager;

/// Errors returned by the buffer pool manager.
#[derive(Debug, Error)]
pub enum BufferPoolError {
    /// The buffer pool lock was poisoned.
    #[error("buffer pool lock poisoned")]
    LockPoisoned,
    /// The underlying disk manager failed.
    #[error("disk manager error: {0}")]
    Io(#[from] std::io::Error),
    /// WAL flush failed.
    #[error("wal error: {0}")]
    Wal(#[from] wal::WalError),
}

/// Convenience alias for buffer pool results.
pub type BufferPoolResult<T> = Result<T, BufferPoolError>;

/// Guard that provides access to a pinned page while holding the pool lock.
pub struct PageGuard<'a> {
    state: MutexGuard<'a, BufferPoolState>,
    frame_id: FrameId,
}

impl<'a> PageGuard<'a> {
    /// Returns the frame id backing this guard.
    pub fn frame_id(&self) -> FrameId {
        self.frame_id
    }
}

impl Deref for PageGuard<'_> {
    type Target = Page;

    fn deref(&self) -> &Self::Target {
        &self.state.pages[self.frame_id]
    }
}

impl DerefMut for PageGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state.pages[self.frame_id]
    }
}

struct BufferPoolState {
    disk_manager: DiskManager,
    replacer: LRUReplacer,
    pages: Vec<Page>,
    page_table: HashMap<PageId, FrameId>,
    free_list: Vec<FrameId>,
    log_manager: Option<Arc<LogManager>>,
}

#[derive(Default)]
struct BufferPoolMetrics {
    fetch_count: AtomicUsize,
}

/// Buffer pool manager for caching pages between disk and memory.
#[derive(Clone)]
pub struct BufferPoolManager {
    inner: Arc<Mutex<BufferPoolState>>,
    metrics: Arc<BufferPoolMetrics>,
}

/// Flush mode for buffer pool writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushMode {
    /// Defer disk sync to later (default behavior).
    Lazy,
    /// Force the disk write to be synced.
    Force,
}

impl BufferPoolManager {
    /// Creates a new buffer pool manager with a fixed number of frames.
    pub fn new(disk_manager: DiskManager, pool_size: usize) -> Self {
        Self::new_with_log(disk_manager, pool_size, None)
    }

    pub fn new_with_log(
        disk_manager: DiskManager,
        pool_size: usize,
        log_manager: Option<Arc<LogManager>>,
    ) -> Self {
        let pages = vec![Page::new(); pool_size];
        let free_list = (0..pool_size).rev().collect();
        let state = BufferPoolState {
            disk_manager,
            replacer: LRUReplacer::new(pool_size),
            pages,
            page_table: HashMap::new(),
            free_list,
            log_manager,
        };
        Self {
            inner: Arc::new(Mutex::new(state)),
            metrics: Arc::new(BufferPoolMetrics::default()),
        }
    }

    fn lock_state(&self) -> BufferPoolResult<MutexGuard<'_, BufferPoolState>> {
        self.inner.lock().map_err(|_| BufferPoolError::LockPoisoned)
    }

    /// Returns the number of page fetches since last reset.
    pub fn fetch_count(&self) -> usize {
        self.metrics.fetch_count.load(Ordering::Relaxed)
    }

    /// Resets the fetch counter to zero.
    pub fn reset_fetch_count(&self) {
        self.metrics.fetch_count.store(0, Ordering::Relaxed);
    }

    fn evict_if_needed(state: &mut BufferPoolState, frame_id: FrameId) -> BufferPoolResult<()> {
        let (disk_manager, pages, page_table) = (
            &mut state.disk_manager,
            &mut state.pages,
            &mut state.page_table,
        );
        if let Some(old_page_id) = pages[frame_id].page_id {
            if pages[frame_id].is_dirty {
                if let Some(log_manager) = &state.log_manager {
                    log_manager.flush(pages[frame_id].lsn())?;
                }
                let data = pages[frame_id].data();
                disk_manager.write_page(old_page_id, data)?;
            }
            page_table.remove(&old_page_id);
        }
        Ok(())
    }

    fn flush_page_data(
        state: &mut BufferPoolState,
        page_id: PageId,
        data: &[u8; crate::PAGE_SIZE],
        lsn: u64,
        force_disk: bool,
    ) -> BufferPoolResult<()> {
        if let Some(log_manager) = &state.log_manager {
            log_manager.flush(lsn)?;
        }
        state.disk_manager.write_page(page_id, data)?;
        if force_disk {
            state.disk_manager.sync_data()?;
        }
        Ok(())
    }

    /// Allocates a new page on disk and pins it in the buffer pool.
    pub fn new_page(&self) -> BufferPoolResult<Option<PageId>> {
        let mut state = self.lock_state()?;
        let frame_id = if let Some(frame_id) = state.free_list.pop() {
            frame_id
        } else if let Some(frame_id) = state.replacer.victim() {
            frame_id
        } else {
            return Ok(None);
        };

        Self::evict_if_needed(&mut state, frame_id)?;

        let page_id = state.disk_manager.allocate_page()?;
        {
            let page = &mut state.pages[frame_id];
            page.reset_memory();
            page.page_id = Some(page_id);
            page.pin_count = 1;
        }
        state.page_table.insert(page_id, frame_id);
        state.replacer.pin(frame_id);
        Ok(Some(page_id))
    }

    /// Fetches a page into memory and pins it, returning a guarded reference.
    pub fn fetch_page(&self, page_id: PageId) -> BufferPoolResult<Option<PageGuard<'_>>> {
        self.metrics.fetch_count.fetch_add(1, Ordering::Relaxed);
        let mut state = self.lock_state()?;
        if let Some(&frame_id) = state.page_table.get(&page_id) {
            let page = &mut state.pages[frame_id];
            page.pin_count += 1;
            state.replacer.pin(frame_id);
            return Ok(Some(PageGuard { state, frame_id }));
        }

        let frame_id = if let Some(frame_id) = state.free_list.pop() {
            frame_id
        } else if let Some(frame_id) = state.replacer.victim() {
            frame_id
        } else {
            return Ok(None);
        };

        Self::evict_if_needed(&mut state, frame_id)?;
        {
            let state = &mut *state;
            let (disk_manager, pages) = (&mut state.disk_manager, &mut state.pages);
            let page = &mut pages[frame_id];
            page.reset_memory();
            disk_manager.read_page(page_id, page.data_mut())?;
            page.page_id = Some(page_id);
            page.pin_count = 1;
        }
        state.page_table.insert(page_id, frame_id);
        state.replacer.pin(frame_id);
        Ok(Some(PageGuard { state, frame_id }))
    }

    /// Unpins a page and optionally marks it dirty.
    pub fn unpin_page(&self, page_id: PageId, is_dirty: bool) -> BufferPoolResult<bool> {
        let mut state = self.lock_state()?;
        let frame_id = match state.page_table.get(&page_id) {
            Some(&frame_id) => frame_id,
            None => return Ok(false),
        };
        let page = &mut state.pages[frame_id];
        if page.pin_count == 0 {
            return Ok(false);
        }
        if is_dirty {
            page.is_dirty = true;
        }
        page.pin_count -= 1;
        if page.pin_count == 0 {
            state.replacer.unpin(frame_id);
        }
        Ok(true)
    }

    /// Flushes a page to disk, if present.
    pub fn flush_page(&self, page_id: PageId) -> BufferPoolResult<bool> {
        self.flush_page_with_mode(page_id, FlushMode::Lazy)
    }

    pub fn flush_page_with_mode(&self, page_id: PageId, mode: FlushMode) -> BufferPoolResult<bool> {
        let mut state = self.lock_state()?;
        let frame_id = match state.page_table.get(&page_id) {
            Some(&frame_id) => frame_id,
            None => return Ok(false),
        };
        let (data, lsn) = {
            let page = &mut state.pages[frame_id];
            let data = *page.data();
            let lsn = page.lsn();
            page.is_dirty = false;
            (data, lsn)
        };
        Self::flush_page_data(&mut state, page_id, &data, lsn, mode == FlushMode::Force)?;
        Ok(true)
    }

    /// Flushes all dirty pages to disk.
    pub fn flush_all_pages(&self) -> BufferPoolResult<()> {
        self.flush_all_pages_with_mode(FlushMode::Lazy)
    }

    pub fn flush_all_pages_with_mode(&self, mode: FlushMode) -> BufferPoolResult<()> {
        let mut state = self.lock_state()?;
        let page_ids = state
            .pages
            .iter()
            .filter_map(|page| page.page_id)
            .collect::<Vec<_>>();
        for page_id in page_ids {
            let frame_id = match state.page_table.get(&page_id) {
                Some(&frame_id) => frame_id,
                None => continue,
            };
            let (data, lsn, is_dirty) = {
                let page = &mut state.pages[frame_id];
                let data = *page.data();
                let lsn = page.lsn();
                let is_dirty = page.is_dirty;
                page.is_dirty = false;
                (data, lsn, is_dirty)
            };
            if !is_dirty {
                continue;
            }
            Self::flush_page_data(&mut state, page_id, &data, lsn, mode == FlushMode::Force)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PAGE_LSN_SIZE, PAGE_SIZE};
    use std::fs;
    use std::path::PathBuf;

    struct TestContext {
        path: PathBuf,
    }

    impl TestContext {
        fn new(test_name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("chronos_bpm_{}.db", test_name));
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
            Self { path }
        }
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn setup_bpm(test_name: &str, pool_size: usize) -> (TestContext, BufferPoolManager) {
        let ctx = TestContext::new(test_name);
        let disk_manager = DiskManager::open(ctx.path.to_str().unwrap()).unwrap();
        let bpm = BufferPoolManager::new(disk_manager, pool_size);
        (ctx, bpm)
    }

    #[test]
    fn test_lru_replacer() {
        let mut replacer = LRUReplacer::new(3);
        replacer.unpin(1);
        replacer.unpin(2);
        replacer.unpin(3);

        assert_eq!(replacer.size(), 3);
        assert_eq!(replacer.victim(), Some(1));

        replacer.pin(2);
        replacer.unpin(4);

        assert_eq!(replacer.victim(), Some(3));
        assert_eq!(replacer.victim(), Some(4));
        assert_eq!(replacer.victim(), None);
    }

    #[test]
    fn test_new_page() {
        let (_ctx, bpm) = setup_bpm("new_page", 2);
        let page_id = bpm.new_page().unwrap().expect("expected new page");

        let state = bpm.inner.lock().unwrap();
        let frame_id = *state.page_table.get(&page_id).expect("missing mapping");
        let page = &state.pages[frame_id];

        assert_eq!(page.page_id, Some(page_id));
        assert_eq!(page.pin_count, 1);
        assert!(!page.is_dirty);
    }

    #[test]
    fn test_fetch_page() {
        let (_ctx, bpm) = setup_bpm("fetch_page", 1);
        let page_id = bpm.new_page().unwrap().unwrap();
        assert!(bpm.unpin_page(page_id, false).unwrap());

        let frame_id_before = {
            let state = bpm.inner.lock().unwrap();
            *state.page_table.get(&page_id).unwrap()
        };

        {
            let mut guard = bpm.fetch_page(page_id).unwrap().unwrap();
            guard.write_bytes(PAGE_LSN_SIZE, b"hi");
        }
        assert!(bpm.unpin_page(page_id, true).unwrap());

        let frame_id_after = {
            let state = bpm.inner.lock().unwrap();
            *state.page_table.get(&page_id).unwrap()
        };
        assert_eq!(frame_id_before, frame_id_after);

        let second_id = bpm.new_page().unwrap().unwrap();
        assert!(bpm.unpin_page(second_id, false).unwrap());

        let guard = bpm.fetch_page(page_id).unwrap().unwrap();
        assert_eq!(guard.read_bytes(PAGE_LSN_SIZE, 2).unwrap(), b"hi");
        drop(guard);
        assert!(bpm.unpin_page(page_id, false).unwrap());
    }

    #[test]
    fn test_binary_data() {
        let (_ctx, bpm) = setup_bpm("binary_data", 2);
        let page_id = bpm.new_page().unwrap().unwrap();
        assert!(bpm.unpin_page(page_id, false).unwrap());

        let mut payload = [0u8; PAGE_SIZE];
        payload[0] = 0xAB;
        payload[PAGE_SIZE - 1] = 0xCD;

        {
            let mut guard = bpm.fetch_page(page_id).unwrap().unwrap();
            guard.data_mut().copy_from_slice(&payload);
        }
        assert!(bpm.unpin_page(page_id, true).unwrap());
        bpm.flush_page(page_id).unwrap();

        let guard = bpm.fetch_page(page_id).unwrap().unwrap();
        assert_eq!(guard.data(), &payload);
        drop(guard);
        assert!(bpm.unpin_page(page_id, false).unwrap());
    }

    #[test]
    fn test_buffer_exhaustion() {
        let (_ctx, bpm) = setup_bpm("buffer_exhaustion", 5);
        let mut page_ids = Vec::new();

        for idx in 0u8..10u8 {
            let page_id = bpm.new_page().unwrap().unwrap();
            assert!(bpm.unpin_page(page_id, false).unwrap());

            {
                let mut guard = bpm.fetch_page(page_id).unwrap().unwrap();
                guard.data_mut().fill(idx);
            }
            assert!(bpm.unpin_page(page_id, true).unwrap());

            page_ids.push(page_id);
        }

        bpm.flush_all_pages().unwrap();

        for (idx, page_id) in page_ids.iter().enumerate() {
            let guard = bpm.fetch_page(*page_id).unwrap().unwrap();
            assert_eq!(guard.data()[0], idx as u8);
            drop(guard);
            assert!(bpm.unpin_page(*page_id, false).unwrap());
        }
    }
}
