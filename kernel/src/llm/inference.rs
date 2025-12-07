//! Core Transformer Inference
//! 
//! Implements the Llama 2 forward pass.

use super::model::{Config, RunState, Weights};
use super::tensor::{matmul, matmul_acc, rms_norm, softmax, add, mul, silu, rotary_embedding}; // Note: need to re-verify tensor exports

/// Transformer Forward Pass
/// 
/// Computes the next token logits processing one token at a time.
/// 
/// # Arguments
/// * `token` - The input token (0..vocab_size)
/// * `pos` - The current position in the sequence
/// * `config` - Model configuration
/// * `state` - Mutable runtime state (activations)
/// * `weights` - Read-only model weights
pub fn forward(
    token: usize,
    pos: usize,
    config: &Config,
    state: &mut RunState,
    weights: &Weights,
) {
    let dim = config.dim;
    let hidden_dim = config.hidden_dim;
    let head_dim = dim / config.n_heads;
    
    // 1. Token Embedding
    // x = embedding[token]
    {
        let embed_offset = token * dim;
        // In a real system, we'd bounds check vs weights length
        state.x.data.copy_from_slice(&weights.token_embedding_table[embed_offset..embed_offset + dim]);
        // Note: Llama 2 typically doesn't normalize here immediately?
        // Actually Llama 2 architecture:
        // x = embedding
        // for layer in layers:
        //   x_norm = rms(x)
        //   ...
    }
    
    // Loop over layers
    for layer in 0..config.n_layers {
        
        // 2. Attention RMSPreNorm
        // xb = rms_norm(x)
        rms_norm(
            &mut state.xb.data, // Unsafe cast usually requires slice, but we can pass slice
            &state.x.data,
            weights.get_layer_weight(weights.rms_att_weight, layer, dim),
            1e-5
        );
        let xb = &state.xb.data; // Immutable ref for matmul
        
        // 3. QKV Projections
        // q = xb @ wq
        // k = xb @ wk
        // v = xb @ wv
        {
             // These are accumulated matmuls? No, standard.
             // wq is [dim * dim]
             matmul(
                 &mut state.q.data, // target
                 xb, // input
                 weights.get_layer_weight(weights.wq, layer, dim * dim),
                 dim, // output dim (n)
                 dim  // input dim (d)
             );
             
             // wk is [dim * kv_dim]
             // config.n_kv_heads * head_dim
             let kv_dim = (config.dim * config.n_kv_heads) / config.n_heads;
             matmul(
                 &mut state.k.data,
                 xb,
                 weights.get_layer_weight(weights.wk, layer, dim * kv_dim),
                 kv_dim,
                 dim
             );
             
             matmul(
                 &mut state.v.data,
                 xb,
                 weights.get_layer_weight(weights.wv, layer, dim * kv_dim),
                 kv_dim,
                 dim
             );
        }
        
        // 4. RoPE (Rotary Positional Embeddings)
        rotary_embedding(
            state.q.data.as_mut_slice(),
            state.k.data.as_mut_slice(),
            pos,
            head_dim,
            config.n_kv_heads,
            config.n_heads
        );
        
        // 5. KV Cache Update
        // Store k, v into cache at [layer, pos]
        {
            let kv_dim = (config.dim * config.n_kv_heads) / config.n_heads;
            let offset = layer * config.seq_len * kv_dim + pos * kv_dim;
            
            // Unsafe copy to avoid borrow checker hell with overlapping mutable borrows if structured poorly
            // We can just copy_from_slice
            state.key_cache.data[offset..offset+kv_dim].copy_from_slice(&state.k.data);
            state.value_cache.data[offset..offset+kv_dim].copy_from_slice(&state.v.data);
        }
        
        // 6. Multi-Head Attention
        // For each head...
        {
            let kv_dim = (config.dim * config.n_kv_heads) / config.n_heads;
            let att_size = config.n_heads * config.seq_len; // buffer size
            
            // We need to iterate over heads.
            for h in 0..config.n_heads {
                 // Get q for this head
                 let q_offset = h * head_dim;
                 let q_head = &state.q.data[q_offset..q_offset + head_dim];
                 
                 // Get attention scores buffer for this head (seq_len size)
                 // Layout of state.att: [head, seq_len] or [seq_len, head]? 
                 // Usually flattened: att[h * seq_len + t]
                 let att_offset = h * config.seq_len;
                 let att_head = &mut state.att.data[att_offset..att_offset + config.seq_len];
                 
                 // Calculate attention scores against ALL previous positions (0..=pos)
                 for t in 0..=pos {
                     // Get k from cache for this head at pos t
                     // K Cache: [layer, seq_len, kv_dim]
                     // GQA: Map head h to kv_head h_kv
                     let h_kv = h / (config.n_heads / config.n_kv_heads);
                     let k_offset = layer * config.seq_len * kv_dim + t * kv_dim + h_kv * head_dim;
                     
                     let k_vec = &state.key_cache.data[k_offset..k_offset + head_dim];
                     
                     // Dot product
                     let mut score = 0.0;
                     for i in 0..head_dim {
                         score += q_head[i] * k_vec[i];
                     }
                     
                     score /= libm::sqrtf(head_dim as f32);
                     att_head[t] = score;
                 }
                 
                 // Softmax on valid positions
                 softmax(&mut att_head[0..=pos]);
                 
                 // Weighted sum of values -> xb (output buffer)
                 // Output of attention usually goes into a buffer (xb2)
                 let xb_offset = h * head_dim;
                 let xb_head = &mut state.xb2.data[xb_offset..xb_offset + head_dim];
                 
                 // Zero out
                 xb_head.fill(0.0);
                 
                 for t in 0..=pos {
                    let a = att_head[t];
                    
                    // Get v from cache
                    let h_kv = h / (config.n_heads / config.n_kv_heads);
                    let v_offset = layer * config.seq_len * kv_dim + t * kv_dim + h_kv * head_dim;
                    let v_vec = &state.value_cache.data[v_offset..v_offset + head_dim];
                    
                    for i in 0..head_dim {
                        xb_head[i] += a * v_vec[i];
                    }
                 }
            }
        }
        
        // 7. Output Projection
        // x += xb2 @ wo
        // Note: xb2 contains the attention output (concatenated heads)
        // wo is [dim * dim]
        matmul_acc(
            &mut state.x.data, // Accumulate into residual x
            &state.xb2.data,                  // Attention output
            weights.get_layer_weight(weights.wo, layer, dim * dim),
            dim,
            dim
        );
        
        // 8. FFN RMSPreNorm
        // xb = rms_norm(x)
        rms_norm(
            &mut state.xb.data,
            &state.x.data,
            weights.get_layer_weight(weights.rms_ffn_weight, layer, dim),
            1e-5
        );
        let xb = &state.xb.data;
        
        // 9. FFN
        // hb = xb @ w1 (gate)
        // hb2 = xb @ w3 (up)
        matmul(
            &mut state.hb.data,
            xb,
            weights.get_layer_weight(weights.w1, layer, dim * hidden_dim),
            hidden_dim, 
            dim
        );
        matmul(
            &mut state.hb2.data,
            xb,
            weights.get_layer_weight(weights.w3, layer, dim * hidden_dim),
            hidden_dim, 
            dim
        );
        
        // silu(hb)
        silu(state.hb.data.as_mut_slice());
        
        // hb = hb * hb2
        mul(state.hb.data.as_mut_slice(), &state.hb2.data);
        
        // x += hb @ w2 (down)
        matmul_acc(
            &mut state.x.data,
            &state.hb.data,
            weights.get_layer_weight(weights.w2, layer, hidden_dim * dim),
            dim,
            hidden_dim // Input to w2 is hidden_dim
        );
    }
    
    // 10. Final RMSNorm
    // xb = rms_norm(x)
    // We can reuse xb or x (inplace? no rms_norm is out-of-place usually to allow residual, 
    // but here it's final so we can overwrite or use buffer)
    // Let's use xb
    rms_norm(
        &mut state.xb.data,
        &state.x.data,
        weights.rms_final_weight,
        1e-5
    );
    
    // 11. Classifier
    // logits = xb @ w_cls
    matmul(
        &mut state.logits.data,
        &state.xb.data,
        match weights.w_cls {
            Some(w) => w,
            None => weights.token_embedding_table, // Weight tying
        },
        config.vocab_size,
        dim
    );
}
