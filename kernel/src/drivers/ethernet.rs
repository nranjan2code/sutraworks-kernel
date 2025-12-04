//! Ethernet Driver (MAC + PHY)
//!
//! Implements Ethernet controller for Raspberry Pi 5 (RP1 integrated MAC).
//! Supports 1Gbps link with DMA ring buffers.
//!
//! # Architecture
//! - **TX Ring**: Circular buffer for outgoing packets
//! - **RX Ring**: Circular buffer for incoming packets
//! - **DMA**: Zero-copy packet transmission
//! - **Interrupts**: Efficient packet notification

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::kernel::memory::{self, PAGE_SIZE};
use core::ptr::NonNull;

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTERS (RP1 Ethernet MAC)
// ═══════════════════════════════════════════════════════════════════════════════

const ETH_BASE: usize = 0x1_0000_0000 + 0x00100000;  // Ethernet MAC on RP1

const ETH_MAC_CONFIG: usize = 0x0000;
const ETH_MAC_FRAME_FILTER: usize = 0x0004;
const ETH_MAC_ADDR_HIGH: usize = 0x0040;
const ETH_MAC_ADDR_LOW: usize = 0x0044;

const ETH_DMA_BUS_MODE: usize = 0x1000;
const ETH_DMA_TX_POLL: usize = 0x1004;
const ETH_DMA_RX_POLL: usize = 0x1008;
const ETH_DMA_RX_DESC_ADDR: usize = 0x100C;
const ETH_DMA_TX_DESC_ADDR: usize = 0x1010;
const ETH_DMA_STATUS: usize = 0x1014;
const ETH_DMA_OPERATION_MODE: usize = 0x1018;

// MAC Config Flags
const MAC_CONFIG_RE: u32 = 1 << 2;  // Receiver Enable
const MAC_CONFIG_TE: u32 = 1 << 3;  // Transmitter Enable

// DMA Descriptor Flags
const DMA_DESC_OWN: u32 = 1 << 31;  // Owned by DMA
const DMA_DESC_FS: u32 = 1 << 29;   // First Segment
const DMA_DESC_LS: u32 = 1 << 28;   // Last Segment
const DMA_DESC_RCH: u32 = 1 << 14;  // Second Address Chained

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// DMA Descriptor (Enhanced format)
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct DmaDescriptor {
    pub status: u32,
    pub control: u32,
    pub buffer1_addr: u32,
    pub buffer2_addr: u32,  // Next descriptor address (chaining)
}

impl DmaDescriptor {
    pub const fn new() -> Self {
        Self {
            status: 0,
            control: 0,
            buffer1_addr: 0,
            buffer2_addr: 0,
        }
    }
}

/// Ethernet Frame (max size 1518 bytes)
pub const ETH_FRAME_SIZE: usize = 1518;

#[repr(C, align(16))]
pub struct EthernetFrame {
    pub data: [u8; ETH_FRAME_SIZE],
    pub length: usize,
}

impl EthernetFrame {
    pub const fn new() -> Self {
        Self {
            data: [0; ETH_FRAME_SIZE],
            length: 0,
        }
    }
}

/// MAC Address
#[derive(Clone, Copy, Debug)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub const BROADCAST: Self = MacAddr([0xFF; 6]);

    pub fn from_bytes(bytes: [u8; 6]) -> Self {
        MacAddr(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

const RING_SIZE: usize = 8;  // Number of descriptors in TX/RX rings

pub struct EthernetDriver {
    base_addr: usize,
    mac_addr: MacAddr,

    // TX Ring
    tx_desc_ring: Option<NonNull<DmaDescriptor>>,
    tx_buffers: [Option<NonNull<EthernetFrame>>; RING_SIZE],
    tx_head: usize,

    // RX Ring
    rx_desc_ring: Option<NonNull<DmaDescriptor>>,
    rx_buffers: [Option<NonNull<EthernetFrame>>; RING_SIZE],
    rx_head: usize,

    initialized: bool,
}

impl EthernetDriver {
    pub const fn new() -> Self {
        Self {
            base_addr: ETH_BASE,
            mac_addr: MacAddr([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            tx_desc_ring: None,
            tx_buffers: [None; RING_SIZE],
            tx_head: 0,
            rx_desc_ring: None,
            rx_buffers: [None; RING_SIZE],
            rx_head: 0,
            initialized: false,
        }
    }

    /// Initialize Ethernet controller
    pub fn init(&mut self, mac_addr: MacAddr) -> Result<(), &'static str> {
        kprintln!("[ETH] Initializing Ethernet...");

        self.mac_addr = mac_addr;

        // Reset DMA
        self.reset_dma()?;

        // Configure MAC address
        self.set_mac_address(mac_addr);

        // Allocate TX descriptor ring
        let tx_ring_mem = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("TX ring alloc failed")?;
        unsafe { core::ptr::write_bytes(tx_ring_mem.as_ptr(), 0, PAGE_SIZE) };
        self.tx_desc_ring = Some(tx_ring_mem.cast());

        // Allocate TX buffers
        for i in 0..RING_SIZE {
            let buf_mem = unsafe { memory::alloc_dma(core::mem::size_of::<EthernetFrame>()) }
                .ok_or("TX buffer alloc failed")?;
            self.tx_buffers[i] = Some(buf_mem.cast());
        }

        // Initialize TX descriptors
        let tx_ring = unsafe { self.tx_desc_ring.unwrap().as_ptr() };
        for i in 0..RING_SIZE {
            let desc = unsafe { &mut *tx_ring.add(i) };
            let buf_addr = self.tx_buffers[i].unwrap().as_ptr() as u32;
            let next_desc_addr = if i == RING_SIZE - 1 {
                tx_ring as u32  // Wrap to first descriptor
            } else {
                unsafe { tx_ring.add(i + 1) as u32 }
            };

            desc.buffer1_addr = buf_addr;
            desc.buffer2_addr = next_desc_addr;
            desc.control = DMA_DESC_RCH;  // Chained
        }

        // Set TX descriptor base address
        unsafe {
            arch::write32(self.base_addr + ETH_DMA_TX_DESC_ADDR, tx_ring as u32);
        }

        // Allocate RX descriptor ring
        let rx_ring_mem = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("RX ring alloc failed")?;
        unsafe { core::ptr::write_bytes(rx_ring_mem.as_ptr(), 0, PAGE_SIZE) };
        self.rx_desc_ring = Some(rx_ring_mem.cast());

        // Allocate RX buffers
        for i in 0..RING_SIZE {
            let buf_mem = unsafe { memory::alloc_dma(core::mem::size_of::<EthernetFrame>()) }
                .ok_or("RX buffer alloc failed")?;
            self.rx_buffers[i] = Some(buf_mem.cast());
        }

        // Initialize RX descriptors
        let rx_ring = unsafe { self.rx_desc_ring.unwrap().as_ptr() };
        for i in 0..RING_SIZE {
            let desc = unsafe { &mut *rx_ring.add(i) };
            let buf_addr = self.rx_buffers[i].unwrap().as_ptr() as u32;
            let next_desc_addr = if i == RING_SIZE - 1 {
                rx_ring as u32
            } else {
                unsafe { rx_ring.add(i + 1) as u32 }
            };

            desc.buffer1_addr = buf_addr;
            desc.buffer2_addr = next_desc_addr;
            desc.control = DMA_DESC_RCH | (ETH_FRAME_SIZE as u32);
            desc.status = DMA_DESC_OWN;  // Give ownership to DMA
        }

        // Set RX descriptor base address
        unsafe {
            arch::write32(self.base_addr + ETH_DMA_RX_DESC_ADDR, rx_ring as u32);
        }

        // Enable MAC transmitter and receiver
        unsafe {
            let mac_config = arch::read32(self.base_addr + ETH_MAC_CONFIG);
            arch::write32(self.base_addr + ETH_MAC_CONFIG, mac_config | MAC_CONFIG_TE | MAC_CONFIG_RE);
        }

        // Start DMA transmission and reception
        unsafe {
            arch::write32(self.base_addr + ETH_DMA_OPERATION_MODE, 0x00202001);  // Start TX/RX
        }

        self.initialized = true;
        kprintln!("[ETH] Initialized with MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            mac_addr.0[0], mac_addr.0[1], mac_addr.0[2],
            mac_addr.0[3], mac_addr.0[4], mac_addr.0[5]);

        Ok(())
    }

    /// Send an Ethernet frame
    pub fn send_frame(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Not initialized");
        }

        // Check for fatal errors before sending
        if let Err(e) = self.check_and_recover() {
            kprintln!("[ETH] Recovery failed: {}", e);
            return Err("Ethernet hardware error");
        }

        if data.len() > ETH_FRAME_SIZE {
            return Err("Frame too large");
        }

        // Get current TX descriptor
        let tx_ring = unsafe { self.tx_desc_ring.unwrap().as_ptr() };
        let desc = unsafe { &mut *tx_ring.add(self.tx_head) };

        // Check if descriptor is available
        if (desc.status & DMA_DESC_OWN) != 0 {
            return Err("TX ring full");
        }

        // Copy data to TX buffer
        let tx_buffer = unsafe { &mut *self.tx_buffers[self.tx_head].unwrap().as_ptr() };
        tx_buffer.data[..data.len()].copy_from_slice(data);
        tx_buffer.length = data.len();

        // Setup descriptor
        desc.control = DMA_DESC_RCH | (data.len() as u32);
        desc.status = DMA_DESC_OWN | DMA_DESC_FS | DMA_DESC_LS;  // First+Last segment, give to DMA

        // Advance head
        self.tx_head = (self.tx_head + 1) % RING_SIZE;

        // Notify DMA (poll demand)
        unsafe {
            arch::write32(self.base_addr + ETH_DMA_TX_POLL, 1);
        }

        Ok(())
    }

    /// Receive an Ethernet frame (non-blocking)
    pub fn recv_frame(&mut self, buffer: &mut [u8]) -> Result<usize, &'static str> {
        if !self.initialized {
            return Err("Not initialized");
        }

        // Check for fatal errors
        if let Err(e) = self.check_and_recover() {
            kprintln!("[ETH] Recovery failed: {}", e);
            return Err("Ethernet hardware error");
        }

        // Get current RX descriptor
        let rx_ring = unsafe { self.rx_desc_ring.unwrap().as_ptr() };
        let desc = unsafe { &mut *rx_ring.add(self.rx_head) };

        // Check if frame is available
        if (desc.status & DMA_DESC_OWN) != 0 {
            return Err("No frame available");
        }

        // Read frame length (bits 16:29 of status)
        let frame_len = ((desc.status >> 16) & 0x3FFF) as usize;

        if frame_len > buffer.len() {
            // Give descriptor back to DMA
            desc.status = DMA_DESC_OWN;
            return Err("Buffer too small");
        }

        // Copy data from RX buffer
        let rx_buffer = unsafe { &*self.rx_buffers[self.rx_head].unwrap().as_ptr() };
        buffer[..frame_len].copy_from_slice(&rx_buffer.data[..frame_len]);

        // Give descriptor back to DMA
        desc.status = DMA_DESC_OWN;

        // Advance head
        self.rx_head = (self.rx_head + 1) % RING_SIZE;

        // Notify DMA
        unsafe {
            arch::write32(self.base_addr + ETH_DMA_RX_POLL, 1);
        }

        Ok(frame_len)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // INTERNAL HELPERS
    // ═══════════════════════════════════════════════════════════════════════════

    fn reset_dma(&self) -> Result<(), &'static str> {
        unsafe {
            // Software reset
            arch::write32(self.base_addr + ETH_DMA_BUS_MODE, 1);

            // Wait for reset to complete
            let mut timeout = 10000;
            while timeout > 0 {
                if (arch::read32(self.base_addr + ETH_DMA_BUS_MODE) & 1) == 0 {
                    return Ok(());
                }
                timeout -= 1;
            }
        }

        Err("DMA reset timeout")
    }

    fn set_mac_address(&self, addr: MacAddr) {
        unsafe {
            let high = (addr.0[5] as u32) << 8 | (addr.0[4] as u32);
            let low = (addr.0[3] as u32) << 24 | (addr.0[2] as u32) << 16 |
                      (addr.0[1] as u32) << 8 | (addr.0[0] as u32);

            arch::write32(self.base_addr + ETH_MAC_ADDR_HIGH, high);
            arch::write32(self.base_addr + ETH_MAC_ADDR_LOW, low);
        }
    }

    pub fn get_mac_address(&self) -> MacAddr {
        self.mac_addr
    }

    fn check_and_recover(&mut self) -> Result<(), &'static str> {
        let status = unsafe { arch::read32(self.base_addr + ETH_DMA_STATUS) };
        
        // Fatal Bus Error (Bit 13)
        if (status & (1 << 13)) != 0 {
            kprintln!("[ETH] Fatal Bus Error detected (Status: {:#x}). Resetting...", status);
            return self.reinit();
        }
        
        // Process Stopped (TX=Bit 1, RX=Bit 8)
        // Only reset if stopped unexpectedly (we expect it to run)
        if (status & ((1 << 1) | (1 << 8))) != 0 {
             kprintln!("[ETH] DMA Process Stopped (Status: {:#x}). Restarting...", status);
             return self.reinit();
        }
        
        Ok(())
    }

    pub fn reinit(&mut self) -> Result<(), &'static str> {
        kprintln!("[ETH] Re-initializing Ethernet Driver...");
        self.initialized = false;
        // Note: This leaks old ring buffers if we don't free them.
        // For Sprint 8, we prioritize recovery over memory efficiency in this rare case.
        // Free old resources before re-initializing
        self.free_resources();
        let mac = self.mac_addr;
        self.init(mac)
    }

    fn free_resources(&mut self) {
        // Free TX Ring
        if let Some(ptr) = self.tx_desc_ring.take() {
            unsafe { memory::free_dma(ptr.cast(), PAGE_SIZE) };
        }
        for i in 0..RING_SIZE {
            if let Some(ptr) = self.tx_buffers[i].take() {
                unsafe { memory::free_dma(ptr.cast(), core::mem::size_of::<EthernetFrame>()) };
            }
        }

        // Free RX Ring
        if let Some(ptr) = self.rx_desc_ring.take() {
            unsafe { memory::free_dma(ptr.cast(), PAGE_SIZE) };
        }
        for i in 0..RING_SIZE {
            if let Some(ptr) = self.rx_buffers[i].take() {
                unsafe { memory::free_dma(ptr.cast(), core::mem::size_of::<EthernetFrame>()) };
            }
        }
    }
}

// SAFETY: Protected by SpinLock
unsafe impl Send for EthernetDriver {}

pub static ETHERNET: SpinLock<EthernetDriver> = SpinLock::new(EthernetDriver::new());

/// Initialize Ethernet driver
pub fn init(mac_addr: MacAddr) {
    let mut driver = ETHERNET.lock();
    if let Err(e) = driver.init(mac_addr) {
        kprintln!("[ETH] Init failed: {}", e);
    }
}

/// Send an Ethernet frame
pub fn send_frame(data: &[u8]) -> Result<(), &'static str> {
    ETHERNET.lock().send_frame(data)
}

/// Receive an Ethernet frame
pub fn recv_frame(buffer: &mut [u8]) -> Result<usize, &'static str> {
    ETHERNET.lock().recv_frame(buffer)
}

/// Get MAC address
pub fn get_mac_address() -> MacAddr {
    ETHERNET.lock().get_mac_address()
}
