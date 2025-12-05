//! Adaptive Perception Layer
//!
//! This module manages the "Perception Cortex" of the Intent Kernel.
//! It automatically detects available hardware (Hailo-8 AI HAT) and selects
//! the appropriate backend for vision and audio tasks.

extern crate alloc;
use alloc::vec::Vec;
use alloc::boxed::Box;

pub mod vision;
pub mod audio;
pub mod hud;

use crate::drivers::hailo::HailoDriver;
use vision::{ObjectDetector, DetectedObject};
use crate::kernel::memory::neural::NEURAL_ALLOCATOR;
use crate::intent::ConceptID;

use crate::kernel::sync::SpinLock;

/// Wrapper to make HailoDriver implement ObjectDetector
struct HailoSensor {
    driver: SpinLock<HailoDriver>,
}

impl ObjectDetector for HailoSensor {
    fn backend_name(&self) -> &'static str {
        "Hailo-8 AI"
    }

    fn detect(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
        // Use the high-level detect_objects API which includes tensor parsing
        self.driver.lock().detect_objects(image_data, width, height)
    }
}

/// The global Perception Manager.
pub struct PerceptionManager {
    sensors: Vec<Box<dyn ObjectDetector>>,
}

/// Global Perception Manager instance
pub static PERCEPTION_MANAGER: SpinLock<PerceptionManager> = SpinLock::new(PerceptionManager { sensors: Vec::new() });

impl PerceptionManager {
    /// Initialize the Perception Manager.
    /// Probes for hardware and sets up the appropriate backend.
    pub fn new() -> Self {
        let mut sensors: Vec<Box<dyn ObjectDetector>> = Vec::new();
        
        // Probe Hailo
        let mut hailo = HailoDriver::new();
        if hailo.init().is_ok() {
            sensors.push(Box::new(HailoSensor { driver: SpinLock::new(hailo) }));
        }
        
        // Always add CPU Fallback (Sensor Fusion!)
        sensors.push(Box::new(vision::ColorBlobDetector::new_red_detector()));
        sensors.push(Box::new(vision::EdgeDetector));
        
        Self {
            sensors,
        }
    }

    /// Initialize the global manager
    pub fn init() {
        let mut mgr = PERCEPTION_MANAGER.lock();
        *mgr = Self::new();
    }

    /// Get the list of active sensors.
    pub fn sensors(&self) -> &[Box<dyn ObjectDetector>] {
        &self.sensors
    }

    /// Perform object detection using ALL active sensors (Fusion).
    pub fn detect_objects(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
        let mut fused_results = heapless::Vec::<DetectedObject, 16>::new();
        let mut any_success = false;
        
        for sensor in &self.sensors {
            if let Ok(results) = sensor.detect(image_data, width, height) {
                any_success = true;
                // Simple fusion: append all results
                // In a real implementation, we would merge overlapping boxes
                for obj in results {
                    if fused_results.len() < 16 {
                        fused_results.push(obj).ok();
                    }
                }
            }
        }
        
        if !any_success {
             Err("No objects detected by any sensor")
        } else {
             Ok(fused_results)
        }
    }

    /// Perceive the environment and store intents in Neural Memory.
    /// This bridges the gap between "Seeing" (Vision) and "Thinking" (Memory).
    pub fn perceive_and_store(&self, image_data: &[u8], width: u32, height: u32) -> Result<usize, &'static str> {
        let objects = self.detect_objects(image_data, width, height)?;
        let mut count = 0;
        
        let mut allocator = NEURAL_ALLOCATOR.lock();
        
        for obj in objects {
            // Map Class ID to Concept ID (Simple mapping for now)
            // e.g., Class 1 (Red Blob) -> ConceptID(0xCAFE_0001)
            let concept_id = ConceptID(0xCAFE_0000 | obj.class_id as u64);
            
            // Store in Neural Memory
            // We store the object metadata as the "data" payload.
            // The key is the Hypervector.
            unsafe {
                allocator.alloc(
                    core::mem::size_of::<DetectedObject>(), 
                    concept_id, 
                    obj.hypervector.data
                );
            }
            count += 1;
        }
        
        Ok(count)
    }

    /// Process audio samples and store acoustic intents in Neural Memory.
    pub fn perceive_audio(&self, samples: &[i16]) -> Result<bool, &'static str> {
        if let Some(event) = audio::AudioProcessor::process(samples) {
            let mut allocator = NEURAL_ALLOCATOR.lock();
            
            // Map Audio Class to Concept ID
            // Class 1 (Speech) -> 0x500D_0001 (SOUND_SPEECH)
            // Class 2 (Noise) -> 0x500D_0002 (SOUND_NOISE)
            let concept_id = ConceptID(0x500D_0000 | event.class_id as u64);
            
            unsafe {
                allocator.alloc(
                    core::mem::size_of::<audio::AudioEvent>(),
                    concept_id,
                    event.hypervector.data
                );
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}



impl Default for PerceptionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the perception subsystem
pub fn init() {
    PerceptionManager::init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perception::vision::DetectedObject;

    struct VirtualSensor {
        name: &'static str,
        objects: Vec<DetectedObject>,
    }

    impl ObjectDetector for VirtualSensor {
        fn backend_name(&self) -> &'static str { self.name }
        fn detect(&self, _: &[u8], _: u32, _: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
            let mut res = heapless::Vec::new();
            for obj in &self.objects {
                res.push(*obj).ok();
            }
            Ok(res)
        }
    }

    #[test]
    fn test_sensor_fusion() {
        let mut mgr = PerceptionManager { sensors: Vec::new() };
        
        // Sensor 1: Camera (Virtual)
        let cam_obj = DetectedObject { 
            class_id: 1, 
            confidence: 0.9, 
            x: 0.5, y: 0.5, width: 0.1, height: 0.1,
            hypervector: crate::perception::vision::VisualHypervector { data: [1; 16] } 
        };
        let mut cam_objs = Vec::new();
        cam_objs.push(cam_obj);
        
        mgr.sensors.push(Box::new(VirtualSensor { 
            name: "Virtual Camera", 
            objects: cam_objs
        }));
        
        // Sensor 2: Lidar (Virtual)
        let lidar_obj = DetectedObject { 
            class_id: 2, 
            confidence: 0.8, 
            x: 0.2, y: 0.2, width: 0.1, height: 0.1,
            hypervector: crate::perception::vision::VisualHypervector { data: [2; 16] }
        };
        let mut lidar_objs = Vec::new();
        lidar_objs.push(lidar_obj);
        
        mgr.sensors.push(Box::new(VirtualSensor { 
            name: "Virtual Lidar", 
            objects: lidar_objs 
        }));
        
        // Detect (Fuse)
        let results = mgr.detect_objects(&[], 100, 100).unwrap();
        
        // Verify fusion
        assert_eq!(results.len(), 2);
        
        // Verify content
        let has_cam = results.iter().any(|o| o.class_id == 1);
        let has_lidar = results.iter().any(|o| o.class_id == 2);
        
        assert!(has_cam, "Missing camera object");
        assert!(has_lidar, "Missing lidar object");
    }
}
