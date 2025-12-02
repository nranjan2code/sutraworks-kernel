//! Dictionary - Stroke to Intent mapping (Test-friendly implementation)

use crate::stroke::{Stroke, parse_steno_to_bits};
use crate::concept::ConceptID;

/// A dictionary entry mapping a stroke to an intent
#[derive(Clone, Debug)]
pub struct DictEntry {
    pub stroke: Stroke,
    pub concept_id: ConceptID,
    pub name: String,
}

impl DictEntry {
    /// Create a new dictionary entry from raw bits
    pub fn new(stroke_bits: u32, concept_id: ConceptID, name: &str) -> Self {
        Self {
            stroke: Stroke::from_raw(stroke_bits),
            concept_id,
            name: name.to_string(),
        }
    }
    
    /// Create entry from steno notation
    pub fn from_steno(steno: &str, concept_id: ConceptID, name: &str) -> Self {
        Self {
            stroke: Stroke::from_raw(parse_steno_to_bits(steno)),
            concept_id,
            name: name.to_string(),
        }
    }
}

/// Maximum strokes in a sequence
pub const MAX_STROKE_SEQUENCE: usize = 8;

/// A sequence of strokes (for multi-stroke briefs)
#[derive(Clone, Debug)]
pub struct StrokeSequence {
    strokes: Vec<Stroke>,
}

impl StrokeSequence {
    /// Create empty sequence
    pub fn new() -> Self {
        Self { strokes: Vec::new() }
    }
    
    /// Add a stroke to the sequence
    pub fn push(&mut self, stroke: Stroke) -> bool {
        if self.strokes.len() < MAX_STROKE_SEQUENCE {
            self.strokes.push(stroke);
            true
        } else {
            false
        }
    }
    
    /// Clear the sequence
    pub fn clear(&mut self) {
        self.strokes.clear();
    }
    
    /// Get the current length
    pub fn len(&self) -> usize {
        self.strokes.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }
    
    /// Get the last stroke
    pub fn last(&self) -> Option<Stroke> {
        self.strokes.last().copied()
    }
    
    /// Pop the last stroke
    pub fn pop(&mut self) -> Option<Stroke> {
        self.strokes.pop()
    }
    
    /// Get all strokes
    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }
}

impl Default for StrokeSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// The steno dictionary
pub struct StenoDictionary {
    entries: Vec<DictEntry>,
}

impl StenoDictionary {
    /// Create an empty dictionary
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }
    
    /// Initialize with system defaults
    pub fn with_defaults() -> Self {
        let mut dict = Self::new();
        dict.init_defaults();
        dict
    }
    
    /// Initialize with system defaults
    pub fn init_defaults(&mut self) {
        use crate::concept::concepts;
        
        // System Commands
        self.add_entry(DictEntry::from_steno("PH-FPL", concepts::HELP, "HELP"));
        self.add_entry(DictEntry::from_steno("STAT", concepts::STATUS, "STATUS"));
        self.add_entry(DictEntry::from_steno("KHRAOER", concepts::CLEAR, "CLEAR"));
        self.add_entry(DictEntry::from_steno("*", concepts::UNDO, "UNDO"));
        
        // Display Actions
        self.add_entry(DictEntry::from_steno("SHRO", concepts::SHOW, "SHOW"));
        self.add_entry(DictEntry::from_steno("HEU", concepts::HIDE, "HIDE"));
        self.add_entry(DictEntry::from_steno("STKPW-PL", concepts::ZOOM, "ZOOM"));
        
        // Memory Actions
        self.add_entry(DictEntry::from_steno("STOR", concepts::STORE, "STORE"));
        self.add_entry(DictEntry::from_steno("TKHRAOET", concepts::DELETE, "DELETE"));
        self.add_entry(DictEntry::from_steno("KOPD", concepts::COPY, "COPY"));
        
        // Navigation
        self.add_entry(DictEntry::from_steno("TPHEFBGT", concepts::NEXT, "NEXT"));
        self.add_entry(DictEntry::from_steno("PREUF", concepts::PREVIOUS, "PREVIOUS"));
        self.add_entry(DictEntry::from_steno("PWABG", concepts::BACK, "BACK"));
        
        // Compute Actions
        self.add_entry(DictEntry::from_steno("SAERP", concepts::SEARCH, "SEARCH"));
        self.add_entry(DictEntry::from_steno("SORT", concepts::SORT, "SORT"));
        
        // Communication
        self.add_entry(DictEntry::from_steno("SEPBD", concepts::SEND, "SEND"));
        self.add_entry(DictEntry::from_steno("SAEF", concepts::SAVE, "SAVE"));
        self.add_entry(DictEntry::from_steno("HRAOD", concepts::LOAD, "LOAD"));
        
        // Mode Switching
        self.add_entry(DictEntry::from_steno("PHOEPD", concepts::MODE, "MODE"));
        
        // Confirmation
        self.add_entry(DictEntry::from_steno("KWRE", concepts::YES, "YES"));
        self.add_entry(DictEntry::from_steno("TPHO", concepts::NO, "NO"));
    }
    
    /// Add an entry to the dictionary
    pub fn add_entry(&mut self, entry: DictEntry) {
        self.entries.push(entry);
    }
    
    /// Look up a stroke in the dictionary
    pub fn lookup(&self, stroke: Stroke) -> Option<&DictEntry> {
        self.entries.iter().find(|e| e.stroke == stroke)
    }
    
    /// Look up by raw stroke bits
    pub fn lookup_raw(&self, bits: u32) -> Option<&DictEntry> {
        self.lookup(Stroke::from_raw(bits))
    }
    
    /// Look up by steno notation
    pub fn lookup_steno(&self, steno: &str) -> Option<&DictEntry> {
        let bits = parse_steno_to_bits(steno);
        self.lookup(Stroke::from_raw(bits))
    }
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    
    /// Check if dictionary is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    
    /// Get all entries
    pub fn entries(&self) -> &[DictEntry] {
        &self.entries
    }
}

impl Default for StenoDictionary {
    fn default() -> Self {
        Self::new()
    }
}
