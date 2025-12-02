//! English Input/Output Layer
//!
//! This module provides a natural language interface to the steno-native kernel.
//!
//! # Architecture
//!
//! ```
//! English Text → Parser → ConceptID → Intent
//!              ↓
//!         Phrase Map (O(1) lookup)
//!              ↓
//!         Synonym Expansion
//!              ↓
//!         Keyword Extraction
//!
//! Intent Result → Response Generator → Natural Language
//!                      ↓
//!                Template Engine
//!                      ↓
//!                Context-Aware Output
//! ```
//!
//! # Philosophy
//!
//! The kernel remains **steno-native internally**. This layer is a translation
//! bridge that allows users to interact in natural English while the kernel
//! operates on semantic intents.
//!
//! # Performance
//!
//! - Phrase lookup: O(1) via perfect hash
//! - Synonym expansion: O(1) hash lookup
//! - Response generation: Stack-allocated, zero-copy templates
//! - Total overhead: ~10-30μs per command (negligible at <200 WPM)

#![no_std]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::intent::{ConceptID, Intent, IntentData};
use crate::steno::dictionary::concepts;

pub mod parser;
pub mod responses;
pub mod context;
pub mod phrases;
pub mod synonyms;

pub use parser::EnglishParser;
pub use responses::{ResponseGenerator, IntentResult, ResultData};
pub use context::{ConversationContext, UserMode};

// ═══════════════════════════════════════════════════════════════════════════════
// PUBLIC API
// ═══════════════════════════════════════════════════════════════════════════════

/// Parse English text into an intent
pub fn parse(input: &str) -> Option<Intent> {
    EnglishParser::global().parse(input)
}

/// Parse with conversation context
pub fn parse_with_context(input: &str, context: &mut ConversationContext) -> Option<Intent> {
    context.parse(input)
}

/// Generate English response from intent result
pub fn generate_response(intent: &Intent, result: &IntentResult) -> String {
    ResponseGenerator::global().generate(intent, result)
}

/// Initialize the English subsystem
pub fn init() {
    crate::kprintln!("[ENGLISH] Natural language interface initialized");
    crate::kprintln!("[ENGLISH] {} phrases, {} synonyms loaded",
        phrases::PHRASE_COUNT,
        synonyms::SYNONYM_COUNT
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_command() {
        let intent = parse("help");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::HELP);
    }

    #[test]
    fn test_parse_natural_phrase() {
        let intent = parse("show me system status");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::STATUS);
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert!(parse("HELP").is_some());
        assert!(parse("help").is_some());
        assert!(parse("HeLp").is_some());
    }

    #[test]
    fn test_parse_unknown() {
        assert!(parse("asdfasdf").is_none());
    }
}
