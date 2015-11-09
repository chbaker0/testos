#include <stdint.h>
#include <stddef.h>

#include "cpu/port.h"
#include "cpu/gdt.h"
#include "cpu/idt.h"
#include "cpu/interrupt.h"
#include "cpu/pic.h"
#include "cpu/helpers.h"
#include "console.h"

#define BOCHS_BREAKPOINT() asm volatile("xchg %bx, %bx")

struct idt_entry idt_entries[256];

void panic()
{
	console_write_line("Panic handler called");
	INTERRUPT_DISABLE();
	while(1)
		asm volatile("hlt");
}

void test_handler()
{
	console_write_line("Test handler called");

	BOCHS_BREAKPOINT();
}

void setup_flat_gdt()
{
	struct gdt_common_segment_settings common_settings = {0};
	common_settings.granularity = 1;
	common_settings.present = 1;
	common_settings.privilege = 0;
	
	struct gdt_code_segment_settings c_settings = {0};
	c_settings.conforming = 0;
	c_settings.readable = 1;
	c_settings.common = common_settings;
	gdt_set_code_segment(0x08, 0, 0xFFFFF, &c_settings);

	struct gdt_data_segment_settings d_settings = {0};
	d_settings.direction = 0;
	d_settings.writable = 1;
	d_settings.common = common_settings;
	gdt_set_data_segment(0x10, 0, 0xFFFFF, &d_settings);

	gdt_init();
	helpers_reload_all_segments(0x08, 0x10);
}

void kmain()
{
	console_init();
	
	BOCHS_BREAKPOINT();

	setup_flat_gdt();

	BOCHS_BREAKPOINT();

	pic_remap(32, 40);

	for(unsigned int i = 0; i < 256; ++i)
	{
		uintptr_t ih = interrupt_get_trampoline_addr(i);
		interrupt_set_handler(i, NULL);
		idt_entries[i] = idt_make_int_gate(ih, 0x08, 1, 0);
	}
	for(unsigned int i = 0; i < 32; ++i)
	{
		interrupt_set_handler(i, panic);
	}

	idt_load(idt_entries, 255);
	INTERRUPT_ENABLE();

	interrupt_set_handler(0x80, test_handler);

	BOCHS_BREAKPOINT();
	
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
