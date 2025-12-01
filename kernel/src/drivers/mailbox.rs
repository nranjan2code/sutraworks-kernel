//! VideoCore Mailbox Driver for Raspberry Pi 5
//!
//! The mailbox is the primary communication channel between the ARM cores
//! and the VideoCore GPU. It's used for:
//! - Framebuffer allocation
//! - Power management
//! - Clock configuration
//! - Memory allocation
//! - Hardware queries

use crate::arch;
use core::sync::atomic::{AtomicBool, Ordering};

// ═══════════════════════════════════════════════════════════════════════════════
// MAILBOX REGISTERS
// ═══════════════════════════════════════════════════════════════════════════════

// Mailbox base for RPi5 (BCM2712)
const MAILBOX_BASE: usize = 0x1_0000_B880;

// Register offsets
const MBOX_READ: usize = 0x00;
const MBOX_POLL: usize = 0x10;
const MBOX_SENDER: usize = 0x14;
const MBOX_STATUS: usize = 0x18;
const MBOX_CONFIG: usize = 0x1C;
const MBOX_WRITE: usize = 0x20;

// Status bits
const MBOX_FULL: u32 = 0x80000000;
const MBOX_EMPTY: u32 = 0x40000000;

// Channels
const MBOX_CHANNEL_POWER: u8 = 0;
const MBOX_CHANNEL_FRAMEBUF: u8 = 1;
const MBOX_CHANNEL_VUART: u8 = 2;
const MBOX_CHANNEL_VCHIQ: u8 = 3;
const MBOX_CHANNEL_LEDS: u8 = 4;
const MBOX_CHANNEL_BUTTONS: u8 = 5;
const MBOX_CHANNEL_TOUCH: u8 = 6;
const MBOX_CHANNEL_PROP: u8 = 8; // Property channel (ARM -> VC)

// ═══════════════════════════════════════════════════════════════════════════════
// PROPERTY TAGS
// ═══════════════════════════════════════════════════════════════════════════════

// Video Core
pub const TAG_GET_FIRMWARE_REV: u32 = 0x00000001;

// Hardware
pub const TAG_GET_BOARD_MODEL: u32 = 0x00010001;
pub const TAG_GET_BOARD_REV: u32 = 0x00010002;
pub const TAG_GET_MAC_ADDRESS: u32 = 0x00010003;
pub const TAG_GET_BOARD_SERIAL: u32 = 0x00010004;
pub const TAG_GET_ARM_MEMORY: u32 = 0x00010005;
pub const TAG_GET_VC_MEMORY: u32 = 0x00010006;
pub const TAG_GET_CLOCKS: u32 = 0x00010007;

// Power
pub const TAG_GET_POWER_STATE: u32 = 0x00020001;
pub const TAG_GET_TIMING: u32 = 0x00020002;
pub const TAG_SET_POWER_STATE: u32 = 0x00028001;

// Clocks
pub const TAG_GET_CLOCK_STATE: u32 = 0x00030001;
pub const TAG_SET_CLOCK_STATE: u32 = 0x00038001;
pub const TAG_GET_CLOCK_RATE: u32 = 0x00030002;
pub const TAG_SET_CLOCK_RATE: u32 = 0x00038002;
pub const TAG_GET_MAX_CLOCK_RATE: u32 = 0x00030004;
pub const TAG_GET_MIN_CLOCK_RATE: u32 = 0x00030007;
pub const TAG_GET_TURBO: u32 = 0x00030009;
pub const TAG_SET_TURBO: u32 = 0x00038009;

// Voltage
pub const TAG_GET_VOLTAGE: u32 = 0x00030003;
pub const TAG_SET_VOLTAGE: u32 = 0x00038003;
pub const TAG_GET_MAX_VOLTAGE: u32 = 0x00030005;
pub const TAG_GET_MIN_VOLTAGE: u32 = 0x00030008;

// Temperature
pub const TAG_GET_TEMPERATURE: u32 = 0x00030006;
pub const TAG_GET_MAX_TEMPERATURE: u32 = 0x0003000A;

// Memory
pub const TAG_ALLOCATE_MEMORY: u32 = 0x0003000C;
pub const TAG_LOCK_MEMORY: u32 = 0x0003000D;
pub const TAG_UNLOCK_MEMORY: u32 = 0x0003000E;
pub const TAG_RELEASE_MEMORY: u32 = 0x0003000F;

// Framebuffer
pub const TAG_ALLOCATE_BUFFER: u32 = 0x00040001;
pub const TAG_RELEASE_BUFFER: u32 = 0x00048001;
pub const TAG_BLANK_SCREEN: u32 = 0x00040002;
pub const TAG_GET_PHYSICAL_SIZE: u32 = 0x00040003;
pub const TAG_TEST_PHYSICAL_SIZE: u32 = 0x00044003;
pub const TAG_SET_PHYSICAL_SIZE: u32 = 0x00048003;
pub const TAG_GET_VIRTUAL_SIZE: u32 = 0x00040004;
pub const TAG_TEST_VIRTUAL_SIZE: u32 = 0x00044004;
pub const TAG_SET_VIRTUAL_SIZE: u32 = 0x00048004;
pub const TAG_GET_DEPTH: u32 = 0x00040005;
pub const TAG_TEST_DEPTH: u32 = 0x00044005;
pub const TAG_SET_DEPTH: u32 = 0x00048005;
pub const TAG_GET_PIXEL_ORDER: u32 = 0x00040006;
pub const TAG_TEST_PIXEL_ORDER: u32 = 0x00044006;
pub const TAG_SET_PIXEL_ORDER: u32 = 0x00048006;
pub const TAG_GET_ALPHA_MODE: u32 = 0x00040007;
pub const TAG_TEST_ALPHA_MODE: u32 = 0x00044007;
pub const TAG_SET_ALPHA_MODE: u32 = 0x00048007;
pub const TAG_GET_PITCH: u32 = 0x00040008;
pub const TAG_GET_VIRTUAL_OFFSET: u32 = 0x00040009;
pub const TAG_TEST_VIRTUAL_OFFSET: u32 = 0x00044009;
pub const TAG_SET_VIRTUAL_OFFSET: u32 = 0x00048009;
pub const TAG_GET_OVERSCAN: u32 = 0x0004000A;
pub const TAG_TEST_OVERSCAN: u32 = 0x0004400A;
pub const TAG_SET_OVERSCAN: u32 = 0x0004800A;
pub const TAG_GET_PALETTE: u32 = 0x0004000B;
pub const TAG_TEST_PALETTE: u32 = 0x0004400B;
pub const TAG_SET_PALETTE: u32 = 0x0004800B;

// End tag
pub const TAG_END: u32 = 0;

// Request/Response codes
const REQUEST_CODE: u32 = 0;
const RESPONSE_SUCCESS: u32 = 0x80000000;
const RESPONSE_ERROR: u32 = 0x80000001;

// ═══════════════════════════════════════════════════════════════════════════════
// CLOCK IDS
// ═══════════════════════════════════════════════════════════════════════════════

pub const CLOCK_EMMC: u32 = 1;
pub const CLOCK_UART: u32 = 2;
pub const CLOCK_ARM: u32 = 3;
pub const CLOCK_CORE: u32 = 4;
pub const CLOCK_V3D: u32 = 5;
pub const CLOCK_H264: u32 = 6;
pub const CLOCK_ISP: u32 = 7;
pub const CLOCK_SDRAM: u32 = 8;
pub const CLOCK_PIXEL: u32 = 9;
pub const CLOCK_PWM: u32 = 10;
pub const CLOCK_HEVC: u32 = 11;
pub const CLOCK_EMMC2: u32 = 12;
pub const CLOCK_M2MC: u32 = 13;
pub const CLOCK_PIXEL_BVB: u32 = 14;

// ═══════════════════════════════════════════════════════════════════════════════
// DEVICE IDS
// ═══════════════════════════════════════════════════════════════════════════════

pub const DEVICE_SD_CARD: u32 = 0;
pub const DEVICE_UART0: u32 = 1;
pub const DEVICE_UART1: u32 = 2;
pub const DEVICE_USB_HCD: u32 = 3;
pub const DEVICE_I2C0: u32 = 4;
pub const DEVICE_I2C1: u32 = 5;
pub const DEVICE_I2C2: u32 = 6;
pub const DEVICE_SPI: u32 = 7;
pub const DEVICE_CCP2TX: u32 = 8;

// ═══════════════════════════════════════════════════════════════════════════════
// MAILBOX BUFFER
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum buffer size (must be 16-byte aligned)
const BUFFER_SIZE: usize = 256;

/// Mailbox property buffer (aligned to 16 bytes for DMA)
#[repr(C, align(16))]
pub struct PropertyBuffer {
    data: [u32; BUFFER_SIZE / 4],
}

impl PropertyBuffer {
    /// Create a new empty buffer
    pub const fn new() -> Self {
        PropertyBuffer {
            data: [0; BUFFER_SIZE / 4],
        }
    }
    
    /// Get buffer address (must be in first 1GB for GPU access)
    pub fn address(&self) -> u32 {
        // Convert to bus address (add 0xC0000000 for uncached access)
        (self.data.as_ptr() as usize as u32) | 0xC0000000
    }
    
    /// Clear the buffer
    pub fn clear(&mut self) {
        for i in 0..self.data.len() {
            self.data[i] = 0;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MAILBOX DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

static MAILBOX_LOCK: arch::SpinLock<()> = arch::SpinLock::new(());
static MAILBOX_INIT: AtomicBool = AtomicBool::new(false);

/// Read from mailbox register
fn mbox_read(reg: usize) -> u32 {
    unsafe { arch::read32(MAILBOX_BASE + reg) }
}

/// Write to mailbox register
fn mbox_write(reg: usize, value: u32) {
    unsafe { arch::write32(MAILBOX_BASE + reg, value) }
}

/// Wait for mailbox to be ready for writing
fn mbox_wait_write() {
    while (mbox_read(MBOX_STATUS) & MBOX_FULL) != 0 {
        core::hint::spin_loop();
    }
}

/// Wait for mailbox to have data
fn mbox_wait_read() {
    while (mbox_read(MBOX_STATUS) & MBOX_EMPTY) != 0 {
        core::hint::spin_loop();
    }
}

/// Send a message to the mailbox
/// Returns true on success
pub fn call(buffer: &mut PropertyBuffer, channel: u8) -> bool {
    let _guard = MAILBOX_LOCK.lock();
    
    // Memory barrier before GPU access
    unsafe { core::arch::asm!("dmb sy") };
    
    // Prepare message (address with channel in low 4 bits)
    let message = (buffer.address() & !0xF) | (channel as u32 & 0xF);
    
    // Wait for mailbox to be ready
    mbox_wait_write();
    
    // Write to mailbox
    mbox_write(MBOX_WRITE, message);
    
    // Wait for response
    loop {
        mbox_wait_read();
        
        let response = mbox_read(MBOX_READ);
        
        // Check if this is our response (same channel)
        if (response & 0xF) == (channel as u32) {
            // Memory barrier after GPU access
            unsafe { core::arch::asm!("dmb sy") };
            
            // Check response code
            return buffer.data[1] == RESPONSE_SUCCESS;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HIGH-LEVEL API
// ═══════════════════════════════════════════════════════════════════════════════

/// Property request builder
pub struct PropertyRequest<'a> {
    buffer: &'a mut PropertyBuffer,
    index: usize,
}

impl<'a> PropertyRequest<'a> {
    /// Create a new property request
    pub fn new(buffer: &'a mut PropertyBuffer) -> Self {
        buffer.clear();
        // Skip header (size + request code)
        PropertyRequest { buffer, index: 2 }
    }
    
    /// Add a tag with values
    pub fn add_tag(&mut self, tag: u32, values: &[u32]) -> &mut Self {
        let value_size = (values.len() * 4) as u32;
        
        self.buffer.data[self.index] = tag;
        self.buffer.data[self.index + 1] = value_size;
        self.buffer.data[self.index + 2] = 0; // Request indicator
        
        for (i, &v) in values.iter().enumerate() {
            self.buffer.data[self.index + 3 + i] = v;
        }
        
        // Move to next tag position (round up to 4-byte boundary)
        self.index += 3 + ((value_size as usize + 3) / 4);
        self
    }
    
    /// Add a tag with no values
    pub fn add_tag_empty(&mut self, tag: u32, response_size: u32) -> &mut Self {
        self.buffer.data[self.index] = tag;
        self.buffer.data[self.index + 1] = response_size;
        self.buffer.data[self.index + 2] = 0;
        
        self.index += 3 + ((response_size as usize + 3) / 4);
        self
    }
    
    /// Finalize and send the request
    pub fn send(&mut self) -> bool {
        // Add end tag
        self.buffer.data[self.index] = TAG_END;
        
        // Set total size
        self.buffer.data[0] = ((self.index + 1) * 4) as u32;
        self.buffer.data[1] = REQUEST_CODE;
        
        call(self.buffer, MBOX_CHANNEL_PROP)
    }
}

/// Get value at tag offset in buffer
pub fn get_tag_value(buffer: &PropertyBuffer, tag: u32, value_index: usize) -> Option<u32> {
    let mut index = 2;
    
    while index < buffer.data.len() {
        let current_tag = buffer.data[index];
        
        if current_tag == TAG_END {
            break;
        }
        
        let size = buffer.data[index + 1] as usize;
        
        if current_tag == tag {
            let value_offset = index + 3 + value_index;
            if value_offset < buffer.data.len() {
                return Some(buffer.data[value_offset]);
            }
        }
        
        index += 3 + ((size + 3) / 4);
    }
    
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONVENIENCE FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize mailbox
pub fn init() {
    MAILBOX_INIT.store(true, Ordering::SeqCst);
}

/// Get firmware revision
pub fn get_firmware_revision() -> Option<u32> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag_empty(TAG_GET_FIRMWARE_REV, 4);
    
    if req.send() {
        get_tag_value(&buffer, TAG_GET_FIRMWARE_REV, 0)
    } else {
        None
    }
}

/// Get board model
pub fn get_board_model() -> Option<u32> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag_empty(TAG_GET_BOARD_MODEL, 4);
    
    if req.send() {
        get_tag_value(&buffer, TAG_GET_BOARD_MODEL, 0)
    } else {
        None
    }
}

/// Get board revision
pub fn get_board_revision() -> Option<u32> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag_empty(TAG_GET_BOARD_REV, 4);
    
    if req.send() {
        get_tag_value(&buffer, TAG_GET_BOARD_REV, 0)
    } else {
        None
    }
}

/// Get ARM memory base and size
pub fn get_arm_memory() -> Option<(u32, u32)> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag_empty(TAG_GET_ARM_MEMORY, 8);
    
    if req.send() {
        let base = get_tag_value(&buffer, TAG_GET_ARM_MEMORY, 0)?;
        let size = get_tag_value(&buffer, TAG_GET_ARM_MEMORY, 1)?;
        Some((base, size))
    } else {
        None
    }
}

/// Get VideoCore memory base and size
pub fn get_vc_memory() -> Option<(u32, u32)> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag_empty(TAG_GET_VC_MEMORY, 8);
    
    if req.send() {
        let base = get_tag_value(&buffer, TAG_GET_VC_MEMORY, 0)?;
        let size = get_tag_value(&buffer, TAG_GET_VC_MEMORY, 1)?;
        Some((base, size))
    } else {
        None
    }
}

/// Get clock rate in Hz
pub fn get_clock_rate(clock_id: u32) -> Option<u32> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag(TAG_GET_CLOCK_RATE, &[clock_id, 0]);
    
    if req.send() {
        get_tag_value(&buffer, TAG_GET_CLOCK_RATE, 1)
    } else {
        None
    }
}

/// Set clock rate
pub fn set_clock_rate(clock_id: u32, rate_hz: u32) -> Option<u32> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    // clock_id, rate, skip_turbo
    req.add_tag(TAG_SET_CLOCK_RATE, &[clock_id, rate_hz, 0]);
    
    if req.send() {
        get_tag_value(&buffer, TAG_SET_CLOCK_RATE, 1)
    } else {
        None
    }
}

/// Get temperature in millidegrees Celsius
pub fn get_temperature() -> Option<u32> {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    req.add_tag(TAG_GET_TEMPERATURE, &[0, 0]);
    
    if req.send() {
        get_tag_value(&buffer, TAG_GET_TEMPERATURE, 1)
    } else {
        None
    }
}

/// Set power state for a device
pub fn set_power_state(device_id: u32, on: bool, wait: bool) -> bool {
    let mut buffer = PropertyBuffer::new();
    let mut req = PropertyRequest::new(&mut buffer);
    
    let state = (on as u32) | ((wait as u32) << 1);
    req.add_tag(TAG_SET_POWER_STATE, &[device_id, state]);
    
    req.send()
}

// ═══════════════════════════════════════════════════════════════════════════════
// BOARD INFO
// ═══════════════════════════════════════════════════════════════════════════════

/// Board information structure
#[derive(Clone, Copy)]
pub struct BoardInfo {
    pub board_model: u32,
    pub board_revision: u32,
    pub arm_memory: u32,
    pub vc_memory: u32,
    pub serial: u64,
}

/// Get comprehensive board information
pub fn get_board_info() -> Option<BoardInfo> {
    let board_model = get_board_model().unwrap_or(0);
    let board_revision = get_board_revision().unwrap_or(0);
    let (_, arm_memory) = get_arm_memory().unwrap_or((0, 0));
    let (_, vc_memory) = get_vc_memory().unwrap_or((0, 0));
    
    Some(BoardInfo {
        board_model,
        board_revision,
        arm_memory,
        vc_memory,
        serial: 0, // Would need separate query
    })
}
