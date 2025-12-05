/*
 * ╔═══════════════════════════════════════════════════════════════════════════╗
 * ║                    INTENT KERNEL - QEMU LINKER SCRIPT                     ║
 * ║                       QEMU 'virt' Machine Memory Map                      ║
 * ╚═══════════════════════════════════════════════════════════════════════════╝
 *
 * RAM Starts at 0x4000_0000
 */

ENTRY(_start)

/* Memory regions */
MEMORY
{
    BOOT    (rx)  : ORIGIN = 0x40080000,  LENGTH = 64K
    CODE    (rx)  : ORIGIN = 0x40090000,  LENGTH = 2M
    RODATA  (r)   : ORIGIN = 0x40290000,  LENGTH = 1M
    DATA    (rw)  : ORIGIN = 0x40390000,  LENGTH = 2M
    BSS     (rw)  : ORIGIN = 0x40590000,  LENGTH = 1M
    STACK   (rw)  : ORIGIN = 0x40690000,  LENGTH = 1M
    HEAP    (rw)  : ORIGIN = 0x40800000,  LENGTH = 128M
}

SECTIONS
{
    /* ═══════════════════════════════════════════════════════════════════════ */
    /* BOOT SECTION                                                            */
    /* ═══════════════════════════════════════════════════════════════════════ */
    . = 0x40080000;
    
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
PROVIDE(__kernel_start = 0x40080000);
PROVIDE(__kernel_size = __kernel_end - __kernel_start);

/* Capability pool (general purpose allocation) */
PROVIDE(__heap_start = 0x40800000);
PROVIDE(__heap_end = 0x48800000); /* 128MB Heap */
PROVIDE(__heap_size = __heap_end - __heap_start);

/* Intent engine memory */
PROVIDE(__intent_mem_start = 0x48800000);
PROVIDE(__intent_mem_end = 0x50000000); /* 120MB Intent Mem */
PROVIDE(__intent_mem_size = __intent_mem_end - __intent_mem_start);

/* DMA buffer region */
PROVIDE(__dma_start = 0x50000000);
PROVIDE(__dma_end = 0x51000000); /* 16MB DMA */

/* GPU shared memory (Mocked for QEMU) */
PROVIDE(__gpu_mem_start = 0x51000000);
PROVIDE(__gpu_mem_end = 0x52000000);

/* QEMU Virt Peripherals */
PROVIDE(__peripheral_base = 0x08000000); /* Virt MMIO base */
PROVIDE(__uart_base = 0x09000000);       /* PL011 */
PROVIDE(__gpio_base = 0x09030000);       /* PL061 */
PROVIDE(__timer_base = 0x09090000);      /* SP804? No, use system timer */
PROVIDE(__interrupt_base = 0x08000000);  /* GIC V2 */
PROVIDE(__mailbox_base = 0x0);           /* No mailbox on virt */

/* Legacy symbols to satisfy linker */
PROVIDE(__aux_base = 0x0);
PROVIDE(__spi_base = 0x0);
PROVIDE(__i2c_base = 0x0);
