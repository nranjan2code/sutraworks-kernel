//! Concept - Semantic identifiers (Test-friendly implementation)

/// A 64-bit semantic concept identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConceptID(pub u64);

impl ConceptID {
    /// Create from raw value
    pub const fn new(id: u64) -> Self {
        ConceptID(id)
    }
    
    /// Get raw value
    pub const fn raw(&self) -> u64 {
        self.0
    }
    
    /// Unknown/unrecognized concept
    pub const UNKNOWN: ConceptID = ConceptID(0xFFFF_FFFF_FFFF_FFFF);
    
    /// Get category (upper 16 bits of lower 32)
    pub const fn category(&self) -> u16 {
        ((self.0 >> 16) & 0xFFFF) as u16
    }
    
    /// Get subcategory (lower 16 bits of lower 32)
    pub const fn subcategory(&self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }
}

impl std::fmt::Display for ConceptID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Concept(0x{:016X})", self.0)
    }
}

/// Intent data payload
#[derive(Clone, Debug, PartialEq)]
pub enum IntentData {
    None,
    Number(i64),
    Raw(u64),
    Text(String),
}

/// A semantic intent - the result of processing a stroke
#[derive(Clone, Debug)]
pub struct Intent {
    pub concept_id: ConceptID,
    pub confidence: f32,
    pub data: IntentData,
}

impl Intent {
    /// Create a new intent
    pub fn new(concept_id: ConceptID) -> Self {
        Self {
            concept_id,
            confidence: 1.0,
            data: IntentData::None,
        }
    }
    
    /// Create with confidence
    pub fn with_confidence(concept_id: ConceptID, confidence: f32) -> Self {
        Self {
            concept_id,
            confidence,
            data: IntentData::None,
        }
    }
    
    /// Create with data
    pub fn with_data(concept_id: ConceptID, data: IntentData) -> Self {
        Self {
            concept_id,
            confidence: 1.0,
            data,
        }
    }
    
    /// Check if this is an unknown/unrecognized intent
    pub fn is_unknown(&self) -> bool {
        self.concept_id == ConceptID::UNKNOWN || self.confidence < 0.5
    }
}

/// Well-known concept IDs for system operations
pub mod concepts {
    use super::ConceptID;
    
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
