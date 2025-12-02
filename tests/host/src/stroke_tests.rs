//! Stroke Parsing Tests
//!
//! Tests for the stenographic stroke parsing logic.

use intent_kernel_tests::stroke::*;

// ═══════════════════════════════════════════════════════════════════════════════
// BASIC STROKE CONSTRUCTION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stroke_empty() {
    let stroke = Stroke::EMPTY;
    assert!(stroke.is_empty());
    assert_eq!(stroke.raw(), 0);
    assert_eq!(stroke.key_count(), 0);
}

#[test]
fn test_stroke_from_raw() {
    let stroke = Stroke::from_raw(0b101); // Bits 0 and 2 = # and T-
    assert!(stroke.has_key(0)); // #
    assert!(!stroke.has_key(1)); // S- not pressed
    assert!(stroke.has_key(2)); // T-
    assert_eq!(stroke.key_count(), 2);
}

#[test]
fn test_stroke_from_keys() {
    let stroke = Stroke::from_keys(&[1, 2, 4, 6]); // S-, T-, P-, H-
    assert!(stroke.has_key(1));
    assert!(stroke.has_key(2));
    assert!(!stroke.has_key(3)); // K- not pressed
    assert!(stroke.has_key(4));
    assert!(!stroke.has_key(5)); // W- not pressed
    assert!(stroke.has_key(6));
    assert_eq!(stroke.key_count(), 4);
}

#[test]
fn test_stroke_mask_23_bits() {
    // Should mask to only 23 bits
    let stroke = Stroke::from_raw(0xFFFFFFFF);
    assert_eq!(stroke.raw(), 0x7FFFFF); // Only lower 23 bits
}

// ═══════════════════════════════════════════════════════════════════════════════
// SPECIAL STROKES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_asterisk_correction() {
    let star = Stroke::STAR;
    assert!(star.is_correction());
    assert!(star.has_key(10)); // Asterisk is bit 10
    assert_eq!(star.key_count(), 1);
}

#[test]
fn test_asterisk_with_other_keys_not_correction() {
    // Asterisk + S- is not a correction stroke
    let stroke = Stroke::from_raw((1 << 10) | (1 << 1));
    assert!(!stroke.is_correction());
    assert!(stroke.has_key(10));
    assert!(stroke.has_key(1));
}

#[test]
fn test_number_key() {
    let num = Stroke::NUM;
    assert!(num.is_number());
    assert!(num.has_key(0));
}

// ═══════════════════════════════════════════════════════════════════════════════
// STENO NOTATION PARSING
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_parse_left_consonants() {
    let bits = parse_steno_to_bits("STPH");
    assert_eq!(bits & (1 << 1), 1 << 1); // S-
    assert_eq!(bits & (1 << 2), 1 << 2); // T-
    assert_eq!(bits & (1 << 4), 1 << 4); // P-
    assert_eq!(bits & (1 << 6), 1 << 6); // H-
}

#[test]
fn test_parse_left_consonants_all() {
    let bits = parse_steno_to_bits("STKPWHR");
    assert!(bits & (1 << 1) != 0); // S-
    assert!(bits & (1 << 2) != 0); // T-
    assert!(bits & (1 << 3) != 0); // K-
    assert!(bits & (1 << 4) != 0); // P-
    assert!(bits & (1 << 5) != 0); // W-
    assert!(bits & (1 << 6) != 0); // H-
    assert!(bits & (1 << 7) != 0); // R-
}

#[test]
fn test_parse_vowels() {
    let bits = parse_steno_to_bits("AOEU");
    assert!(bits & (1 << 8) != 0);  // A-
    assert!(bits & (1 << 9) != 0);  // O-
    assert!(bits & (1 << 11) != 0); // -E
    assert!(bits & (1 << 12) != 0); // -U
}

#[test]
fn test_parse_asterisk() {
    let bits = parse_steno_to_bits("*");
    assert_eq!(bits, 1 << 10);
}

#[test]
fn test_parse_right_consonants_with_hyphen() {
    let bits = parse_steno_to_bits("-FRPBLGTSDZ");
    assert!(bits & (1 << 13) != 0); // -F
    assert!(bits & (1 << 14) != 0); // -R
    assert!(bits & (1 << 15) != 0); // -P
    assert!(bits & (1 << 16) != 0); // -B
    assert!(bits & (1 << 17) != 0); // -L
    assert!(bits & (1 << 18) != 0); // -G
    assert!(bits & (1 << 19) != 0); // -T
    assert!(bits & (1 << 20) != 0); // -S
    assert!(bits & (1 << 21) != 0); // -D
    assert!(bits & (1 << 22) != 0); // -Z
}

#[test]
fn test_parse_simple_word_cat() {
    // KAT = K-, A-, -T
    let bits = parse_steno_to_bits("KAT");
    assert!(bits & (1 << 3) != 0);  // K-
    assert!(bits & (1 << 8) != 0);  // A-
    assert!(bits & (1 << 19) != 0); // -T
}

#[test]
fn test_parse_simple_word_the() {
    // -T is on the right
    let bits = parse_steno_to_bits("-T");
    assert!(bits & (1 << 19) != 0); // -T
    assert_eq!(bits.count_ones(), 1);
}

#[test]
fn test_parse_left_vs_right_s() {
    // S- (left) vs -S (right)
    let left = parse_steno_to_bits("S");
    assert!(left & (1 << 1) != 0); // S-
    
    let right = parse_steno_to_bits("-S");
    assert!(right & (1 << 20) != 0); // -S
}

#[test]
fn test_parse_left_vs_right_t() {
    let left = parse_steno_to_bits("T");
    assert!(left & (1 << 2) != 0); // T-
    
    let right = parse_steno_to_bits("-T");
    assert!(right & (1 << 19) != 0); // -T
}

#[test]
fn test_parse_vowels_mark_center() {
    // After a vowel, consonants go to right side
    let bits = parse_steno_to_bits("SAT");
    assert!(bits & (1 << 1) != 0);  // S- (left)
    assert!(bits & (1 << 8) != 0);  // A-
    assert!(bits & (1 << 19) != 0); // -T (right, after vowel)
}

// ═══════════════════════════════════════════════════════════════════════════════
// NUMBER PARSING
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_parse_numbers() {
    // 1 = # + S
    let bits = parse_steno_to_bits("1");
    assert!(bits & (1 << 0) != 0); // #
    assert!(bits & (1 << 1) != 0); // S-
    
    // 2 = # + T
    let bits = parse_steno_to_bits("2");
    assert!(bits & (1 << 0) != 0); // #
    assert!(bits & (1 << 2) != 0); // T-
    
    // 5 = # + A
    let bits = parse_steno_to_bits("5");
    assert!(bits & (1 << 0) != 0); // #
    assert!(bits & (1 << 8) != 0); // A-
    
    // 0 = # + O
    let bits = parse_steno_to_bits("0");
    assert!(bits & (1 << 0) != 0); // #
    assert!(bits & (1 << 9) != 0); // O-
}

// ═══════════════════════════════════════════════════════════════════════════════
// STROKE OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stroke_union() {
    let s1 = Stroke::from_raw(0b0011); // bits 0, 1
    let s2 = Stroke::from_raw(0b1100); // bits 2, 3
    let union = s1.union(&s2);
    assert_eq!(union.raw(), 0b1111);
}

#[test]
fn test_stroke_intersection() {
    let s1 = Stroke::from_raw(0b0111); // bits 0, 1, 2
    let s2 = Stroke::from_raw(0b1110); // bits 1, 2, 3
    let inter = s1.intersection(&s2);
    assert_eq!(inter.raw(), 0b0110); // bits 1, 2
}

// ═══════════════════════════════════════════════════════════════════════════════
// RTFCRE OUTPUT
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rtfcre_output_simple() {
    let stroke = stroke_from_steno("KAT");
    let rtfcre = stroke.to_rtfcre();
    // Should contain K, A, T in some form
    assert!(rtfcre.contains('K'));
    assert!(rtfcre.contains('A'));
}

#[test]
fn test_rtfcre_output_asterisk() {
    let stroke = Stroke::STAR;
    let rtfcre = stroke.to_rtfcre();
    assert!(rtfcre.contains('*'));
}

// ═══════════════════════════════════════════════════════════════════════════════
// STROKE FROM STENO CONVENIENCE
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stroke_from_steno_convenience() {
    let stroke = stroke_from_steno("STPH");
    assert_eq!(stroke.key_count(), 4);
    assert!(stroke.has_key(1)); // S-
    assert!(stroke.has_key(2)); // T-
    assert!(stroke.has_key(4)); // P-
    assert!(stroke.has_key(6)); // H-
}

// ═══════════════════════════════════════════════════════════════════════════════
// KEY CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_key_count() {
    assert_eq!(NUM_KEYS, 23);
    assert_eq!(KEYS.len(), 23);
}

#[test]
fn test_key_order() {
    assert_eq!(KEYS[0], "#");
    assert_eq!(KEYS[1], "S-");
    assert_eq!(KEYS[10], "*");
    assert_eq!(KEYS[22], "-Z");
}
