# Intent Kernel - Copilot Instructions

You are working on **Intent Kernel**, a bare-metal stenographic operating system for Raspberry Pi 5.

## Core Philosophy
1.  **Strokes, Not Characters**: This is a **stenographic kernel**. The native input unit is a steno stroke (23-bit binary pattern), not characters or words.
    *   ❌ No character parsing. No word tokenization. No NLP.
    *   ✅ Strokes → Intents (DIRECT mapping via dictionary).
2.  **Dual Input Mode**: Users can input steno strokes OR English text.
    *   English is converted to strokes via reverse dictionary lookup.
    *   The kernel remains steno-native internally.
    *   Example: `"help"` → finds stroke `PH-FPL` → executes `HELP` intent.
3.  **Pure Rust**: Zero external dependencies. No `libc`. Minimal crates.
    *   Everything must be implemented from scratch or using `core`.
4.  **Green Computing**: The kernel must sleep (`wfi`) when idle. Avoid busy loops.
5.  **No Backward Compatibility**: We are building the future, not preserving the past.

## Stenographic Architecture

The kernel uses the **Plover steno layout** (23 keys):
```
Key Order: #, S-, T-, K-, P-, W-, H-, R-, A-, O-, *, -E, -U, -F, -R, -P, -B, -L, -G, -T, -S, -D, -Z
Bit:       0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22
```

**Primary Flow (Steno)**:
```
Steno Machine → Stroke (23-bit) → Dictionary Lookup → Intent → Executor
```

**Secondary Flow (English)**:
```
Keyboard → English Word → Reverse Lookup → Stroke → Intent → Executor
```

No tokenization. No parsing. No embeddings. Pure stroke→intent mapping.

## Coding Standards
-   **No Std**: Always use `#![no_std]`.
-   **Alloc**: Use `alloc::` types (`Vec`, `Box`, `Arc`) sparingly.
-   **Async**: Use `async/await` for all I/O. The kernel is reactive.
-   **Safety**:
    -   Mark `unsafe` blocks clearly.
    -   Use `SpinLock` for shared mutable state.
    -   Prefer `Capability` tokens over raw pointers.

## Key Modules
-   **steno/**: Stenographic input engine
    -   `stroke.rs`: `Stroke` struct (23-bit), RTFCRE conversion
    -   `dictionary.rs`: Stroke→Intent mapping, reverse lookup, `concepts` module
    -   `engine.rs`: `StenoEngine` processes strokes
-   **intent/**: Intent execution
    -   `ConceptID`: 64-bit semantic identifier
    -   `Intent`: Result of stroke processing
    -   `IntentExecutor`: Executes intents with capability checks
-   **drivers/**: Hardware drivers
    -   `uart.rs`: Serial I/O
    -   `framebuffer.rs`: Display output
    -   `console.rs`: Text console on framebuffer (`cprint!`, `cprintln!`)
    -   `usb/`: USB HID for steno machines
-   **kernel/**: Core subsystems (memory, scheduler, capabilities)

## Common Patterns

### Processing a Stroke
```rust
// From steno notation
if let Some(intent) = steno::process_steno("STPH") {
    intent::execute(&intent);
}

// From raw bits (hardware)
if let Some(intent) = steno::process_raw(0x7F) {
    intent::execute(&intent);
}

// From English text (reverse lookup)
if let Some(intent) = steno::process_english("help") {
    intent::execute(&intent);
}
```

### Adding Dictionary Entry
```rust
dictionary.add_entry(DictEntry::from_steno(
    "KAT",                        // Steno notation
    ConceptID(0x0008_0001),       // Concept ID
    "CAT"                         // Debug name
));
```

### Capability Check
```rust
if !has_capability(CapabilityType::System) {
    return Err("Permission denied");
}
```

## What NOT To Do
-   ❌ Do not add character/word parsing
-   ❌ Do not add NLP or tokenization
-   ❌ Do not use embeddings or vector similarity
-   ❌ Do not maintain backward compatibility with old code
