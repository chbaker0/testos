#ifndef _IDT_H_INCLUDED_
#define _IDT_H_INCLUDED_

#include <stdint.h>

struct __attribute__ ((__packed__)) idt_entry
{
    unsigned long long offset_0_15 : 16;
	unsigned long long selector : 16;
	unsigned long long zero : 8; // ???
	unsigned long long type : 4;
	unsigned long long storage_segment : 1;
	unsigned long long priv_level : 2;
	unsigned long long present : 1;
	unsigned long long offset_16_31 : 16;
};

struct idt_entry idt_make_int_gate(uint32_t offset, uint16_t selector, uint8_t present, uint8_t priv);

void idt_load(struct idt_entry *entries, uint16_t size);

#endif // _IDT_H_INCLUDED_
