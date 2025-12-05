//! FAT32 Filesystem Driver
//!
//! Implements read/write support for FAT32 partitions.

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::sync::Arc;
use crate::kernel::sync::SpinLock;
use crate::fs::vfs::{FileOps, Filesystem, DirEntry, BlockDevice, FileStat, SeekFrom};
use crate::kprintln;

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// FAT32 Boot Sector (BPB)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Fat32BootSector {
    pub jmp_boot: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entry_count: u16, // 0 for FAT32
    pub total_sectors_16: u16, // 0 for FAT32
    pub media: u8,
    pub fat_size_16: u16,      // 0 for FAT32
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    
    // FAT32 Extended Fields
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    pub reserved: [u8; 12],
    pub drive_number: u8,
    pub reserved1: u8,
    pub boot_signature: u8, // 0x29
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],   // "FAT32   "
}

/// FAT Directory Entry (32 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FatDirEntry {
    pub name: [u8; 11],     // 8.3 Name
    pub attr: u8,
    pub nt_res: u8,
    pub crt_time_tenth: u8,
    pub crt_time: u16,
    pub crt_date: u16,
    pub last_acc_date: u16,
    pub cluster_high: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    pub cluster_low: u16,
    pub size: u32,
}

// Attributes
pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN:    u8 = 0x02;
pub const ATTR_SYSTEM:    u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE:   u8 = 0x20;
pub const ATTR_LONG_NAME: u8 = 0x0F;

// ═══════════════════════════════════════════════════════════════════════════════
// FAT32 FILESYSTEM IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
pub struct Fat32FileSystem {
    device: Arc<dyn BlockDevice>,
    bpb: Fat32BootSector,
    fat_start_sector: u32,
    data_start_sector: u32,
}

impl Fat32FileSystem {
    pub fn new(device: Arc<dyn BlockDevice>) -> Result<Arc<Self>, &'static str> {
        // Read Boot Sector (Sector 0)
        let mut buf = [0u8; 512];
        device.read_sector(0, &mut buf)?;
        
        let bpb = unsafe { core::ptr::read(buf.as_ptr() as *const Fat32BootSector) };
        
        // Verify Signature
        if bpb.bytes_per_sector != 512 {
            return Err("Unsupported sector size (must be 512)");
        }
        
        // Calculate offsets
        let fat_start_sector = bpb.reserved_sectors as u32;
        let fat_size = bpb.fat_size_32;
        let data_start_sector = fat_start_sector + (bpb.num_fats as u32 * fat_size);
        
        kprintln!("[FAT32] Mounted. Vol: {:?}", core::str::from_utf8(&bpb.volume_label).unwrap_or("???"));
        kprintln!("[FAT32] Cluster Size: {} bytes", bpb.sectors_per_cluster as u32 * 512);
        
        Ok(Arc::new(Self {
            device,
            bpb,
            fat_start_sector,
            data_start_sector,
        }))
    }

    pub fn mount(device: Arc<dyn BlockDevice>) -> Result<Arc<Self>, &'static str> {
        Self::new(device)
    }
    
    /// Convert Cluster Number to Sector Number
    fn cluster_to_sector(&self, cluster: u32) -> u32 {
        let _root_cluster = self.bpb.root_cluster;
        // First Data Sector + (Cluster - 2) * SectorsPerCluster
        // Note: Clusters start at 2.
        self.data_start_sector + (cluster - 2) * self.bpb.sectors_per_cluster as u32
    }
    
    /// Read next cluster from FAT
    fn get_next_cluster(&self, cluster: u32) -> Result<Option<u32>, &'static str> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start_sector + (fat_offset / 512);
        let ent_offset = (fat_offset % 512) as usize;
        
        let mut buf = [0u8; 512];
        self.device.read_sector(fat_sector, &mut buf)?;
        
        let entry = u32::from_le_bytes(buf[ent_offset..ent_offset+4].try_into().unwrap()) & 0x0FFFFFFF;
        
        if entry >= 0x0FFFFFF8 {
            Ok(None) // End of Chain
        } else if entry == 0x0FFFFFF7 {
            Err("Bad Cluster")
        } else {
            Ok(Some(entry))
        }
    }
    
    /// Read a cluster into a buffer
    fn read_cluster(&self, cluster: u32, buf: &mut [u8]) -> Result<usize, &'static str> {
        let start_sector = self.cluster_to_sector(cluster);
        let count = self.bpb.sectors_per_cluster as u32;
        let sector_size = 512;
        
        for i in 0..count {
            let offset = (i * sector_size) as usize;
            let buf_len = buf.len();
            if offset >= buf_len { break; }
            
            let slice = &mut buf[offset..core::cmp::min(offset + 512, buf_len)];
            // We need a full sector buffer
            let mut sector_buf = [0u8; 512];
            self.device.read_sector(start_sector + i, &mut sector_buf)?;
            
            slice.copy_from_slice(&sector_buf[0..slice.len()]);
        }
        
        Ok((count * sector_size) as usize)
    }

    /// Read directory entries from a cluster chain
    fn read_directory_entries(&self, start_cluster: u32) -> Result<Vec<FatDirEntry>, &'static str> {
        let mut entries = Vec::new();
        let mut current_cluster = start_cluster;
        let cluster_size = self.bpb.sectors_per_cluster as usize * 512;
        let mut buf = vec![0; cluster_size];

        loop {
            // Read cluster
            self.read_cluster(current_cluster, &mut buf)?;
            
            // Parse entries
            for i in 0..(cluster_size / 32) {
                let offset = i * 32;
                let entry_raw = &buf[offset..offset+32];
                
                // Check for end of directory
                if entry_raw[0] == 0x00 {
                    return Ok(entries);
                }
                
                // Check for deleted entry
                if entry_raw[0] == 0xE5 {
                    continue;
                }
                
                let entry = unsafe { core::ptr::read(entry_raw.as_ptr() as *const FatDirEntry) };
                
                // Skip Long File Name (LFN) entries for now
                if entry.attr == ATTR_LONG_NAME {
                    continue;
                }
                
                entries.push(entry);
            }
            
            // Next cluster
            match self.get_next_cluster(current_cluster)? {
                Some(next) => current_cluster = next,
                None => break,
            }
        }
        
        Ok(entries)
    }

    /// Find an entry in a directory by name
    fn find_entry(&self, dir_cluster: u32, name: &str) -> Result<Option<FatDirEntry>, &'static str> {
        let entries = self.read_directory_entries(dir_cluster)?;
        
        for entry in entries {
            let entry_name = parse_fat_name(&entry.name);
            if entry_name.eq_ignore_ascii_case(name) {
                return Ok(Some(entry));
            }
        }
        
        Ok(None)
    }
}

impl Filesystem for Fat32FileSystem {
    fn open(&self, path: &str, _flags: usize) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        let mut current_cluster = self.bpb.root_cluster;
        let mut path_parts = path.split('/').filter(|s| !s.is_empty()).peekable();
        
        if path_parts.peek().is_none() {
             return Err("Cannot open root directory");
        }
        
        while let Some(name) = path_parts.next() {
            match self.find_entry(current_cluster, name)? {
                Some(entry) => {
                    if path_parts.peek().is_none() {
                        // Found the target file/dir
                        let is_dir = (entry.attr & ATTR_DIRECTORY) != 0;
                        let cluster = ((entry.cluster_high as u32) << 16) | (entry.cluster_low as u32);
                        
                        return Ok(Arc::new(SpinLock::new(Fat32File {
                            fs: Arc::new(self.clone()),
                            first_cluster: cluster,
                            current_cluster: cluster,
                            current_offset: 0,
                            size: entry.size as u64,
                            is_dir,
                        })));
                    } else {
                        // It's a directory component
                        if (entry.attr & ATTR_DIRECTORY) == 0 {
                            return Err("Not a directory");
                        }
                        current_cluster = ((entry.cluster_high as u32) << 16) | (entry.cluster_low as u32);
                    }
                },
                None => return Err("File not found"),
            }
        }
        Err("Logic error")
    }
    
    fn create(&self, _path: &str) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        Err("Read-only filesystem")
    }
    
    fn mkdir(&self, _path: &str) -> Result<(), &'static str> {
        Err("Read-only filesystem")
    }
    
    fn remove(&self, _path: &str) -> Result<(), &'static str> {
        Err("Read-only filesystem")
    }
    
    fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, &'static str> {
        // Assume root for now
        if path == "/" || path.is_empty() {
            let raw_entries = self.read_directory_entries(self.bpb.root_cluster)?;
            let mut entries = Vec::new();
            
            for raw in raw_entries {
                // Convert 8.3 name to string
                let name = parse_fat_name(&raw.name);
                
                entries.push(DirEntry {
                    name,
                    is_dir: (raw.attr & ATTR_DIRECTORY) != 0,
                    size: raw.size as u64,
                });
            }
            Ok(entries)
        } else {
            Err("Subdirectories not supported yet")
        }
    }
}

/// Helper to parse 8.3 name
fn parse_fat_name(raw: &[u8; 11]) -> String {
    let mut name = String::new();
    // Filename (8 chars)
    for &c in raw.iter().take(8) {
        if c != 0x20 {
            name.push(c as char);
        }
    }
    // Extension (3 chars)
    if raw[8] != 0x20 {
        name.push('.');
        for &c in raw.iter().skip(8).take(3) {
            if c != 0x20 {
                name.push(c as char);
            }
        }
    }
    name
}

// ═══════════════════════════════════════════════════════════════════════════════
// FILE HANDLE
// ═══════════════════════════════════════════════════════════════════════════════

pub struct Fat32File {
    fs: Arc<Fat32FileSystem>,
    first_cluster: u32,
    current_cluster: u32,
    current_offset: u64,
    size: u64,
    is_dir: bool,
}

impl FileOps for Fat32File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if self.current_offset >= self.size {
            return Ok(0); // EOF
        }
        
        let cluster_size = self.fs.bpb.sectors_per_cluster as u64 * 512;
        let mut bytes_read = 0;
        let mut buf_offset = 0;
        
        while buf_offset < buf.len() && self.current_offset < self.size {
            // Calculate offset within current cluster
            let cluster_offset = (self.current_offset % cluster_size) as usize;
            let bytes_to_read = core::cmp::min(
                buf.len() - buf_offset,
                core::cmp::min(
                    (cluster_size as usize) - cluster_offset,
                    (self.size - self.current_offset) as usize
                )
            );
            
            // Read cluster
            let mut cluster_buf = vec![0; cluster_size as usize];
            self.fs.read_cluster(self.current_cluster, &mut cluster_buf)?;
            
            // Copy data
            buf[buf_offset..buf_offset+bytes_to_read].copy_from_slice(&cluster_buf[cluster_offset..cluster_offset+bytes_to_read]);
            
            // Update state
            bytes_read += bytes_to_read;
            buf_offset += bytes_to_read;
            self.current_offset += bytes_to_read as u64;
            
            // Move to next cluster if needed
            if (self.current_offset % cluster_size) == 0 && self.current_offset < self.size {
                match self.fs.get_next_cluster(self.current_cluster)? {
                    Some(next) => self.current_cluster = next,
                    None => break, // Should not happen if size is correct
                }
            }
        }
        
        Ok(bytes_read)
    }
    
    fn write(&mut self, _buf: &[u8]) -> Result<usize, &'static str> {
        Err("Read-only")
    }
    
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str> {
        let new_pos = match pos {
            SeekFrom::Start(off) => off,
            SeekFrom::Current(off) => (self.current_offset as i64 + off) as u64,
            SeekFrom::End(off) => (self.size as i64 + off) as u64,
        };

        if new_pos > self.size {
            return Err("Seek beyond EOF");
        }

        // Optimize: If seeking forward within same cluster, just update offset
        let cluster_size = self.fs.bpb.sectors_per_cluster as u64 * 512;
        let current_cluster_idx = self.current_offset / cluster_size;
        let new_cluster_idx = new_pos / cluster_size;

        if new_cluster_idx == current_cluster_idx {
            self.current_offset = new_pos;
            return Ok(new_pos);
        }

        // If seeking backwards or to a different cluster, we might need to restart from first_cluster
        // For simplicity, always restart from first_cluster if cluster changes
        // (Optimization: could traverse from current if moving forward)
        
        let mut cluster = self.first_cluster;
        for _ in 0..new_cluster_idx {
            match self.fs.get_next_cluster(cluster)? {
                Some(next) => cluster = next,
                None => return Err("Seek error: broken cluster chain"),
            }
        }
        
        self.current_cluster = cluster;
        self.current_offset = new_pos;
        
        Ok(new_pos)
    }
    
    fn close(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    
    fn stat(&self) -> Result<FileStat, &'static str> {
        Ok(FileStat {
            size: self.size,
            mode: if self.is_dir { 0o040777 } else { 0o100777 },
            inode: self.first_cluster as u64,
        })
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}
