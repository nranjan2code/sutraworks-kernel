//! Kernel Native Large Language Model Engine
//!
//! Implements a "System 2" cognitive engine using a simplified Llama 2 architecture.
//! Allows the kernel to "think" (inference) on input intents.

pub mod tensor;
pub mod model;
pub mod tokenizer;
pub mod inference;

// Re-exports
pub use model::{Config, RunState, Weights};
pub use tensor::Tensor;
