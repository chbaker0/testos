#include <stdint.h>

#include "port.h"
#include "console.h"
#include "gdt.h"
#include "helpers.h"

struct gdt_entry flat_gdt[3] = {0};

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
