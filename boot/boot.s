// ╔═══════════════════════════════════════════════════════════════════════════╗
// ║                         INTENT KERNEL BOOTLOADER                          ║
// ║                        Raspberry Pi 5 (BCM2712)                           ║
// ║                                                                           ║
// ║  World-class bare metal initialization with:                              ║
// ║  • Multi-core support (4x Cortex-A76)                                     ║
// ║  • Full exception handling                                                ║
// ║  • EL3 → EL2 → EL1 transitions                                            ║
// ║  • FPU/SIMD initialization                                                ║
// ║  • Cache and memory barrier support                                       ║
// ╚═══════════════════════════════════════════════════════════════════════════╝

.section ".text.boot"

.global _start
.global _vectors

// ════════════════════════════════════════════════════════════════════════════
// ENTRY POINT - First instruction executed
// ════════════════════════════════════════════════════════════════════════════
_start:
    // Immediately disable all interrupts
    msr     daifset, #0xf
    
    // Get core ID - only core 0 initializes the system
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff
    cbnz    x0, _secondary_entry

// ════════════════════════════════════════════════════════════════════════════
// PRIMARY CORE INITIALIZATION
// ════════════════════════════════════════════════════════════════════════════
_primary_entry:
    // Save DTB pointer (firmware passes it in x0, but we got core ID there)
    // Re-read it from x21 where firmware also stores it
    adrp    x1, __dtb_ptr
    add     x1, x1, :lo12:__dtb_ptr
    str     xzr, [x1]               // Clear for now
    
    // Determine current exception level
    mrs     x0, CurrentEL
    lsr     x0, x0, #2
    
    cmp     x0, #3
    b.eq    _from_el3
    cmp     x0, #2  
    b.eq    _from_el2
    b       _at_el1

// ════════════════════════════════════════════════════════════════════════════
// EL3 → EL2 TRANSITION (Secure Monitor → Hypervisor)
// ════════════════════════════════════════════════════════════════════════════
_from_el3:
    // SCR_EL3: Security Configuration
    mov     x0, #(1 << 10)          // RW: EL2 executes as AArch64
    orr     x0, x0, #(1 << 0)       // NS: Non-secure state
    orr     x0, x0, #(1 << 8)       // HCE: HVC instruction enabled
    orr     x0, x0, #(1 << 7)       // SMD: SMC disabled (no secure monitor)
    msr     scr_el3, x0
    
    // SPSR_EL3: Saved Program Status (return to EL2h, masked interrupts)
    mov     x0, #0b01001            // EL2h mode
    orr     x0, x0, #(0xf << 6)     // Mask DAIF
    msr     spsr_el3, x0
    
    adr     x0, _from_el2
    msr     elr_el3, x0
    eret

// ════════════════════════════════════════════════════════════════════════════
// EL2 → EL1 TRANSITION (Hypervisor → Kernel)
// ════════════════════════════════════════════════════════════════════════════
_from_el2:
    // HCR_EL2: Hypervisor Configuration
    mov     x0, #(1 << 31)          // RW: EL1 executes as AArch64
    msr     hcr_el2, x0
    
    // Disable all traps to EL2
    msr     cptr_el2, xzr           // No coprocessor traps
    msr     hstr_el2, xzr           // No system register traps
    msr     vttbr_el2, xzr          // No stage 2 translation
    
    // Timer configuration - allow EL1 access
    mrs     x0, cnthctl_el2
    orr     x0, x0, #0x3            // EL1PCTEN, EL1PCEN
    msr     cnthctl_el2, x0
    msr     cntvoff_el2, xzr        // No virtual offset
    
    // SPSR_EL2: Return to EL1h with interrupts masked
    mov     x0, #0b00101            // EL1h mode  
    orr     x0, x0, #(0xf << 6)     // Mask DAIF
    msr     spsr_el2, x0
    
    adr     x0, _at_el1
    msr     elr_el2, x0
    eret

// ════════════════════════════════════════════════════════════════════════════
// EL1 KERNEL INITIALIZATION
// ════════════════════════════════════════════════════════════════════════════
_at_el1:
    // ─────────────────────────────────────────────────────────────────────────
    // System Control Register - Start with known state
    // ─────────────────────────────────────────────────────────────────────────
    mrs     x0, sctlr_el1
    bic     x0, x0, #(1 << 0)       // M: MMU disabled initially
    bic     x0, x0, #(1 << 2)       // C: Data cache disabled initially
    bic     x0, x0, #(1 << 12)      // I: Instruction cache disabled initially
    orr     x0, x0, #(1 << 29)      // LSMAOE
    orr     x0, x0, #(1 << 28)      // nTLSMD
    msr     sctlr_el1, x0
    isb
    
    // ─────────────────────────────────────────────────────────────────────────
    // Memory Attributes (for when we enable MMU later)
    // ─────────────────────────────────────────────────────────────────────────
    // Attr0: Device-nGnRnE memory (peripherals)
    // Attr1: Normal memory, Inner/Outer Write-Back Cacheable
    // Attr2: Normal memory, Inner/Outer Non-Cacheable
    ldr     x0, =0x000000000044ff00
    msr     mair_el1, x0
    
    // ─────────────────────────────────────────────────────────────────────────
    // Exception Vectors
    // ─────────────────────────────────────────────────────────────────────────
    adr     x0, _vectors
    msr     vbar_el1, x0
    isb
    
    // ─────────────────────────────────────────────────────────────────────────
    // Stack Setup - Core 0 gets stack at top
    // ─────────────────────────────────────────────────────────────────────────
    ldr     x0, =0x80000            // Stack top (grows down)
    mov     sp, x0
    
    // ─────────────────────────────────────────────────────────────────────────
    // BSS Clear - Zero uninitialized data
    // ─────────────────────────────────────────────────────────────────────────
    adrp    x0, __bss_start
    add     x0, x0, :lo12:__bss_start
    adrp    x1, __bss_end
    add     x1, x1, :lo12:__bss_end
    cmp     x0, x1
    b.ge    _bss_done
    
_bss_loop:
    stp     xzr, xzr, [x0], #16
    cmp     x0, x1
    b.lt    _bss_loop

_bss_done:
    // ─────────────────────────────────────────────────────────────────────────
    // FPU/SIMD Enable
    // ─────────────────────────────────────────────────────────────────────────
    mov     x0, #(3 << 20)          // FPEN: Full access to FP/SIMD
    msr     cpacr_el1, x0
    isb
    
    // ─────────────────────────────────────────────────────────────────────────
    // Enable Caches (optional, can improve performance significantly)
    // ─────────────────────────────────────────────────────────────────────────
    mrs     x0, sctlr_el1
    orr     x0, x0, #(1 << 2)       // C: Enable data cache
    orr     x0, x0, #(1 << 12)      // I: Enable instruction cache
    msr     sctlr_el1, x0
    isb
    
    // ─────────────────────────────────────────────────────────────────────────
    // Jump to Rust Kernel
    // ─────────────────────────────────────────────────────────────────────────
    bl      kernel_main
    
    // Should never return
    b       _halt

// ════════════════════════════════════════════════════════════════════════════
// SECONDARY CORES - Wait for release
// ════════════════════════════════════════════════════════════════════════════
_secondary_entry:
    // Get our core ID
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff
    
    // Calculate our release slot address
    adrp    x1, __core_release
    add     x1, x1, :lo12:__core_release
    add     x1, x1, x0, lsl #3
    
_secondary_wait:
    wfe                             // Wait for event (low power)
    ldr     x2, [x1]                // Check release address
    cbz     x2, _secondary_wait     // Loop if still zero
    
    // We've been released! Set up our stack
    // Each core gets 64KB of stack space
    ldr     x3, =0x80000
    mov     x4, #0x10000            // 64KB
    mul     x4, x0, x4
    sub     sp, x3, x4
    
    // Clear our release slot
    str     xzr, [x1]
    
    // Do EL transitions if needed (secondary cores might start at EL2)
    mrs     x3, CurrentEL
    lsr     x3, x3, #2
    cmp     x3, #2
    b.ne    _secondary_at_el1
    
    // Configure EL2 → EL1 for this core
    mov     x3, #(1 << 31)
    msr     hcr_el2, x3
    msr     cptr_el2, xzr
    
    mrs     x3, cnthctl_el2
    orr     x3, x3, #0x3
    msr     cnthctl_el2, x3
    msr     cntvoff_el2, xzr
    
    mov     x3, #0b00101
    orr     x3, x3, #(0xf << 6)
    msr     spsr_el2, x3
    
    msr     elr_el2, x2
    eret

_secondary_at_el1:
    // Enable FPU for this core
    mov     x3, #(3 << 20)
    msr     cpacr_el1, x3
    isb
    
    // Jump to the entry point
    br      x2

// ════════════════════════════════════════════════════════════════════════════
// EXCEPTION VECTOR TABLE
// ════════════════════════════════════════════════════════════════════════════
.balign 0x800
_vectors:
    // ─────────────────────────────────────────────────────────────────────────
    // Current EL with SP_EL0 (not used)
    // ─────────────────────────────────────────────────────────────────────────
    .balign 0x80
    b       _sync_handler
    .balign 0x80  
    b       _irq_handler
    .balign 0x80
    b       _fiq_handler
    .balign 0x80
    b       _serror_handler

    // ─────────────────────────────────────────────────────────────────────────
    // Current EL with SP_ELx (kernel mode - this is what we use)
    // ─────────────────────────────────────────────────────────────────────────
    .balign 0x80
    b       _sync_handler
    .balign 0x80
    b       _irq_handler
    .balign 0x80
    b       _fiq_handler
    .balign 0x80
    b       _serror_handler

    // ─────────────────────────────────────────────────────────────────────────
    // Lower EL using AArch64 (user mode traps)
    // ─────────────────────────────────────────────────────────────────────────
    .balign 0x80
    b       _sync_lower
    .balign 0x80
    b       _irq_lower
    .balign 0x80
    b       _fiq_handler
    .balign 0x80
    b       _serror_handler

    // ─────────────────────────────────────────────────────────────────────────
    // Lower EL using AArch32 (not supported)
    // ─────────────────────────────────────────────────────────────────────────
    .balign 0x80
    b       _halt
    .balign 0x80
    b       _halt
    .balign 0x80
    b       _halt
    .balign 0x80
    b       _halt

// ════════════════════════════════════════════════════════════════════════════
// CONTEXT SAVE/RESTORE MACROS
// ════════════════════════════════════════════════════════════════════════════

.macro SAVE_ALL
    sub     sp, sp, #288            // 32 GP regs + ELR + SPSR + padding
    stp     x0, x1, [sp, #0]
    stp     x2, x3, [sp, #16]
    stp     x4, x5, [sp, #32]
    stp     x6, x7, [sp, #48]
    stp     x8, x9, [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]
    mrs     x0, elr_el1
    mrs     x1, spsr_el1
    stp     x30, x0, [sp, #240]
    str     x1, [sp, #256]
    // Save exception info
    mrs     x0, esr_el1
    mrs     x1, far_el1
    stp     x0, x1, [sp, #264]
.endm

.macro RESTORE_ALL
    ldr     x1, [sp, #256]
    ldp     x30, x0, [sp, #240]
    msr     spsr_el1, x1
    msr     elr_el1, x0
    ldp     x28, x29, [sp, #224]
    ldp     x26, x27, [sp, #208]
    ldp     x24, x25, [sp, #192]
    ldp     x22, x23, [sp, #176]
    ldp     x20, x21, [sp, #160]
    ldp     x18, x19, [sp, #144]
    ldp     x16, x17, [sp, #128]
    ldp     x14, x15, [sp, #112]
    ldp     x12, x13, [sp, #96]
    ldp     x10, x11, [sp, #80]
    ldp     x8, x9, [sp, #64]
    ldp     x6, x7, [sp, #48]
    ldp     x4, x5, [sp, #32]
    ldp     x2, x3, [sp, #16]
    ldp     x0, x1, [sp, #0]
    add     sp, sp, #288
.endm

// ════════════════════════════════════════════════════════════════════════════
// EXCEPTION HANDLERS
// ════════════════════════════════════════════════════════════════════════════

_sync_handler:
    SAVE_ALL
    mov     x0, sp                  // Exception frame pointer
    bl      handle_exception
    RESTORE_ALL
    eret

_irq_handler:
    SAVE_ALL
    mov     x0, sp
    bl      handle_irq
    RESTORE_ALL
    eret

_fiq_handler:
    SAVE_ALL
    mov     x0, sp
    bl      handle_fiq
    RESTORE_ALL
    eret

_serror_handler:
    SAVE_ALL
    mov     x0, sp
    bl      handle_serror
    RESTORE_ALL
    eret

_sync_lower:
    SAVE_ALL
    mov     x0, sp
    bl      handle_sync_lower
    RESTORE_ALL
    eret

_irq_lower:
    SAVE_ALL
    mov     x0, sp
    bl      handle_irq_lower
    RESTORE_ALL
    eret

// ════════════════════════════════════════════════════════════════════════════
// HALT - Infinite low-power loop
// ════════════════════════════════════════════════════════════════════════════
_halt:
    wfe
    b       _halt

// ════════════════════════════════════════════════════════════════════════════
// EXPORTED FUNCTIONS FOR RUST
// ════════════════════════════════════════════════════════════════════════════

.global enable_interrupts
enable_interrupts:
    msr     daifclr, #0x2           // Enable IRQs only
    ret

.global disable_interrupts  
disable_interrupts:
    mrs     x0, daif                // Return previous state
    msr     daifset, #0x2
    ret

.global enable_all_interrupts
enable_all_interrupts:
    msr     daifclr, #0xf
    ret

.global disable_all_interrupts
disable_all_interrupts:
    mrs     x0, daif
    msr     daifset, #0xf
    ret

.global restore_interrupts
restore_interrupts:
    msr     daif, x0
    ret

.global get_exception_level
get_exception_level:
    mrs     x0, CurrentEL
    lsr     x0, x0, #2
    ret

.global get_core_id
get_core_id:
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff
    ret

.global memory_barrier
memory_barrier:
    dmb     sy
    ret

.global data_sync_barrier
data_sync_barrier:
    dsb     sy
    ret

.global instruction_barrier
instruction_barrier:
    isb
    ret

.global full_barrier
full_barrier:
    dsb     sy
    isb
    ret

.global send_event
send_event:
    dsb     sy
    sev
    ret

.global wait_for_event
wait_for_event:
    wfe
    ret

.global wait_for_interrupt
wait_for_interrupt:
    wfi
    ret

.global read_timer
read_timer:
    isb
    mrs     x0, cntpct_el0
    ret

.global read_timer_freq
read_timer_freq:
    mrs     x0, cntfrq_el0
    ret

.global wake_core
wake_core:
    // x0 = core ID (1-3), x1 = entry point
    cmp     x0, #0
    b.eq    1f                      // Can't wake core 0
    cmp     x0, #4
    b.ge    1f                      // Invalid core
    
    adrp    x2, __core_release
    add     x2, x2, :lo12:__core_release
    str     x1, [x2, x0, lsl #3]
    dsb     sy
    sev
1:  ret

.global halt_core
halt_core:
    b       _halt

// ════════════════════════════════════════════════════════════════════════════
// DATA SECTION
// ════════════════════════════════════════════════════════════════════════════
.section ".data"

.balign 8
.global __dtb_ptr
__dtb_ptr:
    .quad   0

.balign 64
.global __core_release  
__core_release:
    .quad   0                       // Core 0 (primary)
    .quad   0                       // Core 1
    .quad   0                       // Core 2
    .quad   0                       // Core 3
