//! Steno Input Engine (Fastest Path)
//!
//! The fastest human input method, now native to an OS kernel.
//! Strokes bypass all text processing - direct semantic mapping.
//!
//! # Philosophy
//! Stenography has been the ultimate human-to-machine compression for 150 years.
//! A single stroke encodes an entire concept, achieving 200+ WPM throughput.
//! This kernel treats strokes as first-class semantic primitives.
//!
//! # Architecture
//! ```
//! Steno Machine → Raw Stroke (23-bit) → Stroke → Dictionary → Intent → Action
//! ```
//! No tokenization. No parsing. Direct semantic mapping (<0.1μs).
//!
//! # Key Order (Plover english_stenotype.py)
//! ```
//!   #  #  #  #  #  #  #  #  #  #
//!   S  T  P  H  *  F  P  L  T  D
//!   S  K  W  R  *  R  B  G  S  Z
//!         A  O     E  U
//!
//! Bit: 0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 16 17 18 19 20 21 22
//! Key: #  S- T- K- P- W- H- R- A- O- *  -E -U -F -R -P -B -L -G -T -S -D -Z
//! ```

use crate::kernel::sync::SpinLock;
use crate::intent::Intent;

pub mod stroke;
pub mod dictionary;
pub mod engine;
pub mod history;

pub use stroke::{Stroke, KEYS, NUM_KEYS, parse_steno_to_bits, RtfcreBuffer};
pub use dictionary::{StenoDictionary, DictEntry, StrokeSequence, MultiStrokeDictionary, MultiStrokeEntry, concepts};
pub use engine::{StenoEngine, EngineState, EngineStats, StrokeProducer, IntentConsumer};
pub use history::{StrokeHistory, HistoryEntry, HISTORY_SIZE};

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL STENO ENGINE
// ═══════════════════════════════════════════════════════════════════════════════

static STENO_ENGINE: SpinLock<StenoEngine> = SpinLock::new(StenoEngine::new());

/// Initialize the steno subsystem
pub fn init() {
    let mut engine = STENO_ENGINE.lock();
    engine.init();
    crate::kprintln!("[STENO] Steno Input Engine initialized");
    crate::kprintln!("[STENO] Fastest input path: 23 keys, <0.1μs latency.");
}

/// Process a stroke and return intent (if matched)
pub fn process_stroke(stroke: Stroke) -> Option<Intent> {
    let mut engine = STENO_ENGINE.lock();
    engine.process(stroke)
}

/// Process stroke from raw bits (from hardware)
pub fn process_raw(raw: u32) -> Option<Intent> {
    process_stroke(Stroke::from_raw(raw))
}

/// Process stroke from steno notation (e.g., "STPH", "KAT")
pub fn process_steno(steno: &str) -> Option<Intent> {
    let bits = parse_steno_to_bits(steno);
    process_raw(bits)
}

/// Process English text command (reverse lookup)
/// Simulates typing the stroke corresponding to the English word.
pub fn process_english(text: &str) -> Option<Intent> {
    let mut engine = STENO_ENGINE.lock();
    // Look up the stroke for the English word
    if let Some(stroke) = engine.dictionary().lookup_by_name(text) {
        // Process the stroke as if it was typed
        engine.process(stroke)
    } else {
        None
    }
}

/// Get engine statistics
pub fn stats() -> EngineStats {
    let engine = STENO_ENGINE.lock();
    *engine.stats()
}

/// Get current engine state
pub fn state() -> EngineState {
    let engine = STENO_ENGINE.lock();
    engine.state()
}

/// Get stroke history length
pub fn history_len() -> usize {
    let engine = STENO_ENGINE.lock();
    engine.history().len()
}

/// Redo the last undone action
pub fn redo() -> Option<Intent> {
    let mut engine = STENO_ENGINE.lock();
    engine.redo()
}

/// Update engine timestamp (call from timer)
pub fn set_timestamp(ts: u64) {
    let mut engine = STENO_ENGINE.lock();
    engine.set_timestamp(ts);
}
