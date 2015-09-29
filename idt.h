#ifndef _IDT_H_INCLUDED_
#define _IDT_H_INCLUDED_

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

#define IDT_TYPE_32_TASK_GATE 5u
#define IDT_TYPE_16_INT_GATE 6u
#define IDT_TYPE_16_TRAP_GATE 7u
#define IDT_TYPE_32_INT_GATE 14u
#define IDT_TYPE_32_TRAP_GATE 15u

#endif // _IDT_H_INCLUDED_
