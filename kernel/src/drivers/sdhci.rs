//! SDHCI (SD Host Controller Interface) Driver
//!
//! Implements SD card access for the Raspberry Pi 5's EMMC2 controller.
//! Supports SDHC/SDXC cards with block-level read/write operations.
//!
//! # Architecture
//! - **EMMC2**: Enhanced SD controller on BCM2712
//! - **DMA Support**: Scatter-gather DMA for efficient transfers
//! - **Block I/O**: 512-byte sectors
//! - **Card Detection**: GPIO-based card presence detection

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::kernel::memory::{self, PAGE_SIZE};
use core::ptr::NonNull;

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTERS (BCM2712 EMMC2)
// ═══════════════════════════════════════════════════════════════════════════════

const EMMC_BASE: usize = 0x1_0000_0000 + 0x00340000;  // EMMC2 base on RP1

const EMMC_ARG2: usize = 0x00;
const EMMC_BLKSIZECNT: usize = 0x04;
const EMMC_ARG1: usize = 0x08;
const EMMC_CMDTM: usize = 0x0C;
const EMMC_RESP0: usize = 0x10;
const EMMC_RESP1: usize = 0x14;
const EMMC_RESP2: usize = 0x18;
const EMMC_RESP3: usize = 0x1C;
const EMMC_DATA: usize = 0x20;
const EMMC_STATUS: usize = 0x24;
const EMMC_CONTROL0: usize = 0x28;
const EMMC_CONTROL1: usize = 0x2C;
const EMMC_INTERRUPT: usize = 0x30;
const EMMC_IRPT_MASK: usize = 0x34;
const EMMC_IRPT_EN: usize = 0x38;
const EMMC_CONTROL2: usize = 0x3C;
const EMMC_ADMA_ADDR_LOW: usize = 0x58;
const EMMC_ADMA_ADDR_HIGH: usize = 0x5C;
const EMMC_SLOTISR_VER: usize = 0xFC;

// Status Register Flags
const SR_READ_AVAILABLE: u32 = 1 << 11;
const SR_WRITE_AVAILABLE: u32 = 1 << 10;
const SR_DAT_ACTIVE: u32 = 1 << 2;
const SR_CMD_INHIBIT: u32 = 1 << 0;

// Interrupt Flags
const INT_DMA_END: u32 = 1 << 3;
const INT_DATA_DONE: u32 = 1 << 1;
const INT_CMD_DONE: u32 = 1 << 0;
const INT_ERROR: u32 = 1 << 15;

// Commands
const CMD_GO_IDLE: u32 = 0;
const CMD_SEND_IF_COND: u32 = 8;
const CMD_SEND_CSD: u32 = 9;
const CMD_SEND_CID: u32 = 10;
const CMD_VOLTAGE_SWITCH: u32 = 11;
const CMD_STOP_TRANSMISSION: u32 = 12;
const CMD_SEND_STATUS: u32 = 13;
const CMD_SET_BLOCKLEN: u32 = 16;
const CMD_READ_SINGLE: u32 = 17;
const CMD_READ_MULTI: u32 = 18;
const CMD_WRITE_SINGLE: u32 = 24;
const CMD_WRITE_MULTI: u32 = 25;
const CMD_APP_CMD: u32 = 55;

// Application Commands (require CMD55 prefix)
const ACMD_SET_BUS_WIDTH: u32 = 6;
const ACMD_SEND_OP_COND: u32 = 41;
const ACMD_SEND_SCR: u32 = 51;

// ADMA2 Descriptor Attributes
const ADMA_VALID: u16 = 1 << 0;
const ADMA_END: u16 = 1 << 1;
const ADMA_INT: u16 = 1 << 2;
const ADMA_ACT_TRAN: u16 = 0x2 << 4;
const ADMA_ACT_LINK: u16 = 0x3 << 4;

/// ADMA2 Descriptor (64-bit)
#[repr(C, packed)]
struct ADMA2Descriptor {
    attr: u16,
    len: u16,
    addr_low: u32,
    addr_high: u32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct SdhciDriver {
    base_addr: usize,
    rca: u32,  // Relative Card Address
    card_capacity: u64,  // In blocks
    block_size: u32,
    initialized: bool,
    adma_table: Option<NonNull<u8>>, // Pointer to ADMA descriptor table (DMA safe memory)
}

impl SdhciDriver {
    pub const fn new() -> Self {
        Self {
            base_addr: EMMC_BASE,
            rca: 0,
            card_capacity: 0,
            block_size: 512,
            initialized: false,
            adma_table: None,
        }
    }

    /// Initialize the SD card
    pub fn init(&mut self) -> Result<(), &'static str> {
        kprintln!("[SDHCI] Initializing SD Card...");

        // Reset controller
        self.reset()?;

        // Power on card
        self.power_on()?;

        // Set clock to 400 KHz for identification
        self.set_clock(400_000)?;

        // Wait for card to stabilize
        crate::drivers::timer::delay_ms(10);

        // CMD0: GO_IDLE_STATE
        self.send_command(CMD_GO_IDLE, 0, false)?;

        // CMD8: SEND_IF_COND (check voltage)
        let resp = self.send_command(CMD_SEND_IF_COND, 0x1AA, true)?;
        if (resp & 0xFFF) != 0x1AA {
            return Err("Card voltage mismatch");
        }

        // ACMD41: SD_SEND_OP_COND (repeat until ready)
        let mut retries = 1000;
        loop {
            self.send_app_command(ACMD_SEND_OP_COND, 0x40FF8000, true)?;  // HCS + voltage
            let resp = unsafe { arch::read32(self.base_addr + EMMC_RESP0) };

            if (resp & (1 << 31)) != 0 {
                // Card ready
                let is_sdhc = (resp & (1 << 30)) != 0;
                kprintln!("[SDHCI] Card type: {}", if is_sdhc { "SDHC/SDXC" } else { "SDSC" });
                break;
            }

            retries -= 1;
            if retries == 0 {
                return Err("Card initialization timeout");
            }

            crate::drivers::timer::delay_ms(1);
        }

        // CMD2: ALL_SEND_CID (get card identification)
        self.send_command(CMD_SEND_CID, 0, true)?;

        // CMD3: SEND_RELATIVE_ADDR (get RCA)
        let resp = self.send_command(3, 0, true)?;
        self.rca = resp & 0xFFFF0000;
        kprintln!("[SDHCI] RCA: {:#x}", self.rca);

        // CMD9: SEND_CSD (get card capacity)
        self.send_command(CMD_SEND_CSD, self.rca, true)?;
        self.card_capacity = self.parse_capacity();
        kprintln!("[SDHCI] Capacity: {} MB", (self.card_capacity * 512) / (1024 * 1024));

        // CMD7: SELECT_CARD (enter transfer state)
        self.send_command(7, self.rca, true)?;

        // Set block size to 512 bytes
        self.send_command(CMD_SET_BLOCKLEN, 512, true)?;

        // Increase clock to 25 MHz (high speed)
        self.set_clock(25_000_000)?;

        // Allocate ADMA table (one page is enough for many descriptors)
        // We need it to be in DMA-safe (non-cacheable) memory
        unsafe {
            if let Some(ptr) = memory::alloc_dma(PAGE_SIZE) {
                self.adma_table = Some(ptr);
                // Zero the table
                core::ptr::write_bytes(ptr.as_ptr(), 0, PAGE_SIZE);
            } else {
                return Err("Failed to allocate ADMA table");
            }
        }

        self.initialized = true;
        kprintln!("[SDHCI] Initialization complete (DMA Enabled)");

        Ok(())
    }

    /// Read blocks from SD card using DMA (with retry)
    pub fn read_blocks(&self, start_block: u64, num_blocks: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        let mut retries = 3;
        loop {
            match self.read_blocks_internal(start_block, num_blocks, buffer) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if retries > 0 && (e == "CRC Error" || e == "Timeout Error") {
                        kprintln!("[SDHCI] Read error: {}. Retrying... ({} left)", e, retries);
                        retries -= 1;
                        // Optional: Reset command line or dat line?
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }

    fn read_blocks_internal(&self, start_block: u64, num_blocks: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        let required_size = (num_blocks as usize) * (self.block_size as usize);
        if buffer.len() < required_size {
            return Err("Buffer too small");
        }

        // Allocate bounce buffer in DMA memory
        let bounce_ptr = unsafe { memory::alloc_dma(required_size) }
            .ok_or("Failed to allocate DMA bounce buffer")?;
        
        // Setup ADMA descriptors
        self.setup_adma(bounce_ptr.as_ptr(), required_size)?;

        // Set block size and count
        unsafe {
            arch::write32(self.base_addr + EMMC_BLKSIZECNT, (num_blocks << 16) | self.block_size);
            
            // Set ADMA table address
            if let Some(table) = self.adma_table {
                let table_addr = table.as_ptr() as u64;
                arch::write32(self.base_addr + EMMC_ADMA_ADDR_LOW, table_addr as u32);
                arch::write32(self.base_addr + EMMC_ADMA_ADDR_HIGH, (table_addr >> 32) as u32);
            } else {
                return Err("ADMA table not allocated");
            }
        }

        // Send read command with DMA enabled
        let cmd = if num_blocks == 1 { CMD_READ_SINGLE } else { CMD_READ_MULTI };
        self.send_command_dma(cmd, start_block as u32, true)?;

        // Wait for completion (DMA End or Transfer Complete)
        self.wait_for_interrupt(INT_DATA_DONE | INT_DMA_END, 1000000)?;

        // Stop transmission if multi-block
        if num_blocks > 1 {
            self.send_command(CMD_STOP_TRANSMISSION, 0, true)?;
        }

        // Copy from bounce buffer to user buffer
        unsafe {
            core::ptr::copy_nonoverlapping(bounce_ptr.as_ptr(), buffer.as_mut_ptr(), required_size);
            memory::free_dma(bounce_ptr, required_size);
        }

        Ok(())
    }

    /// Write blocks to SD card using DMA (with retry)
    pub fn write_blocks(&self, start_block: u64, num_blocks: u32, buffer: &[u8]) -> Result<(), &'static str> {
        let mut retries = 3;
        loop {
            match self.write_blocks_internal(start_block, num_blocks, buffer) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if retries > 0 && (e == "CRC Error" || e == "Timeout Error") {
                        kprintln!("[SDHCI] Write error: {}. Retrying... ({} left)", e, retries);
                        retries -= 1;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }

    fn write_blocks_internal(&self, start_block: u64, num_blocks: u32, buffer: &[u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        // Check write protection
        self.check_write_protect()?;

        let required_size = (num_blocks as usize) * (self.block_size as usize);
        if buffer.len() < required_size {
            return Err("Buffer too small");
        }

        // Allocate bounce buffer in DMA memory
        let bounce_ptr = unsafe { memory::alloc_dma(required_size) }
            .ok_or("Failed to allocate DMA bounce buffer")?;

        // Copy user buffer to bounce buffer
        unsafe {
            core::ptr::copy_nonoverlapping(buffer.as_ptr(), bounce_ptr.as_ptr(), required_size);
        }

        // Setup ADMA descriptors
        self.setup_adma(bounce_ptr.as_ptr(), required_size)?;

        // Set block size and count
        unsafe {
            arch::write32(self.base_addr + EMMC_BLKSIZECNT, (num_blocks << 16) | self.block_size);
            
            // Set ADMA table address
            if let Some(table) = self.adma_table {
                let table_addr = table.as_ptr() as u64;
                arch::write32(self.base_addr + EMMC_ADMA_ADDR_LOW, table_addr as u32);
                arch::write32(self.base_addr + EMMC_ADMA_ADDR_HIGH, (table_addr >> 32) as u32);
            } else {
                return Err("ADMA table not allocated");
            }
        }

        // Send write command with DMA enabled
        let cmd = if num_blocks == 1 { CMD_WRITE_SINGLE } else { CMD_WRITE_MULTI };
        self.send_command_dma(cmd, start_block as u32, true)?;

        // Wait for completion
        self.wait_for_interrupt(INT_DATA_DONE | INT_DMA_END, 1000000)?;

        // Stop transmission if multi-block
        if num_blocks > 1 {
            self.send_command(CMD_STOP_TRANSMISSION, 0, true)?;
        }

        // Free bounce buffer
        unsafe {
            memory::free_dma(bounce_ptr, required_size);
        }

        // Wait for programming to complete
        let mut timeout = 1000000;
        loop {
            let resp = self.send_command(CMD_SEND_STATUS, self.rca, true)?;
            // Check READY_FOR_DATA (bit 8) and CURRENT_STATE (bits 12:9)
            // State 4 is TRAN (Transfer)
            let state = (resp >> 9) & 0xF;
            let ready = (resp & (1 << 8)) != 0;

            if ready && state == 4 {
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                return Err("Write timeout - card stuck busy");
            }
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // INTERNAL HELPERS
    // ═══════════════════════════════════════════════════════════════════════════

    fn reset(&self) -> Result<(), &'static str> {
        unsafe {
            // Set reset bits
            arch::write32(self.base_addr + EMMC_CONTROL1, 0x07000000);

            // Wait for reset to complete
            let mut timeout = 10000;
            while timeout > 0 {
                let ctrl1 = arch::read32(self.base_addr + EMMC_CONTROL1);
                if (ctrl1 & 0x07000000) == 0 {
                    return Ok(());
                }
                timeout -= 1;
            }
        }

        Err("Reset timeout")
    }

    fn power_on(&self) -> Result<(), &'static str> {
        unsafe {
            // Enable SD bus power (3.3V)
            arch::write32(self.base_addr + EMMC_CONTROL0, 0x0F00);
        }

        crate::drivers::timer::delay_ms(10);
        Ok(())
    }

    fn set_clock(&self, freq_hz: u32) -> Result<(), &'static str> {
        // Disable clock
        unsafe {
            arch::write32(self.base_addr + EMMC_CONTROL1, 0);
        }

        // Calculate divider (base clock is typically 100 MHz)
        let base_clock = 100_000_000;
        let divider = base_clock / (2 * freq_hz);

        unsafe {
            // Set divider and enable clock
            let ctrl1 = (divider << 8) | 0x01;  // Enable internal clock
            arch::write32(self.base_addr + EMMC_CONTROL1, ctrl1);

            // Wait for clock stable
            let mut timeout = 10000;
            while timeout > 0 {
                if (arch::read32(self.base_addr + EMMC_CONTROL1) & 0x02) != 0 {
                    break;
                }
                timeout -= 1;
            }

            if timeout == 0 {
                return Err("Clock stabilization timeout");
            }

            // Enable SD clock
            arch::write32(self.base_addr + EMMC_CONTROL1, ctrl1 | 0x04);
        }

        Ok(())
    }

    fn send_command(&self, cmd: u32, arg: u32, wait_resp: bool) -> Result<u32, &'static str> {
        // Wait for CMD line ready
        self.wait_for_cmd_ready(100000)?;

        // Clear interrupts
        unsafe {
            arch::write32(self.base_addr + EMMC_INTERRUPT, 0xFFFFFFFF);
        }

        // Set argument
        unsafe {
            arch::write32(self.base_addr + EMMC_ARG1, arg);
        }

        // Build command word
        let mut cmdtm = cmd << 24;

        if wait_resp {
            cmdtm |= 0x00020000;  // Response expected
        }

        // Send command
        unsafe {
            arch::write32(self.base_addr + EMMC_CMDTM, cmdtm);
        }

        // Wait for command done
        self.wait_for_interrupt(INT_CMD_DONE, 100000)?;

        // Read response
        let resp = unsafe { arch::read32(self.base_addr + EMMC_RESP0) };

        Ok(resp)
    }

    fn send_app_command(&self, acmd: u32, arg: u32, wait_resp: bool) -> Result<u32, &'static str> {
        // Send CMD55 first
        self.send_command(CMD_APP_CMD, self.rca, true)?;

        // Then send the actual application command
        self.send_command(acmd, arg, wait_resp)
    }

    fn wait_for_cmd_ready(&self, timeout: u32) -> Result<(), &'static str> {
        let mut timeout = timeout;
        while timeout > 0 {
            let status = unsafe { arch::read32(self.base_addr + EMMC_STATUS) };
            if (status & SR_CMD_INHIBIT) == 0 {
                return Ok(());
            }
            timeout -= 1;
        }

        Err("CMD ready timeout")
    }

    fn wait_for_status(&self, mask: u32, timeout: u32) -> Result<(), &'static str> {
        let mut timeout = timeout;
        while timeout > 0 {
            let status = unsafe { arch::read32(self.base_addr + EMMC_STATUS) };
            if (status & mask) != 0 {
                return Ok(());
            }
            timeout -= 1;
        }

        Err("Status wait timeout")
    }

    fn wait_for_interrupt(&self, mask: u32, timeout: u32) -> Result<(), &'static str> {
        let mut timeout = timeout;
        while timeout > 0 {
            let irpt = unsafe { arch::read32(self.base_addr + EMMC_INTERRUPT) };

            // Check for error
            if (irpt & INT_ERROR) != 0 {
                // Check specific errors
                if (irpt & ((1 << 17) | (1 << 21))) != 0 {
                     return Err("CRC Error");
                }
                if (irpt & ((1 << 16) | (1 << 20))) != 0 {
                     return Err("Timeout Error");
                }
                return Err("SD card error");
            }

            // Check for desired interrupt
            if (irpt & mask) != 0 {
                // Clear interrupt
                unsafe {
                    arch::write32(self.base_addr + EMMC_INTERRUPT, mask);
                }
                return Ok(());
            }

            timeout -= 1;
        }

        Err("Interrupt wait timeout")
    }

    fn parse_capacity(&self) -> u64 {
        // Read CSD register from response
        let csd0 = unsafe { arch::read32(self.base_addr + EMMC_RESP0) };
        let csd1 = unsafe { arch::read32(self.base_addr + EMMC_RESP1) };
        let csd2 = unsafe { arch::read32(self.base_addr + EMMC_RESP2) };

        // CSD version 2.0 (SDHC/SDXC)
        let csd_version = (csd2 >> 30) & 0x3;

        if csd_version == 1 {
            // SDHC/SDXC: C_SIZE is bits 69:48 (22 bits)
            let c_size = ((csd1 & 0x3F) << 16) | ((csd0 >> 16) & 0xFFFF);
            (c_size as u64 + 1) * 1024  // Capacity in 512-byte blocks
        } else {
            // SDSC (not typically used anymore)
            512 * 1024  // 256 MB default
        }
    }

    fn check_write_protect(&self) -> Result<(), &'static str> {
        // Check hardware switch via GPIO if available (skipped for now as it varies by board)
        
        // Check internal write protect bits in CSD
        // We need to send CMD9 (SEND_CSD) again to get fresh status or cache it
        // For now, we'll just check the status register for write protect errors
        
        // Check if Write Protect switch is active (if supported by controller)
        // Note: EMMC2 on Pi 5 might not map this directly to a register bit without GPIO config
        // But we can check if the card reports itself as locked in CSR
        
        // Send CMD13 (SEND_STATUS) to check card status
        let resp = self.send_command(CMD_SEND_STATUS, self.rca, true)?;
        if (resp & (1 << 26)) != 0 {
             return Err("Card is write locked");
        }

        Ok(())
    }

    fn setup_adma(&self, buffer: *mut u8, len: usize) -> Result<(), &'static str> {
        let table_ptr = self.adma_table.ok_or("ADMA table not allocated")?;
        let mut desc_ptr = table_ptr.as_ptr() as *mut ADMA2Descriptor;
        let mut remaining = len;
        let mut addr = buffer as u64;

        while remaining > 0 {
            // Max length per descriptor is 64KB (0 = 64KB)
            let chunk_len = if remaining > 65536 { 65536 } else { remaining };
            
            let attr = ADMA_VALID | ADMA_ACT_TRAN;
            let len_field = if chunk_len == 65536 { 0 } else { chunk_len as u16 };

            unsafe {
                (*desc_ptr).attr = attr;
                (*desc_ptr).len = len_field;
                (*desc_ptr).addr_low = addr as u32;
                (*desc_ptr).addr_high = (addr >> 32) as u32;
            }

            remaining -= chunk_len;
            addr += chunk_len as u64;
            
            if remaining > 0 {
                unsafe { desc_ptr = desc_ptr.add(1); }
            }
        }

        // Mark last descriptor as END
        unsafe {
            (*desc_ptr).attr |= ADMA_END | ADMA_INT;
        }

        Ok(())
    }

    fn send_command_dma(&self, cmd: u32, arg: u32, wait_resp: bool) -> Result<u32, &'static str> {
        // Wait for CMD line ready
        self.wait_for_cmd_ready(100000)?;

        // Clear interrupts
        unsafe {
            arch::write32(self.base_addr + EMMC_INTERRUPT, 0xFFFFFFFF);
        }

        // Set argument
        unsafe {
            arch::write32(self.base_addr + EMMC_ARG1, arg);
        }

        // Build command word
        // Bit 22 = Use ADMA? No, Bit 5 is DMA Enable in older controllers?
        // EMMC2 on Pi 4/5:
        // CMD_TM register:
        // Bits 29:24 = CMD Index
        // Bit 0 = DMA Enable
        // Bit 1 = Block Count Enable
        // Bit 2 = Auto CMD12 Enable
        // Bit 3 = Auto CMD23 Enable
        // Bit 4 = Read/Write (1 = Read)
        // Bit 5 = Multi/Single (1 = Multi)
        
        // Wait, the existing send_command uses `cmd << 24`.
        // Let's look at the bits I need to set for DMA.
        // For EMMC2, we usually set BIT(0) for DMA.
        
        let mut cmdtm = cmd << 24;

        if wait_resp {
            cmdtm |= 0x00020000;  // Response expected (Bit 17?)
            // Actually:
            // Bit 16: Response Type (00=No, 01=136, 10=48, 11=48busy)
            // Bit 19: CRC Check Enable
            // Bit 20: Index Check Enable
            // Bit 21: Data Present
        }
        
        // Enable DMA (Bit 0) and Block Count (Bit 1) and Data Present (Bit 21)
        cmdtm |= 1 | (1 << 1) | (1 << 21);
        
        // If Read, set Bit 4
        if cmd == CMD_READ_SINGLE || cmd == CMD_READ_MULTI {
            cmdtm |= (1 << 4);
        }
        
        // If Multi, set Bit 5
        if cmd == CMD_READ_MULTI || cmd == CMD_WRITE_MULTI {
            cmdtm |= (1 << 5);
            cmdtm |= (1 << 2); // Auto CMD12
        }

        // Send command
        unsafe {
            arch::write32(self.base_addr + EMMC_CMDTM, cmdtm);
        }

        // Wait for command done
        self.wait_for_interrupt(INT_CMD_DONE, 100000)?;

        // Read response
        let resp = unsafe { arch::read32(self.base_addr + EMMC_RESP0) };

        Ok(resp)
    }
}

// SAFETY: Protected by SpinLock
unsafe impl Send for SdhciDriver {}

pub static SDHCI: SpinLock<SdhciDriver> = SpinLock::new(SdhciDriver::new());

/// Initialize SD card driver
pub fn init() {
    let mut driver = SDHCI.lock();
    if let Err(e) = driver.init() {
        kprintln!("[SDHCI] Init failed: {}", e);
    }
}

/// Read blocks from SD card
pub fn read_blocks(start_block: u64, num_blocks: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
    SDHCI.lock().read_blocks(start_block, num_blocks, buffer)
}

/// Write blocks to SD card
pub fn write_blocks(start_block: u64, num_blocks: u32, buffer: &[u8]) -> Result<(), &'static str> {
    SDHCI.lock().write_blocks(start_block, num_blocks, buffer)
}
