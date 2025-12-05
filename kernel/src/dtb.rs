//! Device Tree Blob (DTB) Parser
//!
//! Minimal parser to read the machine compatible string from the FDT.

use core::slice;
use core::str;

#[cfg(not(feature = "test_mocks"))]
extern "C" {
    static __dtb_ptr: u64;
}

#[cfg(feature = "test_mocks")]
#[no_mangle]
static __dtb_ptr: u64 = 0;

/// Machine Type detected from DTB
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MachineType {
    Unknown,
    RaspberryPi5, // bcm2712
    QemuVirt,     // linux,dummy-virt or similar
}

/// Get the detected machine type
pub fn machine_type() -> MachineType {
    let dtb_addr = unsafe { __dtb_ptr };
    
    if dtb_addr == 0 {
        // QEMU 'virt' machine with -kernel does not seem to pass the DTB pointer in x0
        // (or at least not where we expect it).
        // Since Raspberry Pi 5 bootloader definitely passes it, we can safely assume
        // that if x0 is 0, we are running on QEMU.
        return MachineType::QemuVirt;
    }

    // Validate FDT Header Magic (0xd00dfeed - big endian)
    let magic = unsafe { *(dtb_addr as *const u32) };
    if u32::from_be(magic) != 0xd00dfeed {
        return MachineType::Unknown;
    }

    // Read off_struct (offset to structure block)
    let off_struct = unsafe { u32::from_be(*(dtb_addr as *const u32).offset(2)) };
    
    // Read off_strings (offset to strings block)
    let off_strings = unsafe { u32::from_be(*(dtb_addr as *const u32).offset(3)) };

    // Iterate structure block to find root node properties
    let mut struct_ptr = (dtb_addr + off_struct as u64) as *const u32;
    let strings_ptr = (dtb_addr + off_strings as u64) as *const u8;

    unsafe {
        while *struct_ptr != 9 { // FDT_END
            let tag = u32::from_be(*struct_ptr);
            struct_ptr = struct_ptr.add(1);

            match tag {
                1 => { // FDT_BEGIN_NODE
                    // Skip name (null terminated, aligned to 4 bytes)
                    let name_ptr = struct_ptr as *const u8;
                    let mut len = 0;
                    while *name_ptr.add(len) != 0 {
                        len += 1;
                    }
                    len += 1; // null byte
                    // Align to 4 bytes
                    let align = (len + 3) & !3;
                    struct_ptr = (name_ptr.add(align)) as *const u32;
                }
                2 => { // FDT_END_NODE
                    // Nothing to do
                }
                3 => { // FDT_PROP
                    let len = u32::from_be(*struct_ptr);
                    let nameoff = u32::from_be(*struct_ptr.add(1));
                    struct_ptr = struct_ptr.add(2);

                    // Check property name
                    let name_cstr = strings_ptr.add(nameoff as usize);
                    let name = get_str(name_cstr);

                    if name == "compatible" {
                        // Check value
                        let value_ptr = struct_ptr as *const u8;
                        let value = get_str(value_ptr);
                        
                        if value.contains("bcm2712") {
                            return MachineType::RaspberryPi5;
                        } else if value.contains("virt") {
                            return MachineType::QemuVirt;
                        }
                    }

                    // Skip value, aligned to 4 bytes
                    let align = (len as usize + 3) & !3;
                    struct_ptr = (struct_ptr as *const u8).add(align) as *const u32;
                }
                4 => { // FDT_NOP
                    // Ignore
                }
                _ => break,
            }
        }
    }

    MachineType::Unknown
}

unsafe fn get_str(ptr: *const u8) -> &'static str {
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    str::from_utf8_unchecked(slice::from_raw_parts(ptr, len))
}
