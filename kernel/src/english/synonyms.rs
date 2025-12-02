//! Synonym Database - Word equivalence mappings
//!
//! This module handles synonym expansion to increase phrase matching flexibility.

use heapless::FnvIndexMap;

// ═══════════════════════════════════════════════════════════════════════════════
// SYNONYM DATABASE
// ═══════════════════════════════════════════════════════════════════════════════

/// Synonym mapping: (word, canonical form)
pub struct SynonymEntry {
    pub word: &'static str,
    pub canonical: &'static str,
}

impl SynonymEntry {
    pub const fn new(word: &'static str, canonical: &'static str) -> Self {
        Self { word, canonical }
    }
}

/// Complete synonym database
pub static SYNONYMS: &[SynonymEntry] = &[
    // Common contractions
    SynonymEntry::new("what's", "what is"),
    SynonymEntry::new("how's", "how is"),
    SynonymEntry::new("can't", "cannot"),
    SynonymEntry::new("won't", "will not"),
    SynonymEntry::new("don't", "do not"),
    SynonymEntry::new("didn't", "did not"),
    SynonymEntry::new("i'm", "i am"),
    SynonymEntry::new("you're", "you are"),
    SynonymEntry::new("it's", "it is"),

    // Command equivalents
    SynonymEntry::new("quit", "exit"),
    SynonymEntry::new("shutdown", "reboot"),
    SynonymEntry::new("poweroff", "reboot"),
    SynonymEntry::new("halt", "reboot"),

    SynonymEntry::new("info", "status"),
    SynonymEntry::new("information", "status"),
    SynonymEntry::new("details", "status"),

    SynonymEntry::new("display", "show"),
    SynonymEntry::new("reveal", "show"),
    SynonymEntry::new("present", "show"),

    SynonymEntry::new("remove", "hide"),
    SynonymEntry::new("close", "hide"),
    SynonymEntry::new("dismiss", "hide"),

    SynonymEntry::new("store", "save"),
    SynonymEntry::new("keep", "save"),
    SynonymEntry::new("write", "save"),

    SynonymEntry::new("retrieve", "load"),
    SynonymEntry::new("fetch", "load"),
    SynonymEntry::new("get", "load"),

    SynonymEntry::new("find", "search"),
    SynonymEntry::new("locate", "search"),
    SynonymEntry::new("seek", "search"),

    // Informal variants
    SynonymEntry::new("yeah", "yes"),
    SynonymEntry::new("yep", "yes"),
    SynonymEntry::new("yup", "yes"),
    SynonymEntry::new("uh-huh", "yes"),

    SynonymEntry::new("nope", "no"),
    SynonymEntry::new("nah", "no"),
    SynonymEntry::new("uh-uh", "no"),

    SynonymEntry::new("ok", "okay"),
    SynonymEntry::new("k", "okay"),
    SynonymEntry::new("kk", "okay"),

    // Common abbreviations
    SynonymEntry::new("sys", "system"),
    SynonymEntry::new("proc", "process"),
    SynonymEntry::new("mem", "memory"),
    SynonymEntry::new("cfg", "config"),
    SynonymEntry::new("pref", "preferences"),
    SynonymEntry::new("docs", "documentation"),
];

pub const SYNONYM_COUNT: usize = SYNONYMS.len();

// ═══════════════════════════════════════════════════════════════════════════════
// EXPANSION FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Expand synonyms in a phrase
/// Example: "what's the sys info" → "what is the system information"
pub fn expand(phrase: &str) -> alloc::string::String {
    use alloc::string::ToString;
    use alloc::vec::Vec;

    let words: Vec<&str> = phrase.split_whitespace().collect();
    let mut expanded = Vec::with_capacity(words.len());

    for word in words {
        let canonical = lookup(word).unwrap_or(word);
        expanded.push(canonical);
    }

    expanded.join(" ")
}

/// Lookup a single word's canonical form
pub fn lookup(word: &str) -> Option<&'static str> {
    let normalized = word.to_lowercase();

    for entry in SYNONYMS {
        if entry.word == normalized {
            return Some(entry.canonical);
        }
    }

    None
}

/// Check if a word has a synonym
pub fn has_synonym(word: &str) -> bool {
    lookup(word).is_some()
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_exact() {
        assert_eq!(lookup("quit"), Some("exit"));
        assert_eq!(lookup("info"), Some("status"));
        assert_eq!(lookup("yeah"), Some("yes"));
    }

    #[test]
    fn test_lookup_case_insensitive() {
        assert_eq!(lookup("QUIT"), Some("exit"));
        assert_eq!(lookup("Info"), Some("status"));
    }

    #[test]
    fn test_lookup_unknown() {
        assert_eq!(lookup("unknown"), None);
        assert_eq!(lookup("asdf"), None);
    }

    #[test]
    fn test_expand_phrase() {
        assert_eq!(expand("quit now"), "exit now");
        assert_eq!(expand("show sys info"), "show system information");
    }

    #[test]
    fn test_expand_contractions() {
        assert_eq!(expand("what's happening"), "what is happening");
        assert_eq!(expand("how's it going"), "how is it going");
    }

    #[test]
    fn test_has_synonym() {
        assert!(has_synonym("quit"));
        assert!(has_synonym("info"));
        assert!(!has_synonym("unknown"));
    }

    #[test]
    fn test_synonym_count() {
        assert!(SYNONYM_COUNT > 30, "Should have at least 30 synonyms");
    }
}
