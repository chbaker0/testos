ENTRY(kentry)

KERNEL_BASE = -2048M;

SECTIONS
{
	. = KERNEL_BASE + 1M;

	.text ALIGN(4K) : AT(ADDR(.text) - KERNEL_BASE)
	{
		*(.text)
	}

	.rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_BASE)
	{
		*(.rodata)
	}

	.data ALIGN(4K) : AT(ADDR(.data) - KERNEL_BASE)
	{
		*(.data)
	}

	.bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_BASE)
	{
		*(COMMON)
		*(.bss)
	}
}
