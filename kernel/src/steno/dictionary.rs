//! Steno Dictionary - Stroke to Intent Mapping
//!
//! Unlike traditional steno dictionaries that map strokes to text,
//! this kernel maps strokes directly to semantic intents.
//!
//! # Architecture
//! ```
//! Single Stroke:  Stroke → Dictionary Lookup → Intent
//! Multi-Stroke:   [Stroke, Stroke, ...] → MultiStrokeDictionary → Intent
//! ```
//!
//! The dictionary supports both single-stroke and multi-stroke briefs.
//! Multi-stroke briefs are essential for real stenography (e.g., "RAOE/PWOOT" for REBOOT).

use super::stroke::{Stroke, parse_steno_to_bits};
use crate::intent::{ConceptID, Intent, IntentData};

// ═══════════════════════════════════════════════════════════════════════════════
// DICTIONARY ENTRY
// ═══════════════════════════════════════════════════════════════════════════════

/// A dictionary entry mapping a stroke to an intent
#[derive(Clone, Copy)]
pub struct DictEntry {
    /// The stroke pattern (23 bits)
    pub stroke: Stroke,
    /// The concept this stroke maps to
    pub concept_id: ConceptID,
    /// Human-readable name (for debugging)
    pub name: &'static str,
}

impl DictEntry {
    /// Create a new dictionary entry
    pub const fn new(stroke_bits: u32, concept_id: ConceptID, name: &'static str) -> Self {
        Self {
            stroke: Stroke::from_raw(stroke_bits),
            concept_id,
            name,
        }
    }
    
    /// Create entry from steno notation (computed at compile time would be ideal)
    pub fn from_steno(steno: &'static str, concept_id: ConceptID, name: &'static str) -> Self {
        Self {
            stroke: Stroke::from_raw(parse_steno_to_bits(steno)),
            concept_id,
            name,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STENO DICTIONARY
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum number of dictionary entries (compile-time limit for no_std)
pub const MAX_ENTRIES: usize = 1024;

/// The steno dictionary (single-stroke entries)
pub struct StenoDictionary {
    /// Single-stroke entries
    entries: [Option<DictEntry>; MAX_ENTRIES],
    /// Number of valid single-stroke entries
    count: usize,
    /// Multi-stroke dictionary
    multi: MultiStrokeDictionary,
}

impl StenoDictionary {
    /// Create an empty dictionary
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_ENTRIES],
            count: 0,
            multi: MultiStrokeDictionary::new(),
        }
    }
}

impl Default for StenoDictionary {
    fn default() -> Self {
        Self::new()
    }
}
    
impl StenoDictionary {
    /// Initialize with system defaults
    pub fn init_defaults(&mut self) {
        // ═══════════════════════════════════════════════════════════════════
        // SINGLE-STROKE COMMANDS
        // ═══════════════════════════════════════════════════════════════════
        
        // System Commands
        self.add_entry(DictEntry::from_steno("PH-FPL", ConceptID(0x0000_0001), "HELP"));
        self.add_entry(DictEntry::from_steno("STAT", ConceptID(0x0000_0002), "STATUS"));
        self.add_entry(DictEntry::from_steno("KHRAOER", ConceptID(0x0000_0004), "CLEAR"));
        self.add_entry(DictEntry::from_steno("*", ConceptID(0x0000_0005), "UNDO"));
        
        // Display Actions
        self.add_entry(DictEntry::from_steno("SHRO", ConceptID(0x0001_0001), "SHOW"));
        self.add_entry(DictEntry::from_steno("HEU", ConceptID(0x0001_0003), "HIDE"));
        self.add_entry(DictEntry::from_steno("STKPW-PL", ConceptID(0x0001_0004), "ZOOM"));
        
        // Memory Actions
        self.add_entry(DictEntry::from_steno("STOR", ConceptID(0x0002_0001), "STORE"));
        self.add_entry(DictEntry::from_steno("TKHRAOET", ConceptID(0x0002_0003), "DELETE"));
        self.add_entry(DictEntry::from_steno("KOPD", ConceptID(0x0002_0004), "COPY"));
        
        // Navigation
        self.add_entry(DictEntry::from_steno("TPHEFBGT", ConceptID(0x0003_0001), "NEXT"));
        self.add_entry(DictEntry::from_steno("PREUF", ConceptID(0x0003_0002), "PREVIOUS"));
        self.add_entry(DictEntry::from_steno("PWABG", ConceptID(0x0003_0003), "BACK"));
        
        // Compute Actions
        self.add_entry(DictEntry::from_steno("SAERP", ConceptID(0x0004_0002), "SEARCH"));
        self.add_entry(DictEntry::from_steno("SORT", ConceptID(0x0004_0004), "SORT"));
        
        // Communication
        self.add_entry(DictEntry::from_steno("SEPBD", ConceptID(0x0005_0001), "SEND"));
        self.add_entry(DictEntry::from_steno("SAEF", ConceptID(0x0005_0002), "SAVE"));
        self.add_entry(DictEntry::from_steno("HRAOD", ConceptID(0x0005_0003), "LOAD"));
        self.add_entry(DictEntry::from_steno("SKO-PBGT", ConceptID(0x0005_0004), "CONNECT"));
        
        // Mode Switching
        self.add_entry(DictEntry::from_steno("PHOEPD", ConceptID(0x0006_0001), "MODE"));
        
        // Confirmation
        self.add_entry(DictEntry::from_steno("KWRE", ConceptID(0x0007_0001), "YES"));
        self.add_entry(DictEntry::from_steno("TPHO", ConceptID(0x0007_0002), "NO"));
        
        // ═══════════════════════════════════════════════════════════════════
        // MULTI-STROKE COMMANDS (2+ strokes)
        // ═══════════════════════════════════════════════════════════════════
        
        // System
        self.add_multi(MultiStrokeEntry::from_steno("RAOE/PWOOT", ConceptID(0x0000_0003), "REBOOT"));
        self.add_multi(MultiStrokeEntry::from_steno("SHUT/TKOUPB", ConceptID(0x0000_0006), "SHUTDOWN"));
        
        // Display
        self.add_multi(MultiStrokeEntry::from_steno("TKEUS/PHRAEU", ConceptID(0x0001_0002), "DISPLAY"));
        
        // Memory
        self.add_multi(MultiStrokeEntry::from_steno("RAOE/KAUL", ConceptID(0x0002_0002), "RECALL"));
        
        // Navigation
        self.add_multi(MultiStrokeEntry::from_steno("TPOR/WARD", ConceptID(0x0003_0004), "FORWARD"));
        self.add_multi(MultiStrokeEntry::from_steno("SKROL/UP", ConceptID(0x0003_0005), "SCROLL_UP"));
        self.add_multi(MultiStrokeEntry::from_steno("SKROL/TKOUPB", ConceptID(0x0003_0006), "SCROLL_DOWN"));
        
        // Compute
        self.add_multi(MultiStrokeEntry::from_steno("KAUL/KWHRAEUT", ConceptID(0x0004_0001), "CALCULATE"));
        self.add_multi(MultiStrokeEntry::from_steno("TPEUL/TER", ConceptID(0x0004_0003), "FILTER"));
        
        // Mode
        self.add_multi(MultiStrokeEntry::from_steno("TKOG/-L", ConceptID(0x0006_0002), "TOGGLE"));
        self.add_multi(MultiStrokeEntry::from_steno("SET/-G", ConceptID(0x0006_0003), "SETTINGS"));
        
        // Confirmation
        self.add_multi(MultiStrokeEntry::from_steno("KAUPB/TPEURPL", ConceptID(0x0007_0003), "CONFIRM"));
        self.add_multi(MultiStrokeEntry::from_steno("KAPB/SEL", ConceptID(0x0007_0004), "CANCEL"));
        
        // File Operations
        self.add_multi(MultiStrokeEntry::from_steno("TPAOEU/-L", ConceptID(0x0008_0001), "FILE"));
        self.add_multi(MultiStrokeEntry::from_steno("O/PEPB", ConceptID(0x0008_0002), "OPEN"));
        self.add_multi(MultiStrokeEntry::from_steno("KHROE/-S", ConceptID(0x0008_0003), "CLOSE"));
        self.add_multi(MultiStrokeEntry::from_steno("TPHU/TPAOEU/-L", ConceptID(0x0008_0004), "NEW_FILE"));
        
        // System Info
        self.add_multi(MultiStrokeEntry::from_steno("PHEPL/REU", ConceptID(0x0009_0001), "MEMORY"));
        self.add_multi(MultiStrokeEntry::from_steno("KP-U/EUPB/TPO", ConceptID(0x0009_0002), "CPU_INFO"));
        self.add_multi(MultiStrokeEntry::from_steno("UP/TAOEUPL", ConceptID(0x0009_0003), "UPTIME"));
    }
    
    /// Add a single-stroke entry to the dictionary
    pub fn add_entry(&mut self, entry: DictEntry) {
        if self.count < MAX_ENTRIES {
            self.entries[self.count] = Some(entry);
            self.count += 1;
        }
    }
    
    /// Add a multi-stroke entry
    pub fn add_multi(&mut self, entry: MultiStrokeEntry) {
        self.multi.add_entry(entry);
    }
    
    /// Look up a single stroke in the dictionary
    pub fn lookup(&self, stroke: Stroke) -> Option<&DictEntry> {
        for i in 0..self.count {
            if let Some(ref entry) = self.entries[i] {
                if entry.stroke == stroke {
                    return Some(entry);
                }
            }
        }
        None
    }
    
    /// Look up a multi-stroke sequence
    pub fn lookup_multi(&self, sequence: &StrokeSequence) -> Option<Intent> {
        self.multi.sequence_to_intent(sequence)
    }
    
    /// Check if a sequence could match a multi-stroke entry
    /// Returns: (has_exact_match, has_prefix_match)
    pub fn check_multi_prefix(&self, sequence: &StrokeSequence) -> (bool, bool) {
        self.multi.check_prefix(sequence)
    }
    
    /// Look up by raw stroke bits
    pub fn lookup_raw(&self, bits: u32) -> Option<&DictEntry> {
        self.lookup(Stroke::from_raw(bits))
    }

    /// Look up a stroke by its English name (case-insensitive)
    /// Searches both single-stroke and multi-stroke dictionaries
    pub fn lookup_by_name(&self, name: &str) -> Option<Stroke> {
        // Check single-stroke first
        for i in 0..self.count {
            if let Some(ref entry) = self.entries[i] {
                if entry.name.eq_ignore_ascii_case(name) {
                    return Some(entry.stroke);
                }
            }
        }
        // Check multi-stroke (return first stroke of sequence)
        for i in 0..self.multi.count {
            if let Some(ref entry) = self.multi.entries[i] {
                if entry.name.eq_ignore_ascii_case(name) {
                    return entry.sequence.get(0);
                }
            }
        }
        None
    }
    
    /// Convert a single stroke to an intent
    pub fn stroke_to_intent(&self, stroke: Stroke) -> Option<Intent> {
        self.lookup(stroke).map(|entry| {
            Intent {
                concept_id: entry.concept_id,
                confidence: 1.0,
                data: IntentData::None,
                name: entry.name,
            }
        })
    }
    
    /// Get the number of single-stroke entries
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Get the number of multi-stroke entries
    pub fn multi_len(&self) -> usize {
        self.multi.len()
    }
    
    /// Check if dictionary is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0 && self.multi.is_empty()
    }
    
    /// Get reference to multi-stroke dictionary
    pub fn multi(&self) -> &MultiStrokeDictionary {
        &self.multi
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTI-STROKE SEQUENCES
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum strokes in a sequence
pub const MAX_STROKE_SEQUENCE: usize = 8;

/// A sequence of strokes (for multi-stroke briefs)
#[derive(Clone, Copy)]
pub struct StrokeSequence {
    strokes: [Stroke; MAX_STROKE_SEQUENCE],
    len: usize,
}

impl StrokeSequence {
    /// Create empty sequence
    pub const fn new() -> Self {
        Self {
            strokes: [Stroke::EMPTY; MAX_STROKE_SEQUENCE],
            len: 0,
        }
    }

    
    /// Create from a slice of strokes
    pub fn from_strokes(strokes: &[Stroke]) -> Self {
        let mut seq = Self::new();
        for &s in strokes.iter().take(MAX_STROKE_SEQUENCE) {
            seq.push(s);
        }
        seq
    }
    
    /// Parse from steno notation with slashes (e.g., "RAOE/PWOOT")
    pub fn from_steno(steno: &str) -> Self {
        let mut seq = Self::new();
        for part in steno.split('/') {
            let bits = parse_steno_to_bits(part);
            if bits != 0 {
                seq.push(Stroke::from_raw(bits));
            }
        }
        seq
    }
    
    /// Add a stroke to the sequence
    pub fn push(&mut self, stroke: Stroke) -> bool {
        if self.len < MAX_STROKE_SEQUENCE {
            self.strokes[self.len] = stroke;
            self.len += 1;
            true
        } else {
            false
        }
    }
    
    /// Clear the sequence
    pub fn clear(&mut self) {
        self.len = 0;
    }
    
    /// Get the current length
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Get stroke at index
    pub fn get(&self, index: usize) -> Option<Stroke> {
        if index < self.len {
            Some(self.strokes[index])
        } else {
            None
        }
    }
    
    /// Get the last stroke (if any)
    pub fn last(&self) -> Option<Stroke> {
        if self.len > 0 {
            Some(self.strokes[self.len - 1])
        } else {
            None
        }
    }
    
    /// Pop the last stroke
    pub fn pop(&mut self) -> Option<Stroke> {
        if self.len > 0 {
            self.len -= 1;
            Some(self.strokes[self.len])
        } else {
            None
        }
    }
    
    /// Check if this sequence matches another
    pub fn matches(&self, other: &StrokeSequence) -> bool {
        if self.len != other.len {
            return false;
        }
        for i in 0..self.len {
            if self.strokes[i] != other.strokes[i] {
                return false;
            }
        }
        true
    }
    
    /// Check if this sequence starts with another (prefix match)
    pub fn starts_with(&self, prefix: &StrokeSequence) -> bool {
        if prefix.len > self.len {
            return false;
        }
        for i in 0..prefix.len {
            if self.strokes[i] != prefix.strokes[i] {
                return false;
            }
        }
        true
    }
    
    /// Get as slice
    pub fn as_slice(&self) -> &[Stroke] {
        &self.strokes[..self.len]
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTI-STROKE DICTIONARY
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum multi-stroke entries
pub const MAX_MULTI_ENTRIES: usize = 256;

/// A multi-stroke dictionary entry
#[derive(Clone, Copy)]
pub struct MultiStrokeEntry {
    /// The stroke sequence (e.g., [RAOE, PWOOT])
    pub sequence: StrokeSequence,
    /// The concept this maps to
    pub concept_id: ConceptID,
    /// Human-readable name
    pub name: &'static str,
}

impl MultiStrokeEntry {
    /// Create from steno notation (e.g., "RAOE/PWOOT")
    pub fn from_steno(steno: &'static str, concept_id: ConceptID, name: &'static str) -> Self {
        Self {
            sequence: StrokeSequence::from_steno(steno),
            concept_id,
            name,
        }
    }
}

/// Multi-stroke dictionary for briefs that require multiple strokes
pub struct MultiStrokeDictionary {
    entries: [Option<MultiStrokeEntry>; MAX_MULTI_ENTRIES],
    count: usize,
}

impl MultiStrokeDictionary {
    /// Create empty dictionary
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_MULTI_ENTRIES],
            count: 0,
        }
    }

    
    /// Add a multi-stroke entry
    pub fn add_entry(&mut self, entry: MultiStrokeEntry) {
        if self.count < MAX_MULTI_ENTRIES && entry.sequence.len() > 1 {
            self.entries[self.count] = Some(entry);
            self.count += 1;
        }
    }
    
    /// Look up an exact sequence match
    pub fn lookup(&self, sequence: &StrokeSequence) -> Option<&MultiStrokeEntry> {
        for i in 0..self.count {
            if let Some(ref entry) = self.entries[i] {
                if entry.sequence.matches(sequence) {
                    return Some(entry);
                }
            }
        }
        None
    }
    
    /// Check if any entry starts with this sequence (potential match)
    /// Returns: (has_exact_match, has_prefix_match)
    pub fn check_prefix(&self, sequence: &StrokeSequence) -> (bool, bool) {
        let mut exact = false;
        let mut prefix = false;
        
        for i in 0..self.count {
            if let Some(ref entry) = self.entries[i] {
                if entry.sequence.matches(sequence) {
                    exact = true;
                }
                if entry.sequence.starts_with(sequence) && entry.sequence.len() > sequence.len() {
                    prefix = true;
                }
            }
        }
        
        (exact, prefix)
    }
    
    /// Convert a matched sequence to an intent
    pub fn sequence_to_intent(&self, sequence: &StrokeSequence) -> Option<Intent> {
        self.lookup(sequence).map(|entry| {
            Intent {
                concept_id: entry.concept_id,
                confidence: 1.0,
                data: IntentData::None,
                name: entry.name,
            }
        })
    }
    
    /// Get number of entries
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONCEPT IDS (Well-Known)
// ═══════════════════════════════════════════════════════════════════════════════

/// Well-known concept IDs for system operations
pub mod concepts {
    use crate::intent::ConceptID;
    
    // System Commands (0x0000_xxxx)
    pub const HELP: ConceptID = ConceptID(0x0000_0001);
    pub const STATUS: ConceptID = ConceptID(0x0000_0002);
    pub const REBOOT: ConceptID = ConceptID(0x0000_0003);
    pub const CLEAR: ConceptID = ConceptID(0x0000_0004);
    pub const UNDO: ConceptID = ConceptID(0x0000_0005);
    pub const SHUTDOWN: ConceptID = ConceptID(0x0000_0006);
    
    // Display Actions (0x0001_xxxx)
    pub const SHOW: ConceptID = ConceptID(0x0001_0001);
    pub const DISPLAY: ConceptID = ConceptID(0x0001_0002);
    pub const HIDE: ConceptID = ConceptID(0x0001_0003);
    pub const ZOOM: ConceptID = ConceptID(0x0001_0004);
    
    // Memory Actions (0x0002_xxxx)
    pub const STORE: ConceptID = ConceptID(0x0002_0001);
    pub const RECALL: ConceptID = ConceptID(0x0002_0002);
    pub const DELETE: ConceptID = ConceptID(0x0002_0003);
    pub const COPY: ConceptID = ConceptID(0x0002_0004);
    
    // Navigation (0x0003_xxxx)
    pub const NEXT: ConceptID = ConceptID(0x0003_0001);
    pub const PREVIOUS: ConceptID = ConceptID(0x0003_0002);
    pub const BACK: ConceptID = ConceptID(0x0003_0003);
    pub const FORWARD: ConceptID = ConceptID(0x0003_0004);
    pub const SCROLL_UP: ConceptID = ConceptID(0x0003_0005);
    pub const SCROLL_DOWN: ConceptID = ConceptID(0x0003_0006);
    
    // Compute (0x0004_xxxx)
    pub const CALCULATE: ConceptID = ConceptID(0x0004_0001);
    pub const SEARCH: ConceptID = ConceptID(0x0004_0002);
    pub const FILTER: ConceptID = ConceptID(0x0004_0003);
    pub const SORT: ConceptID = ConceptID(0x0004_0004);
    
    // Communication (0x0005_xxxx)
    pub const SEND: ConceptID = ConceptID(0x0005_0001);
    pub const SAVE: ConceptID = ConceptID(0x0005_0002);
    pub const LOAD: ConceptID = ConceptID(0x0005_0003);
    pub const CONNECT: ConceptID = ConceptID(0x0005_0004);
    
    // Mode (0x0006_xxxx)
    pub const MODE: ConceptID = ConceptID(0x0006_0001);
    pub const TOGGLE: ConceptID = ConceptID(0x0006_0002);
    pub const SETTINGS: ConceptID = ConceptID(0x0006_0003);
    
    // Confirmation (0x0007_xxxx)
    pub const YES: ConceptID = ConceptID(0x0007_0001);
    pub const NO: ConceptID = ConceptID(0x0007_0002);
    pub const CONFIRM: ConceptID = ConceptID(0x0007_0003);
    pub const CANCEL: ConceptID = ConceptID(0x0007_0004);
    
    // File Operations (0x0008_xxxx)
    pub const FILE: ConceptID = ConceptID(0x0008_0001);
    pub const OPEN: ConceptID = ConceptID(0x0008_0002);
    pub const CLOSE: ConceptID = ConceptID(0x0008_0003);
    pub const NEW_FILE: ConceptID = ConceptID(0x0008_0004);
    
    // System Info (0x0009_xxxx)
    pub const MEMORY: ConceptID = ConceptID(0x0009_0001);
    pub const CPU_INFO: ConceptID = ConceptID(0x0009_0002);
    pub const UPTIME: ConceptID = ConceptID(0x0009_0003);
}

impl Default for StrokeSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MultiStrokeDictionary {
    fn default() -> Self {
        Self::new()
    }
}
