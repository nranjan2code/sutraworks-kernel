# Contributing to Intent Kernel

Welcome! Intent Kernel is an experimental bare-metal operating system for Raspberry Pi 5. We appreciate your interest in contributing.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Architecture Guidelines](#architecture-guidelines)
- [Testing](#testing)
- [Documentation](#documentation)

---

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help others learn
- No harassment or discrimination

---

## Getting Started

### Understanding the Project

Before contributing, familiarize yourself with:

1. **[README.md](../README.md)** - Project overview
2. **[ARCHITECTURE.md](ARCHITECTURE.md)** - System design
3. **[SECURITY.md](SECURITY.md)** - Capability model
4. **[HARDWARE.md](HARDWARE.md)** - BCM2712 specifics

### Finding Issues

Look for issues labeled:
- `good-first-issue` - Great for newcomers
- `help-wanted` - We need assistance
- `documentation` - Docs improvements
- `bug` - Known problems
- `enhancement` - New features

---

## Development Setup

### Prerequisites

```bash
# macOS
brew install rustup arm-none-eabi-gcc qemu

# Linux (Debian/Ubuntu)
sudo apt install gcc-aarch64-linux-gnu qemu-system-arm

# Install Rust toolchain
rustup install nightly
rustup default nightly
rustup target add aarch64-unknown-none
rustup component add rust-src llvm-tools-preview
```

### Building

```bash
git clone https://github.com/your-username/intent-kernel.git
cd intent-kernel
make
```

### Testing

```bash
# QEMU emulation
make qemu

# With debug output
make qemu-debug
```

---

## Code Style

### Rust

We follow the standard Rust style with some bare-metal specific considerations:

```rust
// Use descriptive names
fn initialize_uart_controller() { }  // Good
fn init_uc() { }                      // Bad

// Document unsafe code
unsafe {
    // SAFETY: Register is memory-mapped at UART_BASE,
    // and we have exclusive access during init
    core::ptr::write_volatile(UART_DR, byte as u32);
}

// Use constants for hardware addresses
const UART_BASE: usize = 0x1_0020_1000;
const UART_DR: usize = UART_BASE + 0x00;
const UART_FR: usize = UART_BASE + 0x18;

// Group related items
mod uart {
    const BASE: usize = 0x1_0020_1000;
    
    mod registers {
        pub const DR: usize = 0x00;
        pub const FR: usize = 0x18;
    }
}
```

### Assembly

```asm
// Use descriptive labels
_start:
primary_cpu_init:
    // Comments explain WHY, not WHAT
    // Disable MMU before configuring - required for identity mapping
    mrs x0, sctlr_el1
    bic x0, x0, #1
    msr sctlr_el1, x0
    
    // Align instructions properly
    .balign 8
```

### Documentation Comments

```rust
/// Initialize the UART controller for serial communication.
/// 
/// # Configuration
/// - Baud rate: 115200
/// - Data bits: 8
/// - Stop bits: 1
/// - Parity: None
/// 
/// # Safety
/// Must be called before any UART operations.
/// 
/// # Example
/// ```
/// uart::init();
/// uart::puts("Hello!\n");
/// ```
pub fn init() {
    // ...
}
```

---

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation
- `style` - Formatting
- `refactor` - Code restructuring
- `perf` - Performance improvement
- `test` - Tests
- `chore` - Maintenance

### Examples

```
feat(gpio): add PWM support for GPIO pins

Implement software PWM functionality for any GPIO pin.
Supports configurable frequency and duty cycle.

Closes #42
```

```
fix(uart): correct baud rate divisor calculation

The fractional baud rate divisor was being truncated
instead of rounded, causing timing drift at high speeds.

Fixes #57
```

```
docs(api): add memory allocator documentation

Document the buddy allocator API including:
- Initialization parameters
- Allocation strategies
- Statistics retrieval
```

---

## Pull Request Process

### Before Submitting

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feat/my-feature`
3. **Make your changes**
4. **Run tests**: `make test`
5. **Check formatting**: `cargo fmt --check`
6. **Update documentation** if needed

### PR Requirements

- [ ] Code compiles without warnings
- [ ] Tests pass (if applicable)
- [ ] Documentation updated
- [ ] Commit messages follow guidelines
- [ ] PR description explains changes

### PR Template

```markdown
## Description
Brief description of changes.

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
How was this tested?

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-reviewed code
- [ ] Commented complex sections
- [ ] Updated documentation
- [ ] No new warnings
```

---

## Architecture Guidelines

### Core Principles

1. **Zero Dependencies**: No external crates
2. **No Standard Library**: Pure `#![no_std]`
3. **Capability-Based Security**: All access through capabilities
4. **Intent-Driven**: Natural language interface

### Adding New Drivers

```rust
// drivers/newdevice.rs

//! New Device Driver
//! 
//! Supports XYZ hardware on BCM2712.

use crate::drivers::PERIPHERAL_BASE;

const NEWDEVICE_BASE: usize = PERIPHERAL_BASE + 0xXXXXXX;

/// New device controller
pub struct NewDevice {
    base: usize,
}

impl NewDevice {
    /// Create new device instance
    pub fn new() -> Self {
        NewDevice { base: NEWDEVICE_BASE }
    }
    
    /// Initialize the device
    pub fn init(&self) {
        // SAFETY: Device is memory-mapped at NEWDEVICE_BASE
        unsafe {
            // Initialization sequence
        }
    }
}

// Global instance
static mut DEVICE: Option<NewDevice> = None;

/// Initialize global device
pub fn init() {
    unsafe {
        DEVICE = Some(NewDevice::new());
        DEVICE.as_ref().unwrap().init();
    }
}
```

### Adding New Intents

```rust
// In intent/mod.rs

pub enum Intent {
    // ... existing intents ...
    
    /// New intent description
    NewIntent { param: Type },
}

fn parse_intent(input: &str) -> Intent {
    // ... existing parsing ...
    
    // Add pattern matching for new intent
    if input.contains("new pattern") {
        return Intent::NewIntent { 
            param: extract_param(input) 
        };
    }
}

fn execute_intent(intent: Intent) -> IntentResult {
    match intent {
        // ... existing handlers ...
        
        Intent::NewIntent { param } => {
            // Implementation
            IntentResult::Success("Done".into())
        }
    }
}
```

---

## Testing

### Unit Tests

Since we're `no_std`, testing requires creativity:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Test on host (not target)
    #[test]
    fn test_capability_derive() {
        let mut table = CapabilityTable::new();
        let root = table.create_root(
            ResourceType::Memory { base: 0, size: 1000 },
            Permissions::all()
        );
        
        let child = table.derive(&root, Permissions::READ);
        assert!(child.is_some());
        assert!(!child.unwrap().permissions.contains(Permissions::WRITE));
    }
}
```

### Hardware Testing

```rust
// Debug builds include test harness
#[cfg(debug_assertions)]
pub fn run_tests() {
    uart::puts("[TEST] Starting...\n");
    
    test_memory_allocator();
    test_gpio_loopback();
    test_timer_accuracy();
    
    uart::puts("[TEST] All passed!\n");
}
```

### QEMU Testing

```bash
# Run with QEMU and capture output
make qemu 2>&1 | tee test_output.log
grep -E "(PASS|FAIL)" test_output.log
```

---

## Documentation

### Inline Documentation

Every public item should have documentation:

```rust
/// Brief one-line description.
/// 
/// Longer description if needed. Explain the purpose,
/// usage patterns, and any gotchas.
/// 
/// # Arguments
/// 
/// * `param` - Description of parameter
/// 
/// # Returns
/// 
/// Description of return value.
/// 
/// # Errors
/// 
/// * `ErrorType` - When this error occurs
/// 
/// # Safety
/// 
/// Explain unsafe requirements if applicable.
/// 
/// # Examples
/// 
/// ```
/// let result = function(arg);
/// ```
pub fn function(param: Type) -> Result<Output, Error> {
    // ...
}
```

### Updating Docs

When your change affects:
- Public API → Update [API.md](API.md)
- Build process → Update [BUILDING.md](BUILDING.md)
- Architecture → Update [ARCHITECTURE.md](ARCHITECTURE.md)
- Hardware → Update [HARDWARE.md](HARDWARE.md)

---

## Questions?

- Open an issue for questions
- Tag maintainers for urgent items
- Join discussions for design decisions

---

Thank you for contributing to Intent Kernel! 🚀

---

*Contributing Guide v0.1.0*
