//! Stroke - The fundamental semantic unit (Test-friendly implementation)
//!
//! This module mirrors the kernel's stroke.rs but with std support for testing.

/// Steno keys in Plover order (english_stenotype.py)
pub const KEYS: &[&str] = &[
    "#",                                    // Number key (bit 0)
    "S-", "T-", "K-", "P-", "W-", "H-", "R-", // Left hand (bits 1-7)
    "A-", "O-",                             // Left thumb (bits 8-9)
    "*",                                    // Asterisk (bit 10)
    "-E", "-U",                             // Right thumb (bits 11-12)
    "-F", "-R", "-P", "-B", "-L", "-G",     // Right hand upper (bits 13-18)
    "-T", "-S", "-D", "-Z",                 // Right hand lower (bits 19-22)
];

/// Total number of keys in standard steno layout
pub const NUM_KEYS: usize = 23;

/// Number conversions when # is pressed
pub const NUMBERS: &[(&str, &str)] = &[
    ("S-", "1-"),
    ("T-", "2-"),
    ("P-", "3-"),
    ("H-", "4-"),
    ("A-", "5-"),
    ("O-", "0-"),
    ("-F", "-6"),
    ("-P", "-7"),
    ("-L", "-8"),
    ("-T", "-9"),
];

/// A steno stroke - 23 bits representing pressed keys
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stroke(u32);

impl Stroke {
    /// Empty stroke (no keys pressed)
    pub const EMPTY: Stroke = Stroke(0);
    
    /// Asterisk (correction/undo)
    pub const STAR: Stroke = Stroke(1 << 10);
    
    /// Number key
    pub const NUM: Stroke = Stroke(1 << 0);
    
    /// Create a stroke from raw bits
    #[inline]
    pub const fn from_raw(bits: u32) -> Self {
        Stroke(bits & 0x7FFFFF) // Mask to 23 bits
    }
    
    /// Create a stroke from a slice of key indices
    pub fn from_keys(keys: &[usize]) -> Self {
        let mut bits = 0u32;
        for &key in keys {
            if key < NUM_KEYS {
                bits |= 1 << key;
            }
        }
        Stroke(bits)
    }
    
    /// Get the raw bits
    #[inline]
    pub const fn raw(&self) -> u32 {
        self.0
    }
    
    /// Check if a specific key is pressed
    #[inline]
    pub const fn has_key(&self, key_index: usize) -> bool {
        if key_index < NUM_KEYS {
            (self.0 & (1 << key_index)) != 0
        } else {
            false
        }
    }
    
    /// Check if this is the correction stroke (asterisk only)
    #[inline]
    pub const fn is_correction(&self) -> bool {
        self.0 == (1 << 10) // Only asterisk bit set
    }
    
    /// Check if this stroke uses the number key
    #[inline]
    pub const fn is_number(&self) -> bool {
        (self.0 & 1) != 0
    }
    
    /// Check if this is an empty stroke
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }
    
    /// Count the number of keys pressed
    #[inline]
    pub const fn key_count(&self) -> u32 {
        self.0.count_ones()
    }
    
    /// Create union of two strokes
    #[inline]
    pub const fn union(&self, other: &Stroke) -> Stroke {
        Stroke(self.0 | other.0)
    }
    
    /// Create intersection of two strokes
    #[inline]
    pub const fn intersection(&self, other: &Stroke) -> Stroke {
        Stroke(self.0 & other.0)
    }
    
    /// Convert to RTFCRE notation string
    pub fn to_rtfcre(&self) -> String {
        let mut result = String::new();
        let bits = self.0;
        
        // Check if we need numbers
        let has_number = (bits & 1) != 0;
        
        // Track sides
        let has_left = (bits & 0xFE) != 0;      // S- through R- (bits 1-7)
        let has_vowel = (bits & 0x1F00) != 0;   // A-, O-, *, -E, -U (bits 8-12)
        let _has_right = (bits & 0x7FE000) != 0; // -F through -Z (bits 13-22)
        
        // Output each pressed key
        for i in 0..NUM_KEYS {
            if (bits & (1 << i)) != 0 {
                let key = KEYS[i];
                
                // Handle number conversions
                if has_number && i > 0 {
                    if let Some(num) = number_for_key(key) {
                        result.push_str(num.trim_matches('-'));
                        continue;
                    }
                }
                
                if i == 0 {
                    if !has_number {
                        result.push('#');
                    }
                } else if i <= 7 {
                    // Left consonants - strip trailing hyphen
                    result.push_str(key.trim_end_matches('-'));
                } else if i <= 12 {
                    // Vowels and asterisk - strip hyphens
                    result.push_str(key.trim_matches('-'));
                } else {
                    // Right consonants
                    if !has_left && !has_vowel && result.is_empty() {
                        result.push('-');
                    }
                    result.push_str(key.trim_start_matches('-'));
                }
            }
        }
        
        result
    }
}

impl std::fmt::Display for Stroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_rtfcre())
    }
}

/// Get number replacement for a key
fn number_for_key(key: &str) -> Option<&'static str> {
    for &(k, n) in NUMBERS {
        if k == key {
            return Some(n);
        }
    }
    None
}

/// Parse steno notation and return stroke bits
pub fn parse_steno_to_bits(steno: &str) -> u32 {
    let mut bits = 0u32;
    let bytes = steno.as_bytes();
    let len = bytes.len();
    
    // Track position (left side vs right side)
    let mut past_center = false;
    
    let has_hyphen = bytes.contains(&b'-');
    let hyphen_pos = bytes.iter().position(|&b| b == b'-').unwrap_or(len);
    
    for (i, &b) in bytes.iter().enumerate() {
        let c = b as char;
        
        // Check if we're past the hyphen
        let is_right_side = has_hyphen && i > hyphen_pos;
        
        match c {
            '#' => bits |= 1 << 0,
            '-' => {
                past_center = true;
            }
            'S' => {
                if past_center || is_right_side {
                    bits |= 1 << 20; // -S
                } else {
                    bits |= 1 << 1; // S-
                }
            }
            'T' => {
                if past_center || is_right_side {
                    bits |= 1 << 19; // -T
                } else {
                    bits |= 1 << 2; // T-
                }
            }
            'K' => bits |= 1 << 3,  // K- (left only)
            'P' => {
                if past_center || is_right_side {
                    bits |= 1 << 15; // -P
                } else {
                    bits |= 1 << 4; // P-
                }
            }
            'W' => bits |= 1 << 5,  // W- (left only)
            'H' => bits |= 1 << 6,  // H- (left only)
            'R' => {
                if past_center || is_right_side {
                    bits |= 1 << 14; // -R
                } else {
                    bits |= 1 << 7; // R-
                }
            }
            'A' => {
                bits |= 1 << 8; // A-
                past_center = true;
            }
            'O' => {
                bits |= 1 << 9; // O-
                past_center = true;
            }
            '*' => {
                bits |= 1 << 10; // *
                past_center = true;
            }
            'E' => {
                bits |= 1 << 11; // -E
                past_center = true;
            }
            'U' => {
                bits |= 1 << 12; // -U
                past_center = true;
            }
            'F' => bits |= 1 << 13, // -F (right only)
            'B' => bits |= 1 << 16, // -B (right only)
            'L' => bits |= 1 << 17, // -L (right only)
            'G' => bits |= 1 << 18, // -G (right only)
            'D' => bits |= 1 << 21, // -D (right only)
            'Z' => bits |= 1 << 22, // -Z (right only)
            // Numbers when # is present
            '0' => bits |= (1 << 0) | (1 << 9),  // # + O
            '1' => bits |= (1 << 0) | (1 << 1),  // # + S
            '2' => bits |= (1 << 0) | (1 << 2),  // # + T
            '3' => bits |= (1 << 0) | (1 << 4),  // # + P
            '4' => bits |= (1 << 0) | (1 << 6),  // # + H
            '5' => bits |= (1 << 0) | (1 << 8),  // # + A
            '6' => bits |= (1 << 0) | (1 << 13), // # + -F
            '7' => bits |= (1 << 0) | (1 << 15), // # + -P
            '8' => bits |= (1 << 0) | (1 << 17), // # + -L
            '9' => bits |= (1 << 0) | (1 << 19), // # + -T
            _ => {}
        }
    }
    
    bits
}

/// Create a Stroke from steno notation
pub fn stroke_from_steno(steno: &str) -> Stroke {
    Stroke::from_raw(parse_steno_to_bits(steno))
}
