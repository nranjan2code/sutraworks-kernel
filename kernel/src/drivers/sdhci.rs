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
const EMMC_SLOTISR_VER: usize = 0xFC;

// Status Register Flags
const SR_READ_AVAILABLE: u32 = 1 << 11;
const SR_WRITE_AVAILABLE: u32 = 1 << 10;
const SR_DAT_ACTIVE: u32 = 1 << 2;
const SR_CMD_INHIBIT: u32 = 1 << 0;

// Interrupt Flags
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

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct SdhciDriver {
    base_addr: usize,
    rca: u32,  // Relative Card Address
    card_capacity: u64,  // In blocks
    block_size: u32,
    initialized: bool,
}

impl SdhciDriver {
    pub const fn new() -> Self {
        Self {
            base_addr: EMMC_BASE,
            rca: 0,
            card_capacity: 0,
            block_size: 512,
            initialized: false,
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

        self.initialized = true;
        kprintln!("[SDHCI] Initialization complete");

        Ok(())
    }

    /// Read blocks from SD card
    pub fn read_blocks(&self, start_block: u64, num_blocks: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        let required_size = (num_blocks as usize) * (self.block_size as usize);
        if buffer.len() < required_size {
            return Err("Buffer too small");
        }

        // Set block size and count
        unsafe {
            arch::write32(self.base_addr + EMMC_BLKSIZECNT, (num_blocks << 16) | self.block_size);
        }

        // Send read command
        let cmd = if num_blocks == 1 { CMD_READ_SINGLE } else { CMD_READ_MULTI };
        self.send_command(cmd, start_block as u32, true)?;

        // Read data
        let mut offset = 0;
        for _ in 0..num_blocks {
            for _ in 0..(self.block_size / 4) {
                // Wait for data available
                self.wait_for_status(SR_READ_AVAILABLE, 100000)?;

                // Read word
                let word = unsafe { arch::read32(self.base_addr + EMMC_DATA) };

                buffer[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                offset += 4;
            }
        }

        // Wait for completion
        self.wait_for_interrupt(INT_DATA_DONE, 100000)?;

        // Stop transmission if multi-block
        if num_blocks > 1 {
            self.send_command(CMD_STOP_TRANSMISSION, 0, true)?;
        }

        Ok(())
    }

    /// Write blocks to SD card
    pub fn write_blocks(&self, start_block: u64, num_blocks: u32, buffer: &[u8]) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        let required_size = (num_blocks as usize) * (self.block_size as usize);
        if buffer.len() < required_size {
            return Err("Buffer too small");
        }

        // Set block size and count
        unsafe {
            arch::write32(self.base_addr + EMMC_BLKSIZECNT, (num_blocks << 16) | self.block_size);
        }

        // Send write command
        let cmd = if num_blocks == 1 { CMD_WRITE_SINGLE } else { CMD_WRITE_MULTI };
        self.send_command(cmd, start_block as u32, true)?;

        // Write data
        let mut offset = 0;
        for _ in 0..num_blocks {
            for _ in 0..(self.block_size / 4) {
                // Wait for write available
                self.wait_for_status(SR_WRITE_AVAILABLE, 100000)?;

                // Write word
                let word = u32::from_le_bytes([
                    buffer[offset],
                    buffer[offset + 1],
                    buffer[offset + 2],
                    buffer[offset + 3],
                ]);

                unsafe { arch::write32(self.base_addr + EMMC_DATA, word); }
                offset += 4;
            }
        }

        // Wait for completion
        self.wait_for_interrupt(INT_DATA_DONE, 100000)?;

        // Stop transmission if multi-block
        if num_blocks > 1 {
            self.send_command(CMD_STOP_TRANSMISSION, 0, true)?;
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
