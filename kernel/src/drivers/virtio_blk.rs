//! VirtIO Block Driver (MMIO)
//! 
//! Minimal implementation for QEMU 'virt' machine (Device ID 2).
//! Supports Read-Only operations for now (sufficient for loading apps).

use crate::kprintln;
use crate::kernel::memory::{self, PAGE_SIZE};
use core::ptr::NonNull;

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTERS (MMIO) - Common items could be shared, but repeated here for independence
// ═══════════════════════════════════════════════════════════════════════════════

const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
const VIRTIO_MMIO_VERSION: usize = 0x004;
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_DRIVER_FEATURES_SEL: usize = 0x024;
const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028; 
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;
const VIRTIO_MMIO_QUEUE_ALIGN: usize = 0x03c;     
const VIRTIO_MMIO_QUEUE_PFN: usize = 0x040;       
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;
const VIRTIO_MMIO_STATUS: usize = 0x070;

const MAGIC: u32 = 0x74726976; // "virt"
const DEVICE_ID_BLOCK: u32 = 2; // Block Device

// ═══════════════════════════════════════════════════════════════════════════════
// VIRTQUEUE
// ═══════════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Clone, Copy)]
struct VRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VRingAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 16],
    event: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VRingUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VRingUsed {
    flags: u16,
    idx: u16,
    ring: [VRingUsedElem; 16],
    event: u16,
}

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;

const QUEUE_SIZE: u16 = 16;

// Block Request Type
const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

#[repr(C)]
#[derive(Debug)]
struct VirtioBlkReqHeader {
    r#type: u32,
    reserved: u32,
    sector: u64,
}

struct VirtQueue {
    base: usize, 
    desc: NonNull<VRingDesc>,
    avail: NonNull<VRingAvail>,
    used: NonNull<VRingUsed>,
    last_used_idx: u16,
    idx: u32, 
}

struct VirtioBlock {
    base: usize,
    queue: Option<VirtQueue>,
    initialized: bool,
}

impl VirtioBlock {
    const fn new() -> Self {
        Self {
            base: 0,
            queue: None,
            initialized: false,
        }
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // 1. Scan MMIO region
        let mut base = 0x0a000000;
        let end = 0x0a003e00; // 32 devices
        
        while base < end {
            let magic = unsafe { crate::arch::read32(base + VIRTIO_MMIO_MAGIC_VALUE) };
            if magic == MAGIC {
                let device_id = unsafe { crate::arch::read32(base + VIRTIO_MMIO_DEVICE_ID) };
                if device_id == DEVICE_ID_BLOCK {
                    self.base = base;
                    break;
                }
            }
            base += 0x200;
        }

        if self.base == 0 {
            return Err("No VirtIO-Block device found");
        }
        
        kprintln!("[VIRTIO] Found Block Device at {:#x}", self.base);

        // 2. Reset Device
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, 0); }
        
        // 3. Status: ACKNOWLEDGE | DRIVER
        let mut status = 3; 
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        // 4. Features (Negotiate)
        unsafe {
             crate::arch::write32(self.base + VIRTIO_MMIO_DRIVER_FEATURES_SEL, 0);
             crate::arch::write32(self.base + VIRTIO_MMIO_DRIVER_FEATURES, 0); 
        }

        // 5. Status: FEATURES_OK
        status |= 8;
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        let new_status = unsafe { crate::arch::read32(self.base + VIRTIO_MMIO_STATUS) };
        if (new_status & 8) == 0 {
            return Err("Features not accepted");
        }
        
        // 6. Setup Queue 0
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_GUEST_PAGE_SIZE, PAGE_SIZE as u32); }
        self.queue = Some(self.setup_queue(0)?);

        // 7. Status: DRIVER_OK
        status |= 4; // DRIVER_OK
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        self.initialized = true;
        Ok(())
    }

    fn setup_queue(&self, idx: u32) -> Result<VirtQueue, &'static str> {
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_SEL, idx); }
        
        let num_max = unsafe { crate::arch::read32(self.base + VIRTIO_MMIO_QUEUE_NUM_MAX) };
        if num_max == 0 || num_max < QUEUE_SIZE as u32 {
            return Err("Queue not available or too small");
        }
        
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NUM, QUEUE_SIZE as u32); }
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_ALIGN, 4096); }
        
        let page2 = unsafe { memory::alloc_pages(2) }.ok_or("Queue OOM")?;
        unsafe { core::ptr::write_bytes(page2.as_ptr(), 0, PAGE_SIZE*2) };
        let phys = page2.as_ptr() as u64;
        let pfn = (phys / PAGE_SIZE as u64) as u32;
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_PFN, pfn); }
        
        let desc = page2.cast::<VRingDesc>();
        let avail_offset = 16 * 16;
        let avail = unsafe { page2.as_ptr().add(avail_offset) }.cast::<VRingAvail>();
        let used_offset = 4096;
        let used = unsafe { page2.as_ptr().add(used_offset) }.cast::<VRingUsed>();
        
        Ok(VirtQueue {
            base: phys as usize,
            desc: unsafe { NonNull::new_unchecked(desc.as_ptr()) },
            avail: unsafe { NonNull::new_unchecked(avail) },
            used: unsafe { NonNull::new_unchecked(used) },
            last_used_idx: 0,
            idx,
        })
    }
    
    // Read 512 bytes (one sector)
    fn read_sector(&mut self, sector: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        // kprintln!("[VIRTIO] read_sector {}", sector);
        if !self.initialized { return Err("Not init"); }
        if buf.len() < 512 { return Err("Buf too small"); }
        let q = self.queue.as_mut().unwrap();
        
        // Header
        let header = VirtioBlkReqHeader {
            r#type: VIRTIO_BLK_T_IN, // Read
            reserved: 0,
            sector,
        };
        let mut status_byte = 0xFF; // Init to non-zero
        
        // kprintln!("[VIRTIO] Read Sector {} to buf {:p}", sector, buf.as_ptr());

        let d0 = unsafe { q.desc.as_ptr().add(0) };
        let d1 = unsafe { q.desc.as_ptr().add(1) };
        let d2 = unsafe { q.desc.as_ptr().add(2) };
        
        unsafe {
            // Header
            (*d0).addr = &header as *const _ as u64;
            (*d0).len = core::mem::size_of::<VirtioBlkReqHeader>() as u32;
            (*d0).flags = DESC_F_NEXT;
            (*d0).next = 1;
            
            // Data
            (*d1).addr = buf.as_ptr() as u64;
            (*d1).len = 512;
            (*d1).flags = DESC_F_NEXT | DESC_F_WRITE; 
            (*d1).next = 2;
            
            // Status
            (*d2).addr = &mut status_byte as *mut _ as u64;
            (*d2).len = 1;
            (*d2).flags = DESC_F_WRITE;
            (*d2).next = 0;
            
            // Submit
            let idx = (*q.avail.as_ptr()).idx;
            (*q.avail.as_ptr()).ring[(idx % 16) as usize] = 0; 
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            (*q.avail.as_ptr()).idx = idx + 1;
        }
        
        // Notify
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NOTIFY, 0); }
        
        // Busy Wait
        // kprintln!("[VIRTIO] Waiting for interrupt/used...");
        let start = crate::drivers::timer::uptime_ms();
        loop {
            let used_idx = unsafe { (*q.used.as_ptr()).idx };
            if used_idx != q.last_used_idx {
                q.last_used_idx = used_idx;
                break;
            }
            if crate::drivers::timer::uptime_ms() - start > 1000 {
                return Err("VirtIO Block Timeout");
            }
        }
        
        // kprintln!("         Status: {}", status_byte);

        if status_byte == 0 {
            // Debug: Check first bytes
            // kprintln!("         Data: {:02x} {:02x} {:02x} {:02x} ...", buf[0], buf[1], buf[2], buf[510]);
            Ok(())
        } else {
            Err("IO Error")
        }
    }
    
    // Write 512 bytes
    fn write_sector(&mut self, sector: u64, buf: &[u8]) -> Result<(), &'static str> {
         if !self.initialized { return Err("Not init"); }
        if buf.len() < 512 { return Err("Buf too small"); }
        let q = self.queue.as_mut().unwrap();
        
        let header = VirtioBlkReqHeader {
            r#type: VIRTIO_BLK_T_OUT, // Write
            reserved: 0,
            sector,
        };
        let mut status_byte = 0u8;
        
        let d0 = unsafe { q.desc.as_ptr().add(0) };
        let d1 = unsafe { q.desc.as_ptr().add(1) };
        let d2 = unsafe { q.desc.as_ptr().add(2) };
        
        unsafe {
            // Header
            (*d0).addr = &header as *const _ as u64;
            (*d0).len = core::mem::size_of::<VirtioBlkReqHeader>() as u32;
            (*d0).flags = DESC_F_NEXT;
            (*d0).next = 1;
            
            // Data (Read-only for device)
            (*d1).addr = buf.as_ptr() as u64;
            (*d1).len = 512;
            (*d1).flags = DESC_F_NEXT; // Read-only
            (*d1).next = 2;
            
            // Status (Write-only)
            (*d2).addr = &mut status_byte as *mut _ as u64;
            (*d2).len = 1;
            (*d2).flags = DESC_F_WRITE;
            (*d2).next = 0;
            
            // Submit
            let idx = (*q.avail.as_ptr()).idx;
            (*q.avail.as_ptr()).ring[(idx % 16) as usize] = 0; 
            (*q.avail.as_ptr()).idx = idx + 1;
        }
        
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NOTIFY, 0); }
        
        loop {
            let used_idx = unsafe { (*q.used.as_ptr()).idx };
            if used_idx != q.last_used_idx {
                q.last_used_idx = used_idx;
                break;
            }
        }
        
        if status_byte == 0 { Ok(()) } else { Err("Write Error") }
    }
}

unsafe impl Send for VirtioBlock {}

pub static DRIVER: crate::arch::SpinLock<VirtioBlock> = crate::arch::SpinLock::new(VirtioBlock::new());

pub fn init() -> Result<(), &'static str> {
    DRIVER.lock().init()
}

pub fn read_sector(sector: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    DRIVER.lock().read_sector(sector, buf)
}

pub fn write_sector(sector: u64, buf: &[u8]) -> Result<(), &'static str> {
    DRIVER.lock().write_sector(sector, buf)
}
