//! Heads-Up Display (HUD) for Intent Kernel
//!
//! Visualizes the stenographic stream in real-time.
//! This is the primary interface for the user - not a window manager,
//! but a high-speed visualization of intent.

use crate::drivers::framebuffer::{self, Color, Framebuffer};
use crate::steno::Stroke;
use crate::intent::Intent;

// ═══════════════════════════════════════════════════════════════════════════════
// HUD CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

const BG_COLOR: Color = Color::BLACK;
const ACCENT_COLOR: Color = Color::rgb(0, 255, 200); // Cyan-ish
const TEXT_COLOR: Color = Color::WHITE;
const DIM_COLOR: Color = Color::rgb(80, 80, 80);

const TAPE_WIDTH: u32 = 300;
const TAPE_X: u32 = 50;
const TAPE_Y: u32 = 100;
const TAPE_LINES: u32 = 20;
const LINE_HEIGHT: u32 = 24;

const INTENT_X: u32 = 400;
const INTENT_Y: u32 = 100;

// ═══════════════════════════════════════════════════════════════════════════════
// HUD STATE
// ═══════════════════════════════════════════════════════════════════════════════

/// A simple ring buffer for text lines
struct TextLog<const N: usize> {
    lines: [heapless::String<32>; N],
    head: usize,
    count: usize,
}

impl<const N: usize> TextLog<N> {
    fn new() -> Self {
        const EMPTY_STRING: heapless::String<32> = heapless::String::new();
        Self {
            lines: [EMPTY_STRING; N],
            head: 0,
            count: 0,
        }
    }

    fn push(&mut self, text: &str) {
        self.lines[self.head] = heapless::String::from(text);
        self.head = (self.head + 1) % N;
        if self.count < N {
            self.count += 1;
        }
    }

    /// Iterate from oldest to newest
    fn iter(&self) -> impl Iterator<Item = &str> {
        let start = if self.count < N { 0 } else { self.head };
        (0..self.count).map(move |i| {
            let idx = (start + i) % N;
            self.lines[idx].as_str()
        })
    }
}

pub struct Hud {
    width: u32,
    height: u32,
    tape_log: TextLog<{ TAPE_LINES as usize }>,
    intent_log: TextLog<{ TAPE_LINES as usize }>,
}

impl Hud {
    pub fn new() -> Self {
        let (width, height) = framebuffer::with(|fb| (fb.width(), fb.height())).unwrap_or((0, 0));

        Self {
            width,
            height,
            tape_log: TextLog::new(),
            intent_log: TextLog::new(),
        }
    }

    /// Draw the entire HUD
    pub fn draw(&mut self) {
        framebuffer::with(|fb| {
            // Clear screen (or just the HUD areas to be faster)
            fb.clear(BG_COLOR);

            self.draw_header(fb);
            self.draw_steno_tape(fb);
            self.draw_intent_log(fb);
            self.draw_status_bar(fb);
        });
    }

    fn draw_header(&self, fb: &mut Framebuffer) {
        // Top bar
        fb.fill_rect(0, 0, self.width, 40, Color::rgb(20, 20, 30));
        fb.hline(0, 40, self.width, ACCENT_COLOR);

        fb.draw_string(20, 12, "INTENT KERNEL v0.3", ACCENT_COLOR, None);
        fb.draw_string(self.width - 200, 12, "SYSTEM: ONLINE", Color::GREEN, None);
    }

    fn draw_steno_tape(&self, fb: &mut Framebuffer) {
        // Draw tape background
        fb.fill_rect(TAPE_X, TAPE_Y, TAPE_WIDTH, TAPE_LINES * LINE_HEIGHT, Color::rgb(10, 10, 15));
        fb.rect(TAPE_X, TAPE_Y, TAPE_WIDTH, TAPE_LINES * LINE_HEIGHT, DIM_COLOR);

        // Label
        fb.draw_string(TAPE_X, TAPE_Y - 20, "STENO TAPE", DIM_COLOR, None);

        // Draw lines from log
        for (i, line) in self.tape_log.iter().enumerate() {
            let y = TAPE_Y + (i as u32 * LINE_HEIGHT);
            fb.draw_string(TAPE_X + 10, y + 8, line, TEXT_COLOR, None);
        }
    }

    fn draw_intent_log(&self, fb: &mut Framebuffer) {
        // Draw intent log background
        let log_width = self.width - INTENT_X - 50;
        fb.fill_rect(INTENT_X, INTENT_Y, log_width, TAPE_LINES * LINE_HEIGHT, Color::rgb(10, 10, 15));
        fb.rect(INTENT_X, INTENT_Y, log_width, TAPE_LINES * LINE_HEIGHT, DIM_COLOR);

        // Label
        fb.draw_string(INTENT_X, INTENT_Y - 20, "INTENT STREAM", DIM_COLOR, None);

        // Draw lines from log
        for (i, line) in self.intent_log.iter().enumerate() {
            let y = INTENT_Y + (i as u32 * LINE_HEIGHT);
            fb.draw_string(INTENT_X + 10, y + 8, line, ACCENT_COLOR, None);
        }
    }

    fn draw_status_bar(&self, fb: &mut Framebuffer) {
        let y = self.height - 30;
        fb.fill_rect(0, y, self.width, 30, Color::rgb(20, 20, 30));
        fb.hline(0, y, self.width, DIM_COLOR);

        let stats = crate::steno::stats();
        
        // Helper to draw number
        let mut buf = [0u8; 20];
        
        fb.draw_string(20, y + 8, "Strokes:", DIM_COLOR, None);
        if let Ok(s) = u64_to_str(stats.strokes_processed, &mut buf) {
            fb.draw_string(100, y + 8, s, TEXT_COLOR, None);
        }
        
        fb.draw_string(200, y + 8, "Intents:", DIM_COLOR, None);
        if let Ok(s) = u64_to_str(stats.intents_matched, &mut buf) {
            fb.draw_string(280, y + 8, s, TEXT_COLOR, None);
        }
    }

    /// Update the display with a new stroke and optional intent
    pub fn update(&mut self, stroke: Stroke, intent: Option<&Intent>) {
        // 1. Add stroke to tape log
        let rtfcre = stroke.to_rtfcre();
        self.tape_log.push(rtfcre.as_str());

        // 2. Add intent to intent log
        if let Some(intent) = intent {
            self.intent_log.push(intent.name);
        } else {
            self.intent_log.push(""); // Empty line to keep sync if desired, or just don't push
        }

        // Redraw everything (simple but effective for now)
        // Optimization: Only redraw the text areas
        framebuffer::with(|fb| {
            self.draw_steno_tape(fb);
            self.draw_intent_log(fb);
            self.draw_status_bar(fb);
        });
    }
}

// Simple integer to string converter
fn u64_to_str(mut num: u64, buf: &mut [u8]) -> Result<&str, ()> {
    if buf.len() < 20 { return Err(()); }
    
    if num == 0 {
        buf[0] = b'0';
        return Ok("0");
    }
    
    let mut i = 0;
    let mut temp = [0u8; 20];
    
    while num > 0 {
        temp[i] = (num % 10) as u8 + b'0';
        num /= 10;
        i += 1;
    }
    
    for j in 0..i {
        buf[j] = temp[i - 1 - j];
    }
    
    core::str::from_utf8(&buf[..i]).map_err(|_| ())
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL HUD
// ═══════════════════════════════════════════════════════════════════════════════

use crate::arch::SpinLock;

static HUD: SpinLock<Option<Hud>> = SpinLock::new(None);

pub fn init() {
    let mut hud = HUD.lock();
    *hud = Some(Hud::new());
    
    if let Some(h) = hud.as_mut() {
        h.draw();
    }
}

pub fn update(stroke: Stroke, intent: Option<&Intent>) {
    let mut hud = HUD.lock();
    if let Some(h) = hud.as_mut() {
        h.update(stroke, intent);
    }
}
