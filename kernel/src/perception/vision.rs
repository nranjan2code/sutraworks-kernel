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

/// Trait for Object Detection capabilities.
pub trait ObjectDetector: Send {
    /// Detect objects in a raw image buffer.
    fn detect(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str>;
    
    /// Get the name of the backend (e.g., "Hailo-8", "CPU-MobileNet").
    fn backend_name(&self) -> &'static str;
}

/// A simple CPU-based detector that finds blobs of a specific color.
pub struct ColorBlobDetector {
    target_r: u8,
    target_g: u8,
    target_b: u8,
    threshold: u8,
}

impl ColorBlobDetector {
    /// Create a new detector for "Red" objects
    pub fn new_red_detector() -> Self {
        Self {
            target_r: 255,
            target_g: 0,
            target_b: 0,
            threshold: 100, // Distance threshold
        }
    }

    fn color_distance(&self, r: u8, g: u8, b: u8) -> u32 {
        let dr = (r as i32 - self.target_r as i32).abs();
        let dg = (g as i32 - self.target_g as i32).abs();
        let db = (b as i32 - self.target_b as i32).abs();
        (dr + dg + db) as u32
    }
}

impl ObjectDetector for ColorBlobDetector {
    fn backend_name(&self) -> &'static str {
        "CPU-ColorBlob"
    }

    fn detect(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
        // Assume RGB888 format
        if image_data.len() != (width * height * 3) as usize {
            return Err("Invalid image buffer size");
        }

        let mut min_x = width;
        let mut max_x = 0;
        let mut min_y = height;
        let mut max_y = 0;
        let mut count = 0;
        let mut total_confidence = 0.0;

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;
                let r = image_data[idx];
                let g = image_data[idx + 1];
                let b = image_data[idx + 2];

                let dist = self.color_distance(r, g, b);
                
                if dist < self.threshold as u32 {
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                    if y < min_y { min_y = y; }
                    if y > max_y { max_y = y; }
                    count += 1;
                    
                    // Confidence is inverse of distance
                    total_confidence += 1.0 - (dist as f32 / (255.0 * 3.0));
                }
            }
        }

        let mut objects = heapless::Vec::new();

        // If we found a significant blob (> 0.1% of pixels)
        if count > (width * height / 1000) {
            let obj_width = (max_x - min_x) as f32 / width as f32;
            let obj_height = (max_y - min_y) as f32 / height as f32;
            let center_x = min_x as f32 / width as f32 + obj_width / 2.0;
            let center_y = min_y as f32 / height as f32 + obj_height / 2.0;
            let avg_confidence = total_confidence / count as f32;

            let _ = objects.push(DetectedObject {
                class_id: 1, // "Target Color"
                confidence: avg_confidence,
                x: center_x,
                y: center_y,
                width: obj_width,
                height: obj_height,
            });
        }

        Ok(objects)
    }
}

/// Sobel Edge Detector
pub struct EdgeDetector;

impl ObjectDetector for EdgeDetector {
    fn backend_name(&self) -> &'static str {
        "CPU-EdgeDetector"
    }

    fn detect(&self, image_data: &[u8], width: u32, height: u32) -> Result<heapless::Vec<DetectedObject, 16>, &'static str> {
        let mut objects = heapless::Vec::new();
        
        // Simple Sobel Operator (Grayscale)
        // We assume image_data is RGB888 (3 bytes per pixel)
        // We'll scan the center of the image to avoid boundary checks for this prototype
        
        let mut max_grad = 0;
        let mut edge_pixels = 0;
        let mut center_x = 0;
        let mut center_y = 0;
        
        for y in 1..height-1 {
            for x in 1..width-1 {
                let _idx = ((y * width + x) * 3) as usize;
                
                // Convert to grayscale: 0.299R + 0.587G + 0.114B
                // Simplified: (R+G+B)/3
                let gray = |x, y| {
                    let i = ((y * width + x) * 3) as usize;
                    (image_data[i] as i32 + image_data[i+1] as i32 + image_data[i+2] as i32) / 3
                };
                
                // Sobel Kernels
                // Gx: -1 0 1
                //     -2 0 2
                //     -1 0 1
                let gx = -gray(x-1, y-1) + gray(x+1, y-1)
                         -2*gray(x-1, y) + 2*gray(x+1, y)
                         -gray(x-1, y+1) + gray(x+1, y+1);
                         
                // Gy: -1 -2 -1
                //      0  0  0
                //      1  2  1
                let gy = -gray(x-1, y-1) - 2*gray(x, y-1) - gray(x+1, y-1)
                         +gray(x-1, y+1) + 2*gray(x, y+1) + gray(x+1, y+1);
                         
                let magnitude = (gx.abs() + gy.abs()) as u32; // Approx magnitude
                
                if magnitude > 100 { // Threshold
                    edge_pixels += 1;
                    center_x += x;
                    center_y += y;
                    if magnitude > max_grad {
                        max_grad = magnitude;
                    }
                }
            }
        }
        
        if edge_pixels > 100 {
            // Feature Extraction
            let center_x_norm = (center_x as f32) / (edge_pixels as f32) / (width as f32);
            let center_y_norm = (center_y as f32) / (edge_pixels as f32) / (height as f32);
            // let density = (edge_pixels as f32) / ((width * height) as f32);
            let intensity = (max_grad as f32) / 255.0;
            
            let _ = objects.push(DetectedObject {
                class_id: 2, // "Edge/Shape"
                confidence: intensity.min(1.0),
                x: center_x_norm,  // Normalized coordinates [0.0, 1.0]
                y: center_y_norm,
                width: 0.0, // Unknown for edge detection
                height: 0.0, // Unknown for edge detection
            });
        }
        
        Ok(objects)
    }
}
