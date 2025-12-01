//! RamDisk Driver
//!
//! Provides access to the memory region where the bootloader loads the
//! initial RAM filesystem (initramfs/ramfsfile).

/// The physical address where the RamDisk is loaded.
/// This must match `ramfsaddr` in config.txt.
pub const RAMDISK_BASE: usize = 0x2000_0000;

/// The maximum size of the RamDisk (e.g., 64MB).
pub const RAMDISK_SIZE: usize = 64 * 1024 * 1024;

/// Get a slice representing the RamDisk memory.
///
/// # Safety
/// Caller must ensure the memory region is valid and not overlapping with kernel code/heap.
pub unsafe fn get_slice() -> &'static [u8] {
    core::slice::from_raw_parts(RAMDISK_BASE as *const u8, RAMDISK_SIZE)
}
