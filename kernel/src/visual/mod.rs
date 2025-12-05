//! Visual Projection Layer - Semantic Visual Interface
//!
//! The Visual Layer is a broadcast listener that renders semantic state to the screen.
//! Unlike traditional GUIs, it doesn't manage windows—it projects intents.
//!
//! # Architecture
//! ```
//! Intent (ConceptID) → Broadcast (1:N) → Visual Layer → Framebuffer
//! ```
//!
//! The visual layer is just one of many intent handlers.
//! It reflects semantic state, not user gestures.

extern crate alloc;

use alloc::boxed::Box;

use crate::kernel::sync::SpinLock;
use crate::drivers::framebuffer::{self, Color};
use crate::intent::{Intent, HandlerResult, HandlerFn};

pub mod projection;
pub mod compositor;

pub use projection::{Projection, ProjectionPriority, Rect};
pub use compositor::Compositor;

// ═══════════════════════════════════════════════════════════════════════════════
// VISUAL LAYER
// ═══════════════════════════════════════════════════════════════════════════════

/// The main visual layer manager
/// Listens to intent broadcasts and manages projections
pub struct VisualLayer {
    compositor: Compositor,
    enabled: bool,
}

impl VisualLayer {
    /// Create a new visual layer
    pub const fn new() -> Self {
        Self {
            compositor: Compositor::new(),
            enabled: false,
        }
    }
    
    /// Initialize the visual layer
    pub fn init(&mut self) {
        self.enabled = true;
        crate::kprintln!("[VISUAL] Visual Layer initialized");
    }
    
    /// Check if visual layer is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Register a projection
    pub fn register_projection(&mut self, projection: Box<dyn Projection>) {
        self.compositor.add_projection(projection);
    }
    
    /// Handle an intent broadcast
    /// Called by the intent executor as part of 1:N broadcast
    pub fn handle_intent(&mut self, intent: &Intent) -> HandlerResult {
        if !self.enabled {
            return HandlerResult::Handled;
        }
        
        // Notify all projections of the intent
        self.compositor.notify_intent(intent);
        
        // Request redraw
        self.render();
        
        // Always continue - we observe, don't consume
        HandlerResult::Handled
    }
    
    /// Handle a stroke input
    pub fn handle_stroke(&mut self, stroke: &crate::steno::Stroke) {
        if !self.enabled { return; }
        
        self.compositor.notify_stroke(stroke);
        self.render();
    }
    
    /// Render all active projections to the framebuffer
    pub fn render(&mut self) {
        framebuffer::with(|fb| {
            self.compositor.render(fb);
        });
    }
    
    /// Clear the screen
    pub fn clear(&mut self) {
        framebuffer::with(|fb| {
            fb.clear(Color::BLACK);
        });
    }
}

impl Default for VisualLayer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL VISUAL LAYER
// ═══════════════════════════════════════════════════════════════════════════════

static VISUAL_LAYER: SpinLock<VisualLayer> = SpinLock::new(VisualLayer::new());

/// Initialize the global visual layer
pub fn init() {
    let mut layer = VISUAL_LAYER.lock();
    layer.init();
    
    // Register default projections
    use projection::{StatusProjection, HelpProjection};
    layer.register_projection(Box::new(StatusProjection::new()));
    layer.register_projection(Box::new(HelpProjection::new()));
    layer.register_projection(Box::new(projection::StenoTapeProjection::new()));
    layer.register_projection(Box::new(projection::IntentLogProjection::new()));
    layer.register_projection(Box::new(projection::PerceptionOverlay::new()));
    layer.register_projection(Box::new(projection::MemoryGraph::new()));
}

/// Handle an intent (call from intent broadcast)
pub fn handle_intent(intent: &Intent) -> HandlerResult {
    let mut layer = VISUAL_LAYER.lock();
    layer.handle_intent(intent)
}

/// Handle a stroke
pub fn handle_stroke(stroke: &crate::steno::Stroke) {
    let mut layer = VISUAL_LAYER.lock();
    layer.handle_stroke(stroke);
}

/// Force a render of the visual layer
pub fn render() {
    let mut layer = VISUAL_LAYER.lock();
    layer.render();
}

/// Clear the screen
pub fn clear() {
    let mut layer = VISUAL_LAYER.lock();
    layer.clear();
}

/// Register a custom projection
pub fn register_projection(projection: Box<dyn Projection>) {
    let mut layer = VISUAL_LAYER.lock();
    layer.register_projection(projection);
}

/// Create the intent handler function for visual layer
/// This allows registering with the intent executor
pub fn create_handler() -> HandlerFn {
    |intent: &Intent| -> HandlerResult {
        handle_intent(intent)
    }
}
