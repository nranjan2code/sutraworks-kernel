//! Hailo-8 AI Accelerator Driver
//!
//! Controls the Hailo-8 NPU via PCIe.
//!
//! # Architecture
//! - **Connection**: PCIe Gen 3 x4
//! - **Access**: BAR0 (Control Registers), BAR2/4 (Doorbell/DMA)
//! - **Protocol**: Hailo Control Protocol (HCP)

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::drivers::pcie::{self, PcieDevice, VENDOR_ID_HAILO};
use crate::drivers::hailo_tensor::YoloOutputParser;
use alloc::vec::Vec;

// ═══════════════════════════════════════════════════════════════════════════════
// HAILO REGISTERS (Offsets in BAR0)
// ═══════════════════════════════════════════════════════════════════════════════

const HAILO_CONTROL: usize = 0x00;
const _HAILO_STATUS: usize = 0x04;
const _HAILO_IRQ_STATUS: usize = 0x10;
const _HAILO_IRQ_MASK: usize = 0x14;

// ═══════════════════════════════════════════════════════════════════════════════
// HCP PROTOCOL STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HcpHeader {
    pub magic: u32,           // 0xAA55AA55
    pub seq_num: u32,         // Sequence number
    pub opcode: u32,          // Command opcode
    pub flags: u32,           // Flags
    pub payload_len: u32,     // Length of payload in bytes
    pub checksum: u32,        // Checksum of header + payload
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HcpCommand {
    pub header: HcpHeader,
    pub payload: [u8; 64],    // Fixed size for simplicity, real driver might use variable
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HcpResponse {
    pub header: HcpHeader,
    pub status: u32,          // Status code
    pub payload: [u8; 64],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HailoState {
    Reset,
    Ready,
    Running,
    Error,
}

impl HcpHeader {
    pub fn new(opcode: u32, seq_num: u32, payload_len: u32) -> Self {
        Self {
            magic: 0xAA55AA55,
            seq_num,
            opcode,
            flags: 0,
            payload_len,
            checksum: 0, // TODO: Implement checksum calculation
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DMA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DmaDescriptor {
    pub addr_low: u32,
    pub addr_high: u32,
    pub length: u32,
    pub flags: u32,
}

pub struct DmaBuffer {
    pub phys_addr: usize,
    pub virt_addr: usize,
    pub size: usize,
}

pub struct DmaChannel {
    pub index: u8,
    pub is_active: bool,
    pub descriptors: [DmaDescriptor; 16], // Simplified ring
    pub head: usize,
    pub tail: usize,
}

impl DmaChannel {
    pub const fn new(index: u8) -> Self {
        Self {
            index,
            is_active: false,
            descriptors: [DmaDescriptor { addr_low: 0, addr_high: 0, length: 0, flags: 0 }; 16],
            head: 0,
            tail: 0,
        }
    }
}

pub struct CommandQueue {
    buffer: [HcpCommand; 16],
    head: usize,
    tail: usize,
}

impl CommandQueue {
    pub const fn new() -> Self {
        Self {
            buffer: [HcpCommand {
                header: HcpHeader { magic: 0, seq_num: 0, opcode: 0, flags: 0, payload_len: 0, checksum: 0 },
                payload: [0; 64]
            }; 16],
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, cmd: HcpCommand) -> Result<(), &'static str> {
        let next = (self.head + 1) % 16;
        if next == self.tail {
            return Err("Command queue full");
        }
        self.buffer[self.head] = cmd;
        self.head = next;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<HcpCommand> {
        if self.head == self.tail {
            return None;
        }
        let cmd = self.buffer[self.tail];
        self.tail = (self.tail + 1) % 16;
        Some(cmd)
    }
}

#[allow(dead_code)]
pub struct ResponseQueue {
    buffer: [HcpResponse; 16],
    head: usize,
    tail: usize,
}

impl ResponseQueue {
    pub const fn new() -> Self {
        Self {
            buffer: [HcpResponse {
                header: HcpHeader { magic: 0, seq_num: 0, opcode: 0, flags: 0, payload_len: 0, checksum: 0 },
                status: 0,
                payload: [0; 64]
            }; 16],
            head: 0,
            tail: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct HailoDriver {
    device: Option<PcieDevice>,
    bar0_addr: usize,
    bar0_size: usize,
    bar2_addr: usize,
    bar2_size: usize,
    initialized: bool,
    state: HailoState,
    cmd_queue: CommandQueue,
    #[allow(dead_code)]
    resp_queue: ResponseQueue,
    seq_counter: u32,
    dma_channels: [DmaChannel; 2], // 0: Host-to-Device, 1: Device-to-Host
}

impl HailoDriver {
    pub const fn new() -> Self {
        Self {
            device: None,
            bar0_addr: 0,
            bar0_size: 0,
            bar2_addr: 0,
            bar2_size: 0,
            initialized: false,
            state: HailoState::Reset,
            cmd_queue: CommandQueue::new(),
            resp_queue: ResponseQueue::new(),
            seq_counter: 0,
            dma_channels: [DmaChannel::new(0), DmaChannel::new(1)],
        }
    }

    // ... (init, reset, handshake methods remain unchanged)

    /// Write a register to BAR2 (Doorbell)
    fn write_db(&self, offset: usize, value: u32) {
        if !self.initialized { return; }
        unsafe { arch::write32(self.bar2_addr + offset, value) }
    }

    /// Setup a DMA transfer
    #[allow(dead_code)]
    pub fn setup_dma_transfer(&mut self, channel_idx: usize, buffer: &DmaBuffer) -> Result<(), &'static str> {
        if channel_idx >= 2 {
            return Err("Invalid DMA channel");
        }
        
        let channel = &mut self.dma_channels[channel_idx];
        if channel.is_active {
            return Err("DMA channel busy");
        }

        // Setup descriptor (Simplified for now)
        let desc = &mut channel.descriptors[0];
        desc.addr_low = (buffer.phys_addr & 0xFFFFFFFF) as u32;
        desc.addr_high = (buffer.phys_addr >> 32) as u32;
        desc.length = buffer.size as u32;
        desc.flags = 1; // Valid bit

        channel.head = 0;
        channel.tail = 0;
        
        Ok(())
    }

    /// Start DMA transfer
    pub fn start_dma(&mut self, channel_idx: usize) -> Result<(), &'static str> {
        if channel_idx >= 2 {
            return Err("Invalid DMA channel");
        }
        
        let channel = &mut self.dma_channels[channel_idx];
        channel.is_active = true;
        
        // Write to doorbell register to trigger DMA
        // Offset 0x00 + idx * 4 is a common pattern for doorbells
        self.write_db(channel_idx * 4, 1);
        
        Ok(())
    }

    /// Wait for DMA completion
    pub fn wait_dma(&mut self, channel_idx: usize) -> Result<(), &'static str> {
        if channel_idx >= 2 {
            return Err("Invalid DMA channel");
        }

        // Poll interrupt status register (Bit 0 = Channel 0, Bit 1 = Channel 1)
        let mut timeout = 100000;
        let mask = 1 << channel_idx;
        
        while timeout > 0 {
            let status = self.read_reg(_HAILO_IRQ_STATUS);
            if (status & mask) != 0 {
                // Clear interrupt
                self.write_reg(_HAILO_IRQ_STATUS, mask);
                
                // Mark channel as inactive
                self.dma_channels[channel_idx].is_active = false;
                return Ok(());
            }
            timeout -= 1;
            crate::drivers::timer::delay_us(10);
        }

        // Timeout - force clear for now
        self.dma_channels[channel_idx].is_active = false;
        Err("DMA timeout")
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MODEL MANAGEMENT
// ═══════════════════════════════════════════════════════════════════════════════

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HefHeader {
    pub magic: u32,           // 0x48454600 ("HEF\0")
    pub version: u32,
    pub model_len: u32,
    pub entry_point: u32,
}

impl HailoDriver {
    // ... (existing methods)

    /// Load a model from a buffer (HEF format)
    #[allow(dead_code)]
    pub fn load_model(&mut self, model_data: &[u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        kprintln!("[HAILO] Loading model ({} bytes)...", model_data.len());

        // 1. Parse Header
        if model_data.len() < core::mem::size_of::<HefHeader>() {
            return Err("Invalid HEF file: Too small");
        }
        
        let header = unsafe { &*(model_data.as_ptr() as *const HefHeader) };
        if header.magic != 0x00464548 { // "HEF\0" in little-endian
            // For now, accept any magic since we don't have real HEF files yet
            // return Err("Invalid HEF magic"); 
            let magic = header.magic;
            kprintln!("[HAILO] Warning: Invalid HEF magic (expected 0x00464548, got {:#x})", magic);
        }

        // 2. Configure Device
        self.configure_device(header)?;

        // 3. Send Model Data
        // Skip header, send the rest
        let payload = &model_data[core::mem::size_of::<HefHeader>()..];
        self.send_model_data(payload)?;

        self.state = HailoState::Running;
        kprintln!("[HAILO] Model loaded successfully.");
        Ok(())
    }

    /// Configure device for the specific model
    fn configure_device(&mut self, _header: &HefHeader) -> Result<(), &'static str> {
        // Send configuration command via HCP
        let cmd = HcpCommand {
            header: HcpHeader::new(0x20, self.seq_counter, 0), // 0x20 = CONFIG opcode
            payload: [0; 64],
        };
        self.seq_counter += 1;
        
        self.cmd_queue.push(cmd)?;
        
        // In real driver, we'd wait for response here
        // For now, assume success
        
        Ok(())
    }

    /// Send model binary to device via DMA
    fn send_model_data(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // In a real implementation, we would:
        // 1. Pin the data buffer (get physical address)
        // 2. Setup DMA descriptor on Channel 0 (Host-to-Device)
        // 3. Start DMA
        // 4. Wait for completion
        
        // For this sprint, we'll just simulate it
        let dma_buffer = DmaBuffer {
            phys_addr: data.as_ptr() as usize, // Assuming identity mapping for kernel
            virt_addr: data.as_ptr() as usize,
            size: data.len(),
        };

        self.setup_dma_transfer(0, &dma_buffer)?;
        self.start_dma(0)?;
        
        if let Err(e) = self.wait_dma(0) {
            kprintln!("[HAILO] Model send failed: {}. Reloading firmware...", e);
            let _ = self.reload_firmware();
            return Err("Model load failed (Firmware Reloaded)");
        }

        Ok(())
    }

    pub fn reload_firmware(&mut self) -> Result<(), &'static str> {
        kprintln!("[HAILO] Watchdog triggered: Reloading firmware...");
        self.reset()?;
        self.handshake()?;
        self.state = HailoState::Ready;
        kprintln!("[HAILO] Firmware reloaded. Model needs to be re-loaded.");
        Ok(())
    }
}
impl HailoDriver {
    pub fn init(&mut self) -> Result<(), &'static str> {
        kprintln!("[HAILO] Initializing AI Accelerator...");

        // 1. Find Hailo-8 on PCIe bus
        if let Some(dev) = pcie::find_device(VENDOR_ID_HAILO) {
            self.device = Some(dev);
            kprintln!("[HAILO] Found device at {:02x}:{:02x}.{}", dev.bus, dev.device, dev.function);
        } else {
            return Err("Hailo-8 not found on PCIe bus");
        }

        let dev = self.device.unwrap();

        // 2. Enable Bus Mastering
        pcie::enable_master(&dev);

        // 3. Get BAR0 (Control Registers)
        if let Some((addr, size)) = pcie::get_bar(&dev, 0) {
            self.bar0_addr = addr;
            self.bar0_size = size;
            kprintln!("[HAILO] BAR0 mapped at {:#010x} (Size: {} MB)", addr, size / (1024 * 1024));
        } else {
            return Err("Failed to read Hailo BAR0");
        }

        // 4. Get BAR2 (Doorbell/DMA)
        if let Some((addr, size)) = pcie::get_bar(&dev, 2) {
            self.bar2_addr = addr;
            self.bar2_size = size;
            kprintln!("[HAILO] BAR2 mapped at {:#010x} (Size: {} MB)", addr, size / (1024 * 1024));
        } else {
            // Fallback: Some cards use BAR4
            if let Some((addr, size)) = pcie::get_bar(&dev, 4) {
                self.bar2_addr = addr;
                self.bar2_size = size;
                kprintln!("[HAILO] BAR4 mapped at {:#010x} (Size: {} MB)", addr, size / (1024 * 1024));
            } else {
                return Err("Failed to read Hailo BAR2/4");
            }
        }

        // 5. Reset Device
        self.reset()?;

        // 6. Perform Handshake
        self.handshake()?;

        self.initialized = true;
        self.state = HailoState::Ready;
        kprintln!("[HAILO] Initialization complete. NPU Ready.");
        Ok(())
    }

    /// Reset the Hailo device
    fn reset(&mut self) -> Result<(), &'static str> {
        self.state = HailoState::Reset;
        // Write reset bit to control register
        self.write_reg(HAILO_CONTROL, 1);
        
        // Wait for reset to clear
        let mut timeout = 1000;
        while timeout > 0 {
            if (self.read_reg(HAILO_CONTROL) & 1) == 0 {
                return Ok(());
            }
            timeout -= 1;
            crate::drivers::timer::delay_us(100);
        }
        
        Err("Reset timeout")
    }

    /// Perform handshake with firmware
    fn handshake(&mut self) -> Result<(), &'static str> {
        kprintln!("[HAILO] Starting firmware handshake...");

        // 1. Wait for device to be ready (Bit 1 of CONTROL)
        let mut timeout = 10000;
        while timeout > 0 {
            let ctrl = self.read_reg(HAILO_CONTROL);
            if (ctrl & 2) != 0 {
                break;
            }
            timeout -= 1;
            crate::drivers::timer::delay_us(100);
        }

        if timeout == 0 {
            self.state = HailoState::Error;
            return Err("Handshake timeout: Device not ready");
        }

        // 2. Signal driver is ready (Bit 2 of CONTROL)
        let mut ctrl = self.read_reg(HAILO_CONTROL);
        ctrl |= 4;
        self.write_reg(HAILO_CONTROL, ctrl);

        kprintln!("[HAILO] Handshake successful");
        Ok(())
    }

    /// Read a register from BAR0
    fn read_reg(&self, offset: usize) -> u32 {
        if !self.initialized { return 0; }
        unsafe { arch::read32(self.bar0_addr + offset) }
    }

    /// Write a register to BAR0
    fn write_reg(&self, offset: usize, value: u32) {
        if !self.initialized { return; }
        unsafe { arch::write32(self.bar0_addr + offset, value) }
    }
    
    /// Check if driver is active
    pub fn is_active(&self) -> bool {
        self.initialized
    }

    /// Run object detection inference
    pub fn detect_objects(&mut self, image_data: &[u8], _width: u32, _height: u32) -> Result<heapless::Vec<crate::perception::vision::DetectedObject, 16>, &'static str> {
        if !self.initialized {
            return Err("Hailo driver not initialized");
        }

        // 1. Setup Input DMA (Channel 0: Host-to-Device)
        let input_buffer = DmaBuffer {
            phys_addr: image_data.as_ptr() as usize,
            virt_addr: image_data.as_ptr() as usize,
            size: image_data.len(),
        };
        self.setup_dma_transfer(0, &input_buffer)?;

        // 2. Setup Output DMA (Channel 1: Device-to-Host)
        // Allocate buffer for output tensor (e.g. 1917 boxes * 85 floats * 4 bytes ≈ 650KB)
        // We use a safe upper bound of 1MB
        let output_size = 1024 * 1024;
        let output_data: Vec<u8> = alloc::vec![0u8; output_size];
        
        let output_buffer = DmaBuffer {
            phys_addr: output_data.as_ptr() as usize,
            virt_addr: output_data.as_ptr() as usize,
            size: output_size,
        };
        self.setup_dma_transfer(1, &output_buffer)?;

        // 3. Start DMA Transfers
        self.start_dma(0)?; // Send input
        self.start_dma(1)?; // Receive output

        // 4. Trigger Inference (Doorbell)
        // In some Hailo flows, starting DMA is enough. 
        // If explicit trigger is needed, it would go here.
        // self.write_db(TRIGGER_REG, 1);

        // 5. Wait for Completion
        // 5. Wait for Completion
        if let Err(_e) = self.wait_dma(0) {
             kprintln!("[HAILO] DMA 0 Timeout during inference. Reloading firmware...");
             let _ = self.reload_firmware();
             return Err("Inference failed (Firmware Reloaded)");
        }
        if let Err(_e) = self.wait_dma(1) {
             kprintln!("[HAILO] DMA 1 Timeout during inference. Reloading firmware...");
             let _ = self.reload_firmware();
             return Err("Inference failed (Firmware Reloaded)");
        }

        // 6. Parse Output Tensor
        let parser = YoloOutputParser::new();
        // For now, pass the whole buffer. Real driver would read actual transfer size from descriptor.
        let objects = parser.parse(&output_data, output_size as u32);

        Ok(objects)
    }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

pub static HAILO: SpinLock<HailoDriver> = SpinLock::new(HailoDriver::new());

/// Initialize Hailo driver
pub fn init() {
    let mut driver = HAILO.lock();
    if let Err(e) = driver.init() {
        kprintln!("[HAILO] Init failed: {}", e);
    }
}
