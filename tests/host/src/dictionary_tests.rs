//! Dictionary Tests
//!
//! Tests for the stroke-to-intent dictionary lookup.

use intent_kernel_tests::dictionary::*;
use intent_kernel_tests::stroke::*;
use intent_kernel_tests::concept::*;

// ═══════════════════════════════════════════════════════════════════════════════
// DICTIONARY CREATION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dictionary_new_empty() {
    let dict = StenoDictionary::new();
    assert!(dict.is_empty());
    assert_eq!(dict.len(), 0);
}

#[test]
fn test_dictionary_with_defaults() {
    let dict = StenoDictionary::with_defaults();
    assert!(!dict.is_empty());
    assert!(dict.len() > 10); // Should have many default entries
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENTRY CREATION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dict_entry_from_raw() {
    let entry = DictEntry::new(
        0b1010,
        ConceptID::new(0x1234),
        "TEST",
    );
    
    assert_eq!(entry.stroke.raw(), 0b1010);
    assert_eq!(entry.concept_id.raw(), 0x1234);
    assert_eq!(entry.name, "TEST");
}

#[test]
fn test_dict_entry_from_steno() {
    let entry = DictEntry::from_steno(
        "KAT",
        ConceptID::new(0x5678),
        "CAT",
    );
    
    assert_eq!(entry.concept_id.raw(), 0x5678);
    assert_eq!(entry.name, "CAT");
    // Stroke should have K-, A-, -T
    assert!(entry.stroke.has_key(3));  // K-
    assert!(entry.stroke.has_key(8));  // A-
    assert!(entry.stroke.has_key(19)); // -T
}

// ═══════════════════════════════════════════════════════════════════════════════
// DICTIONARY LOOKUP
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dictionary_add_and_lookup() {
    let mut dict = StenoDictionary::new();
    
    dict.add_entry(DictEntry::from_steno(
        "TEFT",
        ConceptID::new(0x1111),
        "TEST",
    ));
    
    let stroke = stroke_from_steno("TEFT");
    let entry = dict.lookup(stroke);
    
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.name, "TEST");
    assert_eq!(entry.concept_id.raw(), 0x1111);
}

#[test]
fn test_dictionary_lookup_not_found() {
    let dict = StenoDictionary::new();
    
    let stroke = stroke_from_steno("TEFT");
    let entry = dict.lookup(stroke);
    
    assert!(entry.is_none());
}

#[test]
fn test_dictionary_lookup_raw() {
    let mut dict = StenoDictionary::new();
    
    let bits = parse_steno_to_bits("STPH");
    dict.add_entry(DictEntry::new(
        bits,
        ConceptID::new(0x2222),
        "SNIFF",
    ));
    
    let entry = dict.lookup_raw(bits);
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().name, "SNIFF");
}

#[test]
fn test_dictionary_lookup_steno() {
    let mut dict = StenoDictionary::new();
    
    dict.add_entry(DictEntry::from_steno(
        "PWABG",
        ConceptID::new(0x3333),
        "BACK",
    ));
    
    let entry = dict.lookup_steno("PWABG");
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().name, "BACK");
}

// ═══════════════════════════════════════════════════════════════════════════════
// DEFAULT DICTIONARY LOOKUPS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_default_dictionary_help() {
    let dict = StenoDictionary::with_defaults();
    
    let entry = dict.lookup_steno("PH-FPL");
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.name, "HELP");
    assert_eq!(entry.concept_id, concepts::HELP);
}

#[test]
fn test_default_dictionary_undo() {
    let dict = StenoDictionary::with_defaults();
    
    let entry = dict.lookup_steno("*");
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.name, "UNDO");
    assert_eq!(entry.concept_id, concepts::UNDO);
}

#[test]
fn test_default_dictionary_yes_no() {
    let dict = StenoDictionary::with_defaults();
    
    let yes = dict.lookup_steno("KWRE");
    assert!(yes.is_some());
    assert_eq!(yes.unwrap().concept_id, concepts::YES);
    
    let no = dict.lookup_steno("TPHO");
    assert!(no.is_some());
    assert_eq!(no.unwrap().concept_id, concepts::NO);
}

#[test]
fn test_default_dictionary_navigation() {
    let dict = StenoDictionary::with_defaults();
    
    let back = dict.lookup_steno("PWABG");
    assert!(back.is_some());
    assert_eq!(back.unwrap().concept_id, concepts::BACK);
}

// ═══════════════════════════════════════════════════════════════════════════════
// STROKE SEQUENCE
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stroke_sequence_empty() {
    let seq = StrokeSequence::new();
    assert!(seq.is_empty());
    assert_eq!(seq.len(), 0);
    assert!(seq.last().is_none());
}

#[test]
fn test_stroke_sequence_push() {
    let mut seq = StrokeSequence::new();
    
    let s1 = stroke_from_steno("KAT");
    let s2 = stroke_from_steno("TKOG");
    
    assert!(seq.push(s1));
    assert!(seq.push(s2));
    
    assert_eq!(seq.len(), 2);
    assert!(!seq.is_empty());
    assert_eq!(seq.last(), Some(s2));
}

#[test]
fn test_stroke_sequence_pop() {
    let mut seq = StrokeSequence::new();
    
    let s1 = stroke_from_steno("KAT");
    let s2 = stroke_from_steno("TKOG");
    
    seq.push(s1);
    seq.push(s2);
    
    assert_eq!(seq.pop(), Some(s2));
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.pop(), Some(s1));
    assert!(seq.is_empty());
    assert_eq!(seq.pop(), None);
}

#[test]
fn test_stroke_sequence_clear() {
    let mut seq = StrokeSequence::new();
    
    seq.push(stroke_from_steno("KAT"));
    seq.push(stroke_from_steno("TKOG"));
    
    seq.clear();
    
    assert!(seq.is_empty());
    assert_eq!(seq.len(), 0);
}

#[test]
fn test_stroke_sequence_max_length() {
    let mut seq = StrokeSequence::new();
    
    // Should accept up to MAX_STROKE_SEQUENCE
    for i in 0..MAX_STROKE_SEQUENCE {
        let stroke = Stroke::from_raw(i as u32);
        assert!(seq.push(stroke), "Failed to push stroke {}", i);
    }
    
    assert_eq!(seq.len(), MAX_STROKE_SEQUENCE);
    
    // Should reject additional strokes
    assert!(!seq.push(Stroke::from_raw(999)));
    assert_eq!(seq.len(), MAX_STROKE_SEQUENCE);
}

#[test]
fn test_stroke_sequence_strokes() {
    let mut seq = StrokeSequence::new();
    
    let s1 = stroke_from_steno("KAT");
    let s2 = stroke_from_steno("TKOG");
    
    seq.push(s1);
    seq.push(s2);
    
    let strokes = seq.strokes();
    assert_eq!(strokes.len(), 2);
    assert_eq!(strokes[0], s1);
    assert_eq!(strokes[1], s2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTIPLE ENTRIES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dictionary_multiple_entries() {
    let mut dict = StenoDictionary::new();
    
    dict.add_entry(DictEntry::from_steno("KAT", ConceptID::new(1), "CAT"));
    dict.add_entry(DictEntry::from_steno("TKOG", ConceptID::new(2), "DOG"));
    dict.add_entry(DictEntry::from_steno("PWEURS", ConceptID::new(3), "BIRD"));
    
    assert_eq!(dict.len(), 3);
    
    assert!(dict.lookup_steno("KAT").is_some());
    assert!(dict.lookup_steno("TKOG").is_some());
    assert!(dict.lookup_steno("PWEURS").is_some());
    assert!(dict.lookup_steno("TPEURB").is_none()); // FISH not added
}

#[test]
fn test_dictionary_entries_iterator() {
    let mut dict = StenoDictionary::new();
    
    dict.add_entry(DictEntry::from_steno("KAT", ConceptID::new(1), "CAT"));
    dict.add_entry(DictEntry::from_steno("TKOG", ConceptID::new(2), "DOG"));
    
    let entries = dict.entries();
    assert_eq!(entries.len(), 2);
    
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"CAT"));
    assert!(names.contains(&"DOG"));
}
