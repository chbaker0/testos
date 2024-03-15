PHDRS
{
  text PT_LOAD FLAGS(5) /* r-x */;
  data PT_LOAD FLAGS(6) /* rw- */;
}

KERNEL_VIRT_BASE = -2048M;

SECTIONS
{
    . += KERNEL_VIRT_BASE;

    .text ALIGN(4K) :
    {
        ENTRY(_start)
        *(.text .text.*)
    } :text

    .rodata ALIGN(4K) :
    {
        *(.rodata .rodata.*)
        KERNEL_PHYS_BEGIN_SYM = LOADADDR(.text);
        KERNEL_PHYS_END_SYM = LOADADDR(.bss) + SIZEOF(.bss);
    } :data

    .data ALIGN(4K) :
    {
        *(.data .data.*)
    } :data

    .bss ALIGN(4K) :
    {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /DISCARD/ :
    {
        /* These are used for unwinding, which we don't support (yet?) */
        *(.eh_frame)
        *(.eh_frame_hdr)
    }
}
