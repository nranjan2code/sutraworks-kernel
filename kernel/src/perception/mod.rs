//! Adaptive Perception Layer
//!
//! This module manages the "Perception Cortex" of the Intent Kernel.
//! It automatically detects available hardware (Hailo-8 AI HAT) and selects
//! the appropriate backend for vision and audio tasks.

extern crate alloc;
use alloc::vec::Vec;
use alloc::boxed::Box;

pub mod vision;
pub mod hud;

use crate::drivers::hailo::HailoDriver;
use vision::{ObjectDetector, DetectedObject};

/// Wrapper to make HailoDriver implement ObjectDetector
struct HailoSensor {
    driver: HailoDriver,
}

impl ObjectDetector for HailoSensor {
    fn backend_name(&self) -> &'static str {
        "Hailo-8 AI"
    }

    fn detect(&self, _image_data: &[u8], _width: u32, _height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
        // In a real implementation, we would send the image to the Hailo device
        // via self.driver.send_command(...)
        self.driver.send_command(0x03, &[])?; // 0x03 = INFERENCE
        Err("Hailo inference not implemented")
    }
}

/// The global Perception Manager.
pub struct PerceptionManager {
    sensors: Vec<Box<dyn ObjectDetector>>,
}

impl PerceptionManager {
    /// Initialize the Perception Manager.
    /// Probes for hardware and sets up the appropriate backend.
    pub fn new() -> Self {
        let mut sensors: Vec<Box<dyn ObjectDetector>> = Vec::new();
        
        // Probe Hailo
        let mut hailo = HailoDriver::new();
        if hailo.probe() {
            sensors.push(Box::new(HailoSensor { driver: hailo }));
        }
        
        // Always add CPU Fallback (Sensor Fusion!)
        sensors.push(Box::new(vision::ColorBlobDetector::new_red_detector()));
        
        Self {
            sensors,
        }
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
        let cam_obj = DetectedObject { class_id: 1, confidence: 0.9, x: 0.5, y: 0.5, width: 0.1, height: 0.1 };
        let mut cam_objs = Vec::new();
        cam_objs.push(cam_obj);
        
        mgr.sensors.push(Box::new(VirtualSensor { 
            name: "Virtual Camera", 
            objects: cam_objs
        }));
        
        // Sensor 2: Lidar (Virtual)
        let lidar_obj = DetectedObject { class_id: 2, confidence: 0.8, x: 0.2, y: 0.2, width: 0.1, height: 0.1 };
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
