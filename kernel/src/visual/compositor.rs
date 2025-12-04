//! Visual Compositor - Combines Projections
//!
//! The compositor manages multiple projections and renders them
//! to the framebuffer in the correct z-order.

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::drivers::framebuffer::{Color, Framebuffer};
use crate::intent::Intent;

use super::projection::Projection;

// ═══════════════════════════════════════════════════════════════════════════════
// COMPOSITOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum number of projections
const MAX_PROJECTIONS: usize = 16;

/// The visual compositor
/// Manages projections and renders them in z-order
pub struct Compositor {
    projections: Vec<Box<dyn Projection>>,
}

impl Compositor {
    /// Create a new compositor
    pub const fn new() -> Self {
        Self {
            projections: Vec::new(),
        }
    }
    
    /// Add a projection
    pub fn add_projection(&mut self, projection: Box<dyn Projection>) {
        if self.projections.len() < MAX_PROJECTIONS {
            self.projections.push(projection);
            // Sort by priority
            self.projections.sort_by_key(|p| p.priority());
        }
    }
    
    /// Remove a projection by name
    pub fn remove_projection(&mut self, name: &str) -> bool {
        if let Some(idx) = self.projections.iter().position(|p| p.name() == name) {
            self.projections.remove(idx);
            true
        } else {
            false
        }
    }
    
    /// Notify all projections of an intent
    pub fn notify_intent(&mut self, intent: &Intent) {
        for projection in &mut self.projections {
            // Check if this projection responds to this concept
            let responds = {
                let concepts = projection.responds_to();
                concepts.is_empty() || concepts.contains(&intent.concept_id)
            };
            
            if responds {
                projection.on_intent(intent);
            }
        }
    }
    
    /// Notify all projections of a stroke
    pub fn notify_stroke(&mut self, stroke: &crate::steno::Stroke) {
        for projection in &mut self.projections {
            projection.on_stroke(stroke);
        }
    }
    
    /// Render all visible projections
    pub fn render(&self, fb: &mut Framebuffer) {
        // Clear with background color
        fb.clear(Color::BLACK);
        
        // Check for modal projection
        let has_modal = self.projections.iter()
            .any(|p| p.is_visible() && p.is_modal());
        
        // Render in priority order (low to high = back to front)
        for projection in &self.projections {
            if !projection.is_visible() {
                continue;
            }
            
            // If there's a modal, only render modal projections
            if has_modal && !projection.is_modal() {
                // Still render background projections, just dimmed
                self.render_dimmed(fb, projection.as_ref());
            } else {
                projection.render(fb);
            }
        }
    }
    
    /// Render a projection dimmed (for modal overlay effect)
    fn render_dimmed(&self, fb: &mut Framebuffer, projection: &dyn Projection) {
        // First render normally
        projection.render(fb);
        
        // Then overlay a semi-transparent black
        // (In a real impl, we'd use proper alpha blending)
        let region = projection.region();
        for y in region.y..(region.y + region.height).min(fb.height()) {
            for x in region.x..(region.x + region.width).min(fb.width()) {
                let pixel = fb.get_pixel(x, y);
                // Dim to 50% brightness
                let r = pixel.r() / 2;
                let g = pixel.g() / 2;
                let b = pixel.b() / 2;
                fb.set_pixel(x, y, Color::rgb(r, g, b));
            }
        }
    }
    
    /// Get count of visible projections
    pub fn visible_count(&self) -> usize {
        self.projections.iter().filter(|p| p.is_visible()).count()
    }
    
    /// Get total projection count
    pub fn count(&self) -> usize {
        self.projections.len()
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}
