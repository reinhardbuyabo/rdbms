// MODULE DECLARATIONS
// These files exist internally but we decide what to expose below.
mod buffer;
mod disk;
mod page;
mod replacer;

// PUBLIC API EXPORTS
// Users of this crate (like the main DB server) can access these directly.
pub use buffer::{BufferPoolError, BufferPoolManager, BufferPoolResult, FlushMode, PageGuard};
pub use disk::{DiskManager, PAGE_SIZE, PageId};
pub use page::{PAGE_LSN_SIZE, Page};
pub use replacer::{FrameId, LRUReplacer, Replacer};
