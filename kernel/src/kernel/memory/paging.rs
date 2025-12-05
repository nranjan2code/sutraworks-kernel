//! ARM64 Virtual Memory System Architecture (VMSA) Paging
//!
//! Handles translation tables (page tables) for 4KB pages with 4 levels of translation.
//!
//! # Structure
//! - Level 0: 512GB range (one entry covers Level 1 table)
//! - Level 1: 1GB range (Block or Table)
//! - Level 2: 2MB range (Block or Table)
//! - Level 3: 4KB range (Page)

use core::ops::{BitOr, BitOrAssign, BitAnd, BitAndAssign, Not};

/// A 64-bit Page Table Entry (Descriptor)
///
/// Can represent:
/// - A Table Descriptor (pointing to next level table)
/// - A Block Descriptor (mapping a 1GB or 2MB block)
/// - A Page Descriptor (mapping a 4KB page)
/// - An Invalid Entry
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// Create a new empty (invalid) entry
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create an entry from a raw u64
    pub const fn from_raw(val: u64) -> Self {
        Self(val)
    }

    /// Get the raw u64 value
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Check if the entry is valid (bit 0 is set)
    pub fn is_valid(&self) -> bool {
        self.0 & EntryFlags::VALID.bits() != 0
    }

    /// Check if this is a Table descriptor (pointing to another level)
    /// Note: For L3, this bit means 'Page', but we use the same flag for simplicity
    pub fn is_table(&self) -> bool {
        self.0 & EntryFlags::TABLE.bits() != 0
    }

    /// Get the physical address this entry points to
    /// Mask out the attributes (lower 12 bits) and upper attributes (bits 48+)
    pub fn address(&self) -> u64 {
        self.0 & 0x0000_FFFF_FFFF_F000
    }

    /// Set the address and flags
    pub fn set(&mut self, addr: u64, flags: EntryFlags) {
        // Address must be page aligned
        assert!(addr & 0xFFF == 0, "Address must be 4KB aligned");
        self.0 = addr | flags.bits();
    }
}

/// A Page Table (512 entries of 8 bytes each = 4KB)
///
/// Must be aligned to 4KB boundary.
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry(0); 512],
        }
    }

    /// Clear the table (zero all entries)
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            *entry = PageTableEntry::new();
        }
    }

    /// Check if the table is empty (no valid entries)
    pub fn is_empty(&self) -> bool {
        for entry in self.entries.iter() {
            if entry.is_valid() {
                return false;
            }
        }
        true
    }
}

/// Bitflags for Page Table Entry attributes
#[derive(Clone, Copy, Debug)]
pub struct EntryFlags(u64);

impl EntryFlags {
    // --- Core Types ---
    pub const NONE: Self = Self(0);
    pub const VALID: Self = Self(1 << 0);
    pub const TABLE: Self = Self(1 << 1); // Also 'Page' for L3

    // --- Memory Attributes (Index into MAIR) ---
    /// Index 0 in MAIR (Device-nGnRnE)
    pub const ATTR_DEVICE: Self = Self(0 << 2);
    /// Index 1 in MAIR (Normal Memory Write-Back)
    pub const ATTR_NORMAL: Self = Self(1 << 2);
    /// Index 2 in MAIR (Normal Memory Non-Cacheable)
    pub const ATTR_NC: Self = Self(2 << 2);

    // --- Access Permissions (AP) ---
    // AP[1] = RO/RW, AP[0] = EL1/EL0
    /// Read-Write, EL1 only (00)
    pub const AP_RW_EL1: Self = Self(0 << 6);
    /// Read-Write, EL1 & EL0 (01)
    pub const AP_RW_USER: Self = Self(1 << 6);
    /// Read-Only, EL1 only (10)
    pub const AP_RO_EL1: Self = Self(2 << 6);
    /// Read-Only, EL1 & EL0 (11)
    pub const AP_RO_USER: Self = Self(3 << 6);

    // --- Shareability (SH) ---
    /// Non-shareable (00)
    pub const SH_NONE: Self = Self(0 << 8);
    /// Outer Shareable (10)
    pub const SH_OUTER: Self = Self(2 << 8);
    /// Inner Shareable (11)
    pub const SH_INNER: Self = Self(3 << 8);

    // --- Access Flag (AF) ---
    /// Access Flag (must be 1 for access to be allowed without fault)
    pub const AF: Self = Self(1 << 10);

    // --- Execution Permissions ---
    /// Privileged Execute Never (EL1 cannot execute)
    pub const PXN: Self = Self(1 << 53);
    /// Unprivileged Execute Never (EL0 cannot execute)
    pub const UXN: Self = Self(1 << 54);

    pub const fn bits(&self) -> u64 {
        self.0
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for EntryFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for EntryFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for EntryFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for EntryFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for EntryFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VIRTUAL MEMORY MANAGER
// ═══════════════════════════════════════════════════════════════════════════════

use core::ptr::NonNull;
use crate::kernel::sync::SpinLock;

/// Virtual Memory Manager
pub struct VMM {
    root_table: NonNull<PageTable>,
}

// SAFETY: VMM owns the page table and we protect access via SpinLock in global instance
unsafe impl Send for VMM {}

impl VMM {
    /// Create a new VMM with a new root table
    pub fn new() -> Option<Self> {
        // Allocate root table (Level 0)
        // We use the global page allocator
        let page = unsafe { super::alloc_pages(1)? };
        let table = page.as_ptr() as *mut PageTable;
        
        unsafe {
            (*table).zero();
        }
        
        Some(VMM {
            root_table: unsafe { NonNull::new_unchecked(table) },
        })
    }
    
    /// Get the physical address of the root table
    pub fn root_address(&self) -> u64 {
        self.root_table.as_ptr() as u64
    }
    
    /// Map a virtual page to a physical frame
    /// 
    /// # Arguments
    /// * `virt_addr` - Virtual address (must be 4KB aligned)
    /// * `phys_addr` - Physical address (must be 4KB aligned)
    /// * `flags` - Access permissions and attributes
    /// Map a virtual page to a physical frame
    /// 
    /// # Arguments
    /// * `virt_addr` - Virtual address (must be 4KB aligned)
    /// * `phys_addr` - Physical address (must be 4KB aligned)
    /// * `flags` - Access permissions and attributes
    pub unsafe fn map_page(&mut self, virt_addr: u64, phys_addr: u64, flags: EntryFlags) -> Result<(), &'static str> {
        if virt_addr & 0xFFF != 0 || phys_addr & 0xFFF != 0 {
            return Err("Addresses must be page aligned");
        }
        
        // Indexes for each level
        let l0_idx = (virt_addr >> 39) & 0x1FF;
        let l1_idx = (virt_addr >> 30) & 0x1FF;
        let l2_idx = (virt_addr >> 21) & 0x1FF;
        let l3_idx = (virt_addr >> 12) & 0x1FF;
        
        let root = self.root_table.as_mut();
        
        // Level 0 -> Level 1
        let l1_table = self.get_next_table(&mut root.entries[l0_idx as usize])?;
        
        // Level 1 -> Level 2
        let l2_table = self.get_next_table(&mut l1_table.entries[l1_idx as usize])?;
        
        // Level 2 -> Level 3
        let l3_table = self.get_next_table(&mut l2_table.entries[l2_idx as usize])?;
        
        // Level 3 Entry (The Page)
        let entry = &mut l3_table.entries[l3_idx as usize];
        
        // Overwrite is allowed if we are updating permissions, but warn if mapping to different phys
        if entry.is_valid() {
            let old_phys = entry.address();
            if old_phys != phys_addr {
                // return Err("Page already mapped to different physical address");
                // Allow remapping for now
            }
        }
        
        entry.set(phys_addr, flags | EntryFlags::VALID | EntryFlags::TABLE | EntryFlags::AF);
        
        Ok(())
    }

    /// Unmap a virtual page and return the physical address if it was mapped
    pub unsafe fn unmap_page(&mut self, virt_addr: u64) -> Result<Option<u64>, &'static str> {
        if virt_addr & 0xFFF != 0 {
            return Err("Address must be page aligned");
        }

        let l0_idx = (virt_addr >> 39) & 0x1FF;
        let l1_idx = (virt_addr >> 30) & 0x1FF;
        let l2_idx = (virt_addr >> 21) & 0x1FF;
        let l3_idx = (virt_addr >> 12) & 0x1FF;

        let root = self.root_table.as_mut();

        // Traverse, keeping references to tables to check emptiness later
        let l1_entry = &mut root.entries[l0_idx as usize];
        if !l1_entry.is_valid() { return Ok(None); }
        let l1_table = &mut *(l1_entry.address() as *mut PageTable);

        let l2_entry = &mut l1_table.entries[l1_idx as usize];
        if !l2_entry.is_valid() { return Ok(None); }
        let l2_table = &mut *(l2_entry.address() as *mut PageTable);

        let l3_entry = &mut l2_table.entries[l2_idx as usize];
        if !l3_entry.is_valid() { return Ok(None); }
        let l3_table = &mut *(l3_entry.address() as *mut PageTable);

        let entry = &mut l3_table.entries[l3_idx as usize];
        if !entry.is_valid() { return Ok(None); }
        
        let phys = entry.address();
        *entry = PageTableEntry::new(); // Clear the page entry

        // Check if L3 table is empty and free it
        if l3_table.is_empty() {
            let l3_phys = l3_entry.address();
            *l3_entry = PageTableEntry::new(); // Remove from L2
            
            if let Some(ptr) = NonNull::new(l3_phys as *mut u8) {
                super::free_pages(ptr, 1);
            }
            
            // Check if L2 table is empty
            if l2_table.is_empty() {
                let l2_phys = l2_entry.address();
                *l2_entry = PageTableEntry::new(); // Remove from L1
                
                if let Some(ptr) = NonNull::new(l2_phys as *mut u8) {
                    super::free_pages(ptr, 1);
                }
                
                // Check if L1 table is empty
                if l1_table.is_empty() {
                    let l1_phys = l1_entry.address();
                    *l1_entry = PageTableEntry::new(); // Remove from L0
                    
                    if let Some(ptr) = NonNull::new(l1_phys as *mut u8) {
                        super::free_pages(ptr, 1);
                    }
                }
            }
        }
        
        Ok(Some(phys))
    }

    /// Check if a virtual address is mapped
    pub fn is_mapped(&self, virt_addr: u64) -> bool {
        let l0_idx = (virt_addr >> 39) & 0x1FF;
        let l1_idx = (virt_addr >> 30) & 0x1FF;
        let l2_idx = (virt_addr >> 21) & 0x1FF;
        let l3_idx = (virt_addr >> 12) & 0x1FF;

        unsafe {
            let root = self.root_table.as_ref();
            
            let l1_entry = &root.entries[l0_idx as usize];
            if !l1_entry.is_valid() { return false; }
            let l1_table = &*(l1_entry.address() as *const PageTable);

            let l2_entry = &l1_table.entries[l1_idx as usize];
            if !l2_entry.is_valid() { return false; }
            if !l2_entry.is_table() { return true; } // Block mapping (Huge Page)
            let l2_table = &*(l2_entry.address() as *const PageTable);

            let l3_entry = &l2_table.entries[l2_idx as usize];
            if !l3_entry.is_valid() { return false; }
            if !l3_entry.is_table() { return true; } // Block mapping
            let l3_table = &*(l3_entry.address() as *const PageTable);

            let entry = &l3_table.entries[l3_idx as usize];
            entry.is_valid()
        }
    }

    /// Translate a virtual address to physical address
    pub fn translate(&self, virt_addr: u64) -> Option<u64> {
        let l0_idx = (virt_addr >> 39) & 0x1FF;
        let l1_idx = (virt_addr >> 30) & 0x1FF;
        let l2_idx = (virt_addr >> 21) & 0x1FF;
        let l3_idx = (virt_addr >> 12) & 0x1FF;
        let offset = virt_addr & 0xFFF;

        unsafe {
            let root = self.root_table.as_ref();
            
            let l1_entry = &root.entries[l0_idx as usize];
            if !l1_entry.is_valid() { return None; }
            let l1_table = &*(l1_entry.address() as *const PageTable);

            let l2_entry = &l1_table.entries[l1_idx as usize];
            if !l2_entry.is_valid() { return None; }
            if !l2_entry.is_table() { 
                // Block mapping (1GB)
                return Some(l2_entry.address() + (virt_addr & 0x3FFF_FFFF));
            }
            let l2_table = &*(l2_entry.address() as *const PageTable);

            let l3_entry = &l2_table.entries[l2_idx as usize];
            if !l3_entry.is_valid() { return None; }
            if !l3_entry.is_table() { 
                // Block mapping (2MB)
                return Some(l3_entry.address() + (virt_addr & 0x1F_FFFF));
            }
            let l3_table = &*(l3_entry.address() as *const PageTable);

            let entry = &l3_table.entries[l3_idx as usize];
            if !entry.is_valid() { return None; }
            
            Some(entry.address() + offset)
        }
    }
    
    /// Map a 2MB block (Huge Page)
    pub unsafe fn map_block_2mb(&mut self, virt_addr: u64, phys_addr: u64, flags: EntryFlags) -> Result<(), &'static str> {
        if virt_addr & 0x1F_FFFF != 0 || phys_addr & 0x1F_FFFF != 0 {
            return Err("Addresses must be 2MB aligned");
        }
        
        let l0_idx = (virt_addr >> 39) & 0x1FF;
        let l1_idx = (virt_addr >> 30) & 0x1FF;
        let l2_idx = (virt_addr >> 21) & 0x1FF;
        
        let root = self.root_table.as_mut();
        
        // Level 0 -> Level 1
        let l1_table = self.get_next_table(&mut root.entries[l0_idx as usize])?;
        
        // Level 1 -> Level 2
        let l2_table = self.get_next_table(&mut l1_table.entries[l1_idx as usize])?;
        
        // Level 2 Entry (The Block)
        let entry = &mut l2_table.entries[l2_idx as usize];
        
        // Block descriptor: VALID (1) | AF (1) | Not TABLE (0)
        // Note: EntryFlags::TABLE is bit 1. We want it CLEARED for Block.
        // But we pass 'flags' which might have attributes.
        // We need to ensure TABLE bit is cleared.
        let block_flags = (flags | EntryFlags::VALID | EntryFlags::AF) & !EntryFlags::TABLE;
        
        entry.set(phys_addr, block_flags);
        
        Ok(())
    }

    /// Identity map a range of memory
    pub unsafe fn identity_map(&mut self, start: u64, end: u64, flags: EntryFlags) -> Result<(), &'static str> {
        let mut addr = start & !0xFFF;
        let end_aligned = (end + 0xFFF) & !0xFFF;
        
        while addr < end_aligned {
            // Check if we can map 2MB block
            // 1. Address is 2MB aligned
            // 2. We have at least 2MB left
            if addr & 0x1F_FFFF == 0 && addr + 0x20_0000 <= end_aligned {
                self.map_block_2mb(addr, addr, flags)?;
                addr += 0x20_0000; // 2MB
            } else {
                self.map_page(addr, addr, flags)?;
                addr += 4096;
            }
        }
        
        Ok(())
    }
    
    /// Helper to get the next level table, allocating it if necessary
    unsafe fn get_next_table(&self, entry: &mut PageTableEntry) -> Result<&mut PageTable, &'static str> {
        if entry.is_valid() {
            if !entry.is_table() {
                return Err("Huge pages not supported yet");
            }
            // Entry points to physical address of next table
            // In identity mapping/early boot, phys == virt.
            // We assume kernel page tables are always identity mapped.
            let table_addr = entry.address();
            Ok(&mut *(table_addr as *mut PageTable))
        } else {
            // Allocate new table
            let page = super::alloc_pages(1).ok_or("Out of memory for page table")?;
            let table = page.as_ptr() as *mut PageTable;
            (*table).zero();
            
            // Link it
            entry.set(table as u64, EntryFlags::VALID | EntryFlags::TABLE | EntryFlags::AF);
            
            Ok(&mut *table)
        }
    }
}

/// User Address Space
/// 
/// Represents a process's isolated view of memory.
/// Contains a pointer to its own Page Table (TTBR0).
pub struct UserAddressSpace {
    vmm: VMM,
}

impl UserAddressSpace {
    /// Create a new User Address Space
    /// 
    /// This creates a new Page Table and maps the Kernel into it (Privileged Access Only).
    /// This ensures the kernel can run, but the user cannot touch it.
    pub fn new() -> Option<Self> {
        let mut vmm = VMM::new()?;
        
        // Map Kernel as Privileged Only (EL1)
        // We copy the mappings from the global kernel VMM? 
        // Or just re-map the standard regions.
        // Re-mapping is safer and cleaner.
        
        unsafe {
            // 1. Identity map Kernel Code/Data (Normal Memory) - EL1 ONLY
            vmm.identity_map(0, 0x2_0000_0000, EntryFlags::ATTR_NORMAL | EntryFlags::AP_RW_EL1 | EntryFlags::SH_INNER)
                .ok()?;
                
            // 2. Map Peripherals (Device Memory) - EL1 ONLY
            vmm.identity_map(0x10_0000_0000, 0x10_0100_0000, EntryFlags::ATTR_DEVICE | EntryFlags::AP_RW_EL1 | EntryFlags::PXN | EntryFlags::UXN)
                .ok()?;
                
            // For QEMU tests
            #[cfg(test)]
            {
                vmm.identity_map(0x0800_0000, 0x1000_0000, EntryFlags::ATTR_DEVICE | EntryFlags::AP_RW_EL1 | EntryFlags::PXN | EntryFlags::UXN)
                    .ok()?;
                vmm.identity_map(0x4000_0000, 0x8000_0000, EntryFlags::ATTR_NORMAL | EntryFlags::AP_RW_EL1 | EntryFlags::SH_INNER)
                    .ok()?;
            }
        }
        
        Some(UserAddressSpace { vmm })
    }
    
    /// Get the physical address of the root table (for TTBR0)
    pub fn table_base(&self) -> u64 {
        self.vmm.root_address()
    }
    
    /// Map a user memory region
    pub fn map_user(&mut self, virt: u64, phys: u64, size: usize) -> Result<(), &'static str> {
        let flags = EntryFlags::ATTR_NORMAL | EntryFlags::AP_RW_USER | EntryFlags::SH_INNER;
        let end = virt + size as u64;
        let mut v = virt;
        let mut p = phys;
        
        while v < end {
            unsafe { 
                if let Err(e) = self.vmm.map_page(v, p, flags) {
                    crate::kprintln!("[MEM] map_page failed at virt={:#x} phys={:#x}: {}", v, p, e);
                    return Err(e);
                }
            }
            v += 4096;
            p += 4096;
        }
        Ok(())
    }

    /// Check if a virtual address is mapped
    pub fn is_mapped(&self, virt_addr: u64) -> bool {
        self.vmm.is_mapped(virt_addr)
    }

    /// Translate a virtual address
    pub fn translate(&self, virt_addr: u64) -> Option<u64> {
        self.vmm.translate(virt_addr)
    }

    /// Unmap a virtual page
    pub fn unmap_page(&mut self, virt_addr: u64) -> Result<Option<u64>, &'static str> {
        unsafe { self.vmm.unmap_page(virt_addr) }
    }
}

/// Global Kernel VMM
pub static KERNEL_VMM: SpinLock<Option<VMM>> = SpinLock::new(None);

/// Initialize the kernel VMM
pub unsafe fn init() {
    let mut vmm = KERNEL_VMM.lock();
    *vmm = VMM::new();
    
    if let Some(vmm) = vmm.as_mut() {
        crate::kprintln!("[MEM] Initializing VMM...");
        

            // 1. Identity map Kernel Code/Data (Normal Memory)
            // From 0x0 to 0x2_0000_0000 (8GB)
            // This covers the entire physical RAM of the Pi 5
        if crate::dtb::machine_type() == crate::dtb::MachineType::RaspberryPi5 {
            // 1. Identity Map Kernel Code/Data (Normal Memory)
            // Map 0x0000_0000 to 0x2_0000_0000 (8GB) - Covering all RAM
            vmm.identity_map(0, 0x2_0000_0000, EntryFlags::ATTR_NORMAL | EntryFlags::AP_RW_EL1 | EntryFlags::SH_INNER)
                .expect("Failed to map kernel");
                
            // 1b. Remap DMA Region as Non-Cacheable
            // We need to ensure DMA buffers are not cached so hardware sees writes immediately.
            let (dma_start, dma_end) = super::dma_region();
            if dma_end > dma_start {
                crate::kprintln!("[MEM] Remapping DMA region {:#x}-{:#x} as Non-Cacheable", dma_start, dma_end);
                vmm.identity_map(dma_start as u64, dma_end as u64, EntryFlags::ATTR_NC | EntryFlags::AP_RW_EL1 | EntryFlags::SH_INNER)
                    .expect("Failed to remap DMA");
            }
                
            // 2. Map Peripherals (Device Memory)
            // BCM2712 Peripherals start at 0x10_0000_0000 (outside 32-bit range)
            // But we are in 64-bit mode, so it's fine.
            // Map 0x10_0000_0000 to 0x10_0100_0000 (16MB)
            vmm.identity_map(0x10_0000_0000, 0x10_0100_0000, EntryFlags::ATTR_DEVICE | EntryFlags::AP_RW_EL1 | EntryFlags::PXN | EntryFlags::UXN)
                .expect("Failed to map peripherals");
        }

        if crate::dtb::machine_type() == crate::dtb::MachineType::QemuVirt {
            // Map QEMU virt peripherals (UART at 0x09000000)
            // Map 0x08000000 to 0x10000000 (128MB) covering GIC and UART
            vmm.identity_map(0x0800_0000, 0x1000_0000, EntryFlags::ATTR_DEVICE | EntryFlags::AP_RW_EL1 | EntryFlags::PXN | EntryFlags::UXN)
                .expect("Failed to map virt peripherals");
                
            // Map RAM (Normal Memory) - 0x4000_0000 to 0x8000_0000 (1GB)
            vmm.identity_map(0x4000_0000, 0x8000_0000, EntryFlags::ATTR_NORMAL | EntryFlags::AP_RW_EL1 | EntryFlags::SH_INNER)
                .expect("Failed to map RAM");

            // Map PCIe ECAM (0x3f00_0000) - 16MB
            vmm.identity_map(0x3f00_0000, 0x4000_0000, EntryFlags::ATTR_DEVICE | EntryFlags::AP_RW_EL1 | EntryFlags::PXN | EntryFlags::UXN)
                .expect("Failed to map PCIe ECAM");
        }
            
        // 3. Configure MAIR
        // Attr0 = Device-nGnRnE (0x00)
        // Attr1 = Normal Inner/Outer WB RW-Allocate (0xFF)
        // Attr2 = Normal Inner/Outer Non-Cacheable (0x44)
        let mair = (0x00 << 0) | (0xFF << 8) | (0x44 << 16);
        crate::arch::set_mair(mair);
        
        // 4. Configure TCR
        // T0SZ = 16 (48-bit user space)
        // T1SZ = 16 (48-bit kernel space)
        // TG0 = 00 (4KB granule)
        // TG1 = 10 (4KB granule)
        // IPS = 001 (36-bit PA) - Pi 5 supports more but start safe
        // SH = 11 (Inner Shareable)
        // Cacheability = 01 (WB/WA)
        let t0sz = (64 - 48) << 0;
        let t1sz = (64 - 48) << 16;
        let tg0 = 0 << 14; // 4KB
        let tg1 = 2 << 30; // 4KB
        let ips = 1 << 32; // 36-bit PA (64GB)
        let flags = t0sz | t1sz | tg0 | tg1 | ips;
        crate::arch::set_tcr(flags);
        
        // 5. Set TTBR0 and TTBR1
        let root_phys = vmm.root_address();
        crate::arch::set_ttbr0(root_phys);
        crate::arch::set_ttbr1(root_phys); // Use same table for now (identity map)
        
        // 6. Enable MMU
        // SCTLR_EL1.M = 1
        // SCTLR_EL1.C = 1 (Cache enable)
        // SCTLR_EL1.I = 1 (Instruction cache enable)
        crate::arch::tlb_invalidate_all();
        let sctlr = crate::arch::get_sctlr();
        crate::arch::set_sctlr(sctlr | 1 | (1 << 2) | (1 << 12));
        
        crate::kprintln!("[MEM] MMU Enabled!");
        crate::kprintln!("[MEM] VMM Done");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_flags() {
        let flags = EntryFlags::VALID | EntryFlags::TABLE;
        assert!(flags.contains(EntryFlags::VALID));
        assert!(flags.contains(EntryFlags::TABLE));
        assert!(!flags.contains(EntryFlags::PXN));
        
        let val = flags.bits();
        assert_eq!(val, 3); // 1 | 2
    }

    #[test]
    fn test_entry_address() {
        let mut entry = PageTableEntry::new();
        let addr = 0x1234_5000;
        let flags = EntryFlags::VALID;
        
        entry.set(addr, flags);
        
        assert!(entry.is_valid());
        assert_eq!(entry.address(), addr);
        
        // Check that flags didn't mess up address
        assert_eq!(entry.as_u64() & 0xFFF, 1);
    }
}
