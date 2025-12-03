//! Random Projection Matrix for HDC
//!
//! This module implements a sparse ternary matrix for projecting real-valued
//! feature vectors into the 1024-bit binary hypervector space.
//!
//! # Theory
//! Random Projection (RP) is a technique to reduce dimensions while preserving
//! distances (Johnson-Lindenstrauss lemma). In HDC, we use it to map
//! continuous sensor data (e.g. 64 floats) to a 1024-bit semantic signature.
//!
//! We use a "Sparse Ternary Matrix" where weights are {-1, 0, +1}.
//! This is efficient to store and compute.

use alloc::vec::Vec;
use crate::kernel::memory::neural::Hypervector;

/// Size of the input feature vector (e.g. 64 floats from a CNN embedding)
pub const INPUT_DIM: usize = 64;
/// Size of the output hypervector (1024 bits)
pub const OUTPUT_DIM: usize = 1024;

/// A Sparse Ternary Matrix (1024 x 64)
/// Stored as a flat vector for cache efficiency, or we could compress it.
/// For 1024x64 = 65,536 entries.
/// If sparse (density ~10%), we can store indices.
/// For simplicity and "no_std" robustness, we'll store it as a packed format
/// or just a deterministic generator to save memory?
///
/// Saving memory is better. We can regenerate the row on the fly from a seed.
/// But that's slow for frequent inference.
///
/// Let's store it. 65k entries is small (65KB if i8).
pub struct Matrix {
    /// Flattened weights: row-major.
    /// weights[row * INPUT_DIM + col]
    weights: Vec<i8>, 
}

impl Matrix {
    /// Create a new random projection matrix from a seed
    pub fn new_random(seed: u64) -> Self {
        let mut weights = Vec::with_capacity(INPUT_DIM * OUTPUT_DIM);
        let mut rng = XorShift64 { state: seed };

        for _ in 0..(INPUT_DIM * OUTPUT_DIM) {
            // Generate ternary weights {-1, 0, 1}
            // We want sparsity, say 10% non-zero.
            // Random 0..100. If < 5 -> -1, if > 95 -> +1, else 0.
            let r = rng.next() % 100;
            let w = if r < 5 {
                -1
            } else if r >= 95 {
                1
            } else {
                0
            };
            weights.push(w);
        }

        Self { weights }
    }

    /// Project a feature vector into Hypervector space
    /// Output = sign(Matrix * Input)
    pub fn project(&self, input: &[f32]) -> Hypervector {
        if input.len() != INPUT_DIM {
            // In a real kernel we might panic or return error.
            // For now, just truncate or pad implicitly by loop limits.
            // But better to be safe.
        }

        let mut hv = [0u64; 16]; // 1024 bits

        for row in 0..OUTPUT_DIM {
            let mut sum = 0.0;
            for col in 0..INPUT_DIM {
                // Safe access check or assume valid
                if col < input.len() {
                    let w = self.weights[row * INPUT_DIM + col] as f32;
                    sum += w * input[col];
                }
            }

            // Binarize: if sum > 0 -> 1, else 0
            if sum > 0.0 {
                let word_idx = row / 64;
                let bit_idx = row % 64;
                hv[word_idx] |= 1 << bit_idx;
            }
        }

        hv
    }
}

/// Simple RNG for determinism
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}
