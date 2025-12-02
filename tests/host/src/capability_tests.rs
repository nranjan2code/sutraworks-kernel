//! Capability System Tests
//!
//! Tests for the capability-based security system.

use intent_kernel_tests::capability::*;

// ═══════════════════════════════════════════════════════════════════════════════
// PERMISSION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_permission_none() {
    let perm = Permissions::NONE;
    assert!(!perm.has(Permissions::READ));
    assert!(!perm.has(Permissions::WRITE));
    assert!(!perm.has(Permissions::EXECUTE));
}

#[test]
fn test_permission_single() {
    assert!(Permissions::READ.has(Permissions::READ));
    assert!(!Permissions::READ.has(Permissions::WRITE));
    
    assert!(Permissions::WRITE.has(Permissions::WRITE));
    assert!(!Permissions::WRITE.has(Permissions::READ));
}

#[test]
fn test_permission_combine_or() {
    let rw = Permissions::READ.or(Permissions::WRITE);
    assert!(rw.has(Permissions::READ));
    assert!(rw.has(Permissions::WRITE));
    assert!(!rw.has(Permissions::EXECUTE));
}

#[test]
fn test_permission_combine_and() {
    let all = Permissions::ALL;
    let read_only = Permissions::READ;
    
    let result = all.and(read_only);
    assert!(result.has(Permissions::READ));
    assert!(!result.has(Permissions::WRITE));
}

#[test]
fn test_permission_without() {
    let rw = Permissions::READ_WRITE;
    let read_only = rw.without(Permissions::WRITE);
    
    assert!(read_only.has(Permissions::READ));
    assert!(!read_only.has(Permissions::WRITE));
}

#[test]
fn test_permission_all() {
    let all = Permissions::ALL;
    assert!(all.has(Permissions::READ));
    assert!(all.has(Permissions::WRITE));
    assert!(all.has(Permissions::EXECUTE));
    assert!(all.has(Permissions::DELETE));
    assert!(all.has(Permissions::SHARE));
    assert!(all.has(Permissions::DELEGATE));
    assert!(all.has(Permissions::REVOKE));
}

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_capability_null() {
    let cap = Capability::null();
    assert!(!cap.is_valid());
    assert_eq!(cap.cap_type(), CapabilityType::Null);
}

#[test]
fn test_capability_mint_root() {
    let mut table = CapabilityTable::new();
    
    let cap = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::READ_WRITE,
    );
    
    assert!(cap.is_valid());
    assert_eq!(cap.cap_type(), CapabilityType::Memory);
    assert!(cap.has_permission(Permissions::READ));
    assert!(cap.has_permission(Permissions::WRITE));
    assert!(!cap.has_permission(Permissions::EXECUTE));
    assert_eq!(cap.size(), 0x100);
}

#[test]
fn test_capability_validate() {
    let mut table = CapabilityTable::new();
    
    let cap = table.mint_root(
        CapabilityType::Device,
        0x2000,
        0x200,
        Permissions::ALL,
    );
    
    assert!(table.validate(&cap));
}

#[test]
fn test_capability_decrypt_resource() {
    let mut table = CapabilityTable::with_seed(12345);
    
    let cap = table.mint_root(
        CapabilityType::Memory,
        0xDEADBEEF,
        0x1000,
        Permissions::READ,
    );
    
    // Resource should be encrypted in capability
    let decrypted = table.decrypt_resource(&cap);
    assert_eq!(decrypted, 0xDEADBEEF);
}

// ═══════════════════════════════════════════════════════════════════════════════
// DELEGATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_capability_derive_success() {
    let mut table = CapabilityTable::new();
    
    let parent = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::ALL, // Includes DELEGATE
    );
    
    let derived = table.derive(&parent, Permissions::READ);
    assert!(derived.is_some());
    
    let derived = derived.unwrap();
    assert!(derived.has_permission(Permissions::READ));
    assert!(!derived.has_permission(Permissions::WRITE));
    assert!(!derived.has_permission(Permissions::DELEGATE)); // Can't delegate further
}

#[test]
fn test_capability_derive_without_delegate_permission() {
    let mut table = CapabilityTable::new();
    
    let parent = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::READ_WRITE, // No DELEGATE
    );
    
    let derived = table.derive(&parent, Permissions::READ);
    assert!(derived.is_none()); // Should fail
}

#[test]
fn test_capability_derive_reduces_permissions() {
    let mut table = CapabilityTable::new();
    
    let parent = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::READ_WRITE.or(Permissions::DELEGATE),
    );
    
    // Try to derive with more permissions than parent
    let derived = table.derive(&parent, Permissions::ALL);
    assert!(derived.is_some());
    
    let derived = derived.unwrap();
    // Should only have intersection of permissions
    assert!(derived.has_permission(Permissions::READ));
    assert!(derived.has_permission(Permissions::WRITE));
    assert!(!derived.has_permission(Permissions::EXECUTE)); // Parent didn't have this
    assert!(!derived.has_permission(Permissions::DELEGATE)); // Removed during derive
}

// ═══════════════════════════════════════════════════════════════════════════════
// REVOCATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_capability_revoke() {
    let mut table = CapabilityTable::new();
    
    let cap = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::ALL,
    );
    
    assert!(table.validate(&cap));
    
    let success = table.revoke(&cap);
    assert!(success);
    
    assert!(!table.validate(&cap)); // Should be invalid now
}

#[test]
fn test_capability_revoke_without_permission() {
    let mut table = CapabilityTable::new();
    
    let cap = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::READ, // No REVOKE permission
    );
    
    let success = table.revoke(&cap);
    assert!(!success); // Should fail
    
    assert!(table.validate(&cap)); // Should still be valid
}

#[test]
fn test_capability_revoke_cascades_to_children() {
    let mut table = CapabilityTable::new();
    
    let parent = table.mint_root(
        CapabilityType::Memory,
        0x1000,
        0x100,
        Permissions::ALL,
    );
    
    let child = table.derive(&parent, Permissions::READ).unwrap();
    
    // Both should be valid
    assert!(table.validate(&parent));
    assert!(table.validate(&child));
    
    // Revoke parent
    table.revoke(&parent);
    
    // Both should be invalid
    assert!(!table.validate(&parent));
    assert!(!table.validate(&child)); // Child revoked too
}

#[test]
fn test_capability_global_revoke() {
    let mut table = CapabilityTable::new();
    
    let cap1 = table.mint_root(CapabilityType::Memory, 0x1000, 0x100, Permissions::ALL);
    let cap2 = table.mint_root(CapabilityType::Device, 0x2000, 0x200, Permissions::ALL);
    
    assert!(table.validate(&cap1));
    assert!(table.validate(&cap2));
    
    table.global_revoke();
    
    // All capabilities should be invalid
    assert!(!table.validate(&cap1));
    assert!(!table.validate(&cap2));
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATISTICS TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_capability_stats() {
    let mut table = CapabilityTable::new();
    
    let (active, revoked) = table.stats();
    assert_eq!(active, 0);
    assert_eq!(revoked, 0);
    
    let cap1 = table.mint_root(CapabilityType::Memory, 0x1000, 0x100, Permissions::ALL);
    let _cap2 = table.mint_root(CapabilityType::Device, 0x2000, 0x200, Permissions::ALL);
    
    let (active, revoked) = table.stats();
    assert_eq!(active, 2);
    assert_eq!(revoked, 0);
    
    table.revoke(&cap1);
    
    let (active, revoked) = table.stats();
    assert_eq!(active, 1);
    assert_eq!(revoked, 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY TYPE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_capability_types() {
    let mut table = CapabilityTable::new();
    
    let memory_cap = table.mint_root(CapabilityType::Memory, 0, 0, Permissions::READ);
    let device_cap = table.mint_root(CapabilityType::Device, 0, 0, Permissions::READ);
    let system_cap = table.mint_root(CapabilityType::System, 0, 0, Permissions::READ);
    
    assert_eq!(memory_cap.cap_type(), CapabilityType::Memory);
    assert_eq!(device_cap.cap_type(), CapabilityType::Device);
    assert_eq!(system_cap.cap_type(), CapabilityType::System);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR CONDITIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_null_capability_not_valid() {
    let table = CapabilityTable::new();
    let null_cap = Capability::null();
    
    assert!(!table.validate(&null_cap));
}
