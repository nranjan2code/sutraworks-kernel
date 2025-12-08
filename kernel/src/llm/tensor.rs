//! Tensor Operations for LLM Inference
//!
//! Minimal, no_std compatible tensor math library.
//! Focused on Llama 2 requirements (MatMul, RMSNorm, Softmax, RoPE).

use alloc::vec::Vec;
use core::f32::consts::PI;

/// Simple 1D float buffer acting as a tensor
/// Shape is implicit based on context in Llama2.c
pub struct Tensor {
    pub data: Vec<f32>,
}

impl Tensor {
    pub fn new(size: usize) -> Self {
        Self {
            data: alloc::vec![0.0; size],
        }
    }
    
    pub fn zeros(size: usize) -> Self {
        Self::new(size)
    }

    pub fn as_mut_ptr(&mut self) -> *mut f32 {
        self.data.as_mut_ptr()
    }

    pub fn as_ptr(&self) -> *const f32 {
        self.data.as_ptr()
    }
    
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MATH OPERATIONS (Naive Implementations)
// ═══════════════════════════════════════════════════════════════════════════════

/// Root Mean Square Normalization
/// y = (x / rms(x)) * weight
pub fn rms_norm(out: &mut [f32], x: &[f32], weight: &[f32], epsilon: f32) {
    let size = x.len();
    let mut ss = 0.0;
    
    // Sum of squares
    for val in x {
        ss += val * val;
    }
    ss /= size as f32;
    ss += epsilon;
    let inv_ss = 1.0 / libm::sqrtf(ss);
    
    // Normalize and scale
    for i in 0..size {
        out[i] = x[i] * inv_ss * weight[i];
    }
}

/// Softmax (in-place)
/// x_i = exp(x_i) / sum(exp(x))
pub fn softmax(x: &mut [f32]) {
    let size = x.len();
    
    // Find max for numerical stability
    let mut max_val = x[0];
    for i in 1..size {
        if x[i] > max_val {
            max_val = x[i];
        }
    }
    
    // Exp and Sum
    let mut sum = 0.0;
    for i in 0..size {
        x[i] = libm::expf(x[i] - max_val);
        sum += x[i];
    }
    
    // Normalize
    let inv_sum = 1.0 / sum;
    for i in 0..size {
        x[i] *= inv_sum;
    }
}

/// Matrix Multiplication: xout = x @ w
/// x: [d], w: [d, n], xout: [n]
/// weight matrix is typically flattened [n * d] or [d * n] depending on layout.
/// Llama2.c typically uses [d, n] (row-major) meaning W[row*n + col]??
/// No, usually weights are W[output_dim][input_dim] for standard NN linear layer y = Wx + b
/// But let's assume standard pointer arithmetic:
/// xout[i] = dot(x, w + i*d)
/// where d is input dimension, n is output dimension.
pub fn matmul(xout: &mut [f32], x: &[f32], w: &[f32], n: usize, d: usize) {
    // Parallelize? In kernel, we are single threaded per task usually, but we have cores.
    // For now, simple loop.
    
    for i in 0..n {
        let mut val = 0.0;
        let w_row = &w[i*d .. (i+1)*d]; // Access row i (assuming row-major W[n][d])
        
        for j in 0..d {
            val += w_row[j] * x[j];
        }
        xout[i] = val;
    }
}

/// Accumulated Matrix Multiplication: xout += x @ w
pub fn matmul_acc(xout: &mut [f32], x: &[f32], w: &[f32], n: usize, d: usize) {
     for i in 0..n {
        let mut val = 0.0;
        let w_row = &w[i*d .. (i+1)*d];
        
        for j in 0..d {
            val += w_row[j] * x[j];
        }
        xout[i] += val;
    }
}

/// Element-wise addition: x += y
pub fn add(x: &mut [f32], y: &[f32]) {
    for i in 0..x.len() {
        x[i] += y[i];
    }
}

/// Element-wise multiplication (silu approximation gating): x *= y
pub fn mul(x: &mut [f32], y: &[f32]) {
    for i in 0..x.len() {
        x[i] *= y[i];
    }
}

/// SwigGLU / SiLU: x = x * sigmoid(x)
/// Approximation: x * (1 / (1 + exp(-x)))
pub fn silu(x: &mut [f32]) {
    for val in x.iter_mut() {
        *val = *val * (1.0 / (1.0 + libm::expf(-*val)));
    }
}

/// Rotary Positional Embedding (RoPE)
pub fn rotary_embedding(q: &mut [f32], k: &mut [f32], pos: usize, head_dim: usize, n_kv_heads: usize, n_heads: usize) {
    // dim must be even
    for i in (0..head_dim).step_by(2) {
        let head_dim_idx = i;
        let freq = 1.0 / libm::powf(10000.0f32, (head_dim_idx as f32) / (head_dim as f32));
        let val = (pos as f32) * freq;
        let fcr = libm::cosf(val);
        let fci = libm::sinf(val);
        
        // Rotate Q (all heads)
        // Layout: Q[head][head_dim] -> Flattened: Q[head * head_dim + i]
        for h in 0..n_heads {
            let offset = h * head_dim + i;
            let q0 = q[offset];
            let q1 = q[offset + 1];
            q[offset] = q0 * fcr - q1 * fci;
            q[offset + 1] = q0 * fci + q1 * fcr;
        }
        
        // Rotate K (all kv heads)
        for h in 0..n_kv_heads {
            let offset = h * head_dim + i;
            let k0 = k[offset];
            let k1 = k[offset + 1];
            k[offset] = k0 * fcr - k1 * fci;
            k[offset + 1] = k0 * fci + k1 * fcr;
        }
    }
}
