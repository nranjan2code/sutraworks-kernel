//! Minimal BPE Tokenizer
//!
//! Handles decoding of tokens into strings.
//! Supports loading from a binary format or using a dummy vocabulary.

use alloc::vec::Vec;
use alloc::string::String;

pub struct Tokenizer {
    vocab: Vec<String>,
    vocab_scores: Vec<f32>,
    pub vocab_size: usize,
}

impl Tokenizer {
    pub fn new(vocab_size: usize) -> Self {
        Self {
            vocab: Vec::with_capacity(vocab_size),
            vocab_scores: Vec::with_capacity(vocab_size),
            vocab_size,
        }
    }
    
    /// Create a dummy tokenizer (for testing without file)
    pub fn dummy(vocab_size: usize) -> Self {
        let mut t = Self::new(vocab_size);
        for i in 0..vocab_size {
            // Mostly empty, some common words
            let s = match i {
                0 => "<unk>".into(),
                1 => "<s>".into(),
                2 => "</s>".into(),
                _ => alloc::format!("t{}", i),
            };
            t.vocab.push(s);
            t.vocab_scores.push(0.0);
        }
        t
    }
    
    /// Decode a token index to a string slice
    pub fn decode(&self, token: usize, _prev_token: usize) -> Option<&str> {
        // Real Llama 2 tokenizer has logic for spaces based on previous token,
        // but for now simple lookup is enough for PoC.
        if token < self.vocab.len() {
            Some(&self.vocab[token])
        } else {
            None
        }
    }
}
