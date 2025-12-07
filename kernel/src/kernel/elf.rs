//! ELF64 Loader
//!
//! Parses ELF64 binaries and loads them into a UserAddressSpace.

use crate::kernel::memory::paging::{UserAddressSpace, EntryFlags};
use crate::kprintln;

// ═══════════════════════════════════════════════════════════════════════════════
// ELF STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// ELF Header (64-bit)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub magic: [u8; 4],       // 0x7F 'E' 'L' 'F'
    pub class: u8,            // 2 = 64-bit
    pub data: u8,             // 1 = Little Endian
    pub version: u8,          // 1
    pub os_abi: u8,           // 0 = System V
    pub abi_version: u8,
    pub pad: [u8; 7],
    pub type_: u16,           // 2 = Executable
    pub machine: u16,         // 183 = AArch64
    pub version2: u32,        // 1
    pub entry: u64,           // Entry point address
    pub ph_off: u64,          // Program header offset
    pub sh_off: u64,          // Section header offset
    pub flags: u32,
    pub eh_size: u16,
    pub ph_ent_size: u16,
    pub ph_num: u16,
    pub sh_ent_size: u16,
    pub sh_num: u16,
    pub sh_str_ndx: u16,
}

/// Program Header (64-bit)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64ProgramHeader {
    pub type_: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}

// Program Header Types
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;

// Program Header Flags
pub const PF_X: u32 = 1; // Execute
pub const PF_W: u32 = 2; // Write
pub const PF_R: u32 = 4; // Read

// ═══════════════════════════════════════════════════════════════════════════════
// ELF LOADER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct ElfLoader<'a> {
    data: &'a [u8],
    header: &'a Elf64Header,
}

impl<'a> ElfLoader<'a> {
    /// Create a new ELF Loader from a byte slice
    pub fn new(data: &'a [u8]) -> Result<Self, &'static str> {
        if data.len() < core::mem::size_of::<Elf64Header>() {
            return Err("File too small");
        }

        let header = unsafe { &*(data.as_ptr() as *const Elf64Header) };

        // Validate Magic
        if header.magic != [0x7F, b'E', b'L', b'F'] {
            return Err("Invalid ELF Magic");
        }

        // Validate Class (64-bit)
        if header.class != 2 {
            return Err("Not a 64-bit ELF");
        }

        // Validate Endianness (Little)
        if header.data != 1 {
            return Err("Not Little Endian");
        }

        // Validate Machine (AArch64)
        if header.machine != 183 {
            return Err("Not AArch64");
        }

        // Validate Type (Executable)
        if header.type_ != 2 {
            return Err("Not an Executable");
        }

        Ok(Self { data, header })
    }

    /// Get the entry point address
    pub fn entry_point(&self) -> u64 {
        self.header.entry
    }

    /// Load segments into a UserAddressSpace
    pub fn load(&self, vmm: &mut UserAddressSpace) -> Result<(), &'static str> {
        let ph_off = self.header.ph_off as usize;
        let ph_num = self.header.ph_num as usize;
        let ph_ent_size = self.header.ph_ent_size as usize;

        if ph_off + ph_num * ph_ent_size > self.data.len() {
            return Err("Program Headers out of bounds");
        }

        for i in 0..ph_num {
            let offset = ph_off + i * ph_ent_size;
            let ph = unsafe { &*(self.data.as_ptr().add(offset) as *const Elf64ProgramHeader) };

            if ph.type_ == PT_LOAD {
                self.load_segment(ph, vmm)?;
            }
        }

        Ok(())
    }

    fn load_segment(&self, ph: &Elf64ProgramHeader, vmm: &mut UserAddressSpace) -> Result<(), &'static str> {
        if ph.mem_size == 0 {
            return Ok(());
        }

        let vaddr = ph.vaddr;
        let file_size = ph.file_size;
        let mem_size = ph.mem_size;
        let flags = ph.flags;
        
        let flag_str = match flags {
            5 => "R-X", // Code
            6 => "RW-", // Data
            4 => "R--", // RO Data
            _ => "???",
        };

        kprintln!("[ELF] Loading Segment: VAddr={:#x}, FileSize={:#x}, MemSize={:#x}, Flags={:#x} ({})",
            vaddr, file_size, mem_size, flags, flag_str);

        // Calculate pages needed
        // We need to map from vaddr to vaddr + mem_size
        // We allocate physical pages and copy data.

        let start_addr = ph.vaddr;
        let end_addr = start_addr + ph.mem_size;
        
        let start_page = start_addr & !0xFFF;
        let end_page = (end_addr + 0xFFF) & !0xFFF;
        let page_count = ((end_page - start_page) / 4096) as usize;

        // Allocate pages
        // We use specific user page allocation for better accounting/separation
        let pages_ptr = unsafe { crate::kernel::memory::alloc_user_pages(page_count) }
            .ok_or("Out of memory for segment")?;
        
        let phys_base = pages_ptr.as_ptr() as u64;

        // Zero the memory first (for BSS)
        unsafe {
            core::ptr::write_bytes(pages_ptr.as_ptr(), 0, page_count * 4096);
        }

        // Copy file data
        if ph.file_size > 0 {
            let file_offset = ph.offset as usize;
            if file_offset + ph.file_size as usize > self.data.len() {
                return Err("Segment file data out of bounds");
            }
            
            let src = &self.data[file_offset..file_offset + ph.file_size as usize];
            
            // Calculate offset within the first page
            let page_offset = (start_addr & 0xFFF) as usize;
            
            unsafe {
                let dest = pages_ptr.as_ptr().add(page_offset);
                core::ptr::copy_nonoverlapping(src.as_ptr(), dest, src.len());
            }
        }

        // Map pages
        // Determine flags
        let mut flags = EntryFlags::ATTR_NORMAL | EntryFlags::AP_RW_USER | EntryFlags::SH_INNER;
        if (ph.flags & PF_X) == 0 {
            flags |= EntryFlags::UXN; // Not executable
        }
        if (ph.flags & PF_W) == 0 {
            // Read-only?
            // AP_RO_USER = 3 << 6
            // AP_RW_USER = 1 << 6
            // We default to RW for now because we just wrote to it!
            // Ideally we map RW, write data, then remap RO.
            // But for simplicity, let's keep it RW for now or handle it carefully.
            // If we map RO now, we can't write to it via the direct map?
            // Wait, we are writing to the PHYSICAL address via the linear map (identity map in kernel).
            // The user mapping is separate.
            // So we CAN map it RO for the user.
            flags = EntryFlags::ATTR_NORMAL | EntryFlags::AP_RO_USER | EntryFlags::SH_INNER;
            if (ph.flags & PF_X) == 0 {
                flags |= EntryFlags::UXN;
            }
        }

        // Map into User VMM
        let mut curr_virt = start_page;
        let mut curr_phys = phys_base;
        
        for _ in 0..page_count {
            vmm.map_user(curr_virt, curr_phys, 4096)?;
            curr_virt += 4096;
            curr_phys += 4096;
        }

        Ok(())
    }
}
