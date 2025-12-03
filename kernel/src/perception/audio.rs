//! Audio Perception Subsystem
//!
//! This module handles sound processing, feature extraction, and semantic projection.
//! It converts raw audio samples into 1024-bit Audio Hypervectors.
//!
//! # Features
//! - **Zero Crossing Rate (ZCR)**: Measure of noisiness/frequency.
//! - **Short-Time Energy (STE)**: Measure of loudness/activity.
//! - **Random Projection**: Maps features to semantic space.

use crate::perception::vision::RandomProjection;

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
    /// Returns an AudioEvent if significant activity is detected.
    pub fn process(samples: &[i16]) -> Option<AudioEvent> {
        if samples.is_empty() {
            return None;
        }

        // 1. Calculate Short-Time Energy (STE)
        // Sum of squared samples
        let mut energy: f32 = 0.0;
        for &s in samples {
            let val = s as f32 / 32768.0; // Normalize -1.0 to 1.0
            energy += val * val;
        }
        energy /= samples.len() as f32;

        // 2. Calculate Zero Crossing Rate (ZCR)
        // Rate of sign changes
        let mut zcr: f32 = 0.0;
        for i in 1..samples.len() {
            let prev = samples[i-1];
            let curr = samples[i];
            if (prev > 0 && curr <= 0) || (prev <= 0 && curr > 0) {
                zcr += 1.0;
            }
        }
        zcr /= samples.len() as f32;

        // 3. Classification (Simple Heuristic)
        // Silence: Low Energy
        // Speech: High Energy, Low-Mid ZCR
        // Noise: High Energy, High ZCR
        
        let threshold_silence = 0.001;
        let threshold_zcr_noise = 0.3; // If sign changes > 30% of samples, likely noise
        
        if energy < threshold_silence {
            return None; // Silence, ignore
        }
        
        let class_id = if zcr > threshold_zcr_noise {
            2 // Noise
        } else {
            1 // Speech (or Tonal)
        };
        
        // 4. Feature Projection
        // Features: [energy, zcr, 0...]
        let mut features = [0.0f32; 64];
        features[0] = energy;
        features[1] = zcr;
        // Add some "spectral" dummy features based on ZCR to vary the vector
        features[2] = zcr * energy; 
        features[3] = (1.0 - zcr) * energy;
        
        let hv_data = RandomProjection::project(&features);
        
        Some(AudioEvent {
            class_id,
            confidence: (energy * 10.0).min(1.0), // Crude confidence
            energy,
            zcr,
            hypervector: AudioHypervector { data: hv_data.data },
        })
    }
}
