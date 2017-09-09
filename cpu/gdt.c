#include "gdt.h"

static struct gdt_entry gdt[8192];

void gdt_init()
{
	gdt_load(gdt, sizeof(gdt) - 1);
}

static void fill_common(struct gdt_entry *entry, uint32_t base, uint32_t limit)
{
	entry->base_0_15 = base & 0x0000FFFF;
	entry->base_16_23 = (base & 0x00FF0000) >> 16U;
	entry->base_24_31 = (base & 0xFF000000) >> 24U;

	entry->limit_0_15 = limit & 0x0000FFFF;
	entry->limit_16_19 = (limit & 0x000F0000) >> 16U;
}

void gdt_set_code_segment(
	uint16_t segment, uint32_t base, uint32_t limit,
	struct gdt_code_segment_settings *settings
	)
{
	struct gdt_entry entry = {0};
	fill_common(&entry, base, limit);

	entry.flags = 0x04 | (settings->common.granularity << 3U);
	entry.access = 0x18
		| settings->common.accessed
		| (settings->readable << 1U)
		| (settings->conforming << 2U)
		| (settings->common.privilege << 5U)
		| (settings->common.present << 7U);

	asm volatile("pushf; cli");
	gdt[segment / 8U] = entry;
	asm volatile("popf");
}

void gdt_set_data_segment(
	uint16_t segment, uint32_t base, uint32_t limit,
	struct gdt_data_segment_settings *settings
	)
{
	struct gdt_entry entry = {0};
	fill_common(&entry, base, limit);

	entry.flags = 0x04 | (settings->common.granularity << 3U);
	entry.access = 0x10
		| settings->common.accessed
		| (settings->writable << 1U)
		| (settings->direction << 2U)
		| (settings->common.privilege << 5U)
		| (settings->common.present << 7U);

	asm volatile("pushf; cli");
	gdt[segment / 8U] = entry;
	asm volatile("popf");
}
