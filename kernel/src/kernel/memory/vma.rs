//! Virtual Memory Area (VMA) Management
//!
//! Tracks memory regions for user processes to enforce permissions and security.

use alloc::vec::Vec;

/// Memory Permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmaPerms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl VmaPerms {
    pub const R: Self = Self { read: true, write: false, execute: false };
    pub const RW: Self = Self { read: true, write: true, execute: false };
    pub const RX: Self = Self { read: true, write: false, execute: true };
    pub const RWX: Self = Self { read: true, write: true, execute: true };
    
    pub fn new(read: bool, write: bool, execute: bool) -> Self {
        Self { read, write, execute }
    }
}

/// VMA Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmaFlags {
    pub private: bool,
    pub anonymous: bool,
    pub fixed: bool,
}

impl VmaFlags {
    pub const DEFAULT: Self = Self { private: true, anonymous: true, fixed: false };
}

/// Virtual Memory Area
#[derive(Debug, Clone)]
pub struct VMA {
    pub start: u64,
    pub end: u64,
    pub perms: VmaPerms,
    pub flags: VmaFlags,
}

impl VMA {
    pub fn new(start: u64, size: u64, perms: VmaPerms, flags: VmaFlags) -> Self {
        Self {
            start,
            end: start + size,
            perms,
            flags,
        }
    }
    
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }
    
    pub fn overlaps(&self, start: u64, end: u64) -> bool {
        self.start < end && start < self.end
    }
}

/// VMA Manager for a Process
#[derive(Debug, Clone)]
pub struct VmaManager {
    pub vmas: Vec<VMA>,
}

impl VmaManager {
    pub fn new() -> Self {
        Self {
            vmas: Vec::new(),
        }
    }
    
    /// Add a VMA
    pub fn add_vma(&mut self, vma: VMA) -> bool {
        // Check for overlaps
        for existing in &self.vmas {
            if existing.overlaps(vma.start, vma.end) {
                return false;
            }
        }
        self.vmas.push(vma);
        self.vmas.sort_by_key(|v| v.start);
        true
    }
    
    /// Find a VMA containing the address
    pub fn find_vma(&self, addr: u64) -> Option<&VMA> {
        for vma in &self.vmas {
            if vma.contains(addr) {
                return Some(vma);
            }
        }
        None
    }
    
    /// Check permissions for a range
    pub fn check_perms(&self, start: u64, len: u64, perms: VmaPerms) -> bool {
        let end = start + len;
        
        // Simple check: Range must be fully contained in ONE VMA
        // (Real OS might allow spanning multiple contiguous VMAs)
        if let Some(vma) = self.find_vma(start) {
            if vma.end >= end {
                // Check permissions
                if perms.read && !vma.perms.read { return false; }
                if perms.write && !vma.perms.write { return false; }
                if perms.execute && !vma.perms.execute { return false; }
                return true;
            }
        }
        
        false
    }
    
    /// Map memory (mmap)
    /// Finds a free region of size `len` and adds a VMA.
    pub fn mmap(&mut self, len: u64, perms: VmaPerms, flags: VmaFlags) -> Option<u64> {
        // Simple allocator: Find first gap
        // User space starts at 0x1000 (skip null page)
        // Ends at 0x0000_7FFF_FFFF_FFFF (User space limit)
        
        let mut start = 0x100000; // Start at 1MB to avoid low memory conflicts
        let align = 4096;
        
        // Align length to page size
        let len = (len + align - 1) & !(align - 1);
        
        for vma in &self.vmas {
            if start + len <= vma.start {
                // Found a gap before this VMA
                let new_vma = VMA::new(start, len, perms, flags);
                if self.add_vma(new_vma) {
                    return Some(start);
                } else {
                    // Should not happen if logic is correct
                    return None;
                }
            }
            // Move start to end of current VMA (aligned)
            start = (vma.end + align - 1) & !(align - 1);
        }
        
        // Check if fits after last VMA
        if start + len < 0x0000_7FFF_FFFF_FFFF {
             let new_vma = VMA::new(start, len, perms, flags);
             if self.add_vma(new_vma) {
                 return Some(start);
             }
        }
        
        None // OOM
    }
    
    /// Unmap memory (munmap)
    pub fn munmap(&mut self, start: u64, len: u64) -> Option<VMA> {
        let end = start + len;
        
        // Find the VMA that contains the range
        // Note: This assumes the range is fully contained in ONE VMA.
        let idx = self.vmas.iter().position(|v| v.start <= start && v.end >= end)?;
        
        // We need to clone the VMA properties to create the return value and potentially the split part
        let original_perms = self.vmas[idx].perms;
        let original_flags = self.vmas[idx].flags;
        let unmapped_vma = VMA::new(start, len, original_perms, original_flags);
        
        let vma = &mut self.vmas[idx];
        
        if vma.start == start && vma.end == end {
            // Case 1: Exact match - Remove the VMA
            self.vmas.remove(idx);
        } else if vma.start == start {
            // Case 2: Prefix removal - Shrink from start
            vma.start = end;
        } else if vma.end == end {
            // Case 3: Suffix removal - Shrink from end
            vma.end = start;
        } else {
            // Case 4: Middle removal (Split)
            // Current VMA becomes the left part
            let right_start = end;
            let right_len = vma.end - right_start;
            
            vma.end = start; // Shrink current to be left part
            
            // Create right part
            let right_vma = VMA::new(right_start, right_len, original_perms, original_flags);
            
            // Insert right part after the current one
            self.vmas.insert(idx + 1, right_vma);
        }
        
        Some(unmapped_vma)
    }
}
