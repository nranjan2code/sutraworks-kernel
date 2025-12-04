//! Visual Projections - Semantic Visual Elements
//!
//! Projections are ephemeral renderings of semantic state.
//! They appear when intents require visual feedback.

use crate::drivers::framebuffer::{Color, Framebuffer};
use crate::intent::{ConceptID, Intent};
use crate::steno::dictionary::concepts;

// ═══════════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Screen region for a projection
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
    
    /// Check if a point is inside this rect
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width &&
        py >= self.y && py < self.y + self.height
    }
}

/// Projection priority (z-order)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionPriority {
    Background = 0,
    Normal = 1,
    High = 2,
    Modal = 3,
}

// ═══════════════════════════════════════════════════════════════════════════════
// PROJECTION TRAIT
// ═══════════════════════════════════════════════════════════════════════════════

/// A visual projection - renders semantic state to the framebuffer
pub trait Projection: Send {
    /// Unique name for this projection
    fn name(&self) -> &'static str;
    
    /// Concepts this projection responds to (empty = all)
    fn responds_to(&self) -> &[ConceptID] { &[] }
    
    /// Priority (z-order)
    fn priority(&self) -> ProjectionPriority { ProjectionPriority::Normal }
    
    /// Is this projection currently visible?
    fn is_visible(&self) -> bool;
    
    /// Get the region this projection occupies
    fn region(&self) -> Rect;
    
    /// Is this a modal projection (blocks others)?
    fn is_modal(&self) -> bool { false }
    
    /// Notify of an intent
    /// Called when any intent is broadcast
    fn on_intent(&mut self, intent: &Intent);
    
    /// Notify of a raw steno stroke
    fn on_stroke(&mut self, _stroke: &crate::steno::Stroke) {}
    
    /// Render to framebuffer
    /// Only called if is_visible() returns true
    fn render(&self, fb: &mut Framebuffer);
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATUS PROJECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Shows system status when STATUS intent is triggered
pub struct StatusProjection {
    visible: bool,
    show_until: u64,
    last_stats: Option<crate::steno::EngineStats>,
}

impl StatusProjection {
    pub const fn new() -> Self {
        Self {
            visible: false,
            show_until: 0,
            last_stats: None,
        }
    }
}

impl Projection for StatusProjection {
    fn name(&self) -> &'static str { "StatusProjection" }
    
    fn responds_to(&self) -> &[ConceptID] {
        // Only respond to STATUS concept
        &[concepts::STATUS]
    }
    
    fn priority(&self) -> ProjectionPriority { ProjectionPriority::High }
    
    fn is_visible(&self) -> bool { self.visible }
    
    fn region(&self) -> Rect {
        // Top-right corner status box
        Rect::new(1500, 50, 350, 200)
    }
    
    fn on_intent(&mut self, intent: &Intent) {
        if intent.concept_id == concepts::STATUS {
            self.visible = true;
            self.last_stats = Some(crate::steno::stats());
            // Auto-hide after 5 seconds (would need timer integration)
            self.show_until = 5000; 
        }
    }
    
    fn render(&self, fb: &mut Framebuffer) {
        let region = self.region();
        
        // Background with border
        fb.fill_rect(region.x, region.y, region.width, region.height, Color::rgb(20, 25, 35));
        fb.rect(region.x, region.y, region.width, region.height, Color::rgb(0, 200, 150));
        
        // Title
        fb.draw_string(region.x + 10, region.y + 10, "SYSTEM STATUS", Color::rgb(0, 255, 200), None);
        fb.hline(region.x, region.y + 30, region.width, Color::rgb(60, 60, 80));
        
        // Stats
        if let Some(stats) = &self.last_stats {
            let y = region.y + 45;
            fb.draw_string(region.x + 10, y, "Strokes:", Color::rgb(150, 150, 150), None);
            draw_number(fb, region.x + 120, y, stats.strokes_processed);
            
            fb.draw_string(region.x + 10, y + 25, "Intents:", Color::rgb(150, 150, 150), None);
            draw_number(fb, region.x + 120, y + 25, stats.intents_matched);
            
            fb.draw_string(region.x + 10, y + 50, "Corrections:", Color::rgb(150, 150, 150), None);
            draw_number(fb, region.x + 120, y + 50, stats.corrections);
            
            fb.draw_string(region.x + 10, y + 75, "Unrecognized:", Color::rgb(150, 150, 150), None);
            draw_number(fb, region.x + 120, y + 75, stats.unrecognized);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELP PROJECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Shows help overlay when HELP intent is triggered
pub struct HelpProjection {
    visible: bool,
}

impl HelpProjection {
    pub const fn new() -> Self {
        Self { visible: false }
    }
}

impl Projection for HelpProjection {
    fn name(&self) -> &'static str { "HelpProjection" }
    
    fn responds_to(&self) -> &[ConceptID] {
        &[concepts::HELP]
    }
    
    fn priority(&self) -> ProjectionPriority { ProjectionPriority::Modal }
    
    fn is_visible(&self) -> bool { self.visible }
    
    fn is_modal(&self) -> bool { true }
    
    fn region(&self) -> Rect {
        // Centered modal
        Rect::new(400, 200, 600, 400)
    }
    
    fn on_intent(&mut self, intent: &Intent) {
        if intent.concept_id == concepts::HELP {
            self.visible = !self.visible; // Toggle
        }
    }
    
    fn render(&self, fb: &mut Framebuffer) {
        let region = self.region();
        
        // Semi-transparent overlay effect (darken background)
        // In a real impl, we'd use alpha blending
        
        // Modal background
        fb.fill_rect(region.x, region.y, region.width, region.height, Color::rgb(15, 20, 30));
        fb.rect(region.x, region.y, region.width, region.height, Color::rgb(0, 255, 200));
        
        // Title
        fb.draw_string(region.x + 10, region.y + 10, "INTENT KERNEL - HELP", Color::rgb(0, 255, 200), None);
        fb.hline(region.x, region.y + 35, region.width, Color::rgb(60, 60, 80));
        
        // Help content
        let y = region.y + 50;
        let commands = [
            ("STAT / status", "Show system status"),
            ("PH-FPL / help", "Toggle this help"),
            ("SHRO / show", "Show display"),
            ("HEU / hide", "Hide display"),
            ("STOR / store", "Store to memory"),
            ("RAOE/KAUL / recall", "Recall from memory"),
            ("TPHEFBGT / next", "Navigate next"),
            ("PREUF / previous", "Navigate previous"),
            ("KWRE / yes", "Confirm yes"),
            ("TPHO / no", "Confirm no"),
        ];
        
        for (i, (stroke, desc)) in commands.iter().enumerate() {
            let row_y = y + (i as u32 * 28);
            fb.draw_string(region.x + 20, row_y, stroke, Color::rgb(200, 200, 100), None);
            fb.draw_string(region.x + 250, row_y, desc, Color::WHITE, None);
        }
        
        // Footer
        fb.draw_string(region.x + 10, region.y + region.height - 30, 
            "Press HELP again to close", Color::rgb(100, 100, 100), None);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STENO TAPE PROJECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Visualizes the live steno stroke stream
pub struct StenoTapeProjection {
    lines: [heapless::String<32>; 20],
    head: usize,
    count: usize,
}

impl StenoTapeProjection {
    pub const fn new() -> Self {
        const EMPTY: heapless::String<32> = heapless::String::new();
        Self {
            lines: [EMPTY; 20],
            head: 0,
            count: 0,
        }
    }
    
    fn push(&mut self, text: &str) {
        self.lines[self.head] = heapless::String::from(text);
        self.head = (self.head + 1) % 20;
        if self.count < 20 {
            self.count += 1;
        }
    }
}

impl Projection for StenoTapeProjection {
    fn name(&self) -> &'static str { "StenoTape" }
    
    fn is_visible(&self) -> bool { true } // Always visible
    
    fn region(&self) -> Rect {
        Rect::new(50, 100, 300, 480)
    }
    
    fn on_intent(&mut self, _intent: &Intent) {}
    
    fn on_stroke(&mut self, stroke: &crate::steno::Stroke) {
        let rtfcre = stroke.to_rtfcre();
        self.push(rtfcre.as_str());
    }
    
    fn render(&self, fb: &mut Framebuffer) {
        let region = self.region();
        
        // Background
        fb.fill_rect(region.x, region.y, region.width, region.height, Color::rgb(10, 10, 15));
        fb.rect(region.x, region.y, region.width, region.height, Color::rgb(80, 80, 80));
        
        // Label
        fb.draw_string(region.x, region.y - 20, "STENO TAPE", Color::rgb(80, 80, 80), None);
        
        // Draw lines
        let start = if self.count < 20 { 0 } else { self.head };
        for i in 0..self.count {
            let idx = (start + i) % 20;
            let y = region.y + (i as u32 * 24);
            fb.draw_string(region.x + 10, y + 8, self.lines[idx].as_str(), Color::WHITE, None);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT LOG PROJECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Visualizes the stream of recognized intents
pub struct IntentLogProjection {
    lines: [heapless::String<32>; 20],
    head: usize,
    count: usize,
}

impl IntentLogProjection {
    pub const fn new() -> Self {
        const EMPTY: heapless::String<32> = heapless::String::new();
        Self {
            lines: [EMPTY; 20],
            head: 0,
            count: 0,
        }
    }
    
    fn push(&mut self, text: &str) {
        self.lines[self.head] = heapless::String::from(text);
        self.head = (self.head + 1) % 20;
        if self.count < 20 {
            self.count += 1;
        }
    }
}

impl Projection for IntentLogProjection {
    fn name(&self) -> &'static str { "IntentLog" }
    
    fn is_visible(&self) -> bool { true } // Always visible
    
    fn region(&self) -> Rect {
        Rect::new(400, 100, 400, 480)
    }
    
    fn on_intent(&mut self, intent: &Intent) {
        if !intent.name.is_empty() {
            self.push(intent.name);
        }
    }
    
    fn render(&self, fb: &mut Framebuffer) {
        let region = self.region();
        
        // Background
        fb.fill_rect(region.x, region.y, region.width, region.height, Color::rgb(10, 10, 15));
        fb.rect(region.x, region.y, region.width, region.height, Color::rgb(80, 80, 80));
        
        // Label
        fb.draw_string(region.x, region.y - 20, "INTENT STREAM", Color::rgb(80, 80, 80), None);
        
        // Draw lines
        let start = if self.count < 20 { 0 } else { self.head };
        for i in 0..self.count {
            let idx = (start + i) % 20;
            let y = region.y + (i as u32 * 24);
            fb.draw_string(region.x + 10, y + 8, self.lines[idx].as_str(), Color::rgb(0, 255, 200), None);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERCEPTION OVERLAY
// ═══════════════════════════════════════════════════════════════════════════════

/// Visualizes sensor data (simulated)
pub struct PerceptionOverlay {
    visible: bool,
}

impl PerceptionOverlay {
    pub const fn new() -> Self {
        Self { visible: true }
    }
}

impl Projection for PerceptionOverlay {
    fn name(&self) -> &'static str { "PerceptionOverlay" }
    
    fn is_visible(&self) -> bool { self.visible }
    
    fn region(&self) -> Rect {
        Rect::new(850, 100, 300, 200)
    }
    
    fn on_intent(&mut self, _intent: &Intent) {}
    
    fn render(&self, fb: &mut Framebuffer) {
        let region = self.region();
        
        // Background
        fb.fill_rect(region.x, region.y, region.width, region.height, Color::rgb(10, 15, 20));
        fb.rect(region.x, region.y, region.width, region.height, Color::rgb(50, 50, 100));
        
        // Label
        fb.draw_string(region.x, region.y - 20, "PERCEPTION", Color::rgb(50, 50, 100), None);
        
        // Simulated Camera Feed
        let mgr = crate::perception::PERCEPTION_MANAGER.lock();
        let sensors = mgr.sensors();
        
        let mut y_offset = 10;
        for sensor in sensors {
            fb.draw_string(region.x + 10, region.y + y_offset, sensor.backend_name(), Color::GREEN, None);
            y_offset += 20;
        }
        
        if sensors.is_empty() {
             fb.draw_string(region.x + 10, region.y + 10, "No Sensors Active", Color::RED, None);
        }

        // Draw a dummy object box (Placeholder until we expose detections globally)
        fb.rect(region.x + 10, region.y + 50, 280, 130, Color::rgb(30, 30, 30));
        fb.rect(region.x + 50, region.y + 80, 80, 80, Color::YELLOW);
        fb.draw_string(region.x + 50, region.y + 70, "Person (0.92)", Color::YELLOW, None);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MEMORY GRAPH
// ═══════════════════════════════════════════════════════════════════════════════

/// Visualizes semantic memory nodes
pub struct MemoryGraph {
    visible: bool,
}

impl MemoryGraph {
    pub const fn new() -> Self {
        Self { visible: true }
    }
}

impl Projection for MemoryGraph {
    fn name(&self) -> &'static str { "MemoryGraph" }
    
    fn is_visible(&self) -> bool { self.visible }
    
    fn region(&self) -> Rect {
        Rect::new(850, 350, 300, 230)
    }
    
    fn on_intent(&mut self, _intent: &Intent) {}
    
    fn render(&self, fb: &mut Framebuffer) {
        let region = self.region();
        
        // Background
        fb.fill_rect(region.x, region.y, region.width, region.height, Color::rgb(10, 15, 20));
        fb.rect(region.x, region.y, region.width, region.height, Color::rgb(100, 50, 100));
        
        // Label
        fb.draw_string(region.x, region.y - 20, "NEURAL MEMORY", Color::rgb(100, 50, 100), None);
        
        // Draw nodes (Real)
        let allocator = crate::kernel::memory::neural::NEURAL_ALLOCATOR.lock();
        let nodes = allocator.get_all_nodes();
        
        let mut i = 0;
        for node in nodes {
            if i >= 10 { break; } // Limit to 10 nodes for now
            
            // Simple layout: Grid
            let nx = 20 + (i % 3) * 80;
            let ny = 40 + (i / 3) * 40;
            
            let x = region.x + nx as u32;
            let y = region.y + ny as u32;
            
            let color = if (node.id.0 & 0xF000_0000) == 0x5000_0000 { Color::BLUE } else { Color::RED };
            
            fb.fill_rect(x, y, 6, 6, color);
            
            // Draw ID as hex
            let mut buf = [0u8; 16];
            if let Ok(s) = u64_to_hex(node.id.0, &mut buf) {
                 fb.draw_string(x + 10, y, s, color, None);
            }
            
            i += 1;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

fn u64_to_hex(mut num: u64, buf: &mut [u8]) -> Result<&str, ()> {
    if buf.len() < 16 { return Err(()); }
    if num == 0 {
        buf[0] = b'0';
        return Ok("0");
    }
    
    let mut i = 0;
    let mut temp = [0u8; 16];
    
    while num > 0 {
        let digit = (num % 16) as u8;
        temp[i] = if digit < 10 { digit + b'0' } else { digit - 10 + b'A' };
        num /= 16;
        i += 1;
    }
    
    for j in 0..i {
        buf[j] = temp[i - 1 - j];
    }
    
    core::str::from_utf8(&buf[..i]).map_err(|_| ())
}

fn draw_number(fb: &mut Framebuffer, x: u32, y: u32, num: u64) {
    let mut buf = [0u8; 20];
    if let Ok(s) = u64_to_str(num, &mut buf) {
        fb.draw_string(x, y, s, Color::WHITE, None);
    }
}

fn u64_to_str(mut num: u64, buf: &mut [u8]) -> Result<&str, ()> {
    if buf.len() < 20 { return Err(()); }
    
    if num == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).map_err(|_| ());
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
