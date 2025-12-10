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
        
        let entry = u32::from_le_bytes(buf[ent_offset..ent_offset+4].try_into().expect("4-byte slice is valid")) & 0x0FFFFFFF;
        
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

    /// Write text to a cluster (Full overwrite of sectors)
    fn write_cluster(&self, cluster: u32, buf: &[u8]) -> Result<usize, &'static str> {
        let start_sector = self.cluster_to_sector(cluster);
        let count = self.bpb.sectors_per_cluster as u32;
        let sector_size = 512;
        
        let mut written = 0;
        for i in 0..count {
             let offset = (i * sector_size) as usize;
             if offset >= buf.len() { break; }
             
             let len = core::cmp::min(512, buf.len() - offset);
             // Read-Modify-Write if partial sector?
             // For now assume logic handles it or we just write full sectors if possible.
             // If len < 512, we MUST read first.
             
             let mut sector_buf = [0u8; 512];
             if len < 512 {
                 self.device.read_sector(start_sector + i, &mut sector_buf)?;
             }
             
             sector_buf[0..len].copy_from_slice(&buf[offset..offset+len]);
             self.device.write_sector(start_sector + i, &sector_buf)?;
             
             written += len;
        }
        Ok(written)
    }

    /// Write a FAT entry
    fn write_fat_entry(&self, cluster: u32, value: u32) -> Result<(), &'static str> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start_sector + (fat_offset / 512);
        let ent_offset = (fat_offset % 512) as usize;
        
        let mut buf = [0u8; 512];
        self.device.read_sector(fat_sector, &mut buf)?;
        
        let old = u32::from_le_bytes(buf[ent_offset..ent_offset+4].try_into().unwrap());
        let val = (old & 0xF0000000) | (value & 0x0FFFFFFF);
        
        buf[ent_offset..ent_offset+4].copy_from_slice(&val.to_le_bytes());
        
        self.device.write_sector(fat_sector, &buf)?;
        if self.bpb.num_fats > 1 {
             let fat_size_sectors = self.bpb.fat_size_32;
             self.device.write_sector(fat_sector + fat_size_sectors, &buf)?;
        }
        Ok(())
    }

    /// Find a free cluster
    fn find_free_cluster(&self) -> Result<u32, &'static str> {
        let fat_size = self.bpb.fat_size_32;
        for i in 0..fat_size {
            let sector = self.fat_start_sector + i;
            let mut buf = [0u8; 512];
            self.device.read_sector(sector, &mut buf)?;
            
            for j in 0..128 {
                let val = u32::from_le_bytes(buf[j*4..(j+1)*4].try_into().unwrap()) & 0x0FFFFFFF;
                if val == 0 {
                    let cluster = i * 128 + j as u32;
                    if cluster >= 2 { return Ok(cluster); }
                }
            }
        }
        Err("Disk full")
    }

    /// Allocate a new cluster
    fn alloc_cluster(&self, prev_cluster: Option<u32>) -> Result<u32, &'static str> {
        let next = self.find_free_cluster()?;
        self.write_fat_entry(next, 0x0FFFFFFF)?; // Mark EOC
        
        if let Some(prev) = prev_cluster {
            self.write_fat_entry(prev, next)?;
        }
        
        // Zero init
        let sector = self.cluster_to_sector(next);
        let count = self.bpb.sectors_per_cluster as u32;
        let zero = [0u8; 512];
        for i in 0..count {
            self.device.write_sector(sector+i, &zero)?;
        }
        Ok(next)
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

    /// Read a directory entry
    fn read_entry(&self, cluster: u32, offset: usize) -> Result<FatDirEntry, &'static str> {
        let start_sector = self.cluster_to_sector(cluster);
        let sector_offset = offset / 512;
        let byte_offset = offset % 512;
        
        let mut buf = [0u8; 512];
        self.device.read_sector(start_sector + sector_offset as u32, &mut buf)?;
        
        let entry_ptr = unsafe { buf.as_ptr().add(byte_offset) as *const FatDirEntry };
        Ok(unsafe { core::ptr::read_unaligned(entry_ptr) })
    }

    /// Create a new file entry in a directory
    fn create_file(&self, dir_cluster: u32, name: &str) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
         // 1. Iterate dir to find free slot
         let mut current_cluster = dir_cluster;
         let mut target_cluster = dir_cluster;
         let mut target_offset = 0;
         let mut found = false;
         
         loop {
             let start_sec = self.cluster_to_sector(current_cluster);
             for i in 0..self.bpb.sectors_per_cluster {
                 let mut buf = [0u8; 512];
                 self.device.read_sector(start_sec + i as u32, &mut buf)?;
                 
                 for j in 0..16 {
                     let offset = j * 32;
                     if buf[offset] == 0 || buf[offset] == 0xE5 {
                         target_cluster = current_cluster;
                         target_offset = (i as usize * 512) + offset;
                         found = true;
                         break;
                     }
                 }
                 if found { break; }
             }
             if found { break; }
             
             match self.get_next_cluster(current_cluster)? {
                 Some(next) => current_cluster = next,
                 None => {
                     // Extend
                     let next = self.alloc_cluster(Some(current_cluster))?;
                     target_cluster = next;
                     target_offset = 0;
                     found = true;
                     break;
                 }
             }
         }
         
         // 2. Prepare entry
         let mut entry: FatDirEntry = unsafe { core::mem::zeroed() };
         let short_name = make_short_name(name);
         entry.name = short_name;
         entry.attr = 0x20; // Archive
         entry.size = 0;
         entry.cluster_high = 0;
         entry.cluster_low = 0;
         
         // 3. Write entry
         self.write_entry(target_cluster, target_offset, entry)?;
         
         Ok(Arc::new(SpinLock::new(Fat32File {
             fs: Arc::new(self.clone()),
             first_cluster: 0,
             current_cluster: 0,
             current_offset: 0,
             size: 0,
             is_dir: false,
             entry_cluster: target_cluster,
             entry_offset: target_offset,
         })))
    }

    /// Write a directory entry
    fn write_entry(&self, cluster: u32, offset: usize, entry: FatDirEntry) -> Result<(), &'static str> {
         let start_sector = self.cluster_to_sector(cluster);
         let sector_offset = offset / 512;
         let byte_offset = offset % 512;
         
         let mut buf = [0u8; 512];
         self.device.read_sector(start_sector + sector_offset as u32, &mut buf)?;
         
         let ptr = unsafe { buf.as_mut_ptr().add(byte_offset) as *mut FatDirEntry };
         unsafe { ptr.write_unaligned(entry) };
         
         self.device.write_sector(start_sector + sector_offset as u32, &buf)?;
         Ok(())
    }

    /// Find an entry in a directory by name (Returns Entry + Cluster + Offset)
    fn find_entry(&self, dir_cluster: u32, name: &str) -> Result<Option<(FatDirEntry, u32, usize)>, &'static str> {
        let mut current_cluster = dir_cluster;
        loop {
            // Iterate sectors in cluster
            let start_sec = self.cluster_to_sector(current_cluster);
            for i in 0..self.bpb.sectors_per_cluster {
                 let mut buf = [0u8; 512];
                 self.device.read_sector(start_sec + i as u32, &mut buf)?;
                 
                 for j in 0..16 { // 16 entries per sector
                     let offset = j * 32;
                     let entry_raw = &buf[offset..offset+32];
                     
                     if entry_raw[0] == 0 { return Ok(None); }
                     if entry_raw[0] == 0xE5 { continue; }
                     
                     let entry = unsafe { core::ptr::read(entry_raw.as_ptr() as *const FatDirEntry) };
                     if entry.attr == ATTR_LONG_NAME || (entry.attr & ATTR_VOLUME_ID) != 0 { continue; }
                     
                     let entry_name = parse_fat_name(&entry.name);
                     if entry_name.eq_ignore_ascii_case(name) {
                         let cluster_offset = (i as usize * 512) + offset;
                         return Ok(Some((entry, current_cluster, cluster_offset)));
                     }
                 }
            }
            
            match self.get_next_cluster(current_cluster)? {
                Some(next) => current_cluster = next,
                None => break,
            }
        }
        Ok(None)
    }
}

impl Filesystem for Fat32FileSystem {
    fn open(&self, path: &str, flags: usize) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        let mut current_cluster = self.bpb.root_cluster;
        let mut path_parts = path.split('/').filter(|s| !s.is_empty()).peekable();
        
        if path_parts.peek().is_none() {
             // Root directory
             return Ok(Arc::new(SpinLock::new(Fat32File {
                 fs: Arc::new(self.clone()),
                 first_cluster: self.bpb.root_cluster,
                 current_cluster: self.bpb.root_cluster,
                 current_offset: 0,
                 size: 0,
                 is_dir: true,
                 entry_cluster: 0,
                 entry_offset: 0,
             })));
        }
        
        while let Some(name) = path_parts.next() {
            match self.find_entry(current_cluster, name)? {
                Some((entry, entry_cluster, entry_offset)) => {
                    if path_parts.peek().is_none() {
                        // Found file/dir
                        let is_dir = (entry.attr & ATTR_DIRECTORY) != 0;
                        let cluster = ((entry.cluster_high as u32) << 16) | (entry.cluster_low as u32);
                        
                        // Check flags
                        if (flags & crate::fs::vfs::O_TRUNC) != 0 && !is_dir {
                             // Truncate logic (not implemented yet, but we allow write)
                        }

                        return Ok(Arc::new(SpinLock::new(Fat32File {
                            fs: Arc::new(self.clone()),
                            first_cluster: cluster,
                            current_cluster: cluster,
                            current_offset: 0,
                            size: entry.size as u64,
                            is_dir,
                            entry_cluster,
                            entry_offset,
                        })));
                    } else {
                        // Directory
                        if (entry.attr & ATTR_DIRECTORY) == 0 { return Err("Not a directory"); }
                        current_cluster = ((entry.cluster_high as u32) << 16) | (entry.cluster_low as u32);
                    }
                },
                None => {
                    // Create if O_CREAT?
                    if (flags & crate::fs::vfs::O_CREAT) != 0 && path_parts.peek().is_none() {
                        return self.create_file(current_cluster, name);
                    }
                    return Err("File not found");
                }
            }
        }
        Err("Logic error")
    }
    
    fn create(&self, path: &str) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        // Find parent
        let path = path.trim_matches('/');
        if path.contains('/') { return Err("Subdirs not supported yet"); }
        self.create_file(self.bpb.root_cluster, path)
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

fn make_short_name(name: &str) -> [u8; 11] {
    let mut res = [0x20u8; 11];
    let bytes = name.as_bytes();
    
    // Split extension
    let (base, ext) = if let Some(idx) = bytes.iter().rposition(|&c| c == b'.') {
        (&bytes[0..idx], &bytes[idx+1..])
    } else {
        (bytes, &[][..])
    };
    
    // Copy base (up to 8, uppercase)
    for (i, &b) in base.iter().take(8).enumerate() {
        res[i] = b.to_ascii_uppercase();
    }
    
    // Copy ext (up to 3, uppercase)
    for (i, &b) in ext.iter().take(3).enumerate() {
        res[8+i] = b.to_ascii_uppercase();
    }
    res
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
    entry_cluster: u32,
    entry_offset: usize,
}

impl FileOps for Fat32File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if !self.is_dir && self.current_offset >= self.size {
            return Ok(0); // EOF
        }
        
        let cluster_size = self.fs.bpb.sectors_per_cluster as u64 * 512;
        let mut bytes_read = 0;
        let mut buf_offset = 0;
        
        while buf_offset < buf.len() && (self.is_dir || self.current_offset < self.size) {
            // Calculate offset within current cluster
            let cluster_offset = (self.current_offset % cluster_size) as usize;
            let bytes_to_read = core::cmp::min(
                buf.len() - buf_offset,
                (cluster_size as usize) - cluster_offset,
            );
            
            let bytes_to_read = if self.is_dir {
                bytes_to_read
            } else {
                core::cmp::min(bytes_to_read, (self.size - self.current_offset) as usize)
            };
            
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
            if self.current_offset.is_multiple_of(cluster_size) && self.current_offset < self.size {
                match self.fs.get_next_cluster(self.current_cluster)? {
                    Some(next) => self.current_cluster = next,
                    None => break, // Should not happen if size is correct
                }
            }
        }
        
        Ok(bytes_read)
    }
    
    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        if self.is_dir { return Err("Cannot write to dir"); }
        if self.first_cluster == 0 {
            // Alloc first cluster
            let c = self.fs.alloc_cluster(None)?;
            self.first_cluster = c;
            self.current_cluster = c;
            
            // Update directory entry with first cluster
            let mut entry = self.fs.read_entry(self.entry_cluster, self.entry_offset)?;
            entry.cluster_high = (c >> 16) as u16;
            entry.cluster_low = (c & 0xFFFF) as u16;
            self.fs.write_entry(self.entry_cluster, self.entry_offset, entry)?;
        }
        
        let mut total_written = 0;
        let cluster_size = self.fs.bpb.sectors_per_cluster as u64 * 512;
        
        while total_written < buf.len() {
             // Calculate local offset
             let offset_in_cluster = (self.current_offset % cluster_size) as usize;
             let space_in_cluster = (cluster_size as usize) - offset_in_cluster;
             let to_write = core::cmp::min(buf.len() - total_written, space_in_cluster);
             
             // Check if we need to extend
             if offset_in_cluster == 0 && self.current_offset > 0 {
                 // Boundary reached, check if we need new cluster
                 // Actually we can just check if current_cluster is EOC?
                 // Simple logic: if offset matches size limit of chain so far, append.
                 // Better: check FAT for current_cluster.
                 // For now, assume if we write past end, we alloc.
             }
             
             // Write data to cluster
             let mut cluster_buf = vec![0u8; cluster_size as usize];
             // Read current content if partial write
             self.fs.read_cluster(self.current_cluster, &mut cluster_buf)?;
             
             cluster_buf[offset_in_cluster..offset_in_cluster+to_write]
                 .copy_from_slice(&buf[total_written..total_written+to_write]);
                 
             self.fs.write_cluster(self.current_cluster, &cluster_buf)?;
             
             total_written += to_write;
             self.current_offset += to_write as u64;
             
             // Move to next cluster
             if self.current_offset % cluster_size == 0 && total_written < buf.len() {
                 match self.fs.get_next_cluster(self.current_cluster)? {
                     Some(next) => self.current_cluster = next,
                     None => {
                         let next = self.fs.alloc_cluster(Some(self.current_cluster))?;
                         self.current_cluster = next;
                     }
                 }
             }
        }
        
        if self.current_offset > self.size {
            self.size = self.current_offset;
            // Update directory entry size
            if self.entry_cluster != 0 {
                let mut entry = self.fs.read_entry(self.entry_cluster, self.entry_offset)?;
                entry.size = self.size as u32;
                self.fs.write_entry(self.entry_cluster, self.entry_offset, entry)?;
            }
        }
        
        Ok(total_written)
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
    
    fn readdir(&mut self) -> Result<Option<DirEntry>, &'static str> {
        if !self.is_dir {
            return Err("Not a directory");
        }
        
        loop {
            let mut buf = [0u8; 32];
            match self.read(&mut buf) {
                Ok(32) => {
                    // Check for end of directory
                    if buf[0] == 0x00 { return Ok(None); }
                    
                    // Check for deleted entry
                    if buf[0] == 0xE5 { continue; }
                    
                    let entry = unsafe { core::ptr::read(buf.as_ptr() as *const FatDirEntry) };
                    
                    // Skip LFN and Volume ID
                    if entry.attr == ATTR_LONG_NAME || (entry.attr & ATTR_VOLUME_ID) != 0 {
                        continue;
                    }
                    
                    let name = parse_fat_name(&entry.name);
                    
                    return Ok(Some(DirEntry {
                        name,
                        is_dir: (entry.attr & ATTR_DIRECTORY) != 0,
                        size: entry.size as u64,
                    }));
                },
                Ok(_) => return Ok(None), // Partial read or EOF
                Err(e) => return Err(e),
            }
        }
    }
}
