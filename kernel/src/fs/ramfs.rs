//! RamDisk Filesystem (Read-Write Overlay)
//!
//! This module implements a hybrid filesystem that combines a read-only TAR base
//! with a writable in-memory overlay. This allows creating and modifying files
//! at runtime.

use super::{FileSystem, FileEntry};
use super::tar::TarFileSystem;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use heapless::String as HString;

struct RamFile {
    name: String,
    data: Vec<u8>,
    is_deleted: bool,
}

pub struct RamDiskFS {
    base: TarFileSystem,
    overlay: Vec<RamFile>,
}

impl RamDiskFS {
    pub fn new(base: TarFileSystem) -> Self {
        Self {
            base,
            overlay: Vec::new(),
        }
    }

    fn find_overlay(&self, name: &str) -> Option<&RamFile> {
        self.overlay.iter().find(|f| f.name == name)
    }

    fn find_overlay_mut(&mut self, name: &str) -> Option<&mut RamFile> {
        self.overlay.iter_mut().find(|f| f.name == name)
    }

    fn validate_filename(name: &str) -> Result<(), &'static str> {
        if name.is_empty() {
            return Err("Filename cannot be empty");
        }
        if name.len() > 64 {
            return Err("Filename too long (max 64 chars)");
        }
        if name.contains('/') || name.contains('\\') {
            return Err("Subdirectories not supported (flat filesystem)");
        }
        Ok(())
    }
}

impl FileSystem for RamDiskFS {
    fn init(&mut self) -> Result<(), &'static str> {
        self.base.init()
    }

    fn list_files(&self) -> Result<heapless::Vec<FileEntry, 32>, &'static str> {
        // Start with base files
        let mut entries = self.base.list_files()?;
        
        // Filter out deleted base files
        // (In a full implementation, we'd filter. For now, simple append/override)
        
        // Add overlay files
        for file in &self.overlay {
            if file.is_deleted {
                // If it's marked deleted, we should remove it from the list if it was in base
                // But heapless::Vec doesn't support easy removal by name without iteration
                // For this MVP, we'll just skip adding it if it's new, but we can't easily hide base files yet
                continue;
            }

            // Check if already in list (override)
            let mut found = false;
            for entry in entries.iter_mut() {
                if entry.name == file.name.as_str() {
                    entry.size = file.data.len();
                    found = true;
                    break;
                }
            }

            if !found {
                let mut name: HString<64> = HString::new();
                if name.push_str(file.name.as_str()).is_ok() {
                    let _ = entries.push(FileEntry {
                        name,
                        size: file.data.len(),
                        is_dir: false,
                    });
                }
            }
        }

        Ok(entries)
    }

    fn file_size(&self, name: &str) -> Result<usize, &'static str> {
        // Check overlay first
        if let Some(file) = self.find_overlay(name) {
            if file.is_deleted {
                return Err("File not found");
            }
            return Ok(file.data.len());
        }

        // Fallback to base
        self.base.file_size(name)
    }

    fn read_file(&self, name: &str, buffer: &mut [u8]) -> Result<usize, &'static str> {
        // Check overlay first
        if let Some(file) = self.find_overlay(name) {
            if file.is_deleted {
                return Err("File not found");
            }
            if buffer.len() < file.data.len() {
                return Err("Buffer too small");
            }
            buffer[0..file.data.len()].copy_from_slice(&file.data);
            return Ok(file.data.len());
        }

        // Fallback to base
        self.base.read_file(name, buffer)
    }

    fn create_file(&mut self, name: &str) -> Result<(), &'static str> {
        Self::validate_filename(name)?;

        if self.find_overlay(name).is_some() {
            return Err("File already exists");
        }
        
        // Also check base? For now, we allow shadowing base files
        
        self.overlay.push(RamFile {
            name: name.to_string(),
            data: Vec::new(),
            is_deleted: false,
        });
        Ok(())
    }

    fn write_file(&mut self, name: &str, data: &[u8]) -> Result<(), &'static str> {
        Self::validate_filename(name)?;

        if let Some(file) = self.find_overlay_mut(name) {
            if file.is_deleted {
                return Err("File not found");
            }
            file.data = data.to_vec();
            return Ok(());
        }

        // If not in overlay, check if in base. If so, copy-on-write
        if self.base.file_size(name).is_ok() {
            self.overlay.push(RamFile {
                name: name.to_string(),
                data: data.to_vec(),
                is_deleted: false,
            });
            Ok(())
        } else {
            Err("File not found")
        }
    }

    fn delete_file(&mut self, name: &str) -> Result<(), &'static str> {
        if let Some(file) = self.find_overlay_mut(name) {
            file.is_deleted = true;
            file.data.clear();
            return Ok(());
        }

        // If in base, add a tombstone to overlay
        if self.base.file_size(name).is_ok() {
            self.overlay.push(RamFile {
                name: name.to_string(),
                data: Vec::new(),
                is_deleted: true,
            });
            Ok(())
        } else {
            Err("File not found")
        }
    }
}
