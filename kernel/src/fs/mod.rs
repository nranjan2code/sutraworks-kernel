//! Filesystem Abstraction Layer
//!
//! This module defines the common traits for filesystem access in the Intent Kernel.
//! It supports read-only access primarily for loading models and configs.

pub mod tar;
pub mod ramfs;

use heapless::String;
use heapless::Vec;
use self::ramfs::RamDiskFS;

use spin::Mutex;

/// Global Filesystem Instance
pub static FILESYSTEM: Mutex<Option<RamDiskFS>> = Mutex::new(None);

/// Initialize the global filesystem
pub unsafe fn init() {
    let ramdisk_slice = crate::drivers::ramdisk::get_slice();
    // Check for TAR magic (ustar) at offset 257
    if ramdisk_slice.len() > 262 && &ramdisk_slice[257..262] == b"ustar" {
        let base = tar::TarFileSystem::new(crate::drivers::ramdisk::RAMDISK_BASE, crate::drivers::ramdisk::RAMDISK_SIZE);
        *FILESYSTEM.lock() = Some(RamDiskFS::new(base));
    } else {
        let base = tar::TarFileSystem::new(crate::drivers::ramdisk::RAMDISK_BASE, 0);
        *FILESYSTEM.lock() = Some(RamDiskFS::new(base));
    }
}

/// Get the global filesystem (locked)
/// NOTE: Returns a guard, so the caller holds the lock.
pub fn get() -> spin::MutexGuard<'static, Option<RamDiskFS>> {
    FILESYSTEM.lock()
}

/// Represents a file entry in the filesystem.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String<64>,
    pub size: usize,
    pub is_dir: bool,
}

/// Trait for a read-only filesystem.
pub trait FileSystem {
    /// Initialize the filesystem.
    fn init(&mut self) -> Result<(), &'static str>;

    /// List files in the root directory.
    fn list_files(&self) -> Result<Vec<FileEntry, 32>, &'static str>;

    /// Read the entire content of a file into a buffer.
    /// Returns the number of bytes read.
    fn read_file(&self, name: &str, buffer: &mut [u8]) -> Result<usize, &'static str>;

    /// Get the size of a file.
    fn file_size(&self, name: &str) -> Result<usize, &'static str>;

    /// Create a new file.
    fn create_file(&mut self, name: &str) -> Result<(), &'static str>;

    /// Write data to a file (overwrites existing).
    fn write_file(&mut self, name: &str, data: &[u8]) -> Result<(), &'static str>;

    /// Delete a file.
    fn delete_file(&mut self, name: &str) -> Result<(), &'static str>;
}
