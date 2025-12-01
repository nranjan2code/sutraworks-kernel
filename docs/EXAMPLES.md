# Intent Kernel Examples

Practical examples demonstrating Intent Kernel capabilities.

---

## Table of Contents

1. [Basic Serial Communication](#basic-serial-communication)
2. [GPIO Control](#gpio-control)
3. [Timer and Delays](#timer-and-delays)
4. [Memory Allocation](#memory-allocation)
5. [Capability-Based Access Control](#capability-based-access-control)
6. [Interrupt Handling](#interrupt-handling)
7. [Framebuffer Graphics](#framebuffer-graphics)
8. [Intent Processing](#intent-processing)
9. [Complete Application](#complete-application)

---

## Basic Serial Communication

### Hello World

```rust
use crate::drivers::uart;

pub fn hello_world() {
    uart::init();
    uart::puts("Hello from Intent Kernel!\r\n");
}
```

### Interactive Echo

```rust
use crate::drivers::uart::{Uart, init, puts, getc, putc};

pub fn echo_loop() {
    init();
    puts("Echo server started. Type something:\r\n");
    
    loop {
        let c = getc();
        putc(c);
        
        if c == b'\r' {
            putc(b'\n');
        }
        
        // Exit on Ctrl+C
        if c == 0x03 {
            puts("\r\nGoodbye!\r\n");
            break;
        }
    }
}
```

### Command Line Parser

```rust
use crate::drivers::uart;

pub fn simple_shell() {
    uart::init();
    let uart = uart::Uart::new();
    
    let mut buffer = [0u8; 256];
    
    loop {
        uart.puts("shell> ");
        let len = uart.read_line(&mut buffer);
        
        // Parse command
        let cmd = core::str::from_utf8(&buffer[..len]).unwrap_or("");
        let cmd = cmd.trim();
        
        match cmd {
            "help" => {
                uart.puts("Commands: help, info, clear, exit\r\n");
            }
            "info" => {
                uart.puts("Intent Kernel v0.1.0\r\n");
                uart.puts("Platform: Raspberry Pi 5\r\n");
            }
            "clear" => {
                uart.puts("\x1B[2J\x1B[H"); // ANSI clear screen
            }
            "exit" => {
                uart.puts("Halting...\r\n");
                break;
            }
            "" => {} // Empty line
            _ => {
                uart.puts("Unknown command: ");
                uart.puts(cmd);
                uart.puts("\r\n");
            }
        }
    }
}
```

---

## GPIO Control

### LED Blink

```rust
use crate::drivers::gpio::{Gpio, GpioFunction};
use crate::drivers::timer;

const LED_PIN: u32 = 42; // Activity LED

pub fn blink_led() {
    let gpio = Gpio::new();
    timer::init();
    
    // Configure LED pin as output
    gpio.set_function(LED_PIN, GpioFunction::Output);
    
    loop {
        gpio.set_output(LED_PIN, true);  // LED on
        timer::delay_ms(500);
        
        gpio.set_output(LED_PIN, false); // LED off
        timer::delay_ms(500);
    }
}
```

### Button Input with Debounce

```rust
use crate::drivers::gpio::{Gpio, GpioFunction, GpioPull};
use crate::drivers::timer;
use crate::drivers::uart;

const BUTTON_PIN: u32 = 17;
const DEBOUNCE_MS: u64 = 50;

pub fn button_handler() {
    let gpio = Gpio::new();
    uart::init();
    timer::init();
    
    // Configure button pin with pull-up
    gpio.set_function(BUTTON_PIN, GpioFunction::Input);
    gpio.set_pull(BUTTON_PIN, GpioPull::Up);
    
    let mut last_state = true;
    let mut press_count: u32 = 0;
    
    loop {
        let current = gpio.get_input(BUTTON_PIN);
        
        // Detect falling edge (button press, active low)
        if last_state && !current {
            timer::delay_ms(DEBOUNCE_MS);
            
            // Confirm still pressed
            if !gpio.get_input(BUTTON_PIN) {
                press_count += 1;
                uart::puts("Button pressed! Count: ");
                print_number(press_count);
                uart::puts("\r\n");
            }
        }
        
        last_state = current;
        timer::delay_ms(10);
    }
}

fn print_number(n: u32) {
    let mut buf = [0u8; 10];
    let mut i = 0;
    let mut num = n;
    
    if num == 0 {
        uart::putc(b'0');
        return;
    }
    
    while num > 0 {
        buf[i] = b'0' + (num % 10) as u8;
        num /= 10;
        i += 1;
    }
    
    while i > 0 {
        i -= 1;
        uart::putc(buf[i]);
    }
}
```

### PWM-like LED Dimming

```rust
use crate::drivers::gpio::{Gpio, GpioFunction};
use crate::drivers::timer;

const LED_PIN: u32 = 42;
const PWM_PERIOD_US: u64 = 1000; // 1kHz

pub fn led_breathe() {
    let gpio = Gpio::new();
    timer::init();
    
    gpio.set_function(LED_PIN, GpioFunction::Output);
    
    let mut brightness: u64 = 0;
    let mut increasing = true;
    
    loop {
        // Software PWM
        for _ in 0..50 {
            gpio.set_output(LED_PIN, true);
            timer::delay_us(brightness * 10);
            
            gpio.set_output(LED_PIN, false);
            timer::delay_us((100 - brightness) * 10);
        }
        
        // Update brightness
        if increasing {
            brightness += 1;
            if brightness >= 100 {
                increasing = false;
            }
        } else {
            brightness -= 1;
            if brightness == 0 {
                increasing = true;
            }
        }
    }
}
```

---

## Timer and Delays

### Stopwatch

```rust
use crate::drivers::timer;
use crate::drivers::uart;

pub fn stopwatch() {
    timer::init();
    uart::init();
    
    uart::puts("Stopwatch: Press Enter to start/stop\r\n");
    
    let mut running = false;
    let mut start_time: u64 = 0;
    let mut elapsed: u64 = 0;
    
    loop {
        if let Some(c) = uart::try_getc() {
            if c == b'\r' {
                if running {
                    // Stop
                    elapsed += timer::uptime_us() - start_time;
                    running = false;
                    uart::puts("\r\nStopped: ");
                    print_time(elapsed);
                } else {
                    // Start
                    start_time = timer::uptime_us();
                    running = true;
                    uart::puts("\r\nRunning...");
                }
            } else if c == b'r' {
                // Reset
                elapsed = 0;
                running = false;
                uart::puts("\r\nReset");
            }
        }
        
        if running {
            // Update display every 100ms
            let current = elapsed + (timer::uptime_us() - start_time);
            uart::puts("\r");
            print_time(current);
            timer::delay_ms(100);
        }
    }
}

fn print_time(us: u64) {
    let ms = us / 1000;
    let secs = ms / 1000;
    let mins = secs / 60;
    
    uart::puts(&format_time(mins, secs % 60, ms % 1000));
}

fn format_time(mins: u64, secs: u64, ms: u64) -> [u8; 12] {
    // Format: "MM:SS.mmm"
    let mut buf = [b' '; 12];
    buf[0] = b'0' + ((mins / 10) % 10) as u8;
    buf[1] = b'0' + (mins % 10) as u8;
    buf[2] = b':';
    buf[3] = b'0' + ((secs / 10) % 10) as u8;
    buf[4] = b'0' + (secs % 10) as u8;
    buf[5] = b'.';
    buf[6] = b'0' + ((ms / 100) % 10) as u8;
    buf[7] = b'0' + ((ms / 10) % 10) as u8;
    buf[8] = b'0' + (ms % 10) as u8;
    buf
}
```

### Periodic Task Scheduler

```rust
use crate::drivers::timer;

struct Task {
    interval_us: u64,
    last_run: u64,
    handler: fn(),
}

pub struct Scheduler {
    tasks: [Option<Task>; 16],
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            tasks: [None; 16],
        }
    }
    
    pub fn add_task(&mut self, interval_ms: u64, handler: fn()) -> bool {
        for slot in self.tasks.iter_mut() {
            if slot.is_none() {
                *slot = Some(Task {
                    interval_us: interval_ms * 1000,
                    last_run: 0,
                    handler,
                });
                return true;
            }
        }
        false
    }
    
    pub fn run(&mut self) {
        loop {
            let now = timer::uptime_us();
            
            for task in self.tasks.iter_mut().flatten() {
                if now - task.last_run >= task.interval_us {
                    (task.handler)();
                    task.last_run = now;
                }
            }
        }
    }
}

// Example usage
pub fn scheduler_demo() {
    timer::init();
    
    let mut scheduler = Scheduler::new();
    
    scheduler.add_task(1000, || {
        uart::puts("1 second tick\r\n");
    });
    
    scheduler.add_task(5000, || {
        uart::puts("5 second tick\r\n");
    });
    
    scheduler.run();
}
```

---

## Memory Allocation

### Basic Allocation

```rust
use crate::kernel::memory;

pub fn allocation_demo() {
    memory::init();
    
    // Allocate 1KB
    if let Some(ptr) = memory::alloc(1024) {
        uart::puts("Allocated 1KB at: ");
        print_hex(ptr as usize);
        uart::puts("\r\n");
        
        // Use the memory
        unsafe {
            let slice = core::slice::from_raw_parts_mut(ptr, 1024);
            slice.fill(0xAA);
        }
        
        // Free it
        memory::free(ptr);
        uart::puts("Freed 1KB\r\n");
    }
}
```

### Ring Buffer

```rust
use crate::kernel::memory;

pub struct RingBuffer {
    data: *mut u8,
    size: usize,
    head: usize,
    tail: usize,
}

impl RingBuffer {
    pub fn new(size: usize) -> Option<Self> {
        let data = memory::alloc(size)?;
        Some(RingBuffer {
            data,
            size,
            head: 0,
            tail: 0,
        })
    }
    
    pub fn push(&mut self, byte: u8) -> bool {
        let next = (self.head + 1) % self.size;
        if next == self.tail {
            return false; // Full
        }
        
        unsafe {
            *self.data.add(self.head) = byte;
        }
        self.head = next;
        true
    }
    
    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            return None; // Empty
        }
        
        let byte = unsafe { *self.data.add(self.tail) };
        self.tail = (self.tail + 1) % self.size;
        Some(byte)
    }
    
    pub fn len(&self) -> usize {
        if self.head >= self.tail {
            self.head - self.tail
        } else {
            self.size - self.tail + self.head
        }
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        memory::free(self.data);
    }
}
```

---

## Capability-Based Access Control

### Resource Protection

```rust
use crate::kernel::capability::{CapabilityTable, ResourceType, Permissions, Capability};

pub fn capability_demo() {
    let mut table = CapabilityTable::new();
    
    // Create memory capability
    let mem_cap = table.create(
        ResourceType::Memory { base: 0x10000, size: 0x1000 },
        Permissions::READ | Permissions::WRITE | Permissions::DERIVE,
        0 // Owner process ID
    ).expect("Failed to create capability");
    
    uart::puts("Created memory capability\r\n");
    
    // Derive read-only capability for sharing
    let readonly_cap = table.derive(
        &mem_cap,
        Permissions::READ
    ).expect("Failed to derive capability");
    
    uart::puts("Derived read-only capability\r\n");
    
    // Validate access
    if table.validate(&readonly_cap, Permissions::READ) {
        uart::puts("Read access: GRANTED\r\n");
    }
    
    if !table.validate(&readonly_cap, Permissions::WRITE) {
        uart::puts("Write access: DENIED (correct)\r\n");
    }
    
    // Revoke parent (also revokes child)
    table.revoke(&mem_cap);
    uart::puts("Revoked capability tree\r\n");
    
    // Child is now invalid
    if !table.validate(&readonly_cap, Permissions::READ) {
        uart::puts("Derived capability now invalid (correct)\r\n");
    }
}
```

### Device Access Control

```rust
use crate::kernel::capability::{CapabilityTable, ResourceType, Permissions};

pub struct SecureGpio {
    table: CapabilityTable,
    gpio_cap: Option<Capability>,
}

impl SecureGpio {
    pub fn new() -> Self {
        let mut table = CapabilityTable::new();
        
        // Create GPIO capability
        let gpio_cap = table.create(
            ResourceType::Device { device_id: 0x01 },
            Permissions::READ | Permissions::WRITE,
            0
        );
        
        SecureGpio { table, gpio_cap }
    }
    
    pub fn set_pin(&self, cap: &Capability, pin: u32, high: bool) -> Result<(), &'static str> {
        // Validate capability
        if !self.table.validate(cap, Permissions::WRITE) {
            return Err("Permission denied");
        }
        
        // Perform the operation
        let gpio = Gpio::new();
        gpio.set_output(pin, high);
        Ok(())
    }
    
    pub fn get_pin(&self, cap: &Capability, pin: u32) -> Result<bool, &'static str> {
        if !self.table.validate(cap, Permissions::READ) {
            return Err("Permission denied");
        }
        
        let gpio = Gpio::new();
        Ok(gpio.get_input(pin))
    }
}
```

---

## Interrupt Handling

### Timer Interrupt

```rust
use crate::drivers::interrupts;
use crate::drivers::timer;
use core::sync::atomic::{AtomicU64, Ordering};

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

fn timer_handler() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    timer::clear_interrupt();
    timer::set_timeout(1_000_000); // 1 second
}

pub fn interrupt_demo() {
    timer::init();
    interrupts::init();
    
    // Register timer handler
    interrupts::register_handler(30, timer_handler); // Physical timer PPI
    interrupts::enable(30);
    
    // Start timer
    timer::set_timeout(1_000_000);
    
    // Enable interrupts
    arch::enable_interrupts();
    
    // Main loop
    loop {
        let ticks = TICK_COUNT.load(Ordering::Relaxed);
        uart::puts("Ticks: ");
        print_number(ticks as u32);
        uart::puts("\r\n");
        timer::delay_ms(100);
    }
}
```

### UART Interrupt

```rust
use crate::drivers::interrupts;
use crate::drivers::uart;

static mut RX_BUFFER: [u8; 64] = [0; 64];
static mut RX_HEAD: usize = 0;

fn uart_handler() {
    while let Some(c) = uart::try_getc() {
        unsafe {
            RX_BUFFER[RX_HEAD] = c;
            RX_HEAD = (RX_HEAD + 1) % RX_BUFFER.len();
        }
    }
    interrupts::end(153);
}

pub fn async_uart_demo() {
    uart::init();
    interrupts::init();
    
    interrupts::register_handler(153, uart_handler);
    interrupts::enable(153);
    arch::enable_interrupts();
    
    uart::puts("Type characters (interrupt-driven):\r\n");
    
    let mut tail: usize = 0;
    loop {
        let head = unsafe { RX_HEAD };
        while tail != head {
            let c = unsafe { RX_BUFFER[tail] };
            uart::putc(c);
            if c == b'\r' {
                uart::putc(b'\n');
            }
            tail = (tail + 1) % 64;
        }
    }
}
```

---

## Framebuffer Graphics

### Basic Drawing

```rust
use crate::drivers::framebuffer;

pub fn graphics_demo() {
    framebuffer::init(1920, 1080);
    
    // Clear to dark blue
    framebuffer::clear(0x000033);
    
    // Draw colored rectangles
    let fb = framebuffer::Framebuffer::new();
    fb.draw_rect(100, 100, 200, 150, 0xFF0000); // Red
    fb.draw_rect(350, 100, 200, 150, 0x00FF00); // Green
    fb.draw_rect(600, 100, 200, 150, 0x0000FF); // Blue
    
    // Print text
    fb.set_cursor(100, 300);
    fb.print("Intent Kernel Graphics Demo");
}
```

### Animated Bouncing Box

```rust
use crate::drivers::framebuffer::Framebuffer;
use crate::drivers::timer;

pub fn animation_demo() {
    let fb = Framebuffer::new();
    fb.init(1920, 1080, 32);
    
    let width = 1920;
    let height = 1080;
    let box_size = 50;
    
    let mut x: i32 = 100;
    let mut y: i32 = 100;
    let mut dx: i32 = 5;
    let mut dy: i32 = 3;
    
    loop {
        // Clear
        fb.clear(0x000000);
        
        // Draw box
        fb.draw_rect(x as u32, y as u32, box_size, box_size, 0xFF5500);
        
        // Update position
        x += dx;
        y += dy;
        
        // Bounce
        if x <= 0 || x >= (width - box_size) as i32 {
            dx = -dx;
        }
        if y <= 0 || y >= (height - box_size) as i32 {
            dy = -dy;
        }
        
        timer::delay_ms(16); // ~60 FPS
    }
}
```

---

## Intent Processing

### Custom Intent Handler

```rust
use crate::intent::{IntentEngine, IntentResult, Intent};

pub fn custom_intent_demo() {
    let mut engine = IntentEngine::new();
    
    // Register custom handler
    engine.register_handler("blink", |args| {
        let times: u32 = args.parse().unwrap_or(3);
        for _ in 0..times {
            gpio::set_output(42, true);
            timer::delay_ms(200);
            gpio::set_output(42, false);
            timer::delay_ms(200);
        }
        IntentResult::Success(format!("Blinked {} times", times))
    });
    
    // Process intents
    match engine.process("blink 5") {
        IntentResult::Success(msg) => uart::puts(&msg),
        IntentResult::Error(err) => uart::puts(&err),
        _ => {}
    }
}
```

### Voice-Style Commands

```rust
use crate::intent;

pub fn voice_command_demo() {
    uart::init();
    uart::puts("Voice Command Simulation\r\n");
    uart::puts("========================\r\n\r\n");
    
    let commands = [
        "turn on the LED",
        "what is the temperature",
        "show memory status",
        "allocate 4 kilobytes",
        "help me",
    ];
    
    for cmd in commands.iter() {
        uart::puts("> ");
        uart::puts(cmd);
        uart::puts("\r\n");
        
        let result = intent::process(cmd);
        uart::puts("  → ");
        match result {
            IntentResult::Success(msg) => uart::puts(&msg),
            IntentResult::Error(err) => {
                uart::puts("Error: ");
                uart::puts(&err);
            }
            IntentResult::NeedMoreInfo(q) => {
                uart::puts("Please clarify: ");
                uart::puts(&q);
            }
            _ => uart::puts("OK"),
        }
        uart::puts("\r\n\r\n");
    }
}
```

---

## Complete Application

### System Monitor

```rust
use crate::drivers::{uart, timer, framebuffer};
use crate::kernel::memory;

pub fn system_monitor() {
    // Initialize everything
    uart::init();
    timer::init();
    framebuffer::init(1920, 1080);
    memory::init();
    
    let fb = framebuffer::Framebuffer::new();
    
    loop {
        fb.clear(0x001122);
        fb.set_cursor(20, 20);
        
        // Header
        fb.println("╔════════════════════════════════════════╗");
        fb.println("║       Intent Kernel System Monitor     ║");
        fb.println("╠════════════════════════════════════════╣");
        
        // Uptime
        let uptime_ms = timer::uptime_us() / 1000;
        let secs = uptime_ms / 1000;
        let mins = secs / 60;
        let hours = mins / 60;
        fb.print("║ Uptime: ");
        // print formatted time...
        fb.println("                              ║");
        
        // Memory
        let stats = memory::stats();
        fb.print("║ Memory Used:  ");
        // print stats.used...
        fb.println(" bytes                 ║");
        fb.print("║ Memory Free:  ");
        // print stats.free...
        fb.println(" bytes                 ║");
        
        // Temperature
        if let Some(temp) = mailbox::get_temperature() {
            fb.print("║ Temperature:  ");
            // print temp/1000...
            fb.println("°C                      ║");
        }
        
        fb.println("╚════════════════════════════════════════╝");
        
        // Update every second
        timer::delay_ms(1000);
    }
}
```

---

## Building Examples

To build any example:

```bash
# Add example to main.rs
# pub mod examples;
# examples::run_example();

make clean
make
```

Deploy to SD card and boot.

---

*Examples v0.1.0 - Intent Kernel*
