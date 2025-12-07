# Developer Guide: Adding Kernel Intents

This guide explains how to add new semantic capabilities ("Intents") to the Intent Kernel.

## The Semantic Path
In this kernel, an "Intent" is the atomic unit of work. Whether triggered by English commands, Steno chords, or Agents, the execution path is:

`Input (English/Steno) -> ConceptID -> Intent Registry -> Handler`

## Step 1: Define the Concept
All intents start with a ConceptID in `kernel/src/steno/concepts.rs`.

```rust
// kernel/src/steno/concepts.rs
pub const MY_NEW_ACTION: ConceptID = ConceptID::new(0xABCD_1234);
```

## Step 2: Register the Intent
Map the ConceptID to a human-readable name and an execution handler in `kernel/src/intent/mod.rs` (or a subsystem specific module).

```rust
// kernel/src/intent/mod.rs
pub fn init() {
    // ...
    register(concepts::MY_NEW_ACTION, "MyNewAction", handle_my_action);
}

fn handle_my_action(_intent: &Intent) {
    kprintln!("[INTENT] Executing My Action!");
    // Your logic here
}
```

## Step 3: Add English Triggers
Add natural language phrases to trigger this intent in `kernel/src/english/phrases.rs`.

```rust
// kernel/src/english/phrases.rs
pub static PHRASES: &[(&str, ConceptID)] = &[
    // ...
    ("do the thing",  concepts::MY_NEW_ACTION),
    ("perform action", concepts::MY_NEW_ACTION),
];
```

## Step 4: Add Steno Triggers (Optional)
If you want a dedicated steno chord, add it to `kernel/src/steno/dictionary.rs`.

```rust
// kernel/src/steno/dictionary.rs
pub static DICTIONARY: &[(&str, ConceptID)] = &[
    // ...
    ("DO-T", concepts::MY_NEW_ACTION), // "DO-T" chord
];
```

## Checklist
- [ ] ConceptID defined?
- [ ] Handler registered?
- [ ] English phrase mapped?
- [ ] (Optional) Steno chord mapped?
- [ ] (Optional) Steno chord mapped?

## Step 5: Declarative Intents (Sprint 14) ✨
You can now define intents without writing Rust code using **App Manifests**.

Create a `.intent` file:

```yaml
app_name: "My App"
triggers:
  - input: "Do the thing"
flow:
  - goal: "Perform Action"
```

The **Semantic Linker** will automatically bind "Perform Action" to the best available Skill at runtime. This allows you to build complex flows purely by describing *what* you want.

## Step 6: Dynamic Intents (Sprint 15) 🚀
Processes can dynamicall register as intent handlers:

1.  **Process**: Calls `sys_announce(MY_CONCEPT)`.
2.  **Kernel**: Binds `MY_CONCEPT` -> `PID`.
3.  **User**: Types command related to `MY_CONCEPT`.
4.  **Kernel**: Automatically routes message to Process via IPC.

No kernel recompilation required!
