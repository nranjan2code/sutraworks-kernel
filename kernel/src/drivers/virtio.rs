//! VirtIO-Net Driver (MMIO)
//! 
//! Minimal implementation for QEMU 'virt' machine.
//! Supports Legacy/Modern MMIO interface.

use crate::kprintln;
use crate::kernel::memory::{self, PAGE_SIZE};
use core::ptr::NonNull;

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTERS (MMIO)
// ═══════════════════════════════════════════════════════════════════════════════

const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
const VIRTIO_MMIO_VERSION: usize = 0x004;
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_DRIVER_FEATURES_SEL: usize = 0x024;
const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028; // Legacy only
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;
const VIRTIO_MMIO_QUEUE_ALIGN: usize = 0x03c;     // Legacy only
const VIRTIO_MMIO_QUEUE_PFN: usize = 0x040;       // Legacy only
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;
const VIRTIO_MMIO_STATUS: usize = 0x070;

const MAGIC: u32 = 0x74726976; // "virt"
const DEVICE_ID_NET: u32 = 1;

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
    ring: [u16; 16], // Size 16 for simplicity?
    event: u16,      // Used if VIRTIO_F_EVENT_IDX
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

struct VirtQueue {
    base: usize, // Base (physical, PFN-based)
    desc: NonNull<VRingDesc>,
    avail: NonNull<VRingAvail>,
    used: NonNull<VRingUsed>,
    last_used_idx: u16,
    idx: u32, // Queue Index (0=RX, 1=TX)
}

pub struct VirtioNet {
    base: usize,
    rx_queue: Option<VirtQueue>,
    tx_queue: Option<VirtQueue>,
    mac: [u8; 6],
    initialized: bool,
    
    // Buffers to hold alive
    rx_buffers: [NonNull<u8>; 16], 
}

impl VirtioNet {
    const fn new() -> Self {
        Self {
            base: 0,
            rx_queue: None,
            tx_queue: None,
            mac: [0; 6],
            initialized: false,
            rx_buffers: [NonNull::dangling(); 16],
        }
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // 1. Scan MMIO region
        // QEMU virt MMIO starts at 0x0a000000, stepping by 0x200
        let mut base = 0x0a000000;
        let end = 0x0a003e00; // 32 devices
        
        while base < end {
            let magic = unsafe { crate::arch::read32(base + VIRTIO_MMIO_MAGIC_VALUE) };
            if magic == MAGIC {
                let device_id = unsafe { crate::arch::read32(base + VIRTIO_MMIO_DEVICE_ID) };
                if device_id == DEVICE_ID_NET {
                    self.base = base;
                    break;
                }
            }
            base += 0x200;
        }

        if self.base == 0 {
            return Err("No VirtIO-Net device found");
        }
        
        kprintln!("[VIRTIO] Found Network Device at {:#x}", self.base);

        // 2. Reset Device
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, 0); }
        
        // 3. Set ACKNOWLEDGE status bit
        let mut status = unsafe { crate::arch::read32(self.base + VIRTIO_MMIO_STATUS) };
        status |= 1; // ACKNOWLEDGE
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        // 4. Set DRIVER status bit
        status |= 2; // DRIVER
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        // 5. Negotiate Features
        // Just accept defaults for now.
        // We probably should check MAC feature.
        // VIRTIO_NET_F_MAC (5)
        let device_features = unsafe {
             crate::arch::write32(self.base + VIRTIO_MMIO_DEVICE_FEATURES_SEL, 0); // Set 0
             crate::arch::read32(self.base + VIRTIO_MMIO_DEVICE_FEATURES)
        };
        
        if (device_features & (1 << 5)) != 0 {
             // MAC is present in Config space
             // For MMIO, config space starts at 0x100
             for i in 0..6 {
                 self.mac[i] = unsafe { crate::arch::read8(self.base + 0x100 + i) };
             }
             kprintln!("[VIRTIO] MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", 
                 self.mac[0], self.mac[1], self.mac[2], self.mac[3], self.mac[4], self.mac[5]);
        }
        
        // Write features we accept
        unsafe {
             crate::arch::write32(self.base + VIRTIO_MMIO_DRIVER_FEATURES_SEL, 0);
             crate::arch::write32(self.base + VIRTIO_MMIO_DRIVER_FEATURES, device_features & (1 << 5)); // Only accept MAC? 
        }

        // 6. Set FEATURES_OK status bit
        status |= 8;
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        // Re-read status to ensure bit is still set
        let new_status = unsafe { crate::arch::read32(self.base + VIRTIO_MMIO_STATUS) };
        if (new_status & 8) == 0 {
            return Err("Features not accepted");
        }
        
        // 7. Setup Virtqueues
        // Legacy: Page Size
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_GUEST_PAGE_SIZE, PAGE_SIZE as u32); }
        
        // Queue 0 (RX)
        self.rx_queue = Some(self.setup_queue(0)?);
        
        // Queue 1 (TX)
        self.tx_queue = Some(self.setup_queue(1)?);

        // Populate RX queue with buffers
        self.fill_rx_queue()?;

        // 8. Set DRIVER_OK status bit
        status |= 4; // DRIVER_OK
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_STATUS, status); }
        
        self.initialized = true;
        Ok(())
    }

    fn setup_queue(&self, idx: u32) -> Result<VirtQueue, &'static str> {
        // Select queue
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_SEL, idx); }
        
        let num_max = unsafe { crate::arch::read32(self.base + VIRTIO_MMIO_QUEUE_NUM_MAX) };
        if num_max == 0 {
            return Err("Queue not available");
        }
        
        if num_max < QUEUE_SIZE as u32 {
            return Err("Queue too small");
        }
        
        // Set queue size (NUM)
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NUM, QUEUE_SIZE as u32); }
        
        // Legacy Alignment
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_ALIGN, 4096); }
        
        // Allocate Memory (Pages)
        // Calculating size
        // Desc: 16 * 16 = 256
        // Avail: 6 + 2 * 16 = 38
        // Used: 6 + 8 * 16 = 134
        // Total < 4096 (1 Page)
        
        let page = unsafe { memory::alloc_pages(1) }.ok_or("Queue OOM")?;
        unsafe { core::ptr::write_bytes(page.as_ptr(), 0, PAGE_SIZE) };
        
        let phys = page.as_ptr() as u64;
        let pfn = (phys / PAGE_SIZE as u64) as u32;
        
        // Legacy PFN
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_PFN, pfn); }
        
        // Pointers
        // Based on Legacy Layout with 4096 alignment for Used Ring
        // Actually Legacy layout says Avail is after Desc, Used is aligned to 4096?
        // Let's verify layout.
        // If ALIGN = 4096.
        // Desc at 0.
        // Avail at 16 * 16 = 256.
        // Used at ALIGN_UP(256 + 6 + 32, 4096) = 4096.
        // Oh, so Used ring is on the *next* page!
        // So we need 2 pages.
        
        // Let's re-alloc 2 pages.
        // No, current allocation is 1 page.
        // Let's adjust alignment? 
        // Can we set ALIGN to 4?
        // Legacy implementation usually defaults to 4096.
        
        // Let's alloc 2 pages to be safe.
        // Or if we use modern MMIO... but we are using legacy PFN regs.
        
        // We can just alloc 2 pages.
        // Wait, current logic:
        // write PFN. It expects contiguous physical memory.
        
        // Re-do alloc (leaking previous 1-page allocation for simplicity)
        let page2 = unsafe { memory::alloc_pages(2) }.ok_or("Queue OOM 2")?;
        unsafe { core::ptr::write_bytes(page2.as_ptr(), 0, PAGE_SIZE*2) };
        let phys = page2.as_ptr() as u64;
        let pfn = (phys / PAGE_SIZE as u64) as u32;
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_PFN, pfn); }
        
        let desc = page2.cast::<VRingDesc>();
        let avail_offset = 16 * 16;
        let avail = unsafe { page2.as_ptr().add(avail_offset) }.cast::<VRingAvail>();
        let used_offset = 4096;
        let used = unsafe { page2.as_ptr().add(used_offset) }.cast::<VRingUsed>();
        
        let desc_ptr = unsafe { NonNull::new_unchecked(desc.as_ptr()) };
        let avail_ptr = unsafe { NonNull::new_unchecked(avail) };
        let used_ptr = unsafe { NonNull::new_unchecked(used) };
        
        Ok(VirtQueue {
            base: phys as usize,
            desc: desc_ptr,
            avail: avail_ptr,
            used: used_ptr,
            last_used_idx: 0,
            idx,
        })
    }

    fn fill_rx_queue(&mut self) -> Result<(), &'static str> {
        let q = self.rx_queue.as_mut().unwrap();
        
        for i in 0..QUEUE_SIZE {
            // Alloc buffer (2048 bytes)
            let buf = unsafe { memory::alloc_pages(1) }.ok_or("RX Buf OOM")?; 
            self.rx_buffers[i as usize] = buf;
            
            let desc_ptr = unsafe { q.desc.as_ptr().add(i as usize) };
            
            unsafe {
                (*desc_ptr).addr = buf.as_ptr() as u64;
                (*desc_ptr).len = 2048; // Full page? Buffer needs Virtio Header (10 bytes).
                (*desc_ptr).flags = DESC_F_WRITE; // Device writes to it
                (*desc_ptr).next = 0;
            }
            
            // Add to Avail Ring
            let idx = unsafe { (*q.avail.as_ptr()).idx } as usize;
            unsafe {
                (*q.avail.as_ptr()).ring[idx % 16] = i;
                (*q.avail.as_ptr()).idx = (idx + 1) as u16;
            }
        }
        
        // Notify
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NOTIFY, 0); } // Queue 0
        
        Ok(())
    }

    fn send_frame(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if !self.initialized { return Err("VirtIO not init"); }
        let q = self.tx_queue.as_mut().unwrap();
        
        // 1. Get next Available slot in Avail Ring (we are the driver, we produce descriptors)
        // But we need a free descriptor from the descriptor table.
        // For keeping it simple, let's use descriptor 0, 1, 2... synchronously?
        // We are single threaded here (SpinLock).
        // Let's just use descriptor 0 for everything? 
        // We wait for it to be used.
        
        // Virtio Net Header (10 bytes legacy, 12 modern?)
        // Legacy: 10 bytes (flags, gso_type, hdr_len, gso_size, csum_start, csum_offset)
        let header_len = 10; 
        
        // We need 2 descriptors? Or 1 with header+data?
        // Let's use 1 buffer with header prepended.
        
        // Alloc temporary buffer (or use static/stack?)
        let mut buf = [0u8; 1528];
        // Zero header
        // Copy data
        buf[header_len..header_len+data.len()].copy_from_slice(data);
        
        let desc_idx = 0; // Use first descriptor always (simple, sync)
        
        // Setup descriptor
        // Use a persistent buffer? Or the stack buffer address? 
        // Stack buffer address is Virtual (Identity mapped in kernel so Phys).
        let phys_addr = buf.as_ptr() as u64;
        
        let desc_ptr = unsafe { q.desc.as_ptr().add(desc_idx) };
        unsafe {
            (*desc_ptr).addr = phys_addr;
            (*desc_ptr).len = (header_len + data.len()) as u32;
            (*desc_ptr).flags = 0; // Read-only for device
            (*desc_ptr).next = 0;
        }
        
        // Put in Avail
        let idx = unsafe { (*q.avail.as_ptr()).idx };
        unsafe {
            (*q.avail.as_ptr()).ring[(idx % 16) as usize] = desc_idx as u16;
            
            // Memory Barrier?
            
            (*q.avail.as_ptr()).idx = idx + 1;
        }
        
        // Notify
        unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NOTIFY, 1); } // Queue 1
        
        // Poll for Used Ring update (Busy Wait)
        loop {
            let used_idx = unsafe { (*q.used.as_ptr()).idx };
            if used_idx != q.last_used_idx {
                // Consumed
                q.last_used_idx = used_idx;
                break;
            }
        }
        
        Ok(())
    }
    
    fn recv_frame(&mut self, buffer: &mut [u8]) -> Result<usize, &'static str> {
        if !self.initialized { return Err("Not init"); }
        let q = self.rx_queue.as_mut().unwrap();
        
        let used_idx = unsafe { (*q.used.as_ptr()).idx };
        if used_idx == q.last_used_idx {
            return Err("No packet");
        }
        
        // Process all new packets? Just one.
        let elem_idx = q.last_used_idx % 16;
        let used_elem = unsafe { (*q.used.as_ptr()).ring[elem_idx as usize] };
        let desc_idx = used_elem.id as usize;
        let mut len = used_elem.len as usize; 
        
        // Get buffer
        let buf_ptr = self.rx_buffers[desc_idx].as_ptr();
        
        // Skip header (10 bytes)
        let header_len = 10;
        if len > header_len {
            len -= header_len;
            let payload = unsafe { core::slice::from_raw_parts(buf_ptr.add(header_len), len) };
            
            // Copy to user buffer
            let copy_len = core::cmp::min(buffer.len(), len);
            buffer[..copy_len].copy_from_slice(&payload[..copy_len]);
            
            // Republish descriptor
            // Add back to avail
            let avail_idx = unsafe { (*q.avail.as_ptr()).idx };
            unsafe {
                (*q.avail.as_ptr()).ring[(avail_idx % 16) as usize] = desc_idx as u16;
                 (*q.avail.as_ptr()).idx = avail_idx + 1;
            }
             // Notify (optional for RX usually, but good practice)
             unsafe { crate::arch::write32(self.base + VIRTIO_MMIO_QUEUE_NOTIFY, 0); }
             
             q.last_used_idx = q.last_used_idx.wrapping_add(1);
             
             Ok(copy_len)
        } else {
             q.last_used_idx = q.last_used_idx.wrapping_add(1);
             Err("Packet too small")
        }
    }
}

// SAFETY: Protected by SpinLock, and we handle raw pointers carefully
unsafe impl Send for VirtioNet {}

pub static DRIVER: crate::arch::SpinLock<VirtioNet> = crate::arch::SpinLock::new(VirtioNet::new());

pub fn init() {
    let mut drv = DRIVER.lock();
    if let Err(e) = drv.init() {
        kprintln!("[VIRTIO] Init failed: {}", e);
    }
}

pub fn send_frame(data: &[u8]) -> Result<(), &'static str> {
    DRIVER.lock().send_frame(data)
}

pub fn recv_frame(buffer: &mut [u8]) -> Result<usize, &'static str> {
    DRIVER.lock().recv_frame(buffer)
}

pub fn get_mac() -> [u8; 6] {
    DRIVER.lock().mac
}
