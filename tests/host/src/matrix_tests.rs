#[cfg(test)]
mod tests {
    use crate::matrix::{Matrix, INPUT_DIM};
    use std::vec::Vec;

    // Helper for hamming similarity (copied from neural.rs for testing)
    fn hamming_similarity(a: &[u64; 16], b: &[u64; 16]) -> f32 {
        let mut diff_bits: u32 = 0;
        for i in 0..16 {
            diff_bits += (a[i] ^ b[i]).count_ones();
        }
        1.0 - (diff_bits as f32 / 1024.0)
    }

    #[test]
    fn test_matrix_determinism() {
        let seed = 12345;
        let m1 = Matrix::new_random(seed);
        let m2 = Matrix::new_random(seed);
        
        // Check if weights are identical
        // Since we can't access private fields easily in integration tests unless we expose them,
        // we'll test by projecting the same vector.
        let input = [0.5; INPUT_DIM];
        let p1 = m1.project(&input);
        let p2 = m2.project(&input);
        
        assert_eq!(p1, p2, "Deterministic matrix should produce identical projections");
    }

    #[test]
    fn test_lsh_property() {
        let matrix = Matrix::new_random(999);
        
        // Create two similar vectors (Euclidean distance small)
        let mut v1 = [0.0; INPUT_DIM];
        let mut v2 = [0.0; INPUT_DIM];
        let mut v3 = [0.0; INPUT_DIM]; // Distinct vector
        
        for i in 0..INPUT_DIM {
            v1[i] = (i as f32) / 100.0;
            v2[i] = v1[i] + 0.001; // Very small perturbation
            v3[i] = -(i as f32) / 100.0; // Opposite direction
        }
        
        let h1 = matrix.project(&v1);
        let h2 = matrix.project(&v2);
        let h3 = matrix.project(&v3);
        
        let sim_close = hamming_similarity(&h1, &h2);
        let sim_far = hamming_similarity(&h1, &h3);
        
        // LSH Property: Similar inputs -> High similarity
        // Dissimilar inputs -> Low similarity (approx 0.5 for orthogonal/random)
        
        assert!(sim_close > 0.9, "Similar vectors should have high similarity (got {})", sim_close);
        assert!(sim_far < 0.6, "Distinct vectors should have low similarity (got {})", sim_far);
    }
}
