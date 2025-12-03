//! Filesystem Subsystem
//!
//! Exports VFS and specific filesystem implementations.

pub mod vfs;
pub mod fat32;
pub mod pipe;

pub use vfs::{VFS, FileOps, Filesystem, SeekFrom, O_RDONLY, O_WRONLY, O_RDWR, O_CREAT};

/// Initialize Filesystem Subsystem
pub fn init() {
    vfs::init();
}
