//! TAR Filesystem Implementation
//!
//! A simple read-only filesystem that parses a TAR archive in memory.
//! Supports standard ustar format.

use super::{FileSystem, FileEntry};
use heapless::{String, Vec};
use core::str;

pub struct TarFileSystem {
    base_address: usize,
    size: usize,
}

impl TarFileSystem {
    /// Create a new TAR filesystem from a memory region.
    pub fn new(base_address: usize, size: usize) -> Self {
        Self { base_address, size }
    }

    /// Helper to parse an octal number from a byte slice.
    fn parse_octal(bytes: &[u8]) -> usize {
        let mut result = 0;
        for &b in bytes {
            if b < b'0' || b > b'7' {
                break;
            }
            result = result * 8 + (b - b'0') as usize;
        }
        result
    }

    /// Helper to get the file content slice.
    fn get_file_content(&self, offset: usize, size: usize) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts((self.base_address + offset) as *const u8, size)
        }
    }
}

impl FileSystem for TarFileSystem {
    fn init(&mut self) -> Result<(), &'static str> {
        // Basic validation: Check if memory is readable (trivial in bare metal)
        Ok(())
    }

    fn list_files(&self) -> Result<Vec<FileEntry, 32>, &'static str> {
        let mut files = Vec::new();
        let mut offset = 0;

        // Iterate through TAR blocks
        while offset + 512 <= self.size {
            let header = unsafe {
                core::slice::from_raw_parts((self.base_address + offset) as *const u8, 512)
            };

            // Check for empty block (end of archive)
            if header[0] == 0 {
                break;
            }

            // Parse filename (0..100)
            let name_bytes = &header[0..100];
            let name_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(100);
            let name_str = str::from_utf8(&name_bytes[0..name_len]).map_err(|_| "Invalid UTF-8 filename")?;

            // Parse size (124..136)
            let size = Self::parse_octal(&header[124..136]);

            // Parse type flag (156)
            let type_flag = header[156];
            let is_dir = type_flag == b'5';

            // Add to list if it's a regular file or directory
            if type_flag == b'0' || type_flag == 0 || is_dir {
                let mut name: String<64> = String::from(name_str);
                let _ = files.push(FileEntry {
                    name,
                    size,
                    is_dir,
                });
            }

            // Move to next header: 512 header + size rounded up to 512
            // Move to next header: 512 header + size rounded up to 512
            // Use checked arithmetic to prevent overflow attacks
            let padded_size = (size.checked_add(511).unwrap_or(0)) / 512 * 512;
            let next_offset = offset.checked_add(512).and_then(|o| o.checked_add(padded_size)).unwrap_or(self.size + 1);
            
            if next_offset > self.size {
                break; // Corrupt archive or end of buffer
            }

            offset = next_offset;
        }

        Ok(files)
    }

    fn read_file(&self, name: &str, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let mut offset = 0;

        while offset + 512 <= self.size {
            let header = unsafe {
                core::slice::from_raw_parts((self.base_address + offset) as *const u8, 512)
            };

            if header[0] == 0 {
                break;
            }

            let name_bytes = &header[0..100];
            let name_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(100);
            let current_name = str::from_utf8(&name_bytes[0..name_len]).unwrap_or("");
            let size = Self::parse_octal(&header[124..136]);

            if current_name == name {
                if buffer.len() < size {
                    return Err("Buffer too small");
                }
                
                let content = self.get_file_content(offset + 512, size);
                buffer[0..size].copy_from_slice(content);
                return Ok(size);
            }

            // Move to next header: 512 header + size rounded up to 512
            let padded_size = (size.checked_add(511).unwrap_or(0)) / 512 * 512;
            let next_offset = offset.checked_add(512).and_then(|o| o.checked_add(padded_size)).unwrap_or(self.size + 1);
            
            if next_offset > self.size {
                break;
            }

            offset = next_offset;
        }

        Err("File not found")
    }

    fn file_size(&self, name: &str) -> Result<usize, &'static str> {
        let mut offset = 0;

        while offset + 512 <= self.size {
            let header = unsafe {
                core::slice::from_raw_parts((self.base_address + offset) as *const u8, 512)
            };

            if header[0] == 0 {
                break;
            }

            let name_bytes = &header[0..100];
            let name_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(100);
            let current_name = str::from_utf8(&name_bytes[0..name_len]).unwrap_or("");
            let size = Self::parse_octal(&header[124..136]);

            if current_name == name {
                return Ok(size);
            }

            // Move to next header: 512 header + size rounded up to 512
            let padded_size = (size.checked_add(511).unwrap_or(0)) / 512 * 512;
            let next_offset = offset.checked_add(512).and_then(|o| o.checked_add(padded_size)).unwrap_or(self.size + 1);
            
            if next_offset > self.size {
                break;
            }

            offset = next_offset;
        }

        Err("File not found")
    }

    fn create_file(&mut self, _name: &str) -> Result<(), &'static str> {
        Err("Read-only filesystem")
    }

    fn write_file(&mut self, _name: &str, _data: &[u8]) -> Result<(), &'static str> {
        Err("Read-only filesystem")
    }

    fn delete_file(&mut self, _name: &str) -> Result<(), &'static str> {
        Err("Read-only filesystem")
    }
}
