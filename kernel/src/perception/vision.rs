//! Vision Subsystem Traits
//!
//! This module defines the common interfaces for computer vision tasks
//! within the Intent Kernel. It abstracts the underlying hardware (Hailo-8 vs CPU).

/// Represents a detected object in an image.
#[derive(Debug, Clone, Copy)]
pub struct DetectedObject {
    pub class_id: u32,
    pub confidence: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Represents a semantic embedding of an image (e.g., from CLIP).
pub struct ImageEmbedding {
    pub data: [f32; 512], // Fixed size for now, e.g., CLIP-ViT-B/32
}

/// Trait for Object Detection capabilities.
pub trait ObjectDetector {
    /// Detect objects in a raw image buffer.
    fn detect(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str>;
    
    /// Get the name of the backend (e.g., "Hailo-8", "CPU-MobileNet").
    fn backend_name(&self) -> &'static str;
}
