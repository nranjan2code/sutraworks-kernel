//! Adaptive Perception Layer
//!
//! This module manages the "Perception Cortex" of the Intent Kernel.
//! It automatically detects available hardware (Hailo-8 AI HAT) and selects
//! the appropriate backend for vision and audio tasks.

pub mod vision;
pub mod hud;

use crate::drivers::hailo::HailoDriver;
use vision::{ObjectDetector, DetectedObject};

/// The global Perception Manager.
pub struct PerceptionManager {
    hailo_driver: Option<HailoDriver>,
    backend_type: BackendType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackendType {
    HailoHardware,
    CpuFallback,
}

impl PerceptionManager {
    /// Initialize the Perception Manager.
    /// Probes for hardware and sets up the appropriate backend.
    pub fn new() -> Self {
        let mut hailo = HailoDriver::new();
        let present = hailo.probe();

        if present {
            Self {
                hailo_driver: Some(hailo),
                backend_type: BackendType::HailoHardware,
            }
        } else {
            Self {
                hailo_driver: None,
                backend_type: BackendType::CpuFallback,
            }
        }
    }

    /// Get the active backend type.
    pub fn backend_type(&self) -> BackendType {
        self.backend_type
    }

    /// Perform object detection using the active backend.
    pub fn detect_objects(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
        match self.backend_type {
            BackendType::HailoHardware => {
                // In a real implementation, we would send the image to the Hailo device
                // via self.hailo_driver.as_ref().unwrap().send_command(...)
                // For now, we return a mock result indicating hardware acceleration.
                let mut objects = heapless::Vec::new();
                let _ = objects.push(DetectedObject {
                    class_id: 1, // "Person"
                    confidence: 0.99,
                    x: 0.5, y: 0.5, width: 0.2, height: 0.4
                });
                Ok(objects)
            },
            BackendType::CpuFallback => {
                // CPU Fallback: Return a dummy result or error indicating slow path
                let mut objects = heapless::Vec::new();
                let _ = objects.push(DetectedObject {
                    class_id: 0, // "Unknown"
                    confidence: 0.50,
                    x: 0.0, y: 0.0, width: 0.0, height: 0.0
                });
                Ok(objects)
            }
        }
    }
}
