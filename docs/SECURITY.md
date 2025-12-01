# Security Model

Intent Kernel's capability-based security architecture.

---

## Overview

Intent Kernel implements a **capability-based security model**, a fundamental departure from traditional access control lists (ACLs). Every resource access requires presenting an unforgeable capability token.

### Why Capabilities?

| Traditional ACL | Capability-Based |
|-----------------|------------------|
| "Can user X access resource Y?" | "Does this code have a capability for Y?" |
| Identity-centric | Authority-centric |
| Ambient authority | Explicit authority |
| Confused deputy problem | Prevented by design |
| Privilege escalation attacks | Minimal attack surface |

---

## Core Principles

### 1. No Ambient Authority

Code cannot access any resource without explicitly possessing a capability. There are no "root" privileges or special modes that bypass capability checks.

```rust
// This is NOT how Intent Kernel works:
// if user.is_admin() { access_resource(); }

// This IS how Intent Kernel works:
fn access_resource(cap: &Capability) -> Result<(), Error> {
    capability::require(cap, Permissions::READ)?;
    // Only now can we access
}
```

### 2. Unforgeable Tokens

Capabilities cannot be created from thin air. They must be:
- Created by the kernel during boot (root capabilities)
- Derived from an existing capability (with reduced permissions)
- Received from another holder (delegation)

### 3. Principle of Least Privilege

Each component receives only the capabilities it needs, nothing more.

```rust
// Driver receives only the memory range it needs
let driver_cap = derive(
    &root_memory_cap,
    ResourceType::Memory { 
        base: UART_BASE, 
        size: 0x1000 
    },
    Permissions::READ | Permissions::WRITE
);
```

### 4. Transitive Revocation

Revoking a capability automatically revokes all derived capabilities.

```
Parent Capability
    ├── Child 1 ──→ revoked
    ├── Child 2 ──→ revoked
    │   └── Grandchild ──→ revoked
    └── Child 3 ──→ revoked
```

---

## Capability Structure

```rust
pub struct Capability {
    /// Unique identifier (cryptographically secure)
    pub id: u64,
    
    /// What resource this capability grants access to
    pub resource: ResourceType,
    
    /// What operations are permitted
    pub permissions: Permissions,
    
    /// Process/entity that owns this capability
    pub owner: u64,
    
    /// Parent capability (for revocation chain)
    pub parent: Option<u64>,
    
    /// When this capability was created
    pub created_at: u64,
    
    /// Optional expiration time
    pub expires_at: Option<u64>,
}
```

---

## Resource Types

### Memory

```rust
ResourceType::Memory { 
    base: usize,  // Physical address
    size: usize   // Region size
}
```

Controls access to physical memory regions. Essential for:
- DMA buffers
- Memory-mapped I/O
- Shared memory between processes

### Device

```rust
ResourceType::Device { 
    device_id: u32  // Hardware device identifier
}
```

Controls access to hardware devices:
- `0x01` - GPIO controller
- `0x02` - UART
- `0x03` - Timer
- `0x04` - Interrupt controller
- `0x05` - Mailbox
- `0x06` - Framebuffer

### Interrupt

```rust
ResourceType::Interrupt { 
    irq: u32  // Interrupt number
}
```

Controls ability to:
- Register interrupt handlers
- Enable/disable specific interrupts
- Acknowledge interrupts

### Port (Future)

```rust
ResourceType::Port { 
    port: u16  // Port number
}
```

For network/communication port access.

### File (Future)

```rust
ResourceType::File { 
    inode: u64  // File identifier
}
```

File system access control.

### Process (Future)

```rust
ResourceType::Process { 
    pid: u32  // Process ID
}
```

Inter-process control and signaling.

### Intent

```rust
ResourceType::Intent { 
    intent_id: u64  // Intent type
}
```

Controls which intents a component can execute.

---

## Permissions

Permissions are bit flags that can be combined:

```rust
pub struct Permissions(u32);

impl Permissions {
    pub const READ: Permissions    = Permissions(0x0001);
    pub const WRITE: Permissions   = Permissions(0x0002);
    pub const EXECUTE: Permissions = Permissions(0x0004);
    pub const GRANT: Permissions   = Permissions(0x0008);
    pub const REVOKE: Permissions  = Permissions(0x0010);
    pub const DERIVE: Permissions  = Permissions(0x0020);
}
```

### Permission Semantics

| Permission | Meaning |
|------------|---------|
| READ | Can read/observe resource |
| WRITE | Can modify resource |
| EXECUTE | Can execute/invoke resource |
| GRANT | Can transfer capability to others |
| REVOKE | Can revoke derived capabilities |
| DERIVE | Can create child capabilities |

### Permission Rules

1. **Monotonic Attenuation**: Derived capabilities can only have equal or fewer permissions
2. **No Amplification**: Cannot increase permissions through derivation
3. **Explicit Transfer**: GRANT permission required to share capabilities

---

## Capability Operations

### Creation

Only the kernel can create root capabilities:

```rust
// During boot, kernel creates root capabilities
let root_mem = CapabilityTable::create_root(
    ResourceType::Memory { base: 0, size: TOTAL_RAM },
    Permissions::all()
);
```

### Derivation

Create a child capability with restricted permissions:

```rust
let uart_cap = table.derive(
    &root_device_cap,
    ResourceType::Device { device_id: UART_DEVICE },
    Permissions::READ | Permissions::WRITE
)?;
```

### Validation

Check if capability grants required access:

```rust
if table.validate(&cap, Permissions::WRITE) {
    // Permitted
} else {
    // Denied
}
```

### Revocation

Revoke a capability and all its descendants:

```rust
table.revoke(&cap);
// cap and all derived capabilities are now invalid
```

### Transfer

Delegate capability to another entity (requires GRANT permission):

```rust
if cap.permissions.contains(Permissions::GRANT) {
    let transferred = table.transfer(&cap, new_owner)?;
}
```

---

## Security Guarantees

### Isolation

Components cannot access resources they don't have capabilities for:

```
┌─────────────────┐     ┌─────────────────┐
│   Component A   │     │   Component B   │
│                 │     │                 │
│  Caps: [1, 2]   │     │  Caps: [3, 4]   │
└────────┬────────┘     └────────┬────────┘
         │                       │
    ┌────▼────┐             ┌────▼────┐
    │  Cap 1  │             │  Cap 3  │
    │  Cap 2  │             │  Cap 4  │
    └─────────┘             └─────────┘
    (Resources A)           (Resources B)
```

### Confinement

A component can only grant capabilities it possesses:

```rust
// Component has cap with READ permission
let my_cap = Capability { permissions: READ, .. };

// Cannot create cap with WRITE permission
let attempt = derive(&my_cap, Permissions::READ | Permissions::WRITE);
// Error: cannot amplify permissions
```

### Revocation Cascade

```rust
let root = create_root(...);
let child = derive(&root, ...);
let grandchild = derive(&child, ...);

revoke(&root);
// child: invalid
// grandchild: invalid
```

### Time-Bounded Capabilities

```rust
let timed_cap = Capability {
    expires_at: Some(current_time() + 3600), // 1 hour
    ..cap
};

// After 1 hour
if timed_cap.is_expired() {
    // Access denied
}
```

    // Access denied
}
```

### Polymorphic Kernel

Intent Kernel implements advanced runtime randomization to mitigate memory corruption exploits.

#### 1. Hardware Random Number Generator (TRNG)
- **Source**: BCM2712 True Random Number Generator.
- **Usage**: Seeds the kernel's entropy pool at boot.
- **Trust**: Used as the root of trust for all randomization features.

#### 2. Heap ASLR (Address Space Layout Randomization)
- **Mechanism**: The kernel heap base address is randomized at boot.
- **Entropy**: Up to 25% of the total heap size is used as a random offset.
- **Impact**: Makes absolute address prediction for heap objects impossible for attackers.

#### 3. Pointer Guard
- **Mechanism**: Capability resource pointers are encrypted in memory.
- **Key**: A random 64-bit key (`POINTER_KEY`) generated at boot.
- **Operation**: `stored_ptr = raw_ptr ^ POINTER_KEY`.
- **Impact**: Prevents attackers from forging capabilities by simply writing addresses to memory or reading valid pointers from dumps.

---

## Attack Prevention

### Confused Deputy

**Problem**: A privileged program tricked into misusing its authority.

**Solution**: Capabilities must be explicitly passed for each operation.

```rust
// Compiler saves to file given by user
fn save_file(filename: &str) {
    // BAD: Uses compiler's ambient authority
    // file::write(filename, data);
    
    // GOOD: User must provide capability
    fn save_file(file_cap: &Capability, data: &[u8]) {
        capability::require(file_cap, Permissions::WRITE)?;
        // Now safe - user authorized this specific file
    }
}
```

### Privilege Escalation

**Problem**: Gaining higher privileges than initially granted.

**Solution**: Capabilities can only be attenuated, never amplified.

```rust
let readonly = derive(&readwrite, Permissions::READ);
// readonly cannot be upgraded back to readwrite
```

### TOCTOU (Time-of-Check to Time-of-Use)

**Problem**: Resource state changes between check and use.

**Solution**: Capability validation and use are atomic operations.

```rust
fn validated_access(cap: &Capability) {
    // Atomic: validate + access
    let guard = capability::acquire(cap)?;
    // Resource locked while guard held
}
```

---

## Integration with Intent Engine

The intent engine bridges natural language to capabilities:

```
User: "read the temperature"
         │
         ▼
┌─────────────────────────────┐
│     Intent Parser           │
│ Intent::ReadTemperature     │
└─────────────────────────────┘
         │
         ▼
┌─────────────────────────────┐
│   Capability Resolution     │
│ Find cap for mailbox device │
└─────────────────────────────┘
         │
         ▼
┌─────────────────────────────┐
│   Permission Check          │
│ cap.has(Permissions::READ)? │
└─────────────────────────────┘
         │
         ▼
┌─────────────────────────────┐
│   Execute with Capability   │
│ mailbox::get_temperature()  │
└─────────────────────────────┘
         │
         ▼
    Response: "42°C"
```

---

## Future Enhancements

### Cryptographic Capabilities

Sign capabilities with kernel key:

```rust
struct SignedCapability {
    cap: Capability,
    signature: [u8; 64], // Ed25519 signature
}
```

### Network Capabilities

Transmit capabilities across network boundaries:

```rust
let remote_cap = network::receive_capability()?;
if verify_remote_trust(remote_cap.source) {
    accept_capability(remote_cap);
}
```

### Hardware-Backed Capabilities

Store capabilities in TrustZone secure world:

```rust
#[secure_world]
fn store_capability(cap: Capability) {
    // Capability stored in secure memory
}
```

---

## Best Practices

1. **Minimal Capabilities**: Request only what you need
2. **Short Lifetimes**: Use expiring capabilities when possible
3. **Explicit Passing**: Always pass capabilities explicitly
4. **Prompt Revocation**: Revoke capabilities when no longer needed
5. **Audit Trail**: Log capability operations for security analysis

---

*Security Model v0.1.0 - Intent Kernel*
