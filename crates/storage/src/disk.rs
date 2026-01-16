//! DiskManager: Monotonic, crash-safe page allocation/storage for simple RDBMS.
//!
//! Invariants:
//! - Page 0 is a reserved header storing next_page_id as u64 (format: bytes 0..8)
//! - All page writes/allocations persist header to disk
//! - No page id ever reused, no uninitialized garbage pages created
//! - On open, header is loaded (created if absent)

use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Result};
use std::os::unix::fs::FileExt;
use std::path::Path;

pub type PageId = u64;
pub const PAGE_SIZE: usize = 4096;
pub const HEADER_SIZE: usize = PAGE_SIZE; // header occupies page 0

// const HEADER_MAGIC: u64 = 0xD15CAD0BADC0FFEE; // optional, for future extension

struct Header {
    next_page_id: u64, // always points to next free (monotonic, persistent)
}

impl Header {
    fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[..8].copy_from_slice(&self.next_page_id.to_le_bytes());
        buf
    }
    fn from_bytes(buf: &[u8]) -> Self {
        let mut b = [0u8; 8];
        b.copy_from_slice(&buf[..8]);
        let next_page_id = u64::from_le_bytes(b);
        Self { next_page_id }
    }
}

pub struct DiskManager {
    file: File,
    header: Header, // in-memory header (synced on every allocation)
    #[allow(dead_code)]
    path: String, // for possible reopen/use // stored for debugging and future reopen/diagnostics
}

impl DiskManager {
    /// Opens or creates the file; loads or initializes a valid header
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        let mut dm = DiskManager {
            file,
            header: Header { next_page_id: 1 }, // default (if new file)
            path: path.as_ref().to_str().unwrap().to_string(),
        };
        dm.header = dm.load_or_init_header()?;
        Ok(dm)
    }

    /// Loads or initializes the header page (page 0)
    fn load_or_init_header(&mut self) -> Result<Header> {
        let meta = self.file.metadata()?;
        if meta.len() < HEADER_SIZE as u64 {
            // brand new file; initialize header (page 0)
            let header = Header { next_page_id: 1 };
            let buf = header.to_bytes();
            self.file.write_at(&buf, 0)?;
            Ok(header)
        } else {
            // load header from disk (always exactly one page)
            let mut buf = [0u8; HEADER_SIZE];
            self.file.read_at(&mut buf, 0)?;
            Ok(Header::from_bytes(&buf))
        }
    }

    /// Read a page at page_id into buf
    pub fn read_page(&self, page_id: PageId, buf: &mut [u8]) -> Result<()> {
        if buf.len() != PAGE_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, "buf wrong size"));
        }
        let offset = page_id * PAGE_SIZE as u64;
        self.file.read_at(buf, offset)?;
        Ok(())
    }

    /// Write a page at page_id from buf
    pub fn write_page(&mut self, page_id: PageId, buf: &[u8]) -> Result<()> {
        if buf.len() != PAGE_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, "buf wrong size"));
        }
        let offset = page_id * PAGE_SIZE as u64;
        self.file.write_at(buf, offset)?;
        Ok(())
    }

    /// Forces buffered data to disk.
    pub fn sync_data(&self) -> Result<()> {
        self.file.sync_data()
    }

    /// Allocates a new page: extends file, writes zero page, updates + persists header
    pub fn allocate_page(&mut self) -> Result<PageId> {
        let page_id = self.header.next_page_id;
        // Write virgin zeroed page at offset (never reused!)
        let offset = page_id * PAGE_SIZE as u64;
        let zero_buf = [0u8; PAGE_SIZE];
        self.file.write_at(&zero_buf, offset)?;
        // Update header for next_id, persist header *after* data
        self.header.next_page_id += 1;
        let header_bytes = self.header.to_bytes();
        self.file.write_at(&header_bytes, 0)?;
        self.file.sync_data()?; // ensure crash safety
        Ok(page_id)
    }

    /// For tests: returns current next_page_id
    pub fn get_next_page_id(&self) -> PageId {
        self.header.next_page_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    // --- Test Helper: RAII Pattern for Cleanup ---
    // Automatically deletes the test database file when the test function ends.
    struct TestContext {
        path: PathBuf,
    }

    impl TestContext {
        fn new(test_name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("chronos_test_{}.db", test_name));
            // Ensure we start fresh
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
            Self { path }
        }
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            // Clean up file on test exit (pass or fail)
            let _ = fs::remove_file(&self.path);
        }
    }

    // --- 1. Core Persistence & Recovery ---
    #[test]
    fn test_crash_recovery_data_integrity() {
        let ctx = TestContext::new("crash_recovery");
        let path = ctx.path.to_str().unwrap();

        // PHASE 1: Populate Data
        {
            let mut dm = DiskManager::open(path).expect("Failed to open DB");
            let page_id = dm.allocate_page().expect("Alloc failed"); // Page 1

            // Create a distinct pattern
            let mut data = [0u8; PAGE_SIZE];
            data[0..4].copy_from_slice(b"DEAD");
            data[PAGE_SIZE - 4..].copy_from_slice(b"BEEF");

            dm.write_page(page_id, &data).expect("Write failed");
        } // `dm` drops here, simulating process exit

        // PHASE 2: Restart & Verify
        {
            let dm = DiskManager::open(path).expect("Failed to reopen DB");

            // Header Check: Next page should still be 2
            assert_eq!(dm.get_next_page_id(), 2, "Header state not persisted!");

            // Data Check: Read Page 1
            let mut buffer = [0u8; PAGE_SIZE];
            dm.read_page(1, &mut buffer).expect("Read failed");

            // Verify integrity
            assert_eq!(&buffer[0..4], b"DEAD", "Start of page corrupted");
            assert_eq!(&buffer[PAGE_SIZE - 4..], b"BEEF", "End of page corrupted");
        }
    }

    // --- 2. Isolation (No Bleed) ---
    #[test]
    fn test_page_isolation_random_access() {
        let ctx = TestContext::new("isolation");
        let mut dm = DiskManager::open(ctx.path.to_str().unwrap()).unwrap();

        // Allocate 3 pages
        let p1 = dm.allocate_page().unwrap();
        let p2 = dm.allocate_page().unwrap();
        let p3 = dm.allocate_page().unwrap();

        let buf1 = [0xAA; PAGE_SIZE]; // Pattern A
        let buf2 = [0xBB; PAGE_SIZE]; // Pattern B
        let buf3 = [0xCC; PAGE_SIZE]; // Pattern C

        // Write in random order
        dm.write_page(p2, &buf2).unwrap();
        dm.write_page(p1, &buf1).unwrap();
        dm.write_page(p3, &buf3).unwrap();

        // Read back and assert no bleeding
        let mut check_buf = [0u8; PAGE_SIZE];

        dm.read_page(p2, &mut check_buf).unwrap();
        assert_eq!(
            check_buf, [0xBB; PAGE_SIZE],
            "Page 2 corrupted by other writes"
        );

        dm.read_page(p1, &mut check_buf).unwrap();
        assert_eq!(
            check_buf, [0xAA; PAGE_SIZE],
            "Page 1 corrupted by other writes"
        );
    }

    // --- 3. Robustness & API Safety ---
    #[test]
    fn test_invalid_buffer_sizes() {
        let ctx = TestContext::new("safety");
        let mut dm = DiskManager::open(ctx.path.to_str().unwrap()).unwrap();
        let p1 = dm.allocate_page().unwrap();

        // Try writing small buffer
        let small_buf = [0u8; 10];
        let res = dm.write_page(p1, &small_buf);
        assert!(res.is_err(), "Should forbid writing < 4096 bytes");

        // Try reading into large buffer
        let mut big_buf = [0u8; PAGE_SIZE * 2];
        let res = dm.read_page(p1, &mut big_buf);
        assert!(res.is_err(), "Should forbid reading into mismatched buffer");
    }

    // --- 4. Monotonic Growth & Persistence ---
    #[test]
    fn test_large_allocation_sequence() {
        let ctx = TestContext::new("monotonic_growth");
        let path = ctx.path.to_str().unwrap();

        // Run 1: Allocate 50 pages
        {
            let mut dm = DiskManager::open(path).unwrap();
            for i in 1..=50 {
                let pid = dm.allocate_page().unwrap();
                assert_eq!(pid, i as u64);
            }
        }

        // Run 2: Reopen, ensure we start at 51
        {
            let mut dm = DiskManager::open(path).unwrap();
            let pid = dm.allocate_page().unwrap();
            assert_eq!(pid, 51);

            // Verify file size: Header (4k) + 51 pages (4k)
            let metadata = fs::metadata(path).unwrap();
            let expected_size = (HEADER_SIZE + (51 * PAGE_SIZE)) as u64;
            assert_eq!(metadata.len(), expected_size, "Physical file size mismatch");
        }
    }
}
