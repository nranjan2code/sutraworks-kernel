# Intent Kernel - Copilot Instructions

You are working on **Intent Kernel**, a bare-metal operating system for Raspberry Pi 5.

## Core Philosophy
1.  **Forward Looking**: Prefer modern, AI-native abstractions over traditional OS concepts.
    *   ❌ No Files, No Processes, No Shell.
    *   ✅ Capabilities, Intents, Vector Embeddings.
2.  **Pure Rust**: Zero external dependencies. No `libc`. No crates.
    *   Everything must be implemented from scratch or using `core`.
3.  **Green Computing**: The kernel must sleep (`wfi`) when idle. Avoid busy loops.

## Coding Standards
-   **No Std**: Always use `#![no_std]`.
-   **Alloc**: Use `alloc::` types (`Vec`, `Box`, `Arc`) sparingly.
-   **Async**: Use `async/await` for all I/O. The kernel is reactive.
-   **Safety**:
    -   Mark `unsafe` blocks clearly.
    -   Use `SpinLock` for shared mutable state.
    -   Prefer `Capability` tokens over raw pointers.

## Architecture
-   **Intent Engine**: Uses `ConceptID` (u64 hash) and `Embedding` (vector) for understanding.
-   **Async Core**: Uses a custom `Executor` and `Waker` system wired to GIC-400 interrupts.
-   **Drivers**: Located in `kernel/src/drivers/`. Must be interrupt-driven where possible.

## Common Patterns

### Async Driver Method
```rust
pub async fn read_async(&self) -> u8 {
    // Register waker
    // Enable interrupt
    // Return Pending until IRQ fires
}
```

### Capability Check
```rust
if !validate(&cap) {
    return Err("Permission denied");
}
```
