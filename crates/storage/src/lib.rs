// MODULE DECLARATIONS
// These files exist internally but we decide what to expose below.
mod buffer;
mod disk;
mod page;
mod replacer;

// PUBLIC API EXPORTS
// Users of this crate (like the main DB server) can access these directly.
pub use buffer::{BufferPoolError, BufferPoolManager, BufferPoolResult, PageGuard};
pub use disk::{DiskManager, PAGE_SIZE, PageId};
pub use page::Page;
pub use replacer::{FrameId, LRUReplacer, Replacer};
