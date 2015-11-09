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

// Utility functions

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

void gdt_load(struct gdt_entry *entries, uint16_t size);

// GDT management functions

struct gdt_common_segment_settings
{
	unsigned int granularity : 1;
	unsigned int present : 1;
	unsigned int accessed : 1;
	unsigned int privilege : 2;
};

struct gdt_code_segment_settings
{
	unsigned int conforming : 1;
	unsigned int readable : 1;
	struct gdt_common_segment_settings common;
};

struct gdt_data_segment_settings
{
	unsigned int direction : 1;
	unsigned int writable : 1;
	struct gdt_common_segment_settings common;
};

void gdt_init();

void gdt_set_empty_segment(uint16_t segment);

void gdt_set_code_segment(
	uint16_t segment, uint32_t base, uint32_t limit,
	struct gdt_code_segment_settings *settings
	);
void gdt_set_data_segment(
	uint16_t segment, uint32_t base, uint32_t limit,
	struct gdt_data_segment_settings *settings
	);

#endif // _GDT_H_INCLUDED_
