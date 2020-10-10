ENTRY(_start)

PHDRS
{
  text PT_LOAD FLAGS(5) /* r-x */;
  data PT_LOAD FLAGS(6) /* rw- */;
}

KERNEL_BASE = -2048M;

SECTIONS
{
    . = KERNEL_BASE + 1M;

    .text : ALIGN(4K)
    {
        *(.text)
    } :text

    .rodata : ALIGN(4K)
    {
        *(.rodata)
    } :data

    .data : ALIGN(4K)
    {
        *(.data)
    } :data

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss)
    } :data
}
