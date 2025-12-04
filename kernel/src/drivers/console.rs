//! Framebuffer Console
//!
//! Provides text output capabilities on the graphical framebuffer.

use core::fmt;
use crate::drivers::framebuffer::{self, Color, Framebuffer};

// Console configuration
const FONT_WIDTH: u32 = 8;
const FONT_HEIGHT: u32 = 8;
const PADDING: u32 = 4;

/// Global console instance
use crate::arch::SpinLock;
static CONSOLE: SpinLock<Option<Console>> = SpinLock::new(None);

/// Text console
pub struct Console {
    width: u32,
    height: u32,
    cursor_x: u32,
    cursor_y: u32,
    fg_color: Color,
    bg_color: Color,
}

impl Console {
    /// Create a new console
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cursor_x: PADDING,
            cursor_y: PADDING,
            fg_color: Color::WHITE,
            bg_color: Color::BLACK,
        }
    }

    /// Write a string to the console
    pub fn write_str(&mut self, s: &str) {
        framebuffer::with(|fb| {
            for c in s.chars() {
                self.write_char(fb, c);
            }
        });
    }

    /// Write a single character
    fn write_char(&mut self, fb: &mut Framebuffer, c: char) {
        match c {
            '\n' => {
                self.newline(fb);
            }
            '\r' => {
                self.cursor_x = PADDING;
            }
            _ => {
                if self.cursor_x + FONT_WIDTH > self.width - PADDING {
                    self.newline(fb);
                }
                
                fb.draw_char(self.cursor_x, self.cursor_y, c, self.fg_color, Some(self.bg_color));
                self.cursor_x += FONT_WIDTH;
            }
        }
    }

    /// Move to new line
    fn newline(&mut self, fb: &mut Framebuffer) {
        self.cursor_x = PADDING;
        self.cursor_y += FONT_HEIGHT + 2; // Add some line spacing
        
        if self.cursor_y + FONT_HEIGHT > self.height - PADDING {
            // Scroll (simple clear and reset for now, proper scrolling is harder without backing store)
            // For Phase 4, we might want a proper terminal emulator.
            // For now, just wrap to top.
            self.cursor_y = PADDING;
            
            // Clear the screen or just the next line?
            // Let's clear the whole screen for simplicity in this "bare metal" stage
            fb.clear(self.bg_color);
        }
    }
    
    /// Set colors
    pub fn set_colors(&mut self, fg: Color, bg: Color) {
        self.fg_color = fg;
        self.bg_color = bg;
    }
}

/// Initialize the global console
pub fn init() {
    framebuffer::with(|fb| {
        let console = Console::new(fb.width(), fb.height());
        *CONSOLE.lock() = Some(console);
        
        // Clear screen
        fb.clear(Color::BLACK);
    });
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// Print to the console (or serial fallback)
pub fn print(args: fmt::Arguments) {
    if let Some(console) = CONSOLE.lock().as_mut() {
        let _ = fmt::Write::write_fmt(console, args);
    } else {
        // Fallback to Serial
        crate::drivers::uart::print(args);
    }
}

/// Print line to the console (or serial fallback)
pub fn println(args: fmt::Arguments) {
    if let Some(console) = CONSOLE.lock().as_mut() {
        let _ = fmt::Write::write_fmt(console, args);
        console.write_str("\n");
    } else {
        // Fallback to Serial
        crate::drivers::uart::print(args);
        crate::drivers::uart::print(format_args!("\n"));
    }
}

/// Clear the console
pub fn clear() {
    if let Some(console) = CONSOLE.lock().as_mut() {
        console.cursor_x = PADDING;
        console.cursor_y = PADDING;
        framebuffer::with(|fb| {
            fb.clear(console.bg_color);
        });
    }
}

/// Macro for printing to the console
#[macro_export]
macro_rules! cprint {
    ($($arg:tt)*) => {
        $crate::drivers::console::print(format_args!($($arg)*))
    };
}

/// Macro for printing line to the console
#[macro_export]
macro_rules! cprintln {
    () => { $crate::cprint!("\n") };
    ($($arg:tt)*) => {
        $crate::cprint!("{}\n", format_args!($($arg)*))
    };
}
