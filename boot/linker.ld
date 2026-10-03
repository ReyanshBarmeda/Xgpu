ENTRY(start)

SECTIONS
{
    /* GRUB loads kernels at or above 1 MiB by convention */
    . = 1M;

    __kernel_start = .;

    .boot :
    {
        /* the Multiboot2 header must appear within the first 32 KiB
           of the file, so it is placed in its own leading section */
        KEEP(*(.multiboot_header))
    }

    . = ALIGN(4K);

    .text :
    {
        *(.text .text.*)
    }

    . = ALIGN(4K);

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    . = ALIGN(4K);

    .data :
    {
        *(.data .data.*)
        /* rustc emits these for statics holding relocated pointers;
           without them the link silently drops kernel data */
        *(.data.rel.ro .data.rel.ro.*)
        *(.got .got.plt)
    }

    . = ALIGN(4K);

    .bss :
    {
        *(.bss .bss.*)
        *(COMMON)
    }

    . = ALIGN(4K);
    __kernel_end = .;

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame_hdr)
        *(.comment)
        *(.note .note.*)
        *(.gcc_except_table*)
    }
}
