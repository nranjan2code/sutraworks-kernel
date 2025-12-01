# Next Steps: Expanding Test Coverage

## Current Status ✅

Test infrastructure is **working**! QEMU exits cleanly with semihosting on `virt` machine.

```bash
make test-unit  # Completes in <10 seconds
```

## Limitation

The actual unit tests require Pi 5 hardware initialization (UART, memory addresses) which doesn't work on QEMU's `virt` machine. The current test just verifies the harness works.

## Options to Expand Testing

## Option 1: Host-Based Tests (RECOMMENDED)

**Time:** 2-3 hours  
**Difficulty:** Easy  
**Benefit:** Fast tests, no QEMU, runs on Mac natively

Create a separate test crate that tests pure Rust logic:
- ConceptID hashing
- Embedding similarity math
- Capability permission checks
- Intent parsing logic

```bash
# In a new crate with std support
cargo test  # Runs instantly on Mac
```

---

## Option 2: Hardware Tests on Real Pi 5

**Time:** Hardware setup
**Difficulty:** Medium  
**Benefit:** Tests real kernel behavior with real hardware

1. Flash kernel to SD card
2. Connect UART for test output
3. Run tests on actual Pi 5
4. Capture output via serial

---

## Option 3: QEMU raspi4b with Full Boot

**Time:** 6-8 hours  
**Difficulty:** Hard  
**Benefit:** Tests real kernel boot sequence

Note: raspi4b machine doesn't support semihosting exit, so tests would need to:
- Use UART output to signal pass/fail
- External script parses output and kills QEMU
- Or use watchdog timer approach

---

## Recommendation

**Start with Option 1 (Host-Based Tests)** for fast iteration on pure logic, then use Pi 5 hardware for integration tests.

## Current Working Command

```bash
make test-unit  # Works! Uses QEMU virt machine, exits cleanly
```

## Output

```
=== INTENT KERNEL UNIT TESTS ===
Timeout: 10s

=== TESTS COMPLETED ===

✓ All unit tests passed!
```

## Success Criteria

- [x] Test harness compiles
- [x] QEMU exits cleanly (semihosting works)
- [x] Timeout prevents CPU heating
- [ ] Host-based tests for pure Rust logic
- [ ] Hardware tests on real Pi 5
- [ ] CI/CD pipeline

---

**Current Status:** Test infrastructure working. Ready to expand coverage!
