# Developer Guide: Writing Userspace Agents

This guide explains how to write "User Mode" programs for the Intent Kernel.

## Philosophy
In the Intent Kernel, User Mode apps are **Agents**, not Controllers.
- **Controllers** (Traditional): "Read keyboard", "Draw to screen", "Manage hardware".
- **Agents** (Intent-Native): "Request intent", "Calculate", "Sleep", "Wait for signal".

The Kernel handles all Input/Output (Input Loop, Rendering). Your agent simply exists to perform logic or trigger intents.

## ⚠️ Anti-Patterns and Prohibitions

> [!CAUTION]
> **STRICT RULE: NO HYBRID LOGIC**
> User-space agents MUST NOT implement command parsing logic that bypasses the Intent System.

### ❌ The Anti-Pattern (Forbidden)
Do NOT parse strings locally to "fast path" execution. This hides intent from the system.
```rust
// user/init/src/main.rs - DO NOT DO THIS
let input = readline();
if input == "ls" {
    // This bypasses the NLU and Security layers!
    run_internal_ls();
}
```

### ✅ The Correct Pattern
Forward all natural language to the kernel. Let the system resolve it.
```rust
// user/init/src/main.rs
let input = readline();
// The kernel decides what "ls" means (LIST_FILES concept)
sys_parse_intent(input);
```

## Anatomy of an Agent
An agent is a `no_std` Rust binary.

```rust
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        // 1. Perform work
        let result = complex_calculation();
        
        // 2. Report status (debug print)
        print("Calculation complete\n");
        
        // 3. Trigger Semantic Intent (if needed)
        parse_intent("notify result ready");
        
        // 4. Yield/Sleep (Be a good citizen)
        sleep(1000);
    }
}
```

## Available Syscalls
| Syscall | Description | Usage |
|---------|-------------|-------|
| `SYS_PRINT` | Debug output to kernel console | `print("msg")` |
| `SYS_SLEEP` | Sleep for N milliseconds | `sleep(ms)` |
| `SYS_YIELD` | Yield timeslice manually | `yield_now()` |
| `SYS_PARSE_INTENT` | Trigger a kernel intent | `parse_intent("cmd")` |
| `SYS_SPAWN` | Spawn process | `spawn("name")` |
| `SYS_IPC_SEND` | Send message to PID | `ipc_send(pid, msg)` |
| `SYS_IPC_RECV` | Blocking receive | `ipc_recv(&mut buf)` |
| `SYS_ANNOUNCE` | Register capability | `announce(CONCEPT_ID)` |
| `SYS_BIND_UDP` | Bind UDP port | `syscall1(23, port)` |
| `SYS_RECVFROM` | Receive UDP packet | `syscall4(24, fd, ...)` |

## Service Agent Pattern
To create a background service that handles intents:

```rust
fn main() {
    // 1. Announce Capability
    sys_announce(concepts::INCREMENT); // 0xA0001
    
    // 2. Event Loop
    let mut buf = [0u8; 64];
    loop {
        // Block until message arrives
        sys_ipc_recv(&mut buf);
        
        // Handle Action
        handle_increment();
    }
}
```

## Building
Agents are compiled as unrelated binaries:
```bash
cargo build --release --target aarch64-unknown-none
```
The resulting ELF is loaded by the kernel at boot.
