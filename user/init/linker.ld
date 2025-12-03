ENTRY(_start)

SECTIONS
{
    . = 0x400000; /* Load address */

    .text :
    {
        *(.text._start)
        *(.text*)
    }

    .rodata :
    {
        *(.rodata*)
    }

    .data :
    {
        *(.data*)
    }

    .bss :
    {
        *(.bss*)
    }
}
