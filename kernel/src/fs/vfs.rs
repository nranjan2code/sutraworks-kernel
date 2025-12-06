//! Virtual File System (VFS) Layer
//!
//! Provides an abstraction over different filesystems (FAT32, Ext2, etc.).
//! Handles file descriptors, mount points, and standard I/O operations.

use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;
use crate::kernel::sync::SpinLock;
use crate::kprintln;

/// File Open Flags
pub const O_RDONLY: usize = 0;
pub const O_WRONLY: usize = 1;
pub const O_RDWR:   usize = 2;
pub const O_CREAT:  usize = 64;
pub const O_TRUNC:  usize = 512;
pub const O_APPEND: usize = 1024;

/// Seek Whence
pub enum SeekFrom {
    Start(u64),
    Current(i64),
    End(i64),
}

/// File Statistics
#[derive(Debug, Clone, Copy)]
pub struct FileStat {
    pub size: u64,
    pub mode: u32,
    pub inode: u64,
}

/// Directory Entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Trait for File Operations
/// Every open file description implements this.
use core::any::Any;

/// Trait for File Operations
/// Every open file description implements this.
pub trait FileOps: Send + Sync + Any {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str>;
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str>;
    fn close(&mut self) -> Result<(), &'static str>;
    fn stat(&self) -> Result<FileStat, &'static str>;
    fn as_any(&mut self) -> &mut dyn Any;
}

/// Trait for Filesystem Operations
/// Every mounted filesystem implements this.
pub trait Filesystem: Send + Sync {
    fn open(&self, path: &str, flags: usize) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str>;
    fn create(&self, path: &str) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str>;
    fn mkdir(&self, path: &str) -> Result<(), &'static str>;
    fn remove(&self, path: &str) -> Result<(), &'static str>;
    fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, &'static str>;
}

/// Abstract Block Device (e.g., SD Card partition)
pub trait BlockDevice: Send + Sync {
    fn read_sector(&self, sector: u32, buf: &mut [u8]) -> Result<(), &'static str>;
    fn write_sector(&self, sector: u32, buf: &[u8]) -> Result<(), &'static str>;
    fn sync(&self) -> Result<(), &'static str> { Ok(()) } // Default implementation
}

/// File Descriptor
#[derive(Clone)]
pub struct FileDescriptor {
    pub file: Arc<SpinLock<dyn FileOps>>,
    pub flags: usize,
    pub offset: u64,
}

/// Per-Process File Table
pub struct ProcessFileTable {
    pub fds: Vec<Option<FileDescriptor>>,
}

impl ProcessFileTable {
    pub fn new() -> Self {
        Self {
            fds: Vec::new(),
        }
    }

    /// Allocate a new file descriptor
    pub fn alloc_fd(&mut self, file: Arc<SpinLock<dyn FileOps>>, flags: usize) -> Result<usize, &'static str> {
        // Find first free slot
        for (i, slot) in self.fds.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(FileDescriptor { file, flags, offset: 0 });
                return Ok(i);
            }
        }
        
        // No free slot, extend
        self.fds.push(Some(FileDescriptor { file, flags, offset: 0 }));
        Ok(self.fds.len() - 1)
    }

    /// Get a file descriptor
    pub fn get_fd(&mut self, fd: usize) -> Result<&mut FileDescriptor, &'static str> {
        if fd >= self.fds.len() {
            return Err("Invalid FD");
        }
        self.fds[fd].as_mut().ok_or("Invalid FD")
    }

    /// Close a file descriptor
    pub fn close_fd(&mut self, fd: usize) -> Result<(), &'static str> {
        if fd >= self.fds.len() || self.fds[fd].is_none() {
            return Err("Invalid FD");
        }
        
        // Take the FD out to drop it
        let _fd = self.fds[fd].take();
        // The Arc will drop, calling close() if it's the last reference?
        // No, FileOps::close() is manual.
        // We should probably call it if we are the owner?
        // But Arc is shared.
        // The FileOps trait has a close() method.
        // For now, we just drop the reference.
        
        Ok(())
    }

    /// Duplicate a file descriptor
    pub fn dup2(&mut self, oldfd: usize, newfd: usize) -> Result<usize, &'static str> {
        if oldfd >= self.fds.len() || self.fds[oldfd].is_none() {
            return Err("Invalid old FD");
        }
        
        if oldfd == newfd {
            return Ok(newfd);
        }
        
        // If newfd is open, close it
        if newfd < self.fds.len() && self.fds[newfd].is_some() {
            let _ = self.close_fd(newfd);
        }
        
        // Extend if needed
        while self.fds.len() <= newfd {
            self.fds.push(None);
        }
        
        // Clone the FD (Arc increment)
        let fd_struct = self.fds[oldfd].as_ref().expect("oldfd validated above").clone();
        self.fds[newfd] = Some(fd_struct);
        
        Ok(newfd)
    }
}

impl Clone for ProcessFileTable {
    fn clone(&self) -> Self {
        ProcessFileTable {
            fds: self.fds.clone(),
        }
    }
}

impl Default for ProcessFileTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Global VFS Manager
pub struct VfsManager {
    mounts: Vec<(String, Arc<dyn Filesystem>)>, // (Mount Point, FS)
}

impl VfsManager {
    pub const fn new() -> Self {
        Self {
            mounts: Vec::new(),
        }
    }

    /// Mount a filesystem at a path
    pub fn mount(&mut self, path: &str, fs: Arc<dyn Filesystem>) -> Result<(), &'static str> {
        self.mounts.push((String::from(path), fs));
        Ok(())
    }

    /// Find the filesystem responsible for a path
    /// Returns (FS, Relative Path)
    fn resolve_path<'a>(&self, path: &'a str) -> Result<(Arc<dyn Filesystem>, &'a str), &'static str> {
        // Simple longest prefix match
        let mut best_match_len = 0;
        let mut best_fs = None;
        let mut relative_path = path;

        for (mount_point, fs) in &self.mounts {
            if path.starts_with(mount_point) && mount_point.len() > best_match_len {
                best_match_len = mount_point.len();
                best_fs = Some(fs.clone());
                
                // Strip mount point from path
                if path.len() == mount_point.len() {
                    relative_path = "/";
                } else {
                    relative_path = &path[mount_point.len()..];
                }
            }
        }

        if let Some(fs) = best_fs {
            Ok((fs, relative_path))
        } else {
            Err("No filesystem mounted at path")
        }
    }

    pub fn open(&self, path: &str, flags: usize) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        let (fs, rel_path) = self.resolve_path(path)?;
        fs.open(rel_path, flags)
    }

    pub fn create(&self, path: &str) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        let (fs, rel_path) = self.resolve_path(path)?;
        fs.create(rel_path)
    }
    
    pub fn mkdir(&self, path: &str) -> Result<(), &'static str> {
        let (fs, rel_path) = self.resolve_path(path)?;
        fs.mkdir(rel_path)
    }
    
    pub fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, &'static str> {
        let (fs, rel_path) = self.resolve_path(path)?;
        fs.read_dir(rel_path)
    }
}

impl Default for VfsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global VFS Instance
pub static VFS: SpinLock<VfsManager> = SpinLock::new(VfsManager::new());

/// Helper to initialize VFS
pub fn init() {
    // In the future, we will mount the root FS here.
    kprintln!("[VFS] Initialized.");
}
