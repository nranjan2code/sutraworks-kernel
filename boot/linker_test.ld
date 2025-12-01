ENTRY(_start)

MEMORY
{
    RAM (rwx) : ORIGIN = 0x40000000, LENGTH = 1024M
}

SECTIONS
{
    .text.boot : { KEEP(*(.text.boot)) } > RAM
    .text : { *(.text .text.*) } > RAM
    .rodata : { *(.rodata .rodata.*) } > RAM
    .data : { *(.data .data.*) } > RAM
    .bss : { 
        . = ALIGN(16);
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    } > RAM
    
    .stack (NOLOAD) : {
        . = ALIGN(4096);
        . += 64K;
        __stack_top = .;
    } > RAM

    /* Define heap symbols for allocator */
    . = ALIGN(4096);
    __heap_start = .;
    . += 64M;
    __heap_end = .;
    
    /* Mock other symbols needed by kernel */
    __dma_start = .;
    __dma_end = .;
    __gpu_start = .;
    __gpu_end = .;
}
