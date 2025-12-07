//! Llama 2 Architectures Definition
//!
//! Structs defining the model configuration and runtime state.
//! Based on karpathy/llama2.c

use super::tensor::Tensor;
use alloc::vec::Vec;

/// Model Configuration
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub dim: usize,        // Transformer dimension
    pub hidden_dim: usize, // FFN dimension
    pub n_layers: usize,   // Number of layers
    pub n_heads: usize,    // Number of query heads
    pub n_kv_heads: usize, // Number of key/value heads (GQA)
    pub vocab_size: usize, // Vocabulary size
    pub seq_len: usize,    // Max sequence length
    pub shared_classifier: bool, // Weights shared?
}

/// Runtime State (Activations)
/// Allocates memory for the forward pass 
pub struct RunState {
    pub x: Tensor,      // Current activation [dim]
    pub xb: Tensor,     // Buffer [dim]
    pub xb2: Tensor,    // Buffer 2 [dim]
    pub hb: Tensor,     // Buffer [hidden_dim]
    pub hb2: Tensor,    // Buffer [hidden_dim]
    pub q: Tensor,      // Query [dim]
    pub k: Tensor,      // Key [dim] (dim can be smaller if n_kv_heads < n_heads)
    pub v: Tensor,      // Value [dim]
    pub att: Tensor,    // Attention scores [n_heads, seq_len]
    pub logits: Tensor, // Output logits [vocab_size]
    
    // KV Cache
    pub key_cache: Tensor,   // [n_layers, seq_len, dim]
    pub value_cache: Tensor, // [n_layers, seq_len, dim]
}

impl RunState {
    pub fn new(cfg: &Config) -> Self {
        let kv_dim = (cfg.dim * cfg.n_kv_heads) / cfg.n_heads;
        
        Self {
            x: Tensor::zeros(cfg.dim),
            xb: Tensor::zeros(cfg.dim),
            xb2: Tensor::zeros(cfg.dim),
            hb: Tensor::zeros(cfg.hidden_dim),
            hb2: Tensor::zeros(cfg.hidden_dim),
            q: Tensor::zeros(cfg.dim),
            k: Tensor::zeros(kv_dim), // GQA
            v: Tensor::zeros(kv_dim), // GQA
            att: Tensor::zeros(cfg.n_heads * cfg.seq_len),
            logits: Tensor::zeros(cfg.vocab_size),
            
            // Caches: Flattened
            // Size: layer * seq_len * kv_dim
            key_cache: Tensor::zeros(cfg.n_layers * cfg.seq_len * kv_dim),
            value_cache: Tensor::zeros(cfg.n_layers * cfg.seq_len * kv_dim),
        }
    }
}

/// Model Weights (References to data)
/// In a real system, these point to memory-mapped blobs.
/// For no_std/embedded, we might load them into a big Vec<f32>.
pub struct Weights<'a> {
    pub token_embedding_table: &'a [f32], // [vocab_size * dim]
    
    // Per-layer weights
    // We can store them as flattened arrays or simple slices
    // Storing as slice of slices is hard with lifetime hell in Run struct.
    // Llama2.c uses one big float pointer and offsets.
    // Let's adapt that philosophy: We wrap a single big buffer or individual buffers.
    // For safety, let's assume we maintain a struct of slices.
    
    pub rms_att_weight: &'a [f32], // [layers * dim]
    pub rms_ffn_weight: &'a [f32], // [layers * dim]
    
    pub wq: &'a [f32], // [layers * dim * dim]
    pub wk: &'a [f32], // [layers * dim * kv_dim]
    pub wv: &'a [f32], // [layers * dim * kv_dim]
    pub wo: &'a [f32], // [layers * dim * dim]
    
    pub w1: &'a [f32], // [layers * hidden_dim * dim]
    pub w2: &'a [f32], // [layers * dim * hidden_dim]
    pub w3: &'a [f32], // [layers * hidden_dim * dim]
    
    pub rms_final_weight: &'a [f32], // [dim]
    
    // Optional scaling for classifier (if not shared)
    // For TinyStories/Llama2, it's often shared.
    pub w_cls: Option<&'a [f32]>, 
}

impl<'a> Weights<'a> {
    /// Helper to get weight slice for a specific layer
    pub fn get_layer_weight(&self, full_slice: &'a[f32], layer: usize, size: usize) -> &'a[f32] {
        &full_slice[layer * size .. (layer + 1) * size]
    }
}
