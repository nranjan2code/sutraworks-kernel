//! Stroke - The fundamental semantic unit
//!
//! Aligned with Plover's steno.py Stroke implementation.
//! Each stroke is a 23-bit binary pattern representing a complete semantic unit.
//!
//! Key Order (from Plover english_stenotype.py):
//! ```
//! #, S-, T-, K-, P-, W-, H-, R-, A-, O-, *, -E, -U, -F, -R, -P, -B, -L, -G, -T, -S, -D, -Z
//! ```

use core::fmt;

// ═══════════════════════════════════════════════════════════════════════════════
// PLOVER KEY ORDER (Canonical)
// ═══════════════════════════════════════════════════════════════════════════════

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

/// Keys that implicitly contain the hyphen separator
pub const IMPLICIT_HYPHEN_KEYS: &[&str] = &["A-", "O-", "-E", "-U", "*"];

/// Number key identifier
pub const NUMBER_KEY: &str = "#";

/// Undo stroke (asterisk alone)
pub const UNDO_STROKE: &str = "*";

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

// ═══════════════════════════════════════════════════════════════════════════════
// STROKE STRUCT
// ═══════════════════════════════════════════════════════════════════════════════

/// A steno stroke - 23 bits representing pressed keys
///
/// This is the fundamental semantic unit in stenographic input.
/// Each bit corresponds to a key in KEYS order:
/// - Bit 0: # (number key)
/// - Bits 1-7: S-, T-, K-, P-, W-, H-, R- (left consonants)
/// - Bits 8-9: A-, O- (left vowels)
/// - Bit 10: * (asterisk)
/// - Bits 11-12: -E, -U (right vowels)
/// - Bits 13-22: -F, -R, -P, -B, -L, -G, -T, -S, -D, -Z (right consonants)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stroke(u32);

impl Stroke {
    /// Empty stroke (no keys pressed)
    pub const EMPTY: Stroke = Stroke(0);
    
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
    
    /// Create a stroke from steno notation (RTFCRE format)
    ///
    /// Examples: "STPH", "KAT", "-S", "TEFT", "12-9"
    pub fn from_steno(steno: &str) -> Option<Self> {
        let normalized = normalize_steno(steno)?;
        let bits = parse_steno_to_bits(normalized);
        Some(Stroke(bits))
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
    
    /// Convert to RTFCRE notation string
    ///
    /// Returns a static buffer representation (no alloc)
    pub fn to_rtfcre(&self) -> RtfcreBuffer {
        let mut buf = RtfcreBuffer::new();
        let bits = self.0;
        
        // Check if we need numbers
        let has_number = (bits & 1) != 0;
        
        // Track if we have any left-side or vowel keys (for hyphen placement)
        let has_left = (bits & 0x3FE) != 0;  // S- through R- (bits 1-7)
        let has_vowel = (bits & 0x1F00) != 0; // A-, O-, *, -E, -U (bits 8-12)
        let has_right = (bits & 0x7FE000) != 0; // -F through -Z (bits 13-22)
        
        // Output each pressed key
        for (i, key) in KEYS.iter().enumerate().take(NUM_KEYS) {
            if (bits & (1 << i)) != 0 {
                // Handle number conversions
                if has_number && i > 0 {
                    if let Some(num) = number_for_key(key) {
                        buf.push_str(num.trim_matches('-'));
                        continue;
                    }
                }
                
                // Handle hyphen
                if i == 0 {
                    if !has_number {
                        buf.push('#');
                    }
                } else if i <= 7 {
                    // Left consonants - strip trailing hyphen
                    buf.push_str(key.trim_end_matches('-'));
                } else if i <= 12 {
                    // Vowels and asterisk - strip hyphens
                    buf.push_str(key.trim_matches('-'));
                } else {
                    // Right consonants
                    if !has_left && !has_vowel && buf.is_empty() {
                        // Need leading hyphen if no left/vowel keys
                        buf.push('-');
                    }
                    buf.push_str(key.trim_start_matches('-'));
                }
            }
        }
        
        // Add hyphen for right-only strokes that didn't get one
        if has_right && !has_left && !buf.starts_with('-') && !buf.is_empty() {
            // Actually this case is handled above
        }
        
        buf
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
}

impl fmt::Debug for Stroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stroke({})", self.to_rtfcre().as_str())
    }
}

impl fmt::Display for Stroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_rtfcre().as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RTFCRE BUFFER (No-alloc string builder)
// ═══════════════════════════════════════════════════════════════════════════════

/// Fixed-size buffer for RTFCRE stroke notation
pub struct RtfcreBuffer {
    data: [u8; 32],
    len: usize,
}

impl RtfcreBuffer {
    /// Create empty buffer
    pub const fn new() -> Self {
        Self {
            data: [0; 32],
            len: 0,
        }
    }

    
    /// Push a character
    pub fn push(&mut self, c: char) {
        if self.len < 31 {
            self.data[self.len] = c as u8;
            self.len += 1;
        }
    }
    
    /// Push a string
    pub fn push_str(&mut self, s: &str) {
        for c in s.chars() {
            self.push(c);
        }
    }
    
    /// Get as string slice
    pub fn as_str(&self) -> &str {
        // SAFETY: We only push valid ASCII characters
        unsafe { core::str::from_utf8_unchecked(&self.data[..self.len]) }
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Get length
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Check if starts with character
    pub fn starts_with(&self, c: char) -> bool {
        self.len > 0 && self.data[0] == c as u8
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════



/// Get number replacement for a key
fn number_for_key(key: &str) -> Option<&'static str> {
    for &(k, n) in NUMBERS {
        if k == key {
            return Some(n);
        }
    }
    None
}

/// Normalize steno notation
fn normalize_steno(steno: &str) -> Option<&str> {
    // For now, basic validation - non-empty, valid chars
    if steno.is_empty() {
        return None;
    }
    Some(steno)
}



// Use a simpler approach without iterator for now
/// Parse steno notation and return stroke bits
pub fn parse_steno_to_bits(steno: &str) -> u32 {
    let mut bits = 0u32;
    let bytes = steno.as_bytes();
    let mut i = 0;
    let len = bytes.len();
    
    // Track position (left side vs right side)
    let mut past_center = false;
    
    // First pass: check for explicit hyphen or vowels
    for &b in bytes {
        if b == b'-' {
            past_center = true;
            break;
        }
        if b == b'A' || b == b'O' || b == b'*' || b == b'E' || b == b'U' {
            past_center = false; // Will handle vowels specially
            break;
        }
    }
    
    let has_hyphen = bytes.contains(&b'-');
    
    while i < len {
        let c = bytes[i] as char;
        
        match c {
            '#' => bits |= 1 << 0,
            '-' => {
                past_center = true;
            }
            'S' => {
                if past_center || (has_hyphen && i > bytes.iter().position(|&b| b == b'-').unwrap_or(len)) {
                    bits |= 1 << 20; // -S
                } else {
                    bits |= 1 << 1; // S-
                }
            }
            'T' => {
                if past_center || (has_hyphen && i > bytes.iter().position(|&b| b == b'-').unwrap_or(len)) {
                    bits |= 1 << 19; // -T
                } else {
                    bits |= 1 << 2; // T-
                }
            }
            'K' => bits |= 1 << 3,  // K- (left only)
            'P' => {
                if past_center || (has_hyphen && i > bytes.iter().position(|&b| b == b'-').unwrap_or(len)) {
                    bits |= 1 << 15; // -P
                } else {
                    bits |= 1 << 4; // P-
                }
            }
            'W' => bits |= 1 << 5,  // W- (left only)
            'H' => bits |= 1 << 6,  // H- (left only)
            'R' => {
                if past_center || (has_hyphen && i > bytes.iter().position(|&b| b == b'-').unwrap_or(len)) {
                    bits |= 1 << 14; // -R
                } else {
                    bits |= 1 << 7; // R-
                }
            }
            'A' => {
                bits |= 1 << 8; // A-
                past_center = true; // Vowels mark the center
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
            '6' => bits |= (1 << 0) | (1 << 13), // # + F
            '7' => bits |= (1 << 0) | (1 << 15), // # + P
            '8' => bits |= (1 << 0) | (1 << 17), // # + L
            '9' => bits |= (1 << 0) | (1 << 19), // # + T
            _ => {}
        }
        
        i += 1;
    }
    
    bits
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMMON STROKE CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

impl Stroke {
    /// Asterisk (correction/undo)
    pub const STAR: Stroke = Stroke(1 << 10);
    
    /// Number key
    pub const NUM: Stroke = Stroke(1 << 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stroke_from_raw() {
        let stroke = Stroke::from_raw(0b101); // # and T-
        assert!(stroke.has_key(0)); // #
        assert!(stroke.has_key(2)); // T-
        assert!(!stroke.has_key(1)); // S-
    }
    
    #[test]
    fn test_is_correction() {
        let star = Stroke::from_raw(1 << 10);
        assert!(star.is_correction());
        
        let not_star = Stroke::from_raw((1 << 10) | (1 << 1));
        assert!(!not_star.is_correction());
    }
    
    #[test]
    fn test_parse_steno() {
        let bits = parse_steno_to_bits("STPH");
        assert_eq!(bits & (1 << 1), 1 << 1); // S-
        assert_eq!(bits & (1 << 2), 1 << 2); // T-
        assert_eq!(bits & (1 << 4), 1 << 4); // P-
        assert_eq!(bits & (1 << 6), 1 << 6); // H-
    }
}

impl Default for RtfcreBuffer {
    fn default() -> Self {
        Self::new()
    }
}
