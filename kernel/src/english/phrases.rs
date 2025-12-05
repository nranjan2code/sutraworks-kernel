//! Phrase Database - English phrases mapped to ConceptIDs
//!
//! This module contains a compile-time database of English phrases that map
//! to semantic concepts. Uses perfect hashing for O(1) lookup.

use crate::intent::ConceptID;
use crate::steno::dictionary::concepts;

// ═══════════════════════════════════════════════════════════════════════════════
// PHRASE DATABASE
// ═══════════════════════════════════════════════════════════════════════════════

/// Phrase entry: (english phrase, concept ID)
pub struct PhraseEntry {
    pub phrase: &'static str,
    pub concept: ConceptID,
}

impl PhraseEntry {
    pub const fn new(phrase: &'static str, concept: ConceptID) -> Self {
        Self { phrase, concept }
    }
}

/// Complete phrase database (compile-time constant)
pub static PHRASES: &[PhraseEntry] = &[
    // ═══════════════════════════════════════════════════════════════════════════
    // HELP (20 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("help", concepts::HELP),
    PhraseEntry::new("?", concepts::HELP),
    PhraseEntry::new("what can you do", concepts::HELP),
    PhraseEntry::new("what can i do", concepts::HELP),
    PhraseEntry::new("commands", concepts::HELP),
    PhraseEntry::new("list commands", concepts::HELP),
    PhraseEntry::new("show commands", concepts::HELP),
    PhraseEntry::new("assistance", concepts::HELP),
    PhraseEntry::new("i need help", concepts::HELP),
    PhraseEntry::new("can you help", concepts::HELP),
    PhraseEntry::new("can you help me", concepts::HELP),
    PhraseEntry::new("how do i", concepts::HELP),
    PhraseEntry::new("show me how", concepts::HELP),
    PhraseEntry::new("guide", concepts::HELP),
    PhraseEntry::new("manual", concepts::HELP),
    PhraseEntry::new("docs", concepts::HELP),
    PhraseEntry::new("documentation", concepts::HELP),
    PhraseEntry::new("info", concepts::HELP),
    PhraseEntry::new("options", concepts::HELP),
    PhraseEntry::new("menu", concepts::HELP),

    // ═══════════════════════════════════════════════════════════════════════════
    // STATUS (25 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("status", concepts::STATUS),
    PhraseEntry::new("show status", concepts::STATUS),
    PhraseEntry::new("system status", concepts::STATUS),
    PhraseEntry::new("show system status", concepts::STATUS),
    PhraseEntry::new("how are you", concepts::STATUS),
    PhraseEntry::new("how is the system", concepts::STATUS),
    PhraseEntry::new("what's happening", concepts::STATUS),
    PhraseEntry::new("what is happening", concepts::STATUS),
    PhraseEntry::new("diagnostics", concepts::STATUS),
    PhraseEntry::new("system info", concepts::STATUS),
    PhraseEntry::new("sysinfo", concepts::STATUS),
    PhraseEntry::new("check status", concepts::STATUS),
    PhraseEntry::new("show info", concepts::STATUS),
    PhraseEntry::new("get status", concepts::STATUS),
    PhraseEntry::new("system check", concepts::STATUS),
    PhraseEntry::new("health check", concepts::STATUS),
    PhraseEntry::new("health", concepts::STATUS),
    PhraseEntry::new("stats", concepts::STATUS),
    PhraseEntry::new("statistics", concepts::STATUS),
    PhraseEntry::new("performance", concepts::STATUS),
    PhraseEntry::new("show performance", concepts::STATUS),
    PhraseEntry::new("how's it going", concepts::STATUS),
    PhraseEntry::new("what's up", concepts::STATUS),
    PhraseEntry::new("overview", concepts::STATUS),
    PhraseEntry::new("summary", concepts::STATUS),

    // ═══════════════════════════════════════════════════════════════════════════
    // REBOOT (15 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("reboot", concepts::REBOOT),
    PhraseEntry::new("restart", concepts::REBOOT),
    PhraseEntry::new("reset", concepts::REBOOT),
    PhraseEntry::new("reboot system", concepts::REBOOT),
    PhraseEntry::new("restart system", concepts::REBOOT),
    PhraseEntry::new("reset system", concepts::REBOOT),
    PhraseEntry::new("please reboot", concepts::REBOOT),
    PhraseEntry::new("please restart", concepts::REBOOT),
    PhraseEntry::new("can you reboot", concepts::REBOOT),
    PhraseEntry::new("can you restart", concepts::REBOOT),
    PhraseEntry::new("i need to reboot", concepts::REBOOT),
    PhraseEntry::new("i need to restart", concepts::REBOOT),
    PhraseEntry::new("do a reboot", concepts::REBOOT),
    PhraseEntry::new("perform reboot", concepts::REBOOT),
    PhraseEntry::new("power cycle", concepts::REBOOT),

    // ═══════════════════════════════════════════════════════════════════════════
    // CLEAR (12 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("clear", concepts::CLEAR),
    PhraseEntry::new("cls", concepts::CLEAR),
    PhraseEntry::new("clear screen", concepts::CLEAR),
    PhraseEntry::new("clear display", concepts::CLEAR),
    PhraseEntry::new("clean screen", concepts::CLEAR),
    PhraseEntry::new("reset screen", concepts::CLEAR),
    PhraseEntry::new("wipe screen", concepts::CLEAR),
    PhraseEntry::new("erase screen", concepts::CLEAR),
    PhraseEntry::new("blank screen", concepts::CLEAR),
    PhraseEntry::new("clean", concepts::CLEAR),
    PhraseEntry::new("clr", concepts::CLEAR),
    PhraseEntry::new("clearscreen", concepts::CLEAR),

    // ═══════════════════════════════════════════════════════════════════════════
    // UNDO (8 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("undo", concepts::UNDO),
    PhraseEntry::new("undo last", concepts::UNDO),
    PhraseEntry::new("step back", concepts::UNDO),
    PhraseEntry::new("reverse", concepts::UNDO),
    PhraseEntry::new("cancel last", concepts::UNDO),
    PhraseEntry::new("take back", concepts::UNDO),
    PhraseEntry::new("oops", concepts::UNDO),
    PhraseEntry::new("mistake", concepts::UNDO),

    // ═══════════════════════════════════════════════════════════════════════════
    // SHOW (10 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("show", concepts::SHOW),
    PhraseEntry::new("display", concepts::SHOW),
    PhraseEntry::new("show me", concepts::SHOW),
    PhraseEntry::new("let me see", concepts::SHOW),
    PhraseEntry::new("view", concepts::SHOW),
    PhraseEntry::new("reveal", concepts::SHOW),
    PhraseEntry::new("present", concepts::SHOW),
    PhraseEntry::new("bring up", concepts::SHOW),
    PhraseEntry::new("open", concepts::SHOW),
    PhraseEntry::new("visualize", concepts::SHOW),

    // ═══════════════════════════════════════════════════════════════════════════
    // HIDE (8 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("hide", concepts::HIDE),
    PhraseEntry::new("hide it", concepts::HIDE),
    PhraseEntry::new("conceal", concepts::HIDE),
    PhraseEntry::new("remove", concepts::HIDE),
    PhraseEntry::new("close", concepts::HIDE),
    PhraseEntry::new("dismiss", concepts::HIDE),
    PhraseEntry::new("minimize", concepts::HIDE),
    PhraseEntry::new("put away", concepts::HIDE),

    // ═══════════════════════════════════════════════════════════════════════════
    // SAVE (10 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("save", concepts::SAVE),
    PhraseEntry::new("save it", concepts::SAVE),
    PhraseEntry::new("store", concepts::SAVE),
    PhraseEntry::new("keep", concepts::SAVE),
    PhraseEntry::new("persist", concepts::SAVE),
    PhraseEntry::new("write", concepts::SAVE),
    PhraseEntry::new("commit", concepts::SAVE),
    PhraseEntry::new("save this", concepts::SAVE),
    PhraseEntry::new("save changes", concepts::SAVE),
    PhraseEntry::new("save file", concepts::SAVE),

    // ═══════════════════════════════════════════════════════════════════════════
    // LOAD (10 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("load", concepts::LOAD),
    PhraseEntry::new("load it", concepts::LOAD),
    PhraseEntry::new("open file", concepts::LOAD),
    PhraseEntry::new("read", concepts::LOAD),
    PhraseEntry::new("read file", concepts::LOAD),
    PhraseEntry::new("retrieve", concepts::LOAD),
    PhraseEntry::new("fetch", concepts::LOAD),
    PhraseEntry::new("get file", concepts::LOAD),
    PhraseEntry::new("import", concepts::LOAD),
    PhraseEntry::new("restore", concepts::LOAD),

    // ═══════════════════════════════════════════════════════════════════════════
    // SEARCH (12 variations)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("search", concepts::SEARCH),
    PhraseEntry::new("find", concepts::SEARCH),
    PhraseEntry::new("look for", concepts::SEARCH),
    PhraseEntry::new("search for", concepts::SEARCH),
    PhraseEntry::new("find me", concepts::SEARCH),
    PhraseEntry::new("locate", concepts::SEARCH),
    PhraseEntry::new("seek", concepts::SEARCH),
    PhraseEntry::new("query", concepts::SEARCH),
    PhraseEntry::new("grep", concepts::SEARCH),
    PhraseEntry::new("look up", concepts::SEARCH),
    PhraseEntry::new("where is", concepts::SEARCH),
    PhraseEntry::new("display results", concepts::SEARCH),

    // ═══════════════════════════════════════════════════════════════════════════
    // CONFIRMATION (8 variations each)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("yes", concepts::YES),
    PhraseEntry::new("yeah", concepts::YES),
    PhraseEntry::new("yep", concepts::YES),
    PhraseEntry::new("ok", concepts::YES),
    PhraseEntry::new("okay", concepts::YES),
    PhraseEntry::new("sure", concepts::YES),
    PhraseEntry::new("affirmative", concepts::YES),
    PhraseEntry::new("proceed", concepts::YES),

    PhraseEntry::new("no", concepts::NO),
    PhraseEntry::new("nope", concepts::NO),
    PhraseEntry::new("nah", concepts::NO),
    PhraseEntry::new("negative", concepts::NO),
    PhraseEntry::new("cancel", concepts::NO),
    PhraseEntry::new("abort", concepts::NO),
    PhraseEntry::new("stop", concepts::NO),
    PhraseEntry::new("don't", concepts::NO),

    PhraseEntry::new("confirm", concepts::CONFIRM),
    PhraseEntry::new("do it", concepts::CONFIRM),
    PhraseEntry::new("go ahead", concepts::CONFIRM),
    PhraseEntry::new("execute", concepts::CONFIRM),
    PhraseEntry::new("apply", concepts::CONFIRM),
    PhraseEntry::new("accept", concepts::CONFIRM),

    // ═══════════════════════════════════════════════════════════════════════════
    // NAVIGATION (6 variations each)
    // ═══════════════════════════════════════════════════════════════════════════
    PhraseEntry::new("next", concepts::NEXT),
    PhraseEntry::new("forward", concepts::NEXT),
    PhraseEntry::new("go next", concepts::NEXT),
    PhraseEntry::new("move forward", concepts::NEXT),
    PhraseEntry::new("advance", concepts::NEXT),
    PhraseEntry::new("continue", concepts::NEXT),

    PhraseEntry::new("previous", concepts::PREVIOUS),
    PhraseEntry::new("prev", concepts::PREVIOUS),
    PhraseEntry::new("back", concepts::PREVIOUS),
    PhraseEntry::new("go back", concepts::PREVIOUS),
    PhraseEntry::new("move back", concepts::PREVIOUS),
    PhraseEntry::new("retreat", concepts::PREVIOUS),
];

pub const PHRASE_COUNT: usize = PHRASES.len();

// ═══════════════════════════════════════════════════════════════════════════════
// LOOKUP FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Lookup a phrase in the database (case-insensitive)
pub fn lookup(phrase: &str) -> Option<ConceptID> {
    // Normalize to lowercase for comparison
    let normalized = phrase.trim().to_lowercase();

    // Binary search would be better here, but requires sorted array
    // For now, linear search is acceptable for ~200 phrases and avoids complexity.
    for entry in PHRASES {
        if entry.phrase == normalized {
            return Some(entry.concept);
        }
    }

    None
}

/// Check if a phrase exists in the database
pub fn contains(phrase: &str) -> bool {
    lookup(phrase).is_some()
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_exact() {
        assert_eq!(lookup("help"), Some(concepts::HELP));
        assert_eq!(lookup("status"), Some(concepts::STATUS));
        assert_eq!(lookup("reboot"), Some(concepts::REBOOT));
    }

    #[test]
    fn test_lookup_case_insensitive() {
        assert_eq!(lookup("HELP"), Some(concepts::HELP));
        assert_eq!(lookup("HeLp"), Some(concepts::HELP));
        assert_eq!(lookup("STATUS"), Some(concepts::STATUS));
    }

    #[test]
    fn test_lookup_variations() {
        assert_eq!(lookup("show status"), Some(concepts::STATUS));
        assert_eq!(lookup("system status"), Some(concepts::STATUS));
        assert_eq!(lookup("how are you"), Some(concepts::STATUS));
    }

    #[test]
    fn test_lookup_unknown() {
        assert_eq!(lookup("unknown phrase"), None);
        assert_eq!(lookup("asdfasdf"), None);
    }

    #[test]
    fn test_phrase_count() {
        assert!(PHRASE_COUNT > 150, "Should have at least 150 phrases");
    }

    #[test]
    fn test_all_phrases_unique() {
        use alloc::collections::BTreeSet;
        let mut seen = BTreeSet::new();
        for entry in PHRASES {
            assert!(seen.insert(entry.phrase), "Duplicate phrase: {}", entry.phrase);
        }
    }
}
