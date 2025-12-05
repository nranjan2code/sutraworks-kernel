//! Hailo-8 Tensor Parser
//!
//! Parses raw output tensors from Hailo-8 inference jobs into DetectedObject structures.
//! Supports YOLO-style output format (bounding boxes + class probabilities).

use crate::perception::vision::{DetectedObject};

/// YOLO Output Tensor Layout
///
/// Common YOLO formats output a tensor of shape [N, C] where:
/// - N = number of detection boxes (e.g., 1917 for YOLOv5s at 640x640)
/// - C = 5 + num_classes (x, y, w, h, confidence, class0_prob, class1_prob, ...)
///
/// For YOLOv5s with 80 classes: C = 85
/// Box coordinates are in normalized [0, 1] space.
pub struct YoloOutputParser {
    num_classes: usize,
    confidence_threshold: f32,
    nms_threshold: f32,  // Non-Maximum Suppression threshold
}

impl YoloOutputParser {
    /// Create a new parser for YOLO outputs
    pub const fn new() -> Self {
        Self {
            num_classes: 80,  // COCO dataset has 80 classes
            confidence_threshold: 0.5,
            nms_threshold: 0.4,
        }
    }

    /// Parse raw output tensor bytes into detected objects
    ///
    /// # Arguments
    /// * `output_data` - Raw bytes from Hailo output (f32 array serialized)
    /// * `output_size` - Size in bytes
    ///
    /// # Returns
    /// Vector of detected objects (up to 16)
    pub fn parse(&self, output_data: &[u8], output_size: u32) -> heapless::Vec<DetectedObject, 16> {
        let mut objects = heapless::Vec::new();

        // Each detection is 85 floats (4 bytes each) = 340 bytes
        let detection_size = (5 + self.num_classes) * 4;
        let num_detections = (output_size as usize) / detection_size;

        // Temporary buffer for valid detections before NMS
        let mut candidates: heapless::Vec<Detection, 64> = heapless::Vec::new();

        for i in 0..num_detections.min(1917) {  // YOLO produces up to ~2000 boxes
            let offset = i * detection_size;
            if offset + detection_size > output_data.len() {
                break;
            }

            // Parse box coordinates (normalized [0, 1])
            let x = self.read_f32(output_data, offset);
            let y = self.read_f32(output_data, offset + 4);
            let w = self.read_f32(output_data, offset + 8);
            let h = self.read_f32(output_data, offset + 12);
            let confidence = self.read_f32(output_data, offset + 16);

            // Filter by confidence threshold
            if confidence < self.confidence_threshold {
                continue;
            }

            // Find best class
            let mut best_class_id = 0;
            let mut best_class_prob = 0.0f32;

            for class_id in 0..self.num_classes.min(80) {
                let prob_offset = offset + 20 + (class_id * 4);
                let class_prob = self.read_f32(output_data, prob_offset);

                if class_prob > best_class_prob {
                    best_class_prob = class_prob;
                    best_class_id = class_id as u32;
                }
            }

            // Combined score: confidence * class_probability
            let score = confidence * best_class_prob;

            if score < self.confidence_threshold {
                continue;
            }

            // Add to candidates
            if candidates.len() < 64 {
                candidates.push(Detection {
                    x, y, w, h,
                    class_id: best_class_id,
                    confidence: score,
                }).ok();
            }
        }

        // Apply Non-Maximum Suppression (NMS)
        let nms_results = self.apply_nms(&candidates);

        // Convert to DetectedObject
        for det in nms_results {
            if objects.len() < 16 {
                objects.push(DetectedObject {
                    class_id: det.class_id,
                    confidence: det.confidence,
                    x: det.x,
                    y: det.y,
                    width: det.w,
                    height: det.h,
                }).ok();
            }
        }

        objects
    }

    /// Read f32 from byte array (little-endian)
    fn read_f32(&self, data: &[u8], offset: usize) -> f32 {
        if offset + 4 > data.len() {
            return 0.0;
        }

        let bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ];

        f32::from_le_bytes(bytes)
    }

    /// Apply Non-Maximum Suppression to remove overlapping boxes
    fn apply_nms(&self, candidates: &heapless::Vec<Detection, 64>) -> heapless::Vec<Detection, 16> {
        let mut result: heapless::Vec<Detection, 16> = heapless::Vec::new();
        let mut suppressed = [false; 64];

        // Sort by confidence (descending) - using simple bubble sort for embedded
        let mut sorted_indices: heapless::Vec<usize, 64> = heapless::Vec::new();
        for i in 0..candidates.len() {
            sorted_indices.push(i).ok();
        }

        // Bubble sort by confidence (descending)
        for i in 0..sorted_indices.len() {
            for j in 0..sorted_indices.len() - 1 - i {
                let idx1 = sorted_indices[j];
                let idx2 = sorted_indices[j + 1];

                if candidates[idx1].confidence < candidates[idx2].confidence {
                    sorted_indices.swap(j, j + 1);
                }
            }
        }

        // NMS algorithm
        for &i in sorted_indices.iter() {
            if suppressed[i] || result.len() >= 16 {
                continue;
            }

            result.push(candidates[i]).ok();

            // Suppress overlapping boxes
            for j in 0..candidates.len() {
                if i == j || suppressed[j] {
                    continue;
                }

                let iou = self.compute_iou(&candidates[i], &candidates[j]);
                if iou > self.nms_threshold {
                    suppressed[j] = true;
                }
            }
        }

        result
    }

    /// Compute Intersection over Union (IoU)
    fn compute_iou(&self, a: &Detection, b: &Detection) -> f32 {
        // Convert center coordinates to corners
        let a_x1 = a.x - a.w / 2.0;
        let a_y1 = a.y - a.h / 2.0;
        let a_x2 = a.x + a.w / 2.0;
        let a_y2 = a.y + a.h / 2.0;

        let b_x1 = b.x - b.w / 2.0;
        let b_y1 = b.y - b.h / 2.0;
        let b_x2 = b.x + b.w / 2.0;
        let b_y2 = b.y + b.h / 2.0;

        // Compute intersection
        let inter_x1 = a_x1.max(b_x1);
        let inter_y1 = a_y1.max(b_y1);
        let inter_x2 = a_x2.min(b_x2);
        let inter_y2 = a_y2.min(b_y2);

        let inter_w = (inter_x2 - inter_x1).max(0.0);
        let inter_h = (inter_y2 - inter_y1).max(0.0);
        let inter_area = inter_w * inter_h;

        // Compute union
        let a_area = a.w * a.h;
        let b_area = b.w * b.h;
        let union_area = a_area + b_area - inter_area;

        if union_area > 0.0 {
            inter_area / union_area
        } else {
            0.0
        }
    }
}

/// Internal detection representation (before conversion to DetectedObject)
#[derive(Clone, Copy)]
struct Detection {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    class_id: u32,
    confidence: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_tensor_parsing() {
        let parser = YoloOutputParser::new();

        // Mock output: 1 detection with high confidence
        let mut output = vec![0u8; 340];  // 85 floats * 4 bytes

        // Write mock detection: x=0.5, y=0.5, w=0.2, h=0.2, conf=0.9
        output[0..4].copy_from_slice(&0.5f32.to_le_bytes());   // x
        output[4..8].copy_from_slice(&0.5f32.to_le_bytes());   // y
        output[8..12].copy_from_slice(&0.2f32.to_le_bytes());  // w
        output[12..16].copy_from_slice(&0.2f32.to_le_bytes()); // h
        output[16..20].copy_from_slice(&0.9f32.to_le_bytes()); // confidence

        // Class 0 with probability 0.95
        output[20..24].copy_from_slice(&0.95f32.to_le_bytes());

        let objects = parser.parse(&output, 340);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].class_id, 0);
        assert!(objects[0].confidence > 0.8);  // 0.9 * 0.95
    }

    #[test]
    fn test_nms() {
        let parser = YoloOutputParser::new();

        let mut candidates = heapless::Vec::new();

        // Two overlapping boxes - NMS should keep only the higher confidence one
        candidates.push(Detection {
            x: 0.5, y: 0.5, w: 0.2, h: 0.2,
            class_id: 0,
            confidence: 0.9,
        }).ok();

        candidates.push(Detection {
            x: 0.52, y: 0.52, w: 0.21, h: 0.21,
            class_id: 0,
            confidence: 0.7,  // Lower confidence
        }).ok();

        let result = parser.apply_nms(&candidates);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].confidence, 0.9);  // Kept the higher one
    }
}
