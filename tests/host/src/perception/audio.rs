//! Audio Perception Subsystem (Host Test Version)

use crate::matrix::{Matrix, INPUT_DIM};

/// Represents a semantic hypervector of an audio segment (1024-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioHypervector {
    pub data: [u64; 16],
}

/// A detected audio event.
#[derive(Debug, Clone, Copy)]
pub struct AudioEvent {
    pub class_id: u32, // 0=Silence, 1=Speech, 2=Noise
    pub confidence: f32,
    pub energy: f32,
    pub zcr: f32,
    pub hypervector: AudioHypervector,
}

/// Audio Feature Extractor
pub struct AudioProcessor;

impl AudioProcessor {
    /// Process a chunk of audio samples (PCM 16-bit, Mono)
    pub fn process(samples: &[i16]) -> Option<AudioEvent> {
        if samples.is_empty() {
            return None;
        }

        // 1. Calculate Short-Time Energy (STE)
        let mut energy: f32 = 0.0;
        for &s in samples {
            let val = s as f32 / 32768.0;
            energy += val * val;
        }
        energy /= samples.len() as f32;

        // 2. Calculate Zero Crossing Rate (ZCR)
        let mut zcr: f32 = 0.0;
        for i in 1..samples.len() {
            let prev = samples[i-1];
            let curr = samples[i];
            if (prev > 0 && curr <= 0) || (prev <= 0 && curr > 0) {
                zcr += 1.0;
            }
        }
        zcr /= samples.len() as f32;

        let threshold_silence = 0.001;
        let threshold_zcr_noise = 0.3;
        
        if energy < threshold_silence {
            return None;
        }
        
        let class_id = if zcr > threshold_zcr_noise {
            2 // Noise
        } else {
            1 // Speech
        };
        
        // 4. Feature Projection (Mocked RandomProjection logic for host)
        // We reuse the Matrix logic we ported earlier
        let mut features = [0.0f32; INPUT_DIM];
        features[0] = energy;
        features[1] = zcr;
        features[2] = zcr * energy; 
        features[3] = (1.0 - zcr) * energy;
        
        // Create a deterministic matrix for testing
        let matrix = Matrix::new_random(0x12345678);
        let hv_data = matrix.project(&features);
        
        Some(AudioEvent {
            class_id,
            confidence: (energy * 10.0).min(1.0),
            energy,
            zcr,
            hypervector: AudioHypervector { data: hv_data },
        })
    }
}
