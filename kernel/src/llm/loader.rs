use super::model::{Config, Weights};
use crate::fs::vfs::{VFS, O_RDONLY};
use alloc::vec::Vec;
use alloc::vec;
use core::mem::size_of;

/// Owns the data for the model
pub struct OwnedWeights {
    pub config: Config,
    pub data: Vec<f32>,
}

impl OwnedWeights {
    pub fn as_weights<'a>(&'a self) -> Weights<'a> {
        let dim = self.config.dim;
        let hidden_dim = self.config.hidden_dim;
        let n_layers = self.config.n_layers;
        let vocab_size = self.config.vocab_size;
        let kv_dim = (dim * self.config.n_kv_heads) / self.config.n_heads;
        
        let mut offset = 0;
        
        // Helper to slice and advance
        let mut take = |count: usize| -> &'a [f32] {
            let slice = &self.data[offset..offset+count];
            offset += count;
            slice
        };
        
        let token_embedding_table = take(vocab_size * dim);
        let rms_att_weight = take(n_layers * dim);
        let wq = take(n_layers * dim * dim);
        let wk = take(n_layers * dim * kv_dim);
        let wv = take(n_layers * dim * kv_dim);
        let wo = take(n_layers * dim * dim);
        let rms_ffn_weight = take(n_layers * dim);
        let w1 = take(n_layers * hidden_dim * dim);
        let w2 = take(n_layers * dim * hidden_dim);
        let w3 = take(n_layers * hidden_dim * dim);
        let rms_final_weight = take(dim);
        
        // Classifier
        let w_cls = if self.config.shared_classifier {
            None
        } else {
             Some(take(vocab_size * dim))
        };
        
        Weights {
            token_embedding_table,
            rms_att_weight,
            rms_ffn_weight,
            wq,
            wk,
            wv,
            wo,
            w1,
            w2,
            w3,
            rms_final_weight,
            w_cls,
        }
    }
}

pub fn load_model(path: &str) -> Result<OwnedWeights, &'static str> {
    let fs_lock = VFS.lock();
    let file_lock = fs_lock.open(path, O_RDONLY)?;
    let mut file = file_lock.lock();
    
    // 1. Read Config
    // We need to read raw bytes and transmute to Config struct
    // Valid only if Config is #[repr(C)] -- check model.rs
    // For now, let's manually read fields to be safe/portable (or check repr)
    // model.rs: Config is standard Rust struct, likely not safe to memcopy directly unless repr(C).
    // Let's assume the file has a 7-integer header as defined in standard formats.
    
    let mut header = [0u32; 7];
    let header_bytes = unsafe {
        core::slice::from_raw_parts_mut(header.as_mut_ptr() as *mut u8, 28)
    };
    
    if file.read(header_bytes)? != 28 {
        return Err("Failed to read header");
    }
    
    let config = Config {
        dim: header[0] as usize,
        hidden_dim: header[1] as usize,
        n_layers: header[2] as usize,
        n_heads: header[3] as usize,
        n_kv_heads: header[4] as usize,
        vocab_size: header[5] as usize,
        seq_len: header[6] as usize,
        shared_classifier: true, // TODO: Detect from header or magic? Assuming shared for now (check magic?)
    };
    
    // 2. Calculate size
    let kv_dim = (config.dim * config.n_kv_heads) / config.n_heads;
    
    let mut total_params = 0;
    total_params += config.vocab_size * config.dim; // embeddings
    total_params += config.n_layers * config.dim; // rms_att
    total_params += config.n_layers * config.dim * config.dim; // wq
    total_params += config.n_layers * config.dim * kv_dim; // wk
    total_params += config.n_layers * config.dim * kv_dim; // wv
    total_params += config.n_layers * config.dim * config.dim; // wo
    total_params += config.n_layers * config.dim; // rms_ffn
    total_params += config.n_layers * config.hidden_dim * config.dim; // w1
    total_params += config.n_layers * config.dim * config.hidden_dim; // w2
    total_params += config.n_layers * config.hidden_dim * config.dim; // w3
    total_params += config.dim; // rms_final
    
    if !config.shared_classifier {
        total_params += config.vocab_size * config.dim;
    }
    
    // 3. Allocate and Read
    let mut data = vec![0.0f32; total_params];
    let data_bytes = unsafe {
        core::slice::from_raw_parts_mut(
            data.as_mut_ptr() as *mut u8,
            total_params * 4
        )
    };
    
    // Read chunks to avoid huge buffers if needed, or just one go?
    // Fat32 read handles chunking.
    let read_len = file.read(data_bytes)?;
    
    if read_len != total_params * 4 {
         // It might be partial read? FileOps read should try to read all if possible?
         // Our FAT32 read implementation reads until buffer full or EOF.
         return Err("Incomplete weight file");
    }
    
    Ok(OwnedWeights {
        config,
        data,
    })
}
