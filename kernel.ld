ENTRY(kentry)

KERNEL_BASE = -2048M;

PHDRS
{
    text PT_LOAD FILEHDR PHDRS;
    data PT_LOAD;
}

SECTIONS
{
	. = KERNEL_BASE + 1M;

	.text ALIGN(4K) :
	{
		*(.text)
	} :text

	.rodata ALIGN(4K) :
	{
		*(.rodata)
	} :text

	.data ALIGN(4K) :
	{
		*(.data)
	} :data

	.bss ALIGN(4K) :
	{
		*(COMMON)
		*(.bss)
	} :data
}
