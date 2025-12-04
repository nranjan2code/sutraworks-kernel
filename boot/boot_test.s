// ╔═══════════════════════════════════════════════════════════════════════════╗
// ║                         INTENT KERNEL BOOTLOADER                          ║
// ║                        QEMU Virt Machine (Test)                           ║
// ╚═══════════════════════════════════════════════════════════════════════════╝

.section ".text.boot"

.global _start
.global _vectors

// ════════════════════════════════════════════════════════════════════════════
// ENTRY POINT
// ════════════════════════════════════════════════════════════════════════════
_start:
    // DEBUG: Print 'X'
    ldr     x2, =0x09000000
    mov     w1, #'X'
    str     w1, [x2]
    
    // Disable interrupts
    msr     daifset, #0xf

    // DEBUG: Print 'A'
    mov     w1, #'A'
    str     w1, [x2]

    // Check CurrentEL
    mrs     x0, CurrentEL
    lsr     x0, x0, #2
    
    // DEBUG: Print 'E'
    mov     w1, #'E'
    str     w1, [x2]

    cmp     x0, #3
    b.eq    _from_el3
    cmp     x0, #2  
    b.eq    _from_el2
    b       _at_el1

// ════════════════════════════════════════════════════════════════════════════
// EL3 → EL2
// ════════════════════════════════════════════════════════════════════════════
_from_el3:
    // DEBUG: Print '3'
    ldr     x2, =0x09000000
    mov     w1, #'3'
    str     w1, [x2]

    mov     x0, #(1 << 10)          // RW: EL2 executes as AArch64
    orr     x0, x0, #(1 << 0)       // NS: Non-secure state
    orr     x0, x0, #(1 << 8)       // HCE: HVC instruction enabled
    orr     x0, x0, #(1 << 7)       // SMD: SMC disabled
    msr     scr_el3, x0
    
    mov     x0, #0b01001            // EL2h mode
    orr     x0, x0, #(0xf << 6)     // Mask DAIF
    msr     spsr_el3, x0
    
    adr     x0, _from_el2
    msr     elr_el3, x0
    eret

// ════════════════════════════════════════════════════════════════════════════
// EL2 → EL1
// ════════════════════════════════════════════════════════════════════════════
_from_el2:
    // DEBUG: Print '2'
    ldr     x2, =0x09000000
    mov     w1, #'2'
    str     w1, [x2]

    mov     x0, #(1 << 31)          // RW: EL1 executes as AArch64
    msr     hcr_el2, x0
    
    msr     cptr_el2, xzr
    msr     hstr_el2, xzr
    msr     vttbr_el2, xzr
    
    mrs     x0, cnthctl_el2
    orr     x0, x0, #0x3
    msr     cnthctl_el2, x0
    msr     cntvoff_el2, xzr
    
    mov     x0, #0b00101            // EL1h mode  
    orr     x0, x0, #(0xf << 6)     // Mask DAIF
    msr     spsr_el2, x0
    
    adr     x0, _at_el1
    msr     elr_el2, x0
    eret

// ════════════════════════════════════════════════════════════════════════════
// EL1 INITIALIZATION
// ════════════════════════════════════════════════════════════════════════════
_at_el1:
    // DEBUG: Print '1'
    ldr     x2, =0x09000000
    mov     w1, #'1'
    str     w1, [x2]

    // System Control Register
    mrs     x0, sctlr_el1
    bic     x0, x0, #(1 << 0)       // M: MMU disabled
    bic     x0, x0, #(1 << 2)       // C: Data cache disabled
    bic     x0, x0, #(1 << 12)      // I: Instruction cache disabled
    orr     x0, x0, #(1 << 29)      // LSMAOE
    orr     x0, x0, #(1 << 28)      // nTLSMD
    msr     sctlr_el1, x0
    isb
    
    // Stack Setup - use memory BEFORE the kernel to avoid heap collision
    // Kernel loads at 0x40080000. We use the 512KB before that for stack.
    ldr     x0, =0x40080000
    mov     sp, x0

    // Zero BSS
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
    sub     x2, x1, x0
    cbz     x2, 2f
1:  str     xzr, [x0], #8
    sub     x2, x2, #8
    cbnz    x2, 1b
2:

    // Enable FPU
    mrs     x0, cpacr_el1
    orr     x0, x0, #(3 << 20)
    msr     cpacr_el1, x0
    isb

    // Set Vector Table
    adr     x0, _vectors
    msr     vbar_el1, x0
    isb
    
    // Jump to kernel
    bl      kernel_main
    
    // Halt if return
    b       _halt

_halt:
    // DEBUG: Print 'H'
    ldr     x2, =0x09000000
    mov     w1, #'H'
    str     w1, [x2]
1:  wfe
    b       1b

// ════════════════════════════════════════════════════════════════════════════
// EXCEPTION VECTORS
// ════════════════════════════════════════════════════════════════════════════
.balign 0x800
_vectors:
    .align 11
    // Current EL with SP0 - Synchronous
    b .
    
    .align 7
    // Current EL with SP0 - IRQ
    b .
    
    .align 7
    // Current EL with SP0 - FIQ
    b .
    
    .align 7
    // Current EL with SP0 - SError
    b .
    
    .align 7
    // Current EL with SPx - Synchronous
    ldr x0, =0x09000000
    mov w1, 'S'
    str w1, [x0]
    b .

    .align 7
    // Current EL with SPx - IRQ
    ldr x0, =0x09000000
    mov w1, 'Q'
    str w1, [x0]
    b .
    .balign 0x80
    ldr x2, =0x09000000
    mov w1, #'#'
    str w1, [x2]
    b _halt

    .balign 0x80
    ldr x2, =0x09000000
    mov w1, #'$'
    str w1, [x2]
    b _halt

// ════════════════════════════════════════════════════════════════════════════
// DATA
// ════════════════════════════════════════════════════════════════════════════
.section ".data"
.balign 8
.global __dtb_ptr
__dtb_ptr: .quad 0

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
    
    adr     x2, __core_release
    str     x1, [x2, x0, lsl #3]
    dsb     sy
    sev
1:  ret

.global halt_core
halt_core:
    b       _halt

.balign 64
.global __core_release  
__core_release:
    .quad   0                       // Core 0 (primary)
    .quad   0                       // Core 1
    .quad   0                       // Core 2
    .quad   0                       // Core 3
