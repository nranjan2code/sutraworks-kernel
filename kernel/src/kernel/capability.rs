//! Capability-Based Security System
//!
//! Everything in the Intent Kernel is a capability - an unforgeable token
//! that grants specific permissions to specific resources.
//!
//! This replaces traditional Unix-style permissions with fine-grained,
//! transferable, revocable capabilities.

use core::sync::atomic::{AtomicU64, Ordering};
use crate::arch::SpinLock;

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Type of resource a capability grants access to
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CapabilityType {
    /// No capability (null)
    Null = 0,
    
    /// Memory region access
    Memory = 1,
    
    /// Device I/O access
    Device = 2,
    
    /// Interrupt handling
    Interrupt = 3,
    
    /// Timer access
    Timer = 4,
    
    /// Display/framebuffer access
    Display = 5,
    
    /// GPU compute access
    Compute = 6,
    
    /// Network interface
    Network = 7,
    
    /// Storage device
    Storage = 8,
    
    /// Input device (keyboard, mouse, etc.)
    Input = 9,
    
    /// Intent interpreter access
    Intent = 10,
    
    /// Capability management (mint, revoke)
    CapabilityControl = 11,
    
    /// System control (shutdown, reboot)
    System = 12,
}

/// Permission flags for capabilities
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Permissions(u32);

impl Permissions {
    pub const NONE: Permissions = Permissions(0);
    pub const READ: Permissions = Permissions(1 << 0);
    pub const WRITE: Permissions = Permissions(1 << 1);
    pub const EXECUTE: Permissions = Permissions(1 << 2);
    pub const DELETE: Permissions = Permissions(1 << 3);
    pub const SHARE: Permissions = Permissions(1 << 4);      // Can share with others
    pub const DELEGATE: Permissions = Permissions(1 << 5);   // Can create derived caps
    pub const REVOKE: Permissions = Permissions(1 << 6);     // Can revoke derived caps
    
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
}

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY HANDLE
// ═══════════════════════════════════════════════════════════════════════════════

/// Generation counter to detect stale capabilities
static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Pointer Guard Key (Polymorphic Kernel)
/// All capability resource pointers are XORed with this key.
static POINTER_KEY: AtomicU64 = AtomicU64::new(0);

/// Initialize capability security
pub fn init_security(seed: u64) {
    // Mix seed with a constant to ensure it's non-zero
    let key = seed ^ 0xCAFEBABE_DEADBEEF;
    POINTER_KEY.store(key, Ordering::SeqCst);
    crate::kprintln!("[SEC] Pointer Guard initialized");
}

/// A capability handle - the token held by capability owners
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// Unique identifier
    id: u64,
    /// Generation (for revocation detection)
    generation: u64,
    /// Resource type
    cap_type: CapabilityType,
    /// Permissions
    permissions: Permissions,
    /// Resource-specific data (address, device ID, etc.)
    /// ENCRYPTED: This is XORed with POINTER_KEY
    resource: u64,
    /// Resource size/range
    size: u64,
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
    
    /// Get resource address/ID (Decrypted)
    pub fn resource(&self) -> u64 {
        let key = POINTER_KEY.load(Ordering::Relaxed);
        self.resource ^ key
    }
    
    /// Get resource size
    pub fn size(&self) -> u64 {
        self.size
    }
    
    /// Get permissions
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }
    
    /// Derive a new capability with reduced permissions
    pub fn derive(&self, new_permissions: Permissions) -> Option<Capability> {
        if !self.has_permission(Permissions::DELEGATE) {
            return None;
        }
        
        // New permissions cannot exceed original
        let derived_perms = self.permissions.and(new_permissions);
        
        Some(Capability {
            id: next_capability_id(),
            generation: GENERATION.load(Ordering::SeqCst),
            cap_type: self.cap_type,
            permissions: derived_perms.without(Permissions::DELEGATE), // Can't delegate delegated caps
            resource: self.resource,
            size: self.size,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY TABLE
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum capabilities in the system
const MAX_CAPABILITIES: usize = 4096;

/// Capability table entry
#[derive(Clone, Copy)]
struct CapabilityEntry {
    cap: Capability,
    parent_id: u64,       // For revocation chains
    revoked: bool,
}

impl CapabilityEntry {
    const fn empty() -> Self {
        CapabilityEntry {
            cap: Capability::null(),
            parent_id: 0,
            revoked: false,
        }
    }
}

/// Global capability table
static CAP_TABLE_LOCK: SpinLock<()> = SpinLock::new(());
static mut CAP_TABLE: [CapabilityEntry; MAX_CAPABILITIES] = [CapabilityEntry::empty(); MAX_CAPABILITIES];
static NEXT_CAP_ID: AtomicU64 = AtomicU64::new(1);

/// Get next capability ID
fn next_capability_id() -> u64 {
    NEXT_CAP_ID.fetch_add(1, Ordering::SeqCst)
}

/// Find capability entry by ID
fn find_entry(id: u64) -> Option<usize> {
    unsafe {
        for i in 0..MAX_CAPABILITIES {
            if CAP_TABLE[i].cap.id == id {
                return Some(i);
            }
        }
    }
    None
}

/// Find free slot
fn find_free_slot() -> Option<usize> {
    unsafe {
        for i in 0..MAX_CAPABILITIES {
            if CAP_TABLE[i].cap.id == 0 {
                return Some(i);
            }
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Create a new root capability (kernel only)
/// 
/// # Safety
/// This creates an unforgeable capability with full permissions.
/// Only the kernel should call this during initialization.
pub unsafe fn mint_root(
    cap_type: CapabilityType,
    resource: u64,
    size: u64,
    permissions: Permissions,
) -> Option<Capability> {
    let _guard = CAP_TABLE_LOCK.lock();
    
    let slot = find_free_slot()?;
    let id = next_capability_id();
    let gen = GENERATION.load(Ordering::SeqCst);
    
    let key = POINTER_KEY.load(Ordering::Relaxed);
    
    let cap = Capability {
        id,
        generation: gen,
        cap_type,
        permissions,
        resource: resource ^ key, // Encrypt pointer
        size,
    };
    
    CAP_TABLE[slot] = CapabilityEntry {
        cap,
        parent_id: 0,  // Root capability has no parent
        revoked: false,
    };
    
    Some(cap)
}

/// Derive a capability from an existing one
pub fn derive(parent: &Capability, new_permissions: Permissions) -> Option<Capability> {
    let _guard = CAP_TABLE_LOCK.lock();
    
    // Find parent entry
    let parent_slot = find_entry(parent.id)?;
    
    unsafe {
        // Check parent is valid and not revoked
        let parent_entry = &CAP_TABLE[parent_slot];
        if parent_entry.revoked {
            return None;
        }
        
        // Check generation
        if parent_entry.cap.generation != GENERATION.load(Ordering::SeqCst) {
            return None;
        }
        
        // Create derived capability
        let derived = parent_entry.cap.derive(new_permissions)?;
        
        // Store in table
        let slot = find_free_slot()?;
        CAP_TABLE[slot] = CapabilityEntry {
            cap: derived,
            parent_id: parent.id,
            revoked: false,
        };
        
        Some(derived)
    }
}

/// Revoke a capability and all its children
pub fn revoke(cap: &Capability) -> bool {
    if !cap.has_permission(Permissions::REVOKE) {
        return false;
    }
    
    let _guard = CAP_TABLE_LOCK.lock();
    
    unsafe {
        revoke_recursive(cap.id)
    }
}

/// Recursively revoke capability tree
unsafe fn revoke_recursive(id: u64) -> bool {
    // Find and mark as revoked
    if let Some(slot) = find_entry(id) {
        CAP_TABLE[slot].revoked = true;
        
        // Find and revoke all children
        for i in 0..MAX_CAPABILITIES {
            if CAP_TABLE[i].parent_id == id && !CAP_TABLE[i].revoked {
                revoke_recursive(CAP_TABLE[i].cap.id);
            }
        }
        
        true
    } else {
        false
    }
}

/// Validate a capability is still valid
pub fn validate(cap: &Capability) -> bool {
    if !cap.is_valid() {
        return false;
    }
    
    let _guard = CAP_TABLE_LOCK.lock();
    
    if let Some(slot) = find_entry(cap.id) {
        unsafe {
            let entry = &CAP_TABLE[slot];
            
            // Check not revoked
            if entry.revoked {
                return false;
            }
            
            // Check generation matches
            if entry.cap.generation != GENERATION.load(Ordering::SeqCst) {
                return false;
            }
            
            true
        }
    } else {
        false
    }
}

/// Global revocation - invalidates ALL capabilities
/// 
/// # Safety
/// This is a drastic action that should only be done during system reset
pub unsafe fn global_revoke() {
    let _guard = CAP_TABLE_LOCK.lock();
    
    // Increment generation to invalidate all existing caps
    GENERATION.fetch_add(1, Ordering::SeqCst);
    
    // Clear table
    for i in 0..MAX_CAPABILITIES {
        CAP_TABLE[i] = CapabilityEntry::empty();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CAPABILITY-PROTECTED OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Memory capability operations
pub mod memory {
    use super::*;
    
    /// Read from memory using capability
    pub fn read(cap: &Capability, offset: u64, buf: &mut [u8]) -> Result<usize, CapError> {
        if cap.cap_type() != CapabilityType::Memory {
            return Err(CapError::WrongType);
        }
        
        if !cap.has_permission(Permissions::READ) {
            return Err(CapError::PermissionDenied);
        }
        
        if !validate(cap) {
            return Err(CapError::Revoked);
        }
        
        // Bounds check
        let read_len = buf.len() as u64;
        if offset + read_len > cap.size() {
            return Err(CapError::OutOfBounds);
        }
        
        // Perform read
        let src = (cap.resource() + offset) as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), buf.len());
        }
        
        Ok(buf.len())
    }
    
    /// Write to memory using capability
    pub fn write(cap: &Capability, offset: u64, buf: &[u8]) -> Result<usize, CapError> {
        if cap.cap_type() != CapabilityType::Memory {
            return Err(CapError::WrongType);
        }
        
        if !cap.has_permission(Permissions::WRITE) {
            return Err(CapError::PermissionDenied);
        }
        
        if !validate(cap) {
            return Err(CapError::Revoked);
        }
        
        // Bounds check
        let write_len = buf.len() as u64;
        if offset + write_len > cap.size() {
            return Err(CapError::OutOfBounds);
        }
        
        // Perform write
        let dst = (cap.resource() + offset) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), dst, buf.len());
        }
        
        Ok(buf.len())
    }
}

/// Device capability operations
pub mod device {
    use super::*;
    
    /// Read device register
    pub fn read_reg(cap: &Capability, offset: u64) -> Result<u32, CapError> {
        if cap.cap_type() != CapabilityType::Device {
            return Err(CapError::WrongType);
        }
        
        if !cap.has_permission(Permissions::READ) {
            return Err(CapError::PermissionDenied);
        }
        
        if !validate(cap) {
            return Err(CapError::Revoked);
        }
        
        if offset >= cap.size() {
            return Err(CapError::OutOfBounds);
        }
        
        let addr = (cap.resource() + offset) as *const u32;
        Ok(unsafe { core::ptr::read_volatile(addr) })
    }
    
    /// Write device register
    pub fn write_reg(cap: &Capability, offset: u64, value: u32) -> Result<(), CapError> {
        if cap.cap_type() != CapabilityType::Device {
            return Err(CapError::WrongType);
        }
        
        if !cap.has_permission(Permissions::WRITE) {
            return Err(CapError::PermissionDenied);
        }
        
        if !validate(cap) {
            return Err(CapError::Revoked);
        }
        
        if offset >= cap.size() {
            return Err(CapError::OutOfBounds);
        }
        
        let addr = (cap.resource() + offset) as *mut u32;
        unsafe { core::ptr::write_volatile(addr, value) };
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Capability error
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapError {
    /// Capability has wrong type for operation
    WrongType,
    /// Permission denied
    PermissionDenied,
    /// Capability has been revoked
    Revoked,
    /// Access out of bounds
    OutOfBounds,
    /// No capability available
    NoCapability,
    /// System is out of capability slots
    OutOfSlots,
}

// ═══════════════════════════════════════════════════════════════════════════════
// INITIALIZATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize capability system
pub fn init() {
    // Table is already zero-initialized
    // Just ensure generation starts at 1
    GENERATION.store(1, Ordering::SeqCst);
}

/// Statistics
pub struct CapabilityStats {
    pub active: usize,
    pub revoked: usize,
}

/// Get capability statistics
pub fn stats() -> CapabilityStats {
    let _guard = CAP_TABLE_LOCK.lock();
    
    let mut active = 0;
    let mut revoked = 0;
    
    unsafe {
        for i in 0..MAX_CAPABILITIES {
            if CAP_TABLE[i].cap.id != 0 {
                if CAP_TABLE[i].revoked {
                    revoked += 1;
                } else {
                    active += 1;
                }
            }
        }
    }
    
    CapabilityStats { active, revoked }
}
