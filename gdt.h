#ifndef _GDT_H_INCLUDED_
#define _GDT_H_INCLUDED_

#include <stdint.h>

#define GDT_ENTRY_FLAGS_SZ_BIT 4U
#define GDT_ENTRY_FLAGS_GR_BIT 8U
#define GDT_ENTRY_ACCESS_AC_BIT 1U
#define GDT_ENTRY_ACCESS_RW_BIT 2U
#define GDT_ENTRY_ACCESS_DC_BIT 4U
#define GDT_ENTRY_ACCESS_EX_BIT 8U
#define GDT_ENTRY_ACCESS_PRIV_BITS 96U
#define GDT_ENTRY_ACCESS_PR_BIT 128U

struct __attribute__ ((__packed__)) gdt_entry
{
	unsigned long long limit_0_15 : 16;
	unsigned long long base_0_15 : 16;
	unsigned long long base_16_23 : 8;
	unsigned long long access : 8;
	unsigned long long limit_16_19 : 4;
	unsigned long long flags : 4;
	unsigned long long base_24_31 : 8;
};

inline struct gdt_entry gdt_make_entry(uint32_t base, uint32_t limit, uint8_t access, uint8_t flags)
{
	struct gdt_entry result;
	result.limit_0_15  = limit & 0x0000FFFF;
	result.limit_16_19 = limit & 0x000F0000;
	
	result.base_0_15  = base & 0x0000FFFF;
	result.base_16_23 = base & 0x00FF0000;
	result.base_24_31 = base & 0xFF000000;
	result.access = access;
	result.flags = flags;
	return result;
}

void gdt_load(struct gdt_entry *entries, uint16_t size);

#endif // _GDT_H_INCLUDED_
