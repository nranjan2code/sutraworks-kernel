//! English Parser - Natural language to Intent conversion
//!
//! This module implements a multi-stage parsing pipeline:
//! 1. Normalization (lowercase, trim)
//! 2. Synonym expansion
//! 3. Exact phrase matching
//! 4. Keyword extraction
//! 5. Fallback to steno lookup

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::intent::{ConceptID, Intent, IntentData};
use crate::steno;

use super::phrases;
use super::synonyms;

// ═══════════════════════════════════════════════════════════════════════════════
// ENGLISH PARSER
// ═══════════════════════════════════════════════════════════════════════════════

/// English to Intent parser
pub struct EnglishParser {
    // Future: Add caching, ML models, etc.
}

impl EnglishParser {
    /// Create a new parser
    pub const fn new() -> Self {
        Self {}
    }

    /// Get the global parser instance
    pub fn global() -> &'static Self {
        static PARSER: EnglishParser = EnglishParser::new();
        &PARSER
    }

    /// Parse English text into an intent
    ///
    /// # Multi-Stage Pipeline
    ///
    /// 1. **Normalization**: Lowercase, trim whitespace
    /// 2. **Exact Phrase Match**: Check phrase database
    /// 3. **Synonym Expansion**: Expand synonyms and retry
    /// 4. **Keyword Extraction**: Extract keywords and match
    /// 5. **Steno Fallback**: Try as steno notation
    ///
    /// # Examples
    ///
    /// ```
    /// let intent = parser.parse("help");           // Exact match
    /// let intent = parser.parse("show me status"); // Phrase match
    /// let intent = parser.parse("what's up");      // Synonym expansion
    /// let intent = parser.parse("can you help?");  // Keyword extraction
    /// ```
    pub fn parse(&self, input: &str) -> Option<Intent> {
        // Stage 1: Normalization
        let normalized = Self::normalize(input);

        if normalized.is_empty() {
            return None;
        }

        // Stage 2: Exact phrase match
        if let Some(concept) = phrases::lookup(&normalized) {
            return Some(Intent {
                concept_id: concept,
                confidence: 1.0,
                data: IntentData::None,
                name: Self::concept_to_name(concept),
            });
        }

        // Stage 3: Synonym expansion
        let expanded = synonyms::expand(&normalized);
        if expanded != normalized {
            if let Some(concept) = phrases::lookup(&expanded) {
                return Some(Intent {
                    concept_id: concept,
                    confidence: 0.95, // Slightly lower confidence due to synonym expansion
                    data: IntentData::None,
                    name: Self::concept_to_name(concept),
                });
            }
        }

        // Stage 4: Keyword extraction
        if let Some(intent) = self.extract_keywords(&normalized) {
            return Some(intent);
        }

        // Stage 5: Steno fallback
        steno::process_steno(input)
    }

    /// Normalize input text (lowercase, trim)
    fn normalize(input: &str) -> String {
        input.trim().to_lowercase()
    }

    /// Extract keywords and match to concepts
    ///
    /// This handles questions and natural phrases like:
    /// - "can you show me the status?"
    /// - "i need help with this"
    /// - "what is happening?"
    fn extract_keywords(&self, input: &str) -> Option<Intent> {
        use crate::steno::dictionary::concepts;

        let tokens: Vec<&str> = input.split_whitespace().collect();

        // Helper: Check if tokens contain a keyword
        let contains = |keyword: &str| -> bool {
            tokens.iter().any(|&t| t == keyword)
        };

        let contains_any = |keywords: &[&str]| -> bool {
            keywords.iter().any(|&kw| contains(kw))
        };

        // HELP keywords
        if contains_any(&["help", "assist", "guide", "manual", "how"]) {
            return Some(Intent {
                concept_id: concepts::HELP,
                confidence: 0.9,
                data: IntentData::None,
                name: "HELP",
            });
        }

        // STATUS keywords
        if contains_any(&["status", "info", "information", "how", "what", "stats"]) {
            return Some(Intent {
                concept_id: concepts::STATUS,
                confidence: 0.9,
                data: IntentData::None,
                name: "STATUS",
            });
        }

        // REBOOT keywords
        if contains_any(&["reboot", "restart", "reset", "power"]) {
            return Some(Intent {
                concept_id: concepts::REBOOT,
                confidence: 0.9,
                data: IntentData::None,
                name: "REBOOT",
            });
        }

        // CLEAR keywords
        if contains_any(&["clear", "clean", "wipe", "erase"]) {
            return Some(Intent {
                concept_id: concepts::CLEAR,
                confidence: 0.9,
                data: IntentData::None,
                name: "CLEAR",
            });
        }

        // SHOW keywords
        if contains_any(&["show", "display", "view", "see"]) {
            return Some(Intent {
                concept_id: concepts::SHOW,
                confidence: 0.85,
                data: IntentData::None,
                name: "SHOW",
            });
        }

        // HIDE keywords
        if contains_any(&["hide", "close", "dismiss", "remove"]) {
            return Some(Intent {
                concept_id: concepts::HIDE,
                confidence: 0.85,
                data: IntentData::None,
                name: "HIDE",
            });
        }

        // SAVE keywords
        if contains_any(&["save", "store", "write", "keep"]) {
            return Some(Intent {
                concept_id: concepts::SAVE,
                confidence: 0.85,
                data: IntentData::None,
                name: "SAVE",
            });
        }

        // SEARCH keywords
        if contains_any(&["search", "find", "look", "locate"]) {
            return Some(Intent {
                concept_id: concepts::SEARCH,
                confidence: 0.85,
                data: IntentData::None,
                name: "SEARCH",
            });
        }

        // YES/NO keywords
        if contains_any(&["yes", "yeah", "yep", "ok", "okay", "sure"]) {
            return Some(Intent {
                concept_id: concepts::YES,
                confidence: 0.9,
                data: IntentData::None,
                name: "YES",
            });
        }

        if contains_any(&["no", "nope", "nah", "cancel", "abort"]) {
            return Some(Intent {
                concept_id: concepts::NO,
                confidence: 0.9,
                data: IntentData::None,
                name: "NO",
            });
        }

        None
    }

    /// Convert ConceptID to human-readable name
    fn concept_to_name(concept: ConceptID) -> &'static str {
        use crate::steno::dictionary::concepts;

        if concept == concepts::HELP { "HELP" }
        else if concept == concepts::STATUS { "STATUS" }
        else if concept == concepts::REBOOT { "REBOOT" }
        else if concept == concepts::CLEAR { "CLEAR" }
        else if concept == concepts::UNDO { "UNDO" }
        else if concept == concepts::SHOW { "SHOW" }
        else if concept == concepts::HIDE { "HIDE" }
        else if concept == concepts::SAVE { "SAVE" }
        else if concept == concepts::LOAD { "LOAD" }
        else if concept == concepts::SEARCH { "SEARCH" }
        else if concept == concepts::YES { "YES" }
        else if concept == concepts::NO { "NO" }
        else if concept == concepts::CONFIRM { "CONFIRM" }
        else if concept == concepts::CANCEL { "CANCEL" }
        else if concept == concepts::NEXT { "NEXT" }
        else if concept == concepts::PREVIOUS { "PREVIOUS" }
        else { "UNKNOWN" }
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
    fn test_parse_exact_phrase() {
        let parser = EnglishParser::new();
        let intent = parser.parse("help");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::HELP);
    }

    #[test]
    fn test_parse_natural_phrase() {
        let parser = EnglishParser::new();
        let intent = parser.parse("show me system status");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::STATUS);
    }

    #[test]
    fn test_parse_case_insensitive() {
        let parser = EnglishParser::new();
        assert!(parser.parse("HELP").is_some());
        assert!(parser.parse("help").is_some());
        assert!(parser.parse("HeLp").is_some());
    }

    #[test]
    fn test_parse_with_synonyms() {
        let parser = EnglishParser::new();
        let intent = parser.parse("show sys info");
        assert!(intent.is_some());
        // After synonym expansion: "show system information"
    }

    #[test]
    fn test_parse_keyword_extraction() {
        let parser = EnglishParser::new();

        let intent = parser.parse("can you help me?");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::HELP);

        let intent = parser.parse("what is happening?");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::STATUS);
    }

    #[test]
    fn test_parse_questions() {
        let parser = EnglishParser::new();

        assert_eq!(
            parser.parse("what can you do?").unwrap().concept_id,
            concepts::HELP
        );

        assert_eq!(
            parser.parse("how are you?").unwrap().concept_id,
            concepts::STATUS
        );
    }

    #[test]
    fn test_parse_informal() {
        let parser = EnglishParser::new();

        assert_eq!(
            parser.parse("yeah").unwrap().concept_id,
            concepts::YES
        );

        assert_eq!(
            parser.parse("nope").unwrap().concept_id,
            concepts::NO
        );
    }

    #[test]
    fn test_parse_empty() {
        let parser = EnglishParser::new();
        assert!(parser.parse("").is_none());
        assert!(parser.parse("   ").is_none());
    }

    #[test]
    fn test_parse_unknown() {
        let parser = EnglishParser::new();
        assert!(parser.parse("asdfasdfasdf").is_none());
    }

    #[test]
    fn test_confidence_levels() {
        let parser = EnglishParser::new();

        // Exact phrase should have confidence 1.0
        let intent = parser.parse("help").unwrap();
        assert_eq!(intent.confidence, 1.0);

        // Keyword extraction should have confidence ~0.9
        let intent = parser.parse("can you help?").unwrap();
        assert!(intent.confidence >= 0.85 && intent.confidence <= 1.0);
    }

    #[test]
    fn test_normalize() {
        assert_eq!(EnglishParser::normalize("  HELP  "), "help");
        assert_eq!(EnglishParser::normalize("Show Me"), "show me");
    }
}
