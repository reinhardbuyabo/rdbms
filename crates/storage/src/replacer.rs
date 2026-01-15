use std::collections::{HashSet, VecDeque};

/// Identifies a frame in the buffer pool.
pub type FrameId = usize;

/// Eviction policy for buffer pool frames.
pub trait Replacer {
    /// Chooses a victim frame for eviction.
    fn victim(&mut self) -> Option<FrameId>;

    /// Pins a frame, removing it from eviction consideration.
    fn pin(&mut self, frame_id: FrameId);

    /// Unpins a frame, adding it to eviction consideration.
    fn unpin(&mut self, frame_id: FrameId);

    /// Returns the number of evictable frames.
    fn size(&self) -> usize;
}

/// LRU replacer that evicts the least recently unpinned frame.
#[derive(Debug)]
pub struct LRUReplacer {
    order: VecDeque<FrameId>,
    entries: HashSet<FrameId>,
}

impl LRUReplacer {
    /// Creates a new LRU replacer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            order: VecDeque::with_capacity(capacity),
            entries: HashSet::with_capacity(capacity),
        }
    }
}

impl Replacer for LRUReplacer {
    fn victim(&mut self) -> Option<FrameId> {
        let victim = self.order.pop_back()?;
        self.entries.remove(&victim);
        Some(victim)
    }

    fn pin(&mut self, frame_id: FrameId) {
        if self.entries.remove(&frame_id) {
            self.order.retain(|&entry| entry != frame_id);
        }
    }

    fn unpin(&mut self, frame_id: FrameId) {
        if self.entries.insert(frame_id) {
            self.order.push_front(frame_id);
        }
    }

    fn size(&self) -> usize {
        self.order.len()
    }
}
