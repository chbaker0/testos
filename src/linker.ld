PHDRS
{
  text32 PT_LOAD FLAGS(5) /* r-x */;
  text PT_LOAD FLAGS(5) /* r-x */;
  data PT_LOAD FLAGS(6) /* rw- */;
}

KERNEL_BASE = -2048M;

SECTIONS
{
    . = 1M;

    .bootstrap : ALIGN(4K)
    {
        ENTRY(_start)
        KEEP(*(.text.mb2_header))
        *(.bootstrap)
    } :text32

    . += KERNEL_BASE;

    .text : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        KEEP(*(.text.mb2_header))
        *(.text)
    } :text

    .rodata : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        *(.rodata)
    } :data

    .data : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        *(.data)
    } :data

    .bss : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        *(COMMON)
        *(.bss)
    } :data
}
