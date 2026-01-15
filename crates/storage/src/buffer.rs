use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

use thiserror::Error;

use crate::page::Page;
use crate::replacer::{FrameId, LRUReplacer, Replacer};
use crate::{DiskManager, PageId};

/// Errors returned by the buffer pool manager.
#[derive(Debug, Error)]
pub enum BufferPoolError {
    /// The buffer pool lock was poisoned.
    #[error("buffer pool lock poisoned")]
    LockPoisoned,
    /// The underlying disk manager failed.
    #[error("disk manager error: {0}")]
    Io(#[from] std::io::Error),
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
}

/// Buffer pool manager for caching pages between disk and memory.
#[derive(Clone)]
pub struct BufferPoolManager {
    inner: Arc<Mutex<BufferPoolState>>,
}

impl BufferPoolManager {
    /// Creates a new buffer pool manager with a fixed number of frames.
    pub fn new(disk_manager: DiskManager, pool_size: usize) -> Self {
        let pages = vec![Page::new(); pool_size];
        let free_list = (0..pool_size).rev().collect();
        let state = BufferPoolState {
            disk_manager,
            replacer: LRUReplacer::new(pool_size),
            pages,
            page_table: HashMap::new(),
            free_list,
        };
        Self {
            inner: Arc::new(Mutex::new(state)),
        }
    }

    fn lock_state(&self) -> BufferPoolResult<MutexGuard<'_, BufferPoolState>> {
        self.inner.lock().map_err(|_| BufferPoolError::LockPoisoned)
    }

    fn evict_if_needed(state: &mut BufferPoolState, frame_id: FrameId) -> BufferPoolResult<()> {
        let (disk_manager, pages, page_table) = (
            &mut state.disk_manager,
            &mut state.pages,
            &mut state.page_table,
        );
        if let Some(old_page_id) = pages[frame_id].page_id {
            if pages[frame_id].is_dirty {
                let data = pages[frame_id].data();
                disk_manager.write_page(old_page_id, data)?;
            }
            page_table.remove(&old_page_id);
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
        let mut state = self.lock_state()?;
        let frame_id = match state.page_table.get(&page_id) {
            Some(&frame_id) => frame_id,
            None => return Ok(false),
        };
        let state = &mut *state;
        let (disk_manager, pages) = (&mut state.disk_manager, &mut state.pages);
        let page = &mut pages[frame_id];
        disk_manager.write_page(page_id, page.data())?;
        page.is_dirty = false;
        Ok(true)
    }

    /// Flushes all dirty pages to disk.
    pub fn flush_all_pages(&self) -> BufferPoolResult<()> {
        let mut state = self.lock_state()?;
        let state = &mut *state;
        let (disk_manager, pages) = (&mut state.disk_manager, &mut state.pages);
        for page in pages.iter_mut() {
            if let Some(page_id) = page.page_id {
                disk_manager.write_page(page_id, page.data())?;
                page.is_dirty = false;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PAGE_SIZE;
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
            guard.write_bytes(0, b"hi");
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
        assert_eq!(guard.read_bytes(0, 2).unwrap(), b"hi");
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
