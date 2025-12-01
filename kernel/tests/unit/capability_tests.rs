//! Capability System Unit Tests

use intent_kernel::kernel::capability::*;

pub fn test_mint_root_capability() {
    let cap = unsafe {
        mint_root(CapabilityType::Memory, 0x1000, 0x1000, Permissions::READ_WRITE)
    };
    
    assert!(cap.is_some(), "Should be able to mint root capability");
    
    if let Some(cap) = cap {
        assert!(validate(&cap), "Minted capability should be valid");
    }
}

pub fn test_capability_validation() {
    let cap = unsafe {
        mint_root(CapabilityType::Device, 0x2000, 0x100, Permissions::READ)
    }.unwrap();
    
    assert!(validate(&cap), "Valid capability should validate");
}

pub fn test_capability_permissions() {
    // Just test that we can mint capabilities with different permissions
    let cap_read = unsafe {
        mint_root(CapabilityType::Memory, 0x1000, 0x1000, Permissions::READ)
    };
    
    let cap_write = unsafe {
        mint_root(CapabilityType::Memory, 0x2000, 0x1000, Permissions::WRITE)
    };
    
    let cap_all = unsafe {
        mint_root(CapabilityType::Memory, 0x3000, 0x1000, Permissions::ALL)
    };
    
    assert!(cap_read.is_some(), "READ capability should mint");
    assert!(cap_write.is_some(), "WRITE capability should mint");
    assert!(cap_all.is_some(), "ALL capability should mint");
}

pub fn test_multiple_capabilities() {
    // Mint multiple capabilities
    let cap1 = unsafe {
        mint_root(CapabilityType::Memory, 0x1000, 0x1000, Permissions::READ)
    };
    
    let cap2 = unsafe {
        mint_root(CapabilityType::Device, 0x2000, 0x100, Permissions::WRITE)
    };
    
    let cap3 = unsafe {
        mint_root(CapabilityType::Display, 0, 0, Permissions::ALL)
    };
    
    assert!(cap1.is_some(), "First capability should mint");
    assert!(cap2.is_some(), "Second capability should mint");
    assert!(cap3.is_some(), "Third capability should mint");
}
