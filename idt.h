#ifndef _IDT_H_INCLUDED_
#define _IDT_H_INCLUDED_

#include <stdint.h>

#define IDT_ENTRY_TYPE_32_TASK_GATE 5u
#define IDT_ENTRY_TYPE_16_INT_GATE 6u
#define IDT_ENTRY_TYPE_16_TRAP_GATE 7u
#define IDT_ENTRY_TYPE_32_INT_GATE 14u
#define IDT_ENTRY_TYPE_32_TRAP_GATE 15u

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

struct idt_entry idt_make_entry(uint32_t offset, uint8_t selector, uint8_t type, uint8_t priv, uint8_t present);

void idt_load(struct idt_entry *entries, uint16_t size);

#endif // _IDT_H_INCLUDED_
