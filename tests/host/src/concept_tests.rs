//! Concept Tests
//!
//! Tests for semantic concept identifiers and intents.

use intent_kernel_tests::concept::*;

// ═══════════════════════════════════════════════════════════════════════════════
// CONCEPT ID TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_concept_id_new() {
    let id = ConceptID::new(0x1234_5678);
    assert_eq!(id.raw(), 0x1234_5678);
}

#[test]
fn test_concept_id_unknown() {
    assert_eq!(ConceptID::UNKNOWN.raw(), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_concept_id_equality() {
    let id1 = ConceptID::new(0x1234);
    let id2 = ConceptID::new(0x1234);
    let id3 = ConceptID::new(0x5678);
    
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_concept_id_category() {
    // Concept ID format: upper bits are category, lower bits are specific
    let id = ConceptID::new(0x0001_0002); // Category 0x0001, subcategory 0x0002
    assert_eq!(id.category(), 0x0001);
    assert_eq!(id.subcategory(), 0x0002);
}

#[test]
fn test_concept_id_hash() {
    use std::collections::HashSet;
    
    let mut set = HashSet::new();
    set.insert(ConceptID::new(0x1111));
    set.insert(ConceptID::new(0x2222));
    set.insert(ConceptID::new(0x1111)); // Duplicate
    
    assert_eq!(set.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// WELL-KNOWN CONCEPTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_system_concepts() {
    assert_eq!(concepts::HELP.raw(), 0x0000_0001);
    assert_eq!(concepts::STATUS.raw(), 0x0000_0002);
    assert_eq!(concepts::REBOOT.raw(), 0x0000_0003);
    assert_eq!(concepts::CLEAR.raw(), 0x0000_0004);
    assert_eq!(concepts::UNDO.raw(), 0x0000_0005);
}

#[test]
fn test_display_concepts() {
    assert_eq!(concepts::SHOW.category(), 0x0001);
    assert_eq!(concepts::DISPLAY.category(), 0x0001);
    assert_eq!(concepts::HIDE.category(), 0x0001);
    assert_eq!(concepts::ZOOM.category(), 0x0001);
}

#[test]
fn test_memory_concepts() {
    assert_eq!(concepts::STORE.category(), 0x0002);
    assert_eq!(concepts::RECALL.category(), 0x0002);
    assert_eq!(concepts::DELETE.category(), 0x0002);
    assert_eq!(concepts::COPY.category(), 0x0002);
}

#[test]
fn test_navigation_concepts() {
    assert_eq!(concepts::NEXT.category(), 0x0003);
    assert_eq!(concepts::PREVIOUS.category(), 0x0003);
    assert_eq!(concepts::BACK.category(), 0x0003);
    assert_eq!(concepts::FORWARD.category(), 0x0003);
}

#[test]
fn test_confirmation_concepts() {
    assert_eq!(concepts::YES.category(), 0x0007);
    assert_eq!(concepts::NO.category(), 0x0007);
    assert_eq!(concepts::CONFIRM.category(), 0x0007);
    assert_eq!(concepts::CANCEL.category(), 0x0007);
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_intent_new() {
    let intent = Intent::new(concepts::HELP);
    
    assert_eq!(intent.concept_id, concepts::HELP);
    assert_eq!(intent.confidence, 1.0);
    assert_eq!(intent.data, IntentData::None);
}

#[test]
fn test_intent_with_confidence() {
    let intent = Intent::with_confidence(concepts::SEARCH, 0.85);
    
    assert_eq!(intent.concept_id, concepts::SEARCH);
    assert_eq!(intent.confidence, 0.85);
}

#[test]
fn test_intent_with_data() {
    let intent = Intent::with_data(concepts::STORE, IntentData::Number(42));
    
    assert_eq!(intent.concept_id, concepts::STORE);
    assert_eq!(intent.data, IntentData::Number(42));
}

#[test]
fn test_intent_is_unknown_by_concept() {
    let unknown_intent = Intent::new(ConceptID::UNKNOWN);
    assert!(unknown_intent.is_unknown());
    
    let known_intent = Intent::new(concepts::HELP);
    assert!(!known_intent.is_unknown());
}

#[test]
fn test_intent_is_unknown_by_confidence() {
    let low_confidence = Intent::with_confidence(concepts::HELP, 0.3);
    assert!(low_confidence.is_unknown());
    
    let high_confidence = Intent::with_confidence(concepts::HELP, 0.8);
    assert!(!high_confidence.is_unknown());
    
    let threshold = Intent::with_confidence(concepts::HELP, 0.5);
    assert!(!threshold.is_unknown()); // 0.5 is valid
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT DATA TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_intent_data_none() {
    let data = IntentData::None;
    assert_eq!(data, IntentData::None);
}

#[test]
fn test_intent_data_number() {
    let data = IntentData::Number(12345);
    assert_eq!(data, IntentData::Number(12345));
    assert_ne!(data, IntentData::Number(54321));
}

#[test]
fn test_intent_data_raw() {
    let data = IntentData::Raw(0xDEADBEEF);
    assert_eq!(data, IntentData::Raw(0xDEADBEEF));
}

#[test]
fn test_intent_data_text() {
    let data = IntentData::Text("hello".to_string());
    assert_eq!(data, IntentData::Text("hello".to_string()));
    assert_ne!(data, IntentData::Text("world".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISPLAY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_concept_id_display() {
    let id = ConceptID::new(0x0001_0002);
    let display = format!("{}", id);
    assert!(display.contains("0001"));
    assert!(display.contains("0002"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONCEPT ORGANIZATION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_concept_categories_unique() {
    // Each category should have unique concept IDs
    let categories = [
        (0x0000, vec![concepts::HELP, concepts::STATUS, concepts::REBOOT, concepts::CLEAR, concepts::UNDO]),
        (0x0001, vec![concepts::SHOW, concepts::DISPLAY, concepts::HIDE, concepts::ZOOM]),
        (0x0002, vec![concepts::STORE, concepts::RECALL, concepts::DELETE, concepts::COPY]),
        (0x0003, vec![concepts::NEXT, concepts::PREVIOUS, concepts::BACK, concepts::FORWARD]),
        (0x0004, vec![concepts::CALCULATE, concepts::SEARCH, concepts::FILTER, concepts::SORT]),
        (0x0005, vec![concepts::SEND, concepts::SAVE, concepts::LOAD, concepts::CONNECT]),
        (0x0006, vec![concepts::MODE, concepts::TOGGLE, concepts::SETTINGS]),
        (0x0007, vec![concepts::YES, concepts::NO, concepts::CONFIRM, concepts::CANCEL]),
    ];
    
    for (expected_cat, concepts) in &categories {
        for concept in concepts {
            assert_eq!(
                concept.category(),
                *expected_cat,
                "Concept {:?} has wrong category",
                concept
            );
        }
    }
}

#[test]
fn test_all_concepts_unique() {
    use std::collections::HashSet;
    
    let all_concepts = vec![
        concepts::HELP, concepts::STATUS, concepts::REBOOT, concepts::CLEAR, concepts::UNDO,
        concepts::SHOW, concepts::DISPLAY, concepts::HIDE, concepts::ZOOM,
        concepts::STORE, concepts::RECALL, concepts::DELETE, concepts::COPY,
        concepts::NEXT, concepts::PREVIOUS, concepts::BACK, concepts::FORWARD,
        concepts::CALCULATE, concepts::SEARCH, concepts::FILTER, concepts::SORT,
        concepts::SEND, concepts::SAVE, concepts::LOAD, concepts::CONNECT,
        concepts::MODE, concepts::TOGGLE, concepts::SETTINGS,
        concepts::YES, concepts::NO, concepts::CONFIRM, concepts::CANCEL,
    ];
    
    let mut seen = HashSet::new();
    for concept in &all_concepts {
        assert!(
            seen.insert(concept.raw()),
            "Duplicate concept ID: 0x{:016X}",
            concept.raw()
        );
    }
}
