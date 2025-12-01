# Next Steps: Unblocking Test Infrastructure

## Immediate Action Required

We have 14 unit tests ready to run, but they're blocked by linking issues. Here's how to proceed:

## Option 1: Quick Win - Mock Assembly Functions (RECOMMENDED)

**Time:** 1 hour  
**Difficulty:** Easy  
**Benefit:** Get tests running immediately

### Implementation

Add these mock functions to `tests/kernel_tests.rs`:

```rust
// Mock assembly functions for testing
#[no_mangle]
pub extern "C" fn read_timer_freq() -> u64 {
    1_000_000 // 1 MHz mock frequency
}

#[no_mangle]
pub extern "C" fn read_timer() -> u64 {
    0 // Mock timer value
}

#[no_mangle]
pub extern "C" fn data_sync_barrier() {
    // Mock barrier - no-op for tests
}

#[no_mangle]
pub extern "C" fn instruction_barrier() {
    // Mock barrier - no-op for tests
}

// Mock linker symbols
#[no_mangle]
pub static __heap_start: u64 = 0x8000_0000;

#[no_mangle]
pub static __heap_end: u64 = 0x9000_0000;

#[no_mangle]
pub static __dma_start: u64 = 0x9000_0000;

#[no_mangle]
pub static __dma_end: u64 = 0xA000_0000;
```

Then run:
```bash
cd kernel
cargo build --target aarch64-unknown-none --test kernel_tests
```

---

## Option 2: Proper Solution - Link with Boot Assembly

**Time:** 4-6 hours  
**Difficulty:** Medium  
**Benefit:** Tests run in realistic environment

### Implementation

1. Create `kernel/build.rs`:
```rust
fn main() {
    println!("cargo:rerun-if-changed=../boot/boot.s");
    
    // Assemble boot.s
    std::process::Command::new("aarch64-none-elf-as")
        .args(&["-march=armv8.2-a", "-o", "target/boot.o", "../boot/boot.s"])
        .status()
        .expect("Failed to assemble boot.s");
    
    // Link boot.o
    println!("cargo:rustc-link-arg=target/boot.o");
}
```

2. Update `Cargo.toml`:
```toml
[build-dependencies]
# None needed - using std::process::Command
```

---

## Option 3: Integration Tests in QEMU

**Time:** 6-8 hours  
**Difficulty:** Hard  
**Benefit:** Tests real kernel behavior

Skip unit tests for now, focus on integration tests that run the full kernel in QEMU.

---

## Recommendation

**Start with Option 1** to get immediate feedback on test quality, then move to Option 2 for production testing.

## Commands to Run

```bash
# After adding mock functions
cd /Users/nisheethranjan/Projects/sutraworks/intent-kernel/kernel
cargo build --target aarch64-unknown-none --test kernel_tests

# If successful
cargo test --target aarch64-unknown-none --test kernel_tests

# Or via Makefile
cd ..
make test-unit
```

## Expected Output

```
Running tests...

memory::test_heap_stats...	[ok]
memory::test_heap_available...	[ok]
memory::test_heap_regions...	[ok]
memory::test_allocator_stats...	[ok]
capability::test_mint_root_capability...	[ok]
capability::test_capability_validation...	[ok]
capability::test_capability_permissions...	[ok]
capability::test_multiple_capabilities...	[ok]
intent::test_concept_id_hashing...	[ok]
intent::test_embedding_creation...	[ok]
intent::test_embedding_similarity_identical...	[ok]
intent::test_embedding_similarity_different...	[ok]
intent::test_neural_memory_basic...	[ok]
intent::test_neural_memory_threshold...	[ok]

╔═══════════════════════════════════════════════════════════╗
║           ALL TESTS PASSED                                ║
╚═══════════════════════════════════════════════════════════╝
```

## Success Criteria

- [ ] Tests compile without errors
- [ ] Tests run in QEMU
- [ ] All 14 tests pass
- [ ] Can add new tests easily
- [ ] CI/CD ready

---

**Ready to proceed?** Let me know if you want me to implement Option 1 (mock functions) to unblock testing!
