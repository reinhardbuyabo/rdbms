// MODULE DECLARATIONS
// These files exist internally but we decide what to expose below.
mod disk;

// PUBLIC API EXPORTS
// Users of this crate (like the main DB server) can access these directly.
pub use disk::{DiskManager, PAGE_SIZE, PageId};

// COMING SOON:
// mod page;
// mod buffer;
// pub use page::Page;
// pub use buffer::BufferPoolManager;
