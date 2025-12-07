//! English Parser - Natural language to Intent conversion
//!
//! This module implements a multi-stage parsing pipeline:
//! 1. Normalization (lowercase, trim)
//! 2. Synonym expansion
//! 3. Exact phrase matching
//! 4. Keyword extraction
//! 5. Fallback to steno lookup

use alloc::string::String;
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
        if let Some(concept) = phrases::lookup_normalized(&normalized) {
            return Some(Intent {
                concept_id: concept,
                confidence: 1.0,
                data: IntentData::None,
                name: Self::concept_to_name(concept),
                ..Intent::new(concept)
            });
        }

        // Stage 3: Synonym expansion
        let expanded = synonyms::expand(&normalized);
        if expanded != normalized {
            if let Some(concept) = phrases::lookup_normalized(&expanded) {
                return Some(Intent {
                    concept_id: concept,
                    confidence: 0.95, // Slightly lower confidence due to synonym expansion
                    data: IntentData::None,
                    name: Self::concept_to_name(concept),
                    ..Intent::new(concept)
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
        input.trim().trim_matches(|c| c == '?' || c == '!' || c == '.').to_lowercase()
    }

    /// Extract keywords and match to concepts
    ///
    /// Optimized single-pass implementation:
    /// Iterates through words once and finds the highest priority keyword match.
    fn extract_keywords(&self, input: &str) -> Option<Intent> {
        use crate::steno::dictionary::concepts;

        let mut best_match: Option<(u8, ConceptID, &str, f32)> = None; // (Priority, Concept, Name, Confidence)

        for token in input.split_whitespace() {
            // Clean punctuation (already mostly handled by normalize but good to be safe)
            let word = token.trim_matches(|c| c == '?' || c == '!' || c == '.' || c == ',');
            
            // Match against all keywords in one go
            // Priority 0 is highest.
            let match_result = match word {
                // Priority 0: HELP
                "help" | "assist" | "guide" | "manual" | "how" => Some((0, concepts::HELP, "HELP", 0.9)),
                
                // Priority 1: STATUS
                "status" | "info" | "information" | "what" | "stats" => Some((1, concepts::STATUS, "STATUS", 0.9)),
                
                // Priority 2: REBOOT
                "reboot" | "restart" | "reset" | "power" => Some((2, concepts::REBOOT, "REBOOT", 0.9)),
                
                // Priority 3: CLEAR
                "clear" | "clean" | "wipe" | "erase" => Some((3, concepts::CLEAR, "CLEAR", 0.9)),
                
                // Priority 4: SHOW
                "show" | "display" | "view" | "see" => Some((4, concepts::SHOW, "SHOW", 0.85)),
                
                // Priority 5: HIDE
                "hide" | "close" | "dismiss" | "remove" => Some((5, concepts::HIDE, "HIDE", 0.85)),
                
                // Priority 6: SAVE
                "save" | "store" | "write" | "keep" => Some((6, concepts::SAVE, "SAVE", 0.85)),
                
                // Priority 7: SEARCH
                "search" | "find" | "look" | "locate" => Some((7, concepts::SEARCH, "SEARCH", 0.85)),
                
                // Priority 8: YES
                "yes" | "yeah" | "yep" | "ok" | "okay" | "sure" => Some((8, concepts::YES, "YES", 0.9)),
                
                // Priority 9: NO
                "no" | "nope" | "nah" | "cancel" | "abort" => Some((9, concepts::NO, "NO", 0.9)),
                
                // Priority 10: Counter Demo
                "increment" | "up" | "add" | "plus" => Some((10, concepts::INCREMENT, "INCREMENT", 0.9)),
                "decrement" | "down" | "sub" | "minus" => Some((10, concepts::DECREMENT, "DECREMENT", 0.9)),
                "count" | "value" => Some((10, concepts::GET_COUNT, "GET_COUNT", 0.9)),

                // Priority 11: File Operations
                "ls" | "list" | "dir" => Some((11, concepts::LIST_FILES, "LIST_FILES", 0.9)),
                "cat" | "read" | "open" => Some((11, concepts::READ_FILE, "READ_FILE", 0.9)),
                
                _ => None
            };

            if let Some((prio, concept, name, conf)) = match_result {
                // If we found a higher priority match (lower valid prio number), update
                // Or if it's our first match
                if best_match.map_or(true, |(best_p, _, _, _)| prio < best_p) {
                    best_match = Some((prio, concept, name, conf));
                    if prio == 0 { break; } // Optimization: Found highest priority, stop scanning
                }
            }
        }

        best_match.map(|(_, concept_id, name, confidence)| {
            Intent {
                concept_id,
                confidence,
                data: IntentData::None,
                name,
                ..Intent::new(concept_id)
            }
        })
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
        else if concept == concepts::INCREMENT { "INCREMENT" }
        else if concept == concepts::DECREMENT { "DECREMENT" }
        else if concept == concepts::GET_COUNT { "GET_COUNT" }
        else if concept == concepts::LIST_FILES { "LIST_FILES" }
        else if concept == concepts::READ_FILE { "READ_FILE" }
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

impl Default for EnglishParser {
    fn default() -> Self {
        Self::new()
    }
}
