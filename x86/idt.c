#include "idt.h"

struct idt_entry idt_make_int_gate(uint32_t offset, uint16_t selector, uint8_t present, uint8_t priv)
{
	struct idt_entry result = {0};
	result.offset_0_15  = offset & 0x0000FFFF;
	result.offset_16_31 = offset & 0xFFFF0000;
	result.selector = selector;
	result.present = present ? 1 : 0;
	result.priv_level = priv;
	result.type = 0xFE;
	result.storage_segment = 0;
	return result;
}
