ENTRY(kentry)

KERNEL_BASE = -2048M;

SECTIONS
{
	. = KERNEL_BASE + 1M;

	.text ALIGN(4K) :
	{
		*(.text)
	}

	.rodata ALIGN(4K) :
	{
		*(.rodata)
	}

	.data ALIGN(4K) :
	{
		*(.data)
	}

	.bss ALIGN(4K) :
	{
		*(COMMON)
		*(.bss)
	}
}
