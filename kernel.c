#include <stdint.h>

#include "port.h"
#include "console.h"

struct idtr
{
	uint16_t limit;
	uint32_t base;
};

struct idt_entry
{
	uint16_t offset_0_15;
	uint16_t selector;
	uint8_t zero;
	uint8_t type;
	uint16_t offset_16_31;
};

idt_entry idt_entries[256];

#define IDT_32_INT_GATE 14u
#define IDT_32_TRAP_GATE 15u

void kmain()
{
	
	
	console_init();
	
	console_write_line("Hello world from a kernel!");
	console_write_line("This is just a test.");
	console_write_line("ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ");
	console_advance_cursor(2, 3);
	console_write_line("Test number 2");
	console_write_line("Test number 3");
	
	console_scroll(2);
	
	console_write_line("Test scroll");
}
