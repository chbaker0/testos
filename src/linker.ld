PHDRS
{
  text32 PT_LOAD FLAGS(5) /* r-x */;
  data32 PT_LOAD FLAGS(6) /* rw- */;
  text PT_LOAD FLAGS(5) /* r-x */;
  data PT_LOAD FLAGS(6) /* rw- */;
}

KERNEL_VIRT_BASE = -2048M;

SECTIONS
{
    . = 1M;

    .bootstrap.text ALIGN(4K) : AT(.) ALIGN(4K)
    {
        ENTRY(_start)
        KEEP(*(.text.mb2_header))
        *(.bootstrap.text)
    } :text32

    .bootstrap.data ALIGN(4K) : AT(.) ALIGN(4K)
    {
        *(.bootstrap.data)
    } :data32

    .bootstrap.bss ALIGN(4K) : AT(.) ALIGN(4K)
    {
        *(.bootstrap.bss)
    } :data32

    .bootstrap.bss ALIGN(4K) : AT(.) ALIGN(4K)
    {
        *(.bootstrap.bss)
    } :data32

    . += KERNEL_VIRT_BASE;

    .text ALIGN(4K) : AT(. - KERNEL_VIRT_BASE) ALIGN(4K)
    {
        KEEP(*(.text.mb2_header))
        *(.text .text.*)
    } :text

    .rodata ALIGN(4K) : AT(. - KERNEL_VIRT_BASE) ALIGN(4K)
    {
        *(.rodata .rodata.*)
        KERNEL_PHYS_BEGIN_SYM = LOADADDR(.bootstrap.text);
        KERNEL_PHYS_END_SYM = LOADADDR(.bss) + SIZEOF(.bss);
    } :data

    .data ALIGN(4K) : AT(. - KERNEL_VIRT_BASE) ALIGN(4K)
    {
        *(.data .data.*)
    } :data

    .bss ALIGN(4K) : AT(. - KERNEL_VIRT_BASE) ALIGN(4K)
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
