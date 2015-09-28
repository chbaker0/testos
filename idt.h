#ifndef _IDT_H_INCLUDED_
#define _IDT_H_INCLUDED_

struct idt_entry
{
	uint16_t offset_0_15;
	uint16_t selector;
	uint8_t zero;
	uint8_t type;
	uint16_t offset_16_31;
};

#define IDT_32_TASK_GATE 5u
#define IDT_16_INT_GATE 6u
#define IDT_16_TRAP_GATE 7u
#define IDT_32_INT_GATE 14u
#define IDT_32_TRAP_GATE 15u

#endif // _IDT_H_INCLUDED_
