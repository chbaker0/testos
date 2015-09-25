#include "gdt.h"

struct gdt_entry gdt_make_entry(uint32_t base, uint32_t limit, uint8_t access, uint8_t privilege, uint8_t flags)
{
	struct gdt_entry result;
	result.limit_0_15  = limit & 0x0000FFFF;
	result.limit_16_19 = limit & 0x000F0000;
	
	result.base_0_15  = base & 0x0000FFFF;
	result.base_16_23 = base & 0x00FF0000;
	result.base_24_31 = base & 0xFF000000;
	result.access = access;
	result.access = (result.access & ~GDT_ENTRY_ACCESS_PRIV_BITS)
		| ((privilege & 3U) << 5U);
	result.access |= 16U;
	result.flags = flags & 12U;
	return result;
}

