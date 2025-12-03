//! Random Projection Matrix for HDC (Test Implementation)
//!
//! This is a copy of the kernel implementation adapted for the host test harness.

use std::vec::Vec;

/// Size of the input feature vector (e.g. 64 floats from a CNN embedding)
pub const INPUT_DIM: usize = 64;
/// Size of the output hypervector (1024 bits)
pub const OUTPUT_DIM: usize = 1024;

/// 1024-bit Hypervector (16 x 64-bit integers)
pub type Hypervector = [u64; 16];

pub struct Matrix {
    /// Flattened weights: row-major.
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
            // Panic in tests is fine
            panic!("Input dimension mismatch");
        }

        let mut hv = [0u64; 16]; // 1024 bits

        for row in 0..OUTPUT_DIM {
            let mut sum = 0.0;
            for col in 0..INPUT_DIM {
                let w = self.weights[row * INPUT_DIM + col] as f32;
                sum += w * input[col];
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
