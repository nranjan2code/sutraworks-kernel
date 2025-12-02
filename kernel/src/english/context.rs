//! Conversation Context - Stateful natural language understanding
//!
//! This module maintains conversation state to handle:
//! - Follow-up questions ("show it again")
//! - Pronoun resolution ("what about it?")
//! - User profiling (beginner vs advanced mode)
//! - History tracking

use alloc::vec::Vec;
use crate::intent::{ConceptID, Intent};
use super::parser::EnglishParser;
use super::responses::IntentResult;

// ═══════════════════════════════════════════════════════════════════════════════
// USER MODE
// ═══════════════════════════════════════════════════════════════════════════════

/// User experience mode
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UserMode {
    /// New user - verbose help, detailed responses
    Beginner,
    /// Regular user - normal responses
    Intermediate,
    /// Expert user - concise output, can use steno directly
    Advanced,
}

impl UserMode {
    /// Check if this mode prefers verbose output
    pub fn is_verbose(&self) -> bool {
        matches!(self, UserMode::Beginner)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONVERSATION HISTORY ENTRY
// ═══════════════════════════════════════════════════════════════════════════════

/// A single entry in conversation history
#[derive(Clone)]
struct HistoryEntry {
    _input: alloc::string::String,
    _concept: ConceptID,
    _result: Option<IntentResult>,
    _timestamp: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONVERSATION CONTEXT
// ═══════════════════════════════════════════════════════════════════════════════

const MAX_HISTORY: usize = 10;

/// Maintains conversation state for context-aware parsing
pub struct ConversationContext {
    /// Last intent executed
    last_intent: Option<ConceptID>,
    /// Last result
    last_result: Option<IntentResult>,
    /// User mode
    mode: UserMode,
    /// Conversation history (limited to last N)
    history: Vec<HistoryEntry>,
    /// Parser instance
    parser: EnglishParser,
}

impl ConversationContext {
    /// Create a new conversation context
    pub fn new() -> Self {
        Self {
            last_intent: None,
            last_result: None,
            mode: UserMode::Beginner, // Start in beginner mode
            history: Vec::new(),
            parser: EnglishParser::new(),
        }
    }

    /// Parse with conversation context
    pub fn parse(&mut self, input: &str) -> Option<Intent> {

        // Handle context-aware phrases first
        if let Some(intent) = self.resolve_context(input) {
            return Some(intent);
        }

        // Fall back to standard parser
        self.parser.parse(input)
    }

    /// Update context after intent execution
    pub fn update(&mut self, concept: ConceptID, result: IntentResult) {
        self.last_intent = Some(concept);
        self.last_result = Some(result.clone());

        // Add to history
        if self.history.len() >= MAX_HISTORY {
            self.history.remove(0);
        }

        // Note: We don't store input here to avoid allocation issues
        // in real implementation, would need to handle this differently
    }

    /// Get current user mode
    pub fn mode(&self) -> UserMode {
        self.mode
    }

    /// Set user mode
    pub fn set_mode(&mut self, mode: UserMode) {
        self.mode = mode;
    }

    /// Detect and upgrade user mode based on usage
    pub fn auto_upgrade_mode(&mut self) {
        // If user has executed 20+ commands, upgrade from Beginner
        if self.history.len() >= 20 && self.mode == UserMode::Beginner {
            self.mode = UserMode::Intermediate;
        }

        // If user has executed 100+ commands, upgrade to Advanced
        if self.history.len() >= 100 && self.mode == UserMode::Intermediate {
            self.mode = UserMode::Advanced;
        }
    }

    /// Clear conversation history
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.last_intent = None;
        self.last_result = None;
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // CONTEXT RESOLUTION
    // ═══════════════════════════════════════════════════════════════════════════

    /// Resolve context-aware phrases
    fn resolve_context(&self, input: &str) -> Option<Intent> {
        let normalized = input.trim().to_lowercase();

        // "again" or "repeat" - execute last intent
        if self.contains_any(&normalized, &["again", "repeat", "one more time"]) {
            if let Some(concept) = self.last_intent {
                return Some(Intent {
                    concept_id: concept,
                    confidence: 0.95,
                    data: crate::intent::IntentData::None,
                    name: "REPEAT",
                });
            }
        }

        // "more details" - show detailed version of last result
        if self.contains_any(&normalized, &["more", "details", "detailed", "expand"]) {
            if let Some(concept) = self.last_intent {
                return Some(Intent {
                    concept_id: concept,
                    confidence: 0.9,
                    data: crate::intent::IntentData::None,
                    name: "DETAILED",
                });
            }
        }

        // Pronoun resolution: "it", "that", "this"
        if self.contains_any(&normalized, &["it", "that", "this"]) {
            // "show it", "hide that", etc.
            if normalized.contains("show") {
                return Some(Intent {
                    concept_id: crate::steno::dictionary::concepts::SHOW,
                    confidence: 0.85,
                    data: crate::intent::IntentData::None,
                    name: "SHOW",
                });
            }

            if normalized.contains("hide") {
                return Some(Intent {
                    concept_id: crate::steno::dictionary::concepts::HIDE,
                    confidence: 0.85,
                    data: crate::intent::IntentData::None,
                    name: "HIDE",
                });
            }
        }

        // Follow-up questions
        if self.contains_any(&normalized, &["what about", "how about"]) {
            // These need more sophisticated handling
            // For now, just trigger help
            return Some(Intent {
                concept_id: crate::steno::dictionary::concepts::HELP,
                confidence: 0.7,
                data: crate::intent::IntentData::None,
                name: "HELP",
            });
        }

        // Mode switching commands
        if normalized.contains("beginner mode") || normalized.contains("verbose") {
            // This would be handled by a MODE intent
            // For now, return None
        }

        if normalized.contains("expert mode") || normalized.contains("concise") {
            // Same as above
        }

        None
    }

    /// Helper: Check if text contains any of the keywords
    fn contains_any(&self, text: &str, keywords: &[&str]) -> bool {
        keywords.iter().any(|&kw| text.contains(kw))
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steno::dictionary::concepts;

    #[test]
    fn test_new_context() {
        let ctx = ConversationContext::new();
        assert_eq!(ctx.mode(), UserMode::Beginner);
        assert!(ctx.last_intent.is_none());
    }

    #[test]
    fn test_update_context() {
        let mut ctx = ConversationContext::new();
        let result = IntentResult::success();

        ctx.update(concepts::HELP, result);
        assert_eq!(ctx.last_intent, Some(concepts::HELP));
    }

    #[test]
    fn test_repeat_intent() {
        let mut ctx = ConversationContext::new();

        // Execute a command first
        ctx.update(concepts::STATUS, IntentResult::success());

        // "again" should repeat last intent
        let intent = ctx.parse("do it again");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::STATUS);
    }

    #[test]
    fn test_mode_upgrade() {
        let mut ctx = ConversationContext::new();
        assert_eq!(ctx.mode(), UserMode::Beginner);

        // Simulate 20 commands
        for _ in 0..20 {
            ctx.update(concepts::HELP, IntentResult::success());
        }

        ctx.auto_upgrade_mode();
        assert_eq!(ctx.mode(), UserMode::Intermediate);
    }

    #[test]
    fn test_set_mode() {
        let mut ctx = ConversationContext::new();
        ctx.set_mode(UserMode::Advanced);
        assert_eq!(ctx.mode(), UserMode::Advanced);
        assert!(!ctx.mode().is_verbose());
    }

    #[test]
    fn test_clear_history() {
        let mut ctx = ConversationContext::new();
        ctx.update(concepts::HELP, IntentResult::success());

        assert!(ctx.last_intent.is_some());

        ctx.clear_history();
        assert!(ctx.last_intent.is_none());
    }

    #[test]
    fn test_contains_any() {
        let ctx = ConversationContext::new();
        assert!(ctx.contains_any("show it again", &["again", "repeat"]));
        assert!(!ctx.contains_any("show status", &["again", "repeat"]));
    }

    #[test]
    fn test_parse_with_fallback() {
        let mut ctx = ConversationContext::new();

        // Should fall through to standard parser
        let intent = ctx.parse("help");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::HELP);
    }
}
