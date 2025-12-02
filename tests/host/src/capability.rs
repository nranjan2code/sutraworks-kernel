//! Capability - Permission system (Test-friendly implementation)
//!
//! This module mirrors the kernel's capability.rs but with std support for testing.

use std::sync::atomic::{AtomicU64, Ordering};

/// Type of resource a capability grants access to
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CapabilityType {
    Null = 0,
    Memory = 1,
    Device = 2,
    Interrupt = 3,
    Timer = 4,
    Display = 5,
    Compute = 6,
    Network = 7,
    Storage = 8,
    Input = 9,
    Intent = 10,
    CapabilityControl = 11,
    System = 12,
}

/// Permission flags for capabilities
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Permissions(u32);

impl Permissions {
    pub const NONE: Permissions = Permissions(0);
    pub const READ: Permissions = Permissions(1 << 0);
    pub const WRITE: Permissions = Permissions(1 << 1);
    pub const EXECUTE: Permissions = Permissions(1 << 2);
    pub const DELETE: Permissions = Permissions(1 << 3);
    pub const SHARE: Permissions = Permissions(1 << 4);
    pub const DELEGATE: Permissions = Permissions(1 << 5);
    pub const REVOKE: Permissions = Permissions(1 << 6);
    
    pub const ALL: Permissions = Permissions(0x7F);
    pub const READ_WRITE: Permissions = Permissions(0x03);
    pub const READ_EXECUTE: Permissions = Permissions(0x05);
    
    /// Check if permission is set
    pub const fn has(self, perm: Permissions) -> bool {
        (self.0 & perm.0) == perm.0
    }
    
    /// Combine permissions
    pub const fn or(self, other: Permissions) -> Permissions {
        Permissions(self.0 | other.0)
    }
    
    /// Intersect permissions (for delegation)
    pub const fn and(self, other: Permissions) -> Permissions {
        Permissions(self.0 & other.0)
    }
    
    /// Remove permissions
    pub const fn without(self, other: Permissions) -> Permissions {
        Permissions(self.0 & !other.0)
    }
    
    /// Get raw value
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// A capability handle
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Capability {
    pub id: u64,
    pub generation: u64,
    pub cap_type: CapabilityType,
    pub permissions: Permissions,
    pub resource: u64,
    pub size: u64,
}

impl Capability {
    /// Create a null capability
    pub const fn null() -> Self {
        Capability {
            id: 0,
            generation: 0,
            cap_type: CapabilityType::Null,
            permissions: Permissions::NONE,
            resource: 0,
            size: 0,
        }
    }
    
    /// Check if capability is valid
    pub fn is_valid(&self) -> bool {
        self.cap_type != CapabilityType::Null && self.id != 0
    }
    
    /// Check if capability has permission
    pub fn has_permission(&self, perm: Permissions) -> bool {
        self.permissions.has(perm)
    }
    
    /// Get capability type
    pub fn cap_type(&self) -> CapabilityType {
        self.cap_type
    }
    
    /// Get resource
    pub fn resource(&self) -> u64 {
        self.resource
    }
    
    /// Get size
    pub fn size(&self) -> u64 {
        self.size
    }
    
    /// Get permissions
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }
}

/// Capability error
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapError {
    WrongType,
    PermissionDenied,
    Revoked,
    OutOfBounds,
    NoCapability,
    OutOfSlots,
}

/// Capability table for testing
pub struct CapabilityTable {
    entries: Vec<CapabilityEntry>,
    next_id: AtomicU64,
    generation: AtomicU64,
    pointer_key: u64,
}

#[derive(Clone)]
struct CapabilityEntry {
    cap: Capability,
    parent_id: u64,
    revoked: bool,
}

impl CapabilityTable {
    /// Create a new capability table
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: AtomicU64::new(1),
            generation: AtomicU64::new(1),
            pointer_key: 0xCAFEBABE_DEADBEEF,
        }
    }
    
    /// Create a new capability table with security seed
    pub fn with_seed(seed: u64) -> Self {
        Self {
            entries: Vec::new(),
            next_id: AtomicU64::new(1),
            generation: AtomicU64::new(1),
            pointer_key: seed ^ 0xCAFEBABE_DEADBEEF,
        }
    }
    
    /// Mint a root capability
    pub fn mint_root(
        &mut self,
        cap_type: CapabilityType,
        resource: u64,
        size: u64,
        permissions: Permissions,
    ) -> Capability {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let gen = self.generation.load(Ordering::SeqCst);
        
        let cap = Capability {
            id,
            generation: gen,
            cap_type,
            permissions,
            resource: resource ^ self.pointer_key, // Encrypt
            size,
        };
        
        self.entries.push(CapabilityEntry {
            cap,
            parent_id: 0,
            revoked: false,
        });
        
        cap
    }
    
    /// Derive a capability from parent
    pub fn derive(&mut self, parent: &Capability, new_permissions: Permissions) -> Option<Capability> {
        if !parent.has_permission(Permissions::DELEGATE) {
            return None;
        }
        
        // Find parent entry
        let parent_entry = self.entries.iter().find(|e| e.cap.id == parent.id)?;
        
        if parent_entry.revoked {
            return None;
        }
        
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let gen = self.generation.load(Ordering::SeqCst);
        
        let derived_perms = parent.permissions.and(new_permissions).without(Permissions::DELEGATE);
        
        let cap = Capability {
            id,
            generation: gen,
            cap_type: parent.cap_type,
            permissions: derived_perms,
            resource: parent.resource,
            size: parent.size,
        };
        
        self.entries.push(CapabilityEntry {
            cap,
            parent_id: parent.id,
            revoked: false,
        });
        
        Some(cap)
    }
    
    /// Revoke a capability and its children
    pub fn revoke(&mut self, cap: &Capability) -> bool {
        if !cap.has_permission(Permissions::REVOKE) {
            return false;
        }
        
        self.revoke_recursive(cap.id)
    }
    
    fn revoke_recursive(&mut self, id: u64) -> bool {
        // Find and mark as revoked
        let mut children = Vec::new();
        
        for entry in &mut self.entries {
            if entry.cap.id == id {
                entry.revoked = true;
            }
            if entry.parent_id == id && !entry.revoked {
                children.push(entry.cap.id);
            }
        }
        
        // Revoke children
        for child_id in children {
            self.revoke_recursive(child_id);
        }
        
        true
    }
    
    /// Validate a capability
    pub fn validate(&self, cap: &Capability) -> bool {
        if !cap.is_valid() {
            return false;
        }
        
        if let Some(entry) = self.entries.iter().find(|e| e.cap.id == cap.id) {
            !entry.revoked && entry.cap.generation == self.generation.load(Ordering::SeqCst)
        } else {
            false
        }
    }
    
    /// Global revocation
    pub fn global_revoke(&mut self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.entries.clear();
    }
    
    /// Get decrypted resource address
    pub fn decrypt_resource(&self, cap: &Capability) -> u64 {
        cap.resource ^ self.pointer_key
    }
    
    /// Statistics
    pub fn stats(&self) -> (usize, usize) {
        let active = self.entries.iter().filter(|e| !e.revoked).count();
        let revoked = self.entries.iter().filter(|e| e.revoked).count();
        (active, revoked)
    }
}

impl Default for CapabilityTable {
    fn default() -> Self {
        Self::new()
    }
}
