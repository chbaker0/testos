#include <stdint.h>

#include "x86/port.h"
#include "x86/gdt.h"
#include "x86/idt.h"
#include "x86/interrupt.h"
#include "x86/helpers.h"
#include "console.h"

#define BOCHS_BREAKPOINT() asm volatile("xchg %bx, %bx")

struct gdt_entry flat_gdt[3] = {0};

struct idt_entry idt_entries[256];

void panic()
{
	console_write_line("Bye");
	INTERRUPT_DISABLE();
    *(volatile char *) 0xb8000 = 'Z';
}

void kmain()
{
	flat_gdt[1] =
		gdt_make_entry(0x00000000, 0x000FFFFF,
					   GDT_ENTRY_ACCESS_EX_BIT | GDT_ENTRY_ACCESS_PR_BIT
					   | GDT_ENTRY_ACCESS_RW_BIT, 0,
					   GDT_ENTRY_FLAGS_GR_BIT | GDT_ENTRY_FLAGS_SZ_BIT);
	flat_gdt[2] =
		gdt_make_entry(0x00000000, 0x000FFFFF,
					   GDT_ENTRY_ACCESS_PR_BIT | GDT_ENTRY_ACCESS_RW_BIT, 0,
					   GDT_ENTRY_FLAGS_GR_BIT | GDT_ENTRY_FLAGS_SZ_BIT);
	gdt_load(flat_gdt, 3);
	helpers_reload_all_segments(0x08, 0x10);

	BOCHS_BREAKPOINT();

	for(unsigned int i = 0; i < 256; ++i)
	{
		uintptr_t ih = interrupt_get_trampoline_addr(i);
		interrupt_set_handler(i, panic);
		idt_entries[i] = idt_make_int_gate(ih, 0x08, 1, 0);
	}

	idt_load(idt_entries, 255);

	BOCHS_BREAKPOINT();
	
	console_init();
	
	console_write_line("Hello world from a kernel!");
	console_write_line("This is just a test.");
	console_write_line("ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ");
	console_advance_cursor(2, 3);
	console_write_line("Test number 2");
	console_write_line("Test number 3");
	
	console_scroll(2);
	
	console_write_line("Test scroll");

	BOCHS_BREAKPOINT();
	
	INTERRUPT_RAISE(0x80);
}
