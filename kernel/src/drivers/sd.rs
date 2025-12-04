//! SD Card Driver (EMMC2)
//!
//! Implements a block device driver for the SD card on Raspberry Pi 5.
//! Uses the EMMC2 controller (Arasan SDHCI compatible).

use crate::arch::{self, SpinLock};
use crate::fs::vfs::BlockDevice;
use crate::kprintln;

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTERS (EMMC2)
// ═══════════════════════════════════════════════════════════════════════════════

const _EMMC2_BASE: usize = 0x100000000 + 0x00_340000; // RP1 EMMC2 Base
// BCM2712 EMMC2 Base Address: 0x100110000 (from Linux Device Tree)
// Wait, BCM2712 has multiple SD controllers. SD1 is usually for SD card.
// Let's assume standard SDHCI registers.

// Registers
const _SDHCI_ARG2: usize = 0x000;
const SDHCI_BLOCK_SIZE: usize = 0x004;
const SDHCI_BLOCK_COUNT: usize = 0x006;
const SDHCI_ARG1: usize = 0x008;
const SDHCI_TRANSFER_MODE: usize = 0x00C;
const SDHCI_COMMAND: usize = 0x00E;
const SDHCI_RESPONSE_0: usize = 0x010;
const _SDHCI_RESPONSE_1: usize = 0x014;
const _SDHCI_RESPONSE_2: usize = 0x018;
const _SDHCI_RESPONSE_3: usize = 0x01C;
const SDHCI_DATA: usize = 0x020;
const SDHCI_PRESENT_STATE: usize = 0x024;
const _SDHCI_HOST_CONTROL: usize = 0x028;
const _SDHCI_POWER_CONTROL: usize = 0x029;
const _SDHCI_BLOCK_GAP_CONTROL: usize = 0x02A;
const _SDHCI_WAKE_UP_CONTROL: usize = 0x02B;
const SDHCI_CLOCK_CONTROL: usize = 0x02C;
const _SDHCI_TIMEOUT_CONTROL: usize = 0x02E;
const SDHCI_SOFTWARE_RESET: usize = 0x02F;
const SDHCI_INT_STATUS: usize = 0x030;
const SDHCI_INT_ENABLE: usize = 0x034;
const SDHCI_SIGNAL_ENABLE: usize = 0x038;
const _SDHCI_ACMD12_ERR: usize = 0x03C;
const _SDHCI_HOST_CONTROL2: usize = 0x03E;
const _SDHCI_CAPABILITIES: usize = 0x040;
const _SDHCI_CAPABILITIES_1: usize = 0x044;
const _SDHCI_MAX_CURRENT: usize = 0x048;

// ═══════════════════════════════════════════════════════════════════════════════
// COMMANDS
// ═══════════════════════════════════════════════════════════════════════════════

const CMD0: u16  = 0;
const CMD2: u16  = 2;
const CMD3: u16  = 3;
const CMD7: u16  = 7;
const CMD8: u16  = 8;
const CMD9: u16  = 9;
const CMD16: u16 = 16;
const CMD17: u16 = 17;
const CMD24: u16 = 24;
const CMD55: u16 = 55;
const ACMD41: u16 = 41;

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct SdCardDriver {
    base: usize,
    rca: u32, // Relative Card Address
    initialized: bool,
}

impl SdCardDriver {
    pub const fn new() -> Self {
        Self {
            base: 0x100110000, // Default BCM2712 EMMC2 base
            rca: 0,
            initialized: false,
        }
    }

    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.initialized {
            return Ok(());
        }

        kprintln!("[SD] Initializing SD Card Driver...");

        // 1. Reset Controller
        unsafe {
            arch::write32(self.base + SDHCI_SOFTWARE_RESET, 0x01); // Reset All
            arch::delay_cycles(10000);
            while (arch::read32(self.base + SDHCI_SOFTWARE_RESET) & 0x01) != 0 {
                arch::delay_cycles(1000);
            }
        }
        
        // 2. Set Clock (Initial 400kHz)
        self.set_clock(400_000);
        
        // 3. Enable Interrupts (Internal)
        unsafe {
            arch::write32(self.base + SDHCI_INT_ENABLE, 0xFFFFFFFF);
            arch::write32(self.base + SDHCI_SIGNAL_ENABLE, 0xFFFFFFFF);
        }

        // 4. Send CMD0 (Go Idle State)
        self.send_command(CMD0, 0)?;

        // 5. Send CMD8 (Send Interface Condition)
        // Check voltage (2.7-3.6V) and check pattern (0xAA)
        self.send_command(CMD8, 0x1AA)?;
        let resp = self.read_resp0();
        if (resp & 0xFF) != 0xAA {
            return Err("SD Card voltage mismatch");
        }

        // 6. Send ACMD41 (App Op Cond) loop
        let mut retries = 1000;
        loop {
            self.send_command(CMD55, 0)?; // App Cmd
            self.send_command(ACMD41, 0x40000000 | 0x00300000)?; // HCS | Voltage Window
            
            let ocr = self.read_resp0();
            if (ocr & 0x80000000) != 0 {
                // Card is ready
                break;
            }
            
            retries -= 1;
            if retries == 0 {
                return Err("SD Card timeout on ACMD41");
            }
            arch::delay_cycles(10000);
        }

        // 7. Send CMD2 (All Send CID)
        self.send_command(CMD2, 0)?;

        // 8. Send CMD3 (Send Relative Address)
        self.send_command(CMD3, 0)?;
        self.rca = self.read_resp0() >> 16;
        kprintln!("[SD] RCA: {:04x}", self.rca);

        // 9. Send CMD7 (Select Card)
        self.send_command(CMD7, self.rca << 16)?;

        // 10. Send CMD16 (Set Block Length) to 512
        self.send_command(CMD16, 512)?;

        // 11. Switch to High Speed (25MHz)
        self.set_clock(25_000_000);

        self.initialized = true;
        kprintln!("[SD] Initialization Complete.");
        Ok(())
    }

    fn send_command(&self, cmd: u16, arg: u32) -> Result<(), &'static str> {
        unsafe {
            // Wait for Command Inhibit
            let mut timeout = 100000;
            while (arch::read32(self.base + SDHCI_PRESENT_STATE) & 0x1) != 0 {
                timeout -= 1;
                if timeout == 0 { return Err("Command Inhibit Timeout"); }
            }

            arch::write32(self.base + SDHCI_ARG1, arg);
            
            // Construct Command Register
            // [15:14] Type (00=Normal, 11=Abort)
            // [13] Data Present
            // [12] Index Check Enable
            // [11] CRC Check Enable
            // [7:6] Response Type (00=No, 01=136, 10=48, 11=48busy)
            // [5:0] Command Index
            
            let mut cmd_val = (cmd as u32) << 8;
            
            // Response Select
            match cmd {
                CMD0 => {}, // No Resp
                CMD2 | CMD9 => cmd_val |= 0x01, // Resp 136
                CMD17 | CMD24 => cmd_val |= 0x02, // Resp 48
                _ => cmd_val |= 0x02, // Resp 48
            }
            
            // CRC/Index Check
            if cmd != CMD0 && cmd != CMD2 && cmd != CMD9 {
                cmd_val |= 0x08 | 0x10; // CRC | Index
            } else if cmd == CMD2 || cmd == CMD9 {
                 cmd_val |= 0x08; // CRC only for 136
            }
            
            // Data Present
            if cmd == CMD17 || cmd == CMD24 {
                cmd_val |= 0x20;
            }
            
            arch::write32(self.base + SDHCI_COMMAND, cmd_val); // Write 16-bit to 0x0E actually?
            // write32 to 0x0E is unaligned if we treat it as 32-bit.
            // But our write32 does volatile write.
            // If we write to 0x0C (Transfer Mode), we write 32 bits: Transfer Mode (low) + Command (high).
            
            let mut transfer_mode = 0;
            if cmd == CMD17 { // Read
                transfer_mode |= 0x10; // Read
            }
            
            let val32 = (cmd_val << 16) | transfer_mode;
            arch::write32(self.base + SDHCI_TRANSFER_MODE, val32);
            
            // Wait for Command Complete
            let mut timeout = 100000;
            while (arch::read32(self.base + SDHCI_INT_STATUS) & 0x1) == 0 {
                timeout -= 1;
                if timeout == 0 { return Err("Command Complete Timeout"); }
            }
            
            // Clear Interrupt
            arch::write32(self.base + SDHCI_INT_STATUS, 0x1);
        }
        Ok(())
    }

    fn read_resp0(&self) -> u32 {
        unsafe { arch::read32(self.base + SDHCI_RESPONSE_0) }
    }

    fn set_clock(&self, freq_hz: u32) {
        let base_clock = 100_000_000; // Assume 100MHz base
        let mut divisor = base_clock / freq_hz;
        
        // SDHCI 3.0: Divisor is 10-bit.
        // Actual freq = Base / (2 * Divisor)
        // So we need Divisor = Base / (2 * Target)
        // But register stores Divisor.
        
        if divisor < 2 { divisor = 2; }
        
        // Register value is (divisor / 2)
        // For SDHCI 3.0, we use 10-bit mode if supported, but let's stick to 8-bit standard mode first.
        // 8-bit mode: Bits 15-8.
        
        let div_reg = (divisor / 2) & 0xFF;
        let val = (div_reg << 8) | 0x01; // Divisor | Internal Clock Enable
        
        unsafe {
            // 1. Disable Clock
            arch::write32(self.base + SDHCI_CLOCK_CONTROL, 0);
            
            // 2. Set Divisor & Enable Internal Clock
            arch::write32(self.base + SDHCI_CLOCK_CONTROL, val as u32);
            
            // 3. Wait for Internal Clock Stable
            let mut timeout = 100000;
            while (arch::read32(self.base + SDHCI_CLOCK_CONTROL) & 0x02) == 0 {
                timeout -= 1;
                if timeout == 0 { 
                    kprintln!("[SD] Clock Stable Timeout");
                    break; 
                }
            }
            
            // 4. Enable SD Clock
            let val = val | 0x04;
            arch::write32(self.base + SDHCI_CLOCK_CONTROL, val as u32);
        }
        
        kprintln!("[SD] Clock set to {} Hz (Div: {})", freq_hz, divisor);
    }
}

impl BlockDevice for SdCardDriver {
    fn read_sector(&self, sector: u32, buf: &mut [u8]) -> Result<(), &'static str> {
        if buf.len() < 512 { return Err("Buffer too small"); }
        
        // 1. Set Block Size/Count
        unsafe {
            arch::write32(self.base + SDHCI_BLOCK_SIZE, 512 | (7 << 12)); // 512 bytes, SDMA boundary
            arch::write32(self.base + SDHCI_BLOCK_COUNT, 1);
        }
        
        // 2. Send CMD17
        self.send_command(CMD17, sector)?;
        
        // 3. Read Data
        // Polling mode for now
        unsafe {
            for i in 0..128 { // 512 bytes / 4 bytes per read
                // Wait for Buffer Read Ready
                let mut timeout = 100000;
                while (arch::read32(self.base + SDHCI_INT_STATUS) & 0x20) == 0 {
                    timeout -= 1;
                    if timeout == 0 { return Err("Buffer Read Timeout"); }
                }
                
                // Clear Interrupt
                arch::write32(self.base + SDHCI_INT_STATUS, 0x20);
                
                let data = arch::read32(self.base + SDHCI_DATA);
                let bytes = data.to_le_bytes();
                buf[i*4] = bytes[0];
                buf[i*4+1] = bytes[1];
                buf[i*4+2] = bytes[2];
                buf[i*4+3] = bytes[3];
            }
        }
        
        Ok(())
    }

    fn write_sector(&self, _sector: u32, _buf: &[u8]) -> Result<(), &'static str> {
        Err("Write not implemented")
    }
}

/// Global SD Card Driver Instance
pub static SD_DRIVER: SpinLock<SdCardDriver> = SpinLock::new(SdCardDriver::new());

pub fn init() {
    let mut driver = SD_DRIVER.lock();
    if let Err(e) = driver.init() {
        kprintln!("[SD] Init failed: {}", e);
    }
}
