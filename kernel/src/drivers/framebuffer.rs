//! Framebuffer Driver for Raspberry Pi 5
//!
//! Provides direct access to the display through the VideoCore mailbox.
//! Supports high-resolution displays with hardware-accelerated pixel operations.

use crate::drivers::mailbox::{self, PropertyBuffer, PropertyRequest};
use crate::drivers::mailbox::{
    TAG_SET_PHYSICAL_SIZE, TAG_SET_VIRTUAL_SIZE, TAG_SET_DEPTH,
    TAG_SET_PIXEL_ORDER, TAG_ALLOCATE_BUFFER, TAG_GET_PITCH,
    TAG_SET_VIRTUAL_OFFSET, TAG_BLANK_SCREEN, get_tag_value,
};
use core::ptr;

// ═══════════════════════════════════════════════════════════════════════════════
// PIXEL FORMATS
// ═══════════════════════════════════════════════════════════════════════════════

/// Pixel order
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelOrder {
    BGR = 0,
    RGB = 1,
}

/// Color depth
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ColorDepth {
    Bpp8 = 8,
    Bpp16 = 16,
    Bpp24 = 24,
    Bpp32 = 32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// COLOR
// ═══════════════════════════════════════════════════════════════════════════════

/// 32-bit ARGB color
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK: Color = Color(0xFF000000);
    pub const WHITE: Color = Color(0xFFFFFFFF);
    pub const RED: Color = Color(0xFFFF0000);
    pub const GREEN: Color = Color(0xFF00FF00);
    pub const BLUE: Color = Color(0xFF0000FF);
    pub const YELLOW: Color = Color(0xFFFFFF00);
    pub const CYAN: Color = Color(0xFF00FFFF);
    pub const MAGENTA: Color = Color(0xFFFF00FF);
    pub const ORANGE: Color = Color(0xFFFF8000);
    pub const PURPLE: Color = Color(0xFF800080);
    pub const GRAY: Color = Color(0xFF808080);
    pub const DARK_GRAY: Color = Color(0xFF404040);
    pub const LIGHT_GRAY: Color = Color(0xFFC0C0C0);
    
    /// Create from RGB components
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
    
    /// Create from ARGB components
    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
    
    /// Get red component
    pub const fn r(self) -> u8 { ((self.0 >> 16) & 0xFF) as u8 }
    
    /// Get green component
    pub const fn g(self) -> u8 { ((self.0 >> 8) & 0xFF) as u8 }
    
    /// Get blue component
    pub const fn b(self) -> u8 { (self.0 & 0xFF) as u8 }
    
    /// Get alpha component
    pub const fn a(self) -> u8 { ((self.0 >> 24) & 0xFF) as u8 }
    
    /// Blend with another color
    pub fn blend(self, other: Color, alpha: u8) -> Color {
        let a = alpha as u32;
        let inv_a = 255 - a;
        
        let r = ((self.r() as u32 * inv_a + other.r() as u32 * a) / 255) as u8;
        let g = ((self.g() as u32 * inv_a + other.g() as u32 * a) / 255) as u8;
        let b = ((self.b() as u32 * inv_a + other.b() as u32 * a) / 255) as u8;
        
        Color::rgb(r, g, b)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FRAMEBUFFER CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Framebuffer configuration
#[derive(Clone, Copy)]
pub struct FramebufferConfig {
    pub width: u32,
    pub height: u32,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub depth: ColorDepth,
    pub pixel_order: PixelOrder,
}

impl FramebufferConfig {
    /// Create a standard 1080p configuration
    pub const fn hd1080p() -> Self {
        FramebufferConfig {
            width: 1920,
            height: 1080,
            virtual_width: 1920,
            virtual_height: 1080,
            depth: ColorDepth::Bpp32,
            pixel_order: PixelOrder::RGB,
        }
    }
    
    /// Create a standard 720p configuration
    pub const fn hd720p() -> Self {
        FramebufferConfig {
            width: 1280,
            height: 720,
            virtual_width: 1280,
            virtual_height: 720,
            depth: ColorDepth::Bpp32,
            pixel_order: PixelOrder::RGB,
        }
    }
    
    /// Create a 4K configuration
    pub const fn uhd4k() -> Self {
        FramebufferConfig {
            width: 3840,
            height: 2160,
            virtual_width: 3840,
            virtual_height: 2160,
            depth: ColorDepth::Bpp32,
            pixel_order: PixelOrder::RGB,
        }
    }
    
    /// Create a custom configuration
    pub const fn custom(width: u32, height: u32) -> Self {
        FramebufferConfig {
            width,
            height,
            virtual_width: width,
            virtual_height: height,
            depth: ColorDepth::Bpp32,
            pixel_order: PixelOrder::RGB,
        }
    }
    
    /// Enable double buffering
    pub const fn double_buffered(mut self) -> Self {
        self.virtual_height = self.height * 2;
        self
    }
}

impl Default for FramebufferConfig {
    fn default() -> Self {
        Self::hd1080p()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FRAMEBUFFER INFO
// ═══════════════════════════════════════════════════════════════════════════════

/// Information about the allocated framebuffer
#[derive(Clone, Copy)]
pub struct FramebufferInfo {
    pub buffer: *mut u32,
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,       // Bytes per row
    pub depth: u32,
    pub pixel_order: PixelOrder,
}

impl FramebufferInfo {
    /// Get pixels per row
    pub fn pixels_per_row(&self) -> u32 {
        self.pitch / (self.depth / 8)
    }
    
    /// Calculate pixel offset
    pub fn pixel_offset(&self, x: u32, y: u32) -> usize {
        ((y * self.pitch / 4) + x) as usize
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FRAMEBUFFER
// ═══════════════════════════════════════════════════════════════════════════════

/// Framebuffer for direct pixel manipulation
pub struct Framebuffer {
    info: FramebufferInfo,
    current_buffer: u8,  // For double buffering
}

impl Framebuffer {
    /// Allocate and initialize a framebuffer
    pub fn new(config: FramebufferConfig) -> Option<Self> {
        let mut buffer = PropertyBuffer::new();
        let mut req = PropertyRequest::new(&mut buffer);
        
        req.add_tag(TAG_SET_PHYSICAL_SIZE, &[config.width, config.height])
           .add_tag(TAG_SET_VIRTUAL_SIZE, &[config.virtual_width, config.virtual_height])
           .add_tag(TAG_SET_DEPTH, &[config.depth as u32])
           .add_tag(TAG_SET_PIXEL_ORDER, &[config.pixel_order as u32])
           .add_tag(TAG_SET_VIRTUAL_OFFSET, &[0, 0])
           .add_tag(TAG_ALLOCATE_BUFFER, &[16, 0])  // 16-byte alignment
           .add_tag(TAG_GET_PITCH, &[0]);
        
        if !req.send() {
            return None;
        }
        
        // Extract results
        let fb_addr = get_tag_value(&buffer, TAG_ALLOCATE_BUFFER, 0)?;
        let fb_size = get_tag_value(&buffer, TAG_ALLOCATE_BUFFER, 1)?;
        let pitch = get_tag_value(&buffer, TAG_GET_PITCH, 0)?;
        
        // Convert GPU bus address to ARM physical address
        let arm_addr = (fb_addr & 0x3FFFFFFF) as *mut u32;
        
        let info = FramebufferInfo {
            buffer: arm_addr,
            size: fb_size,
            width: config.width,
            height: config.height,
            pitch,
            depth: config.depth as u32,
            pixel_order: config.pixel_order,
        };
        
        Some(Framebuffer {
            info,
            current_buffer: 0,
        })
    }
    
    /// Get framebuffer info
    pub fn info(&self) -> &FramebufferInfo {
        &self.info
    }
    
    /// Get width
    pub fn width(&self) -> u32 {
        self.info.width
    }
    
    /// Get height
    pub fn height(&self) -> u32 {
        self.info.height
    }
    
    /// Clear the screen with a color
    pub fn clear(&mut self, color: Color) {
        let pixels = (self.info.width * self.info.height) as usize;
        let ptr = self.info.buffer;
        
        unsafe {
            for i in 0..pixels {
                ptr::write_volatile(ptr.add(i), color.0);
            }
        }
    }
    
    /// Set a single pixel
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        
        let offset = self.info.pixel_offset(x, y);
        unsafe {
            ptr::write_volatile(self.info.buffer.add(offset), color.0);
        }
    }
    
    /// Get a single pixel
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        if x >= self.info.width || y >= self.info.height {
            return Color::BLACK;
        }
        
        let offset = self.info.pixel_offset(x, y);
        Color(unsafe { ptr::read_volatile(self.info.buffer.add(offset)) })
    }
    
    /// Draw a horizontal line
    pub fn hline(&mut self, x: u32, y: u32, length: u32, color: Color) {
        if y >= self.info.height {
            return;
        }
        
        let x_end = (x + length).min(self.info.width);
        let mut offset = self.info.pixel_offset(x, y);
        
        unsafe {
            for _ in x..x_end {
                ptr::write_volatile(self.info.buffer.add(offset), color.0);
                offset += 1;
            }
        }
    }
    
    /// Draw a vertical line
    pub fn vline(&mut self, x: u32, y: u32, length: u32, color: Color) {
        if x >= self.info.width {
            return;
        }
        
        let y_end = (y + length).min(self.info.height);
        let pitch_pixels = (self.info.pitch / 4) as usize;
        let mut offset = self.info.pixel_offset(x, y);
        
        unsafe {
            for _ in y..y_end {
                ptr::write_volatile(self.info.buffer.add(offset), color.0);
                offset += pitch_pixels;
            }
        }
    }
    
    /// Draw a line (Bresenham's algorithm)
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        
        let mut x = x0;
        let mut y = y0;
        
        loop {
            if x >= 0 && y >= 0 {
                self.set_pixel(x as u32, y as u32, color);
            }
            
            if x == x1 && y == y1 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    
    /// Draw a rectangle outline
    pub fn rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        self.hline(x, y, width, color);
        self.hline(x, y + height - 1, width, color);
        self.vline(x, y, height, color);
        self.vline(x + width - 1, y, height, color);
    }
    
    /// Draw a filled rectangle
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        let y_end = (y + height).min(self.info.height);
        
        for row in y..y_end {
            self.hline(x, row, width, color);
        }
    }
    
    /// Draw a circle outline (Midpoint algorithm)
    pub fn circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        let mut x = radius;
        let mut y = 0;
        let mut err = 0;
        
        while x >= y {
            self.set_pixel((cx + x) as u32, (cy + y) as u32, color);
            self.set_pixel((cx + y) as u32, (cy + x) as u32, color);
            self.set_pixel((cx - y) as u32, (cy + x) as u32, color);
            self.set_pixel((cx - x) as u32, (cy + y) as u32, color);
            self.set_pixel((cx - x) as u32, (cy - y) as u32, color);
            self.set_pixel((cx - y) as u32, (cy - x) as u32, color);
            self.set_pixel((cx + y) as u32, (cy - x) as u32, color);
            self.set_pixel((cx + x) as u32, (cy - y) as u32, color);
            
            y += 1;
            err += 1 + 2 * y;
            if 2 * (err - x) + 1 > 0 {
                x -= 1;
                err += 1 - 2 * x;
            }
        }
    }
    
    /// Draw a filled circle
    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        for y in -radius..=radius {
            // Integer sqrt approximation
            let mut val = radius * radius - y * y;
            let mut half_width = 0;
            if val > 0 {
                let mut bit = 1i32 << 30;
                while bit > val { bit >>= 2; }
                while bit != 0 {
                    if val >= half_width + bit {
                        val -= half_width + bit;
                        half_width = (half_width >> 1) + bit;
                    } else {
                        half_width >>= 1;
                    }
                    bit >>= 2;
                }
            }
            for x in -half_width..=half_width {
                self.set_pixel((cx + x) as u32, (cy + y) as u32, color);
            }
        }
    }
    
    /// Swap buffers (for double buffering)
    pub fn swap(&mut self) -> bool {
        if self.info.height * 2 > self.info.height {
            // We have double buffering
            let new_buffer = 1 - self.current_buffer;
            let y_offset = (new_buffer as u32) * self.info.height;
            
            let mut buffer = PropertyBuffer::new();
            let mut req = PropertyRequest::new(&mut buffer);
            req.add_tag(TAG_SET_VIRTUAL_OFFSET, &[0, y_offset]);
            
            if req.send() {
                self.current_buffer = new_buffer;
                return true;
            }
        }
        false
    }
    
    /// Blank/unblank the screen
    pub fn blank(&mut self, blank: bool) {
        let mut buffer = PropertyBuffer::new();
        let mut req = PropertyRequest::new(&mut buffer);
        req.add_tag(TAG_BLANK_SCREEN, &[if blank { 1 } else { 0 }]);
        req.send();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BASIC FONT (8x8 bitmap)
// ═══════════════════════════════════════════════════════════════════════════════

/// Simple 8x8 font for text rendering
pub struct Font8x8;

impl Font8x8 {
    pub const WIDTH: u32 = 8;
    pub const HEIGHT: u32 = 8;
    
    /// Get glyph data for a character
    pub fn glyph(c: char) -> [u8; 8] {
        // Simple ASCII font (subset for space through ~)
        match c {
            ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            '!' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00],
            '"' => [0x6C, 0x6C, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00],
            '#' => [0x6C, 0x6C, 0xFE, 0x6C, 0xFE, 0x6C, 0x6C, 0x00],
            '0' => [0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00],
            '1' => [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
            '2' => [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x7E, 0x00],
            '3' => [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00],
            '4' => [0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C, 0x0C, 0x00],
            '5' => [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00],
            '6' => [0x1C, 0x30, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00],
            '7' => [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00],
            '8' => [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
            '9' => [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x0C, 0x38, 0x00],
            'A' | 'a' => [0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00],
            'B' | 'b' => [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00],
            'C' | 'c' => [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00],
            'D' | 'd' => [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00],
            'E' | 'e' => [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00],
            'F' | 'f' => [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00],
            'G' | 'g' => [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3E, 0x00],
            'H' | 'h' => [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
            'I' | 'i' => [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
            'J' | 'j' => [0x1E, 0x0C, 0x0C, 0x0C, 0x6C, 0x6C, 0x38, 0x00],
            'K' | 'k' => [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00],
            'L' | 'l' => [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00],
            'M' | 'm' => [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00],
            'N' | 'n' => [0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00],
            'O' | 'o' => [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
            'P' | 'p' => [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00],
            'Q' | 'q' => [0x3C, 0x66, 0x66, 0x66, 0x6A, 0x6C, 0x36, 0x00],
            'R' | 'r' => [0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x66, 0x00],
            'S' | 's' => [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00],
            'T' | 't' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
            'U' | 'u' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
            'V' | 'v' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
            'W' | 'w' => [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00],
            'X' | 'x' => [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00],
            'Y' | 'y' => [0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00],
            'Z' | 'z' => [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00],
            ':' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
            '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
            ',' => [0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30, 0x00],
            '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
            '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7E, 0x00],
            '=' => [0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00],
            '+' => [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00],
            '/' => [0x02, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00],
            '\\' => [0x40, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x02, 0x00],
            '(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
            ')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
            '[' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
            ']' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
            '{' => [0x0E, 0x18, 0x18, 0x70, 0x18, 0x18, 0x0E, 0x00],
            '}' => [0x70, 0x18, 0x18, 0x0E, 0x18, 0x18, 0x70, 0x00],
            '<' => [0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00],
            '>' => [0x60, 0x30, 0x18, 0x0C, 0x18, 0x30, 0x60, 0x00],
            '?' => [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x00, 0x18, 0x00],
            '*' => [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],
            '@' => [0x3C, 0x66, 0x6E, 0x6A, 0x6E, 0x60, 0x3E, 0x00],
            _ => [0xFF, 0x81, 0x81, 0x81, 0x81, 0x81, 0xFF, 0x00], // Unknown char box
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEXT RENDERING
// ═══════════════════════════════════════════════════════════════════════════════

impl Framebuffer {
    /// Draw a character at position
    pub fn draw_char(&mut self, x: u32, y: u32, c: char, fg: Color, bg: Option<Color>) {
        let glyph = Font8x8::glyph(c);
        
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                let px = x + col;
                let py = y + row as u32;
                
                if (bits >> (7 - col)) & 1 != 0 {
                    self.set_pixel(px, py, fg);
                } else if let Some(bg_color) = bg {
                    self.set_pixel(px, py, bg_color);
                }
            }
        }
    }
    
    /// Draw a string at position
    pub fn draw_string(&mut self, x: u32, y: u32, s: &str, fg: Color, bg: Option<Color>) {
        let mut cx = x;
        
        for c in s.chars() {
            if c == '\n' {
                continue;
            }
            
            self.draw_char(cx, y, c, fg, bg);
            cx += Font8x8::WIDTH;
            
            if cx >= self.info.width {
                break;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL FRAMEBUFFER INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

use crate::arch::SpinLock;

static FRAMEBUFFER: SpinLock<Option<Framebuffer>> = SpinLock::new(None);

// SAFETY: Framebuffer owns the memory it points to (MMIO), and we ensure exclusive access via SpinLock.
unsafe impl Send for Framebuffer {}

/// Initialize the global framebuffer
pub fn init(width: u32, height: u32, _depth: u32) -> Result<(), &'static str> {
    let config = FramebufferConfig::custom(width, height);
    
    if let Some(fb) = Framebuffer::new(config) {
        *FRAMEBUFFER.lock() = Some(fb);
        Ok(())
    } else {
        Err("Failed to allocate framebuffer")
    }
}

/// Execute a closure with the framebuffer
pub fn with<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Framebuffer) -> R,
{
    FRAMEBUFFER.lock().as_mut().map(f)
}

/// Clear the screen with a color
pub fn clear(color: Color) {
    with(|fb| fb.clear(color));
}

/// Draw text on the screen
pub fn draw_text(x: u32, y: u32, text: &str, fg: Color) {
    with(|fb| fb.draw_string(x, y, text, fg, None));
}

