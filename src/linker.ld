PHDRS
{
  text32 PT_LOAD FLAGS(5) /* r-x */;
  data32 PT_LOAD FLAGS(6) /* rw- */;
  text PT_LOAD FLAGS(5) /* r-x */;
  data PT_LOAD FLAGS(6) /* rw- */;
}

KERNEL_BASE = -2048M;

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

    . += KERNEL_BASE;

    .text ALIGN(4K) : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        KEEP(*(.text.mb2_header))
        *(.text)
    } :text

    .rodata ALIGN(4K) : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        *(.rodata)
    } :data

    .data ALIGN(4K) : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        *(.data)
    } :data

    .bss ALIGN(4K) : AT(. - KERNEL_BASE) ALIGN(4K)
    {
        *(COMMON)
        *(.bss)
    } :data
}
