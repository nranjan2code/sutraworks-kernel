# AGENT INSTRUCTIONS: The Intent Constitution

> [!CAUTION]
> **READ THIS BEFORE MODIFYING ANY CODE.**
> This document defines the inviolable architectural rules of the Intent Kernel. Violating these rules will break the system's core philosophy and cause "Ghost Binary" issues, logical deadlocks, and severe technical debt.

## 1. The Prime Directive: PURE INTENT ONLY

**Rule**: ALL user actions, commands, and inputs MUST be processed by the Intent System (Neural Engine).

**Forbidden**:
- ❌ Do NOT implement "hybrid" command handling (e.g., checking `if cmd == "ls"` in the shell).
- ❌ Do NOT bypass the `EnglishParser` or `StenoDictionary` for "speed" or "simplicity".
- ❌ Do NOT hardcode command logic in User Space binaries (`user/init`, `user/cli`).

**Required**:
- ✅ The Shell (`user/init`) is a **DUMB FORWARDER**. It takes string input and sends `SYS_PARSE_INTENT` (Syscall 22). It knows NOTHING about what the string means.
- ✅ All logic resides in **Kernel Intent Handlers** or **Registered Capabilities**.

### Why?
The Intent Kernel is a **Semantic Operating System**. If you handle "ls" in the shell, you break the system's ability to:
1.  Understand context (e.g., "list" might mean "list files" or "list devices" depending on context).
2.  Learn user habits (Neural Weights update on intent execution).
3.  Provide uniform security (Centralized Intent Security checks).

---

## 2. The User Space Pattern

When writing user-space code (`user/*`), follow this pattern strictly:

### ❌ The Anti-Pattern (DO NOT DO THIS)
```rust
// user/init/src/main.rs
let input = readline();
if input == "ls" {
    // VIOLATION: Bypassing the kernel's brain!
    let files = sys_getdents(...); 
    print_files(files);
} else {
    sys_parse_intent(input);
}
```

### ✅ The Correct Pattern
```rust
// user/init/src/main.rs
let input = readline();
// CORRECT: The kernel decides what "ls" means.
sys_parse_intent(input); 
```

---

## 3. Implementation Checklist

Before you write a single line of code, answer these questions:

1.  **Does this feature have a ConceptID?**
    - If NO: Define it in `kernel/src/steno/dictionary.rs`.
    - Example: `LIST_FILES = 0x0008_0005`.

2.  **Does it have a Natural Language mapping?**
    - If NO: Add it to `kernel/src/english/parser.rs`.
    - Example: "ls" -> `LIST_FILES`.

3.  **Is logic implemented as a Handler/Skill?**
    - If NO: Create a `IntentHandler` in the kernel OR a `ProcessSkill` in user space that responds to the ConceptID.

---

## 4. Debugging & Verification

If "nothing happens" when you type a command:
1.  **Check the ConceptID**: Is the parser correctly identifying the intent? (Enable `[INTENT]` logs).
2.  **Check the Dispatch**: Is `IntentExecutor` finding a handler?
3.  **Check the Syscall**: Is User Space sending Syscall 22?

**NEVER** fix a bug by hardcoding a bypass. Fix the pipeline.
