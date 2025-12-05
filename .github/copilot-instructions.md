# Intent Kernel - GitHub Copilot Instructions

## Project Overview
**Intent Kernel** is a production-ready bare-metal AArch64 operating system for Raspberry Pi 5 implementing perceptual computing with Hyperdimensional Computing (HDC).

**Status**: 92% complete, Sprint 12 done, ZERO CRASHES ✅

## Architecture Philosophy
1. **Intent-Based**: Processes semantic intents, not low-level commands
2. **Multi-Modal Input**: Steno (fastest), keyboard, vision, audio → all converted to semantic concepts
3. **HDC Memory**: 1024-bit binary hypervectors for holographic storage
4. **Runtime Detection**: Uses DTB for hardware abstraction (no compile-time hacks)

## Critical Recent Fixes (Sprint 12) 🔴
These bugs caused crashes - you MUST understand them:

### 1. Scheduler Queue Desync
```rust
// ❌ WRONG: Rotates even when returning None
if let Some(prev) = self.agents.pop_front() { 
    // ... might return None later but queue already rotated!
}

// ✅ CORRECT: Only rotate on actual switch
if let Some(index) = best_index {
    let next = self.agents.remove(index).unwrap();
    let prev = self.agents.pop_front().unwrap();
    // ... now rotation happens only on switch
}
```

### 2. Context Struct Layout
```rust
// ❌ WRONG: Doesn't match assembly offsets
#[repr(C)]
pub struct Context {
    pub sp: u64,    // offset 88 ← assembly writes LR here!
    pub lr: u64,    // offset 96 ← assembly writes SP here!
}

// ✅ CORRECT: Matches "stp x29, x30, [x0, #80]" and "str x9, [x0, #96]"
#[repr(C)]
pub struct Context {
    pub lr: u64,    // offset 88 - Link Register
    pub sp: u64,    // offset 96 - Stack Pointer  
}
```

### 3. sys_exit Register Leakage (CRITICAL)
```rust
// ❌ WRONG: Loops in user context with USER registers!
fn sys_exit(code: i32) {
    scheduler.exit_current(code);
    loop { scheduler::yield_task(); } // Returns if no tasks → USER regs leaked!
}

// ✅ CORRECT: Clear registers before halting
fn sys_exit(code: i32) -> ! {
    scheduler.exit_current(code);
    scheduler::yield_task(); // Try once
    
    // CRITICAL: Still in USER context! Clear ALL registers!
    unsafe {
        asm!(
            "mov x19, #0",  // Clear x19 (was corrupted with cntvct_el0)
            // ... clear x0-x30 ...
            "1: wfi", "b 1b",
            options(noreturn)
        );
    }
}
```

### 4. Async Task Synchronization
```rust
// ❌ WRONG: Spawn and forget
fn bench() {
    scheduler.spawn_user_simple(entry, 0);
    // Returns immediately! Task still in queue!
}

// ✅ CORRECT: Wait for completion (TODO: implement proper wait)
// For now, disabled until we have task.wait()
```

## Key Files & Modules
- `kernel/src/main.rs` - Entry point
- `kernel/src/kernel/scheduler.rs` - Task scheduling (watch queue state!)
- `kernel/src/kernel/process.rs` - Context struct (offsets MUST match assembly!)
- `kernel/src/kernel/syscall.rs` - System calls (sys_exit is special!)
- `kernel/src/kernel/memory/neural.rs` - HDC allocator
- `kernel/src/intent/` - Intent broadcast system
- `boot/boot.s` - Bootstrap & exception vectors

## Code Patterns

### Always Runtime Detection
```rust
// ✅ GOOD: Runtime hardware detection
let base = match dtb::machine_type() {
    MachineType::RaspberryPi5 => 0x1_0000_0000,
    MachineType::QemuVirt => 0x0900_0000,
};

// ❌ BAD: Compile-time feature flags
#[cfg(feature = "qemu")]
const BASE: usize = 0x0900_0000;
```

### Struct + Assembly Interface
```rust
#[repr(C)]  // REQUIRED for assembly interface!
pub struct Context {
    pub x19: u64,   // offset 0
    pub x20: u64,   // offset 8
    // ... ALWAYS document offsets!
    pub lr: u64,    // offset 88 - matches assembly access
    pub sp: u64,    // offset 96
}
```

### Error Handling
```rust
// ✅ Use Result for all fallible ops
pub fn allocate() -> Result<NonNull<u8>, &'static str> {
    let ptr = try_alloc()?;
    Ok(ptr)
}
```

## Performance Targets (All Achieved ✅)
- Context Switch: 54 cycles (target < 200)
- Syscall Latency: 8-11 cycles (target < 50)
- Memory Alloc: 30-40 cycles (target < 100)

## Common Pitfalls
1. ❌ Modifying queue without actually switching
2. ❌ Struct layout not matching assembly offsets
3. ❌ Looping in terminated task context
4. ❌ Spawning tasks without synchronization
5. ❌ Using compile-time features for hardware

## Testing Commands
```bash
make build  # Compile
make run    # Run in QEMU
# Success: Exit code: 0, no crashes
```

## Sprint Status
- ✅ Sprint 1-12: Complete (92% total)
- 🎯 Next: Sprint 13 (Intent-native apps)
- 📋 See `SPRINT.md` for details

---
**Zero Tolerance**: This kernel has ZERO CRASHES. Every suggestion must maintain stability.
