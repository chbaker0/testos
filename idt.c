#include "idt.h"

struct idt_entry idt_make_entry(uint32_t offset, uint8_t selector, uint8_t type, uint8_t priv, uint8_t present)
{
	struct idt_entry result;
	result.offset_0_15  = offset & 0x0000FFFF;
	result.offset_16_31 = offset & 0xFFFF0000;
	result.selector = selector;
	result.zero = 0;
	result.type = type;
	result.priv_level = priv;
	result.present = present ? 1 : 0;
	result.storage_segment = 0;
	return result;
}
