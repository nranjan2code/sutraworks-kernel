/*
 * ╔═══════════════════════════════════════════════════════════════════════════╗
 * ║                    INTENT KERNEL - LINKER SCRIPT                          ║
 * ║                       Raspberry Pi 5 Memory Map                           ║
 * ╚═══════════════════════════════════════════════════════════════════════════╝
 *
 * Physical Memory Layout (8GB Configuration):
 * ┌────────────────────────────────────────────────────────────────────────────┐
 * │ 0x0000_0000 - 0x0007_FFFF │ Reserved (GPU/VideoCore)              512 KB  │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x0008_0000 - 0x001F_FFFF │ Kernel Code + RO Data               1.5 MB    │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x0020_0000 - 0x003F_FFFF │ Kernel Data + BSS + Stacks            2 MB    │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x0040_0000 - 0x0FFF_FFFF │ Capability Pool (General Purpose)   252 MB    │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x1000_0000 - 0x2FFF_FFFF │ Intent Engine Memory                512 MB    │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x3000_0000 - 0x3BFF_FFFF │ DMA / Hardware Buffers              192 MB    │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x3C00_0000 - 0x3FFF_FFFF │ GPU Shared Memory                    64 MB    │
 * ├────────────────────────────────────────────────────────────────────────────┤
 * │ 0x4000_0000 - 0x1_FFFF_FFFF │ Extended Memory (if >1GB)          ~7 GB    │
 * └────────────────────────────────────────────────────────────────────────────┘
 *
 * Peripheral Memory (BCM2712):
 * │ 0x1_0000_0000 - 0x1_07FF_FFFF │ Main Peripherals                128 MB    │
 * │ 0x1_0800_0000 - ...           │ PCIe, etc.                               │
 */

ENTRY(_start)

/* Memory regions */
MEMORY
{
    BOOT    (rx)  : ORIGIN = 0x80000,     LENGTH = 64K
    CODE    (rx)  : ORIGIN = 0x90000,     LENGTH = 1408K
    RODATA  (r)   : ORIGIN = 0x1F0000,    LENGTH = 64K
    DATA    (rw)  : ORIGIN = 0x200000,    LENGTH = 1M
    BSS     (rw)  : ORIGIN = 0x300000,    LENGTH = 512K
    STACK   (rw)  : ORIGIN = 0x380000,    LENGTH = 512K
}

SECTIONS
{
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* BOOT SECTION - Must be at 0x80000                                       */
    /* ═══════════════════════════════════════════════════════════════════════ */
    . = 0x80000;
    
    .text.boot : {
        KEEP(*(.text.boot))
    } > BOOT
    
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* CODE SECTION                                                            */
    /* ═══════════════════════════════════════════════════════════════════════ */
    .text : {
        __text_start = .;
        *(.text .text.*)
        __text_end = .;
    } > CODE
    
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* READ-ONLY DATA                                                          */
    /* ═══════════════════════════════════════════════════════════════════════ */
    .rodata : {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    } > RODATA
    
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* INITIALIZED DATA                                                        */
    /* ═══════════════════════════════════════════════════════════════════════ */
    .data : {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(8);
        __data_end = .;
    } > DATA
    
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* UNINITIALIZED DATA (BSS)                                                */
    /* ═══════════════════════════════════════════════════════════════════════ */
    .bss (NOLOAD) : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    } > BSS
    
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* STACK SECTION                                                           */
    /* ═══════════════════════════════════════════════════════════════════════ */
    .stack (NOLOAD) : {
        . = ALIGN(4096);
        __stack_bottom = .;
        . += 64K;   /* Core 3 stack */
        __stack_core3 = .;
        . += 64K;   /* Core 2 stack */
        __stack_core2 = .;
        . += 64K;   /* Core 1 stack */
        __stack_core1 = .;
        . += 64K;   /* Core 0 stack (primary) */
        __stack_top = .;
    } > STACK
    
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* KERNEL END MARKER                                                       */
    /* ═══════════════════════════════════════════════════════════════════════ */
    __kernel_end = .;
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* DISCARD UNWANTED SECTIONS                                               */
    /* ═══════════════════════════════════════════════════════════════════════ */
    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
        *(.ARM.*)
    }
}

/* ═══════════════════════════════════════════════════════════════════════════ */
/* EXPORTED SYMBOLS                                                            */
/* ═══════════════════════════════════════════════════════════════════════════ */

/* Memory region boundaries */
PROVIDE(__kernel_start = 0x80000);
PROVIDE(__kernel_size = __kernel_end - __kernel_start);

/* Capability pool (general purpose allocation) */
PROVIDE(__heap_start = 0x400000);
PROVIDE(__heap_end = 0x10000000);
PROVIDE(__heap_size = __heap_end - __heap_start);

/* Intent engine memory (for future LLM/AI processing) */
PROVIDE(__intent_mem_start = 0x10000000);
PROVIDE(__intent_mem_end = 0x30000000);
PROVIDE(__intent_mem_size = __intent_mem_end - __intent_mem_start);

/* DMA buffer region */
PROVIDE(__dma_start = 0x30000000);
PROVIDE(__dma_end = 0x3C000000);

/* GPU shared memory */
PROVIDE(__gpu_mem_start = 0x3C000000);
PROVIDE(__gpu_mem_end = 0x40000000);

/* BCM2712 Peripheral base addresses */
PROVIDE(__peripheral_base = 0x100000000);
PROVIDE(__gpio_base = 0x1000d0000);
PROVIDE(__uart_base = 0x100201000);
PROVIDE(__aux_base = 0x100215000);
PROVIDE(__spi_base = 0x100204000);
PROVIDE(__i2c_base = 0x100205000);
PROVIDE(__timer_base = 0x10000b000);
PROVIDE(__interrupt_base = 0x10000f200);
PROVIDE(__mailbox_base = 0x10000b880);

/* ═══════════════════════════════════════════════════════════════════════════ */
/* DISCARD UNWANTED SECTIONS                                                   */
/* ═══════════════════════════════════════════════════════════════════════════ */

