//! PL011 UART Driver for Raspberry Pi 5
//!
//! Provides reliable serial communication for debugging and user interaction.
#![allow(dead_code)]

use crate::arch::{self, SpinLock};
use crate::drivers;
use core::fmt::{self, Write};

// ═══════════════════════════════════════════════════════════════════════════════
// PL011 REGISTERS
// ═══════════════════════════════════════════════════════════════════════════════

const DR: usize = 0x00;        // Data Register
const RSRECR: usize = 0x04;    // Receive Status / Error Clear
const FR: usize = 0x18;        // Flag Register
const ILPR: usize = 0x20;      // IrDA Low-Power Counter
const IBRD: usize = 0x24;      // Integer Baud Rate Divisor
const FBRD: usize = 0x28;      // Fractional Baud Rate Divisor
const LCRH: usize = 0x2C;      // Line Control Register
const CR: usize = 0x30;        // Control Register
const IFLS: usize = 0x34;      // Interrupt FIFO Level Select
const IMSC: usize = 0x38;      // Interrupt Mask Set/Clear
const RIS: usize = 0x3C;       // Raw Interrupt Status
const MIS: usize = 0x40;       // Masked Interrupt Status
const ICR: usize = 0x44;       // Interrupt Clear Register
const DMACR: usize = 0x48;     // DMA Control Register

// Flag Register bits
const FR_TXFE: u32 = 1 << 7;   // Transmit FIFO empty
const FR_RXFF: u32 = 1 << 6;   // Receive FIFO full
const FR_TXFF: u32 = 1 << 5;   // Transmit FIFO full
const FR_RXFE: u32 = 1 << 4;   // Receive FIFO empty
const FR_BUSY: u32 = 1 << 3;   // UART busy

// Line Control Register bits
const LCRH_SPS: u32 = 1 << 7;      // Stick parity select
const LCRH_WLEN_8: u32 = 0b11 << 5; // 8-bit word length
const LCRH_WLEN_7: u32 = 0b10 << 5; // 7-bit word length
const LCRH_WLEN_6: u32 = 0b01 << 5; // 6-bit word length
const LCRH_WLEN_5: u32 = 0b00 << 5; // 5-bit word length
const LCRH_FEN: u32 = 1 << 4;       // Enable FIFOs
const LCRH_STP2: u32 = 1 << 3;      // Two stop bits
const LCRH_EPS: u32 = 1 << 2;       // Even parity select
const LCRH_PEN: u32 = 1 << 1;       // Parity enable
const LCRH_BRK: u32 = 1 << 0;       // Send break

// Control Register bits
const CR_CTSEN: u32 = 1 << 15;     // CTS hardware flow control
const CR_RTSEN: u32 = 1 << 14;     // RTS hardware flow control
const CR_RTS: u32 = 1 << 11;       // Request to send
const CR_RXE: u32 = 1 << 9;        // Receive enable
const CR_TXE: u32 = 1 << 8;        // Transmit enable
const CR_LBE: u32 = 1 << 7;        // Loopback enable
const CR_UARTEN: u32 = 1 << 0;     // UART enable

// Interrupt bits
const INT_OE: u32 = 1 << 10;       // Overrun error
const INT_BE: u32 = 1 << 9;        // Break error
const INT_PE: u32 = 1 << 8;        // Parity error
const INT_FE: u32 = 1 << 7;        // Framing error
const INT_RT: u32 = 1 << 6;        // Receive timeout
const INT_TX: u32 = 1 << 5;        // Transmit
const INT_RX: u32 = 1 << 4;        // Receive

// ═══════════════════════════════════════════════════════════════════════════════
// UART DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

/// UART configuration
#[derive(Clone, Copy)]
pub struct UartConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
    pub flow_control: bool,
}

#[derive(Clone, Copy)]
pub enum Parity {
    None,
    Even,
    Odd,
}

impl Default for UartConfig {
    fn default() -> Self {
        UartConfig {
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
            flow_control: false,
        }
    }
}

///// Base address is now dynamic
// const UART_BASE: usize = drivers::UART0_BASE;

/// UART driver instance
pub struct Uart {
    base: usize,
    initialized: bool,
}

impl Uart {
    /// Create a new instance of the UART driver
    pub const fn new() -> Self {
        Uart {
            // We can't call drivers::uart_base() here because it's not const.
            // We'll set it to 0 and initialize it properly in init() or use a lazy_static approach.
            // But wait, we need it for read/write.
            // For now, let's just store 0 and update it in init?
            // Or better, make read/write use the dynamic function if base is 0?
            // No, that's slow.
            // Let's rely on init() to set the base.
            base: 0, 
            initialized: false,
        }
    }
    
    /// Read a register
    #[inline]
    fn read(&self, offset: usize) -> u32 {
        let base = if self.base == 0 { drivers::uart_base() } else { self.base };
        unsafe { arch::read32(base + offset) }
    }
    
    /// Write a register
    #[inline]
    fn write(&self, offset: usize, value: u32) {
        let base = if self.base == 0 { drivers::uart_base() } else { self.base };
        unsafe { arch::write32(base + offset, value) }
    }
    
    /// Wait for transmit FIFO to have space
    #[inline]
    fn wait_tx_ready(&self) {
        while self.read(FR) & FR_TXFF != 0 {
            core::hint::spin_loop();
        }
    }
    
    /// Wait for transmit to complete
    #[inline]
    fn wait_tx_complete(&self) {
        while self.read(FR) & FR_BUSY != 0 {
            core::hint::spin_loop();
        }
    }
    
    /// Initialize with configuration
    pub fn init(&mut self, config: &UartConfig) {
        // Ensure base is set
        if self.base == 0 {
            self.base = drivers::uart_base();
        }

        if crate::dtb::machine_type() == crate::dtb::MachineType::QemuVirt {
            // On QEMU virt, UART is pre-initialized.
            // We just need to ensure it's enabled.
            // And we shouldn't wait for TX complete as it might hang if status is weird.
            self.write(CR, CR_UARTEN | CR_TXE | CR_RXE);
            self.initialized = true;
            return;
        }

        // Disable UART
        self.write(CR, 0);
        
        // Wait for any current transmission to complete
        self.wait_tx_complete();
        
        // Flush FIFOs
        self.write(LCRH, 0);
        
        // Clear all interrupts
        self.write(ICR, 0x7FF);
        
        // Set baud rate
        // UART clock is typically 48MHz on Pi 5
        // Divisor = UART_CLK / (16 * Baud)
        // For 115200: 48000000 / (16 * 115200) = 26.041666...
        // IBRD = 26, FBRD = (0.041666 * 64) = 2.666... ≈ 3
        let uart_clk = 48_000_000u32;
        let divisor = (uart_clk * 4) / config.baud_rate;  // Fixed-point with 6 fractional bits
        let ibrd = divisor >> 6;
        let fbrd = divisor & 0x3F;
        
        self.write(IBRD, ibrd);
        self.write(FBRD, fbrd);
        
        // Set line control: 8N1, FIFOs enabled
        let mut lcrh = LCRH_FEN;  // Enable FIFOs
        
        lcrh |= match config.data_bits {
            5 => LCRH_WLEN_5,
            6 => LCRH_WLEN_6,
            7 => LCRH_WLEN_7,
            _ => LCRH_WLEN_8,
        };
        
        if config.stop_bits == 2 {
            lcrh |= LCRH_STP2;
        }
        
        match config.parity {
            Parity::Even => lcrh |= LCRH_PEN | LCRH_EPS,
            Parity::Odd => lcrh |= LCRH_PEN,
            Parity::None => {}
        }
        
        self.write(LCRH, lcrh);
        
        // Set FIFO interrupt levels (1/2 full)
        self.write(IFLS, 0b010_010);
        
        // Enable UART, TX, RX
        let mut cr = CR_UARTEN | CR_TXE | CR_RXE;
        if config.flow_control {
            cr |= CR_CTSEN | CR_RTSEN;
        }
        self.write(CR, cr);
        
        self.initialized = true;
    }
    
    /// Early initialization with default settings (called before full init)
    pub fn early_init(&mut self) {
        #[cfg(not(test))]
        self.init(&UartConfig::default());

        #[cfg(test)]
        {
            // On QEMU virt, UART is already initialized.
            // Just mark as initialized.
            self.initialized = true;
        }
    }
    
    /// Send a single byte
    pub fn send(&self, byte: u8) {
        // self.wait_tx_ready();
        self.write(DR, byte as u32);
    }
    
    /// Send a string
    pub fn send_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send(b'\r');
            }
            self.send(byte);
        }
    }
    
    /// Check if data is available
    pub fn has_data(&self) -> bool {
        self.read(FR) & FR_RXFE == 0
    }
    
    /// Receive a byte (blocking)
    pub fn receive(&self) -> u8 {
        while !self.has_data() {
            core::hint::spin_loop();
        }
        self.read(DR) as u8
    }
    
    /// Try to receive a byte (non-blocking)
    pub fn try_receive(&self) -> Option<u8> {
        if self.has_data() {
            Some(self.read(DR) as u8)
        } else {
            None
        }
    }
    
    /// Enable receive interrupt
    pub fn enable_rx_interrupt(&self) {
        let imsc = self.read(IMSC);
        self.write(IMSC, imsc | INT_RX | INT_RT);
    }
    
    /// Disable receive interrupt
    pub fn disable_rx_interrupt(&self) {
        let imsc = self.read(IMSC);
        self.write(IMSC, imsc & !(INT_RX | INT_RT));
    }
    
    /// Clear interrupts
    pub fn clear_interrupts(&self) {
        self.write(ICR, 0x7FF);
    }

}

pub struct UartReadFuture<'a> {
    uart: &'a SpinLock<Uart>,
}

impl<'a> core::future::Future for UartReadFuture<'a> {
    type Output = u8;

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let uart = self.uart.lock();
        if uart.has_data() {
            // We need to read the data register. The `read` method is private, but `receive` is blocking.
            // However, we know data is available, so `try_receive` should work immediately.
            if let Some(byte) = uart.try_receive() {
                core::task::Poll::Ready(byte)
            } else {
                 core::task::Poll::Pending
            }
        } else {
            // Register waker
            let mut guard = UART_WAKER.lock();
            *guard = Some(cx.waker().clone());
            
            // Ensure interrupts are enabled
            uart.enable_rx_interrupt();
            
            core::task::Poll::Pending
        }
    }
}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.send_str(s);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL UART INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

// SAFETY: PL011Uart is just a wrapper around MMIO, safe to send across threads
unsafe impl Send for Uart {}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL UART INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

static UART0: SpinLock<Uart> = SpinLock::new(Uart::new());
static UART_WAKER: SpinLock<Option<core::task::Waker>> = SpinLock::new(None);

/// UART Interrupt Handler
fn handle_irq(_irq: u32) {
    // Clear interrupts
    UART0.lock().clear_interrupts();
    
    // Wake up the async task
    let mut guard = UART_WAKER.lock();
    if let Some(waker) = guard.take() {
        waker.wake();
    }
}

/// Early initialization (before interrupts, with minimal setup)
pub fn early_init() {
    UART0.lock().early_init();
}

/// Full initialization with custom config
pub fn init_with_config(config: &UartConfig) {
    let mut uart = UART0.lock();
    uart.init(config);
    
    // Register and enable interrupt
    use super::interrupts::{self, IRQ_UART0};
    {
        interrupts::register_handler(IRQ_UART0, handle_irq);
        interrupts::enable(IRQ_UART0);
    }
    uart.enable_rx_interrupt();
}

/// Send a byte
pub fn send(byte: u8) {
    UART0.lock().send(byte);
}

/// Send a string
pub fn send_str(s: &str) {
    UART0.lock().send_str(s);
}

/// Check if data available
pub fn has_data() -> bool {
    UART0.lock().has_data()
}

/// Receive a byte (blocking)
pub fn receive() -> u8 {
    UART0.lock().receive()
}

/// Try to receive a byte
pub fn try_receive() -> Option<u8> {
    UART0.lock().try_receive()
}

/// Async read byte
pub async fn read_byte_async() -> u8 {
    // We need to be careful here. We can't hold the lock across await points.
    // The UartReadFuture needs to access the UART safely.
    // For now, we'll create a temporary future that accesses the global UART.
    UartReadFuture { uart: &UART0 }.await
}

/// Async read a line of input into buffer, return length
pub async fn read_line_async(buffer: &mut [u8]) -> usize {
    let mut idx = 0;
    
    loop {
        let byte = read_byte_async().await;
        
        match byte {
            // Enter
            b'\r' | b'\n' => {
                send(b'\r');
                send(b'\n');
                break;
            }
            // Backspace
            0x7F | 0x08 => {
                if idx > 0 {
                    idx -= 1;
                    send(0x08);
                    send(b' ');
                    send(0x08);
                }
            }
            // Ctrl+C
            0x03 => {
                send(b'^');
                send(b'C');
                send(b'\r');
                send(b'\n');
                idx = 0;
                break;
            }
            // Printable characters
            0x20..=0x7E => {
                if idx < buffer.len() - 1 {
                    buffer[idx] = byte;
                    idx += 1;
                    send(byte);
                }
            }
            // Escape sequences (ignore for now)
            0x1B => {
                // Could handle arrow keys, etc.
            }
            _ => {}
        }
    }
    
    if idx < buffer.len() {
        buffer[idx] = 0;
    }
    idx
}

/// Read a line of input into buffer, return length
pub fn read_line(buffer: &mut [u8]) -> usize {
    let mut idx = 0;
    
    loop {
        let byte = receive();
        
        match byte {
            // Enter
            b'\r' | b'\n' => {
                send(b'\r');
                send(b'\n');
                break;
            }
            // Backspace
            0x7F | 0x08 => {
                if idx > 0 {
                    idx -= 1;
                    send(0x08);
                    send(b' ');
                    send(0x08);
                }
            }
            // Ctrl+C
            0x03 => {
                send(b'^');
                send(b'C');
                send(b'\r');
                send(b'\n');
                idx = 0;
                break;
            }
            // Printable characters
            0x20..=0x7E => {
                if idx < buffer.len() - 1 {
                    buffer[idx] = byte;
                    idx += 1;
                    send(byte);
                }
            }
            // Escape sequences (ignore for now)
            0x1B => {
                // Could handle arrow keys, etc.
            }
            _ => {}
        }
    }
    
    if idx < buffer.len() {
        buffer[idx] = 0;
    }
    idx
}

// ═══════════════════════════════════════════════════════════════════════════════
// PRINT MACROS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print(args: fmt::Arguments) {
    use core::fmt::Write;
    let mut uart = UART0.lock();
    let _ = uart.write_fmt(args);
}
