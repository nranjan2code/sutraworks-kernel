//! Steno Dictionary - Stroke to Intent Mapping
//!
//! Unlike traditional steno dictionaries that map strokes to text,
//! this kernel maps strokes directly to semantic intents.
//!
//! # Architecture
//! ```
//! Stroke → Dictionary Lookup → Intent (not text!)
//! ```
//!
//! The dictionary is a compile-time constant structure for O(1) lookup
//! using perfect hashing on the stroke bits.

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

/// The steno dictionary
pub struct StenoDictionary {
    /// Static entries (system commands)
    entries: [Option<DictEntry>; MAX_ENTRIES],
    /// Number of valid entries
    count: usize,
}

impl StenoDictionary {
    /// Create an empty dictionary
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_ENTRIES],
            count: 0,
        }
    }
    
    /// Initialize with system defaults
    pub fn init_defaults(&mut self) {
        // System Commands
        self.add_entry(DictEntry::from_steno("PH-FPL", ConceptID(0x0000_0001), "HELP"));
        self.add_entry(DictEntry::from_steno("STAT", ConceptID(0x0000_0002), "STATUS"));
        self.add_entry(DictEntry::from_steno("RAOE/PWOOT", ConceptID(0x0000_0003), "REBOOT"));
        self.add_entry(DictEntry::from_steno("KHRAOER", ConceptID(0x0000_0004), "CLEAR"));
        self.add_entry(DictEntry::from_steno("*", ConceptID(0x0000_0005), "UNDO"));
        
        // Display Actions
        self.add_entry(DictEntry::from_steno("SHRO", ConceptID(0x0001_0001), "SHOW"));
        self.add_entry(DictEntry::from_steno("TKEUS/PHRAEU", ConceptID(0x0001_0002), "DISPLAY"));
        self.add_entry(DictEntry::from_steno("HEU", ConceptID(0x0001_0003), "HIDE"));
        self.add_entry(DictEntry::from_steno("STKPW-PL", ConceptID(0x0001_0004), "ZOOM"));
        
        // Memory Actions
        self.add_entry(DictEntry::from_steno("STOR", ConceptID(0x0002_0001), "STORE"));
        self.add_entry(DictEntry::from_steno("RAOE/KAUL", ConceptID(0x0002_0002), "RECALL"));
        self.add_entry(DictEntry::from_steno("TKHRAOET", ConceptID(0x0002_0003), "DELETE"));
        self.add_entry(DictEntry::from_steno("KOPD", ConceptID(0x0002_0004), "COPY"));
        
        // Navigation
        self.add_entry(DictEntry::from_steno("TPHEFBGT", ConceptID(0x0003_0001), "NEXT"));
        self.add_entry(DictEntry::from_steno("PREUF", ConceptID(0x0003_0002), "PREVIOUS"));
        self.add_entry(DictEntry::from_steno("PWABG", ConceptID(0x0003_0003), "BACK"));
        self.add_entry(DictEntry::from_steno("TPOR/WARD", ConceptID(0x0003_0004), "FORWARD"));
        
        // Compute Actions
        self.add_entry(DictEntry::from_steno("KAUL/KWHRAEUT", ConceptID(0x0004_0001), "CALCULATE"));
        self.add_entry(DictEntry::from_steno("SAERP", ConceptID(0x0004_0002), "SEARCH"));
        self.add_entry(DictEntry::from_steno("TPEUL/TER", ConceptID(0x0004_0003), "FILTER"));
        self.add_entry(DictEntry::from_steno("SORT", ConceptID(0x0004_0004), "SORT"));
        
        // Communication
        self.add_entry(DictEntry::from_steno("SEPBD", ConceptID(0x0005_0001), "SEND"));
        self.add_entry(DictEntry::from_steno("SAEF", ConceptID(0x0005_0002), "SAVE"));
        self.add_entry(DictEntry::from_steno("HRAOD", ConceptID(0x0005_0003), "LOAD"));
        self.add_entry(DictEntry::from_steno("SKO-PBGT", ConceptID(0x0005_0004), "CONNECT"));
        
        // Mode Switching
        self.add_entry(DictEntry::from_steno("PHOEPD", ConceptID(0x0006_0001), "MODE"));
        self.add_entry(DictEntry::from_steno("TKOG/-L", ConceptID(0x0006_0002), "TOGGLE"));
        self.add_entry(DictEntry::from_steno("SET/-G", ConceptID(0x0006_0003), "SETTINGS"));
        
        // Confirmation
        self.add_entry(DictEntry::from_steno("KWRE", ConceptID(0x0007_0001), "YES"));
        self.add_entry(DictEntry::from_steno("TPHO", ConceptID(0x0007_0002), "NO"));
        self.add_entry(DictEntry::from_steno("KAUPB/TPEURPL", ConceptID(0x0007_0003), "CONFIRM"));
        self.add_entry(DictEntry::from_steno("KAPB/SEL", ConceptID(0x0007_0004), "CANCEL"));
    }
    
    /// Add an entry to the dictionary
    pub fn add_entry(&mut self, entry: DictEntry) {
        if self.count < MAX_ENTRIES {
            self.entries[self.count] = Some(entry);
            self.count += 1;
        }
    }
    
    /// Look up a stroke in the dictionary
    pub fn lookup(&self, stroke: Stroke) -> Option<&DictEntry> {
        // Linear search for now - can optimize with hash table later
        for i in 0..self.count {
            if let Some(ref entry) = self.entries[i] {
                if entry.stroke == stroke {
                    return Some(entry);
                }
            }
        }
        None
    }
    
    /// Look up by raw stroke bits
    pub fn lookup_raw(&self, bits: u32) -> Option<&DictEntry> {
        self.lookup(Stroke::from_raw(bits))
    }
    
    /// Convert a stroke to an intent
    pub fn stroke_to_intent(&self, stroke: Stroke) -> Option<Intent> {
        self.lookup(stroke).map(|entry| {
            Intent {
                concept_id: entry.concept_id,
                confidence: 1.0, // Dictionary lookup is certain
                data: IntentData::None,
                name: entry.name,
            }
        })
    }
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Check if dictionary is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
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
}
