//! Filesystem Subsystem
//!
//! Exports VFS and specific filesystem implementations.

use alloc::sync::Arc;

pub mod vfs;
pub mod fat32;
pub mod pipe;
pub mod cache;
pub mod console;

pub use vfs::{VFS, FileOps, Filesystem, SeekFrom, O_RDONLY, O_WRONLY, O_RDWR, O_CREAT};

/// Initialize Filesystem Subsystem
pub fn init() {
    vfs::init();
}

/// Mount a filesystem
pub fn mount(path: &str, fs: Arc<dyn Filesystem>) -> Result<(), &'static str> {
    vfs::VFS.lock().mount(path, fs)
}
