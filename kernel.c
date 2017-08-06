#include <stdint.h>
#include <stddef.h>

#include "core/terminal.h"
#include "cpu/port.h"
#include "cpu/gdt.h"
#include "cpu/idt.h"
#include "cpu/interrupt.h"
#include "cpu/pic.h"
#include "cpu/helpers.h"
#include "io/vga.h"
#include "multiboot.h"

#define BOCHS_BREAKPOINT() asm volatile("xchg %bx, %bx")

extern void rustmain();

struct idt_entry idt_entries[256];

void panic()
{
	INTERRUPT_DISABLE();
	while(1)
		asm volatile("hlt");
}

void timer_handler()
{
	pic_eoi(0);
}

void test_handler()
{
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

static struct terminal_buffer termbuf;

static void print_line(const char *str)
{
    terminal_write_line(&termbuf, str);
    vga_display_terminal(&termbuf);
}

void kmain(struct multiboot_info *mbinfo)
{
	setup_flat_gdt();

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
	interrupt_set_handler(32, timer_handler);

	idt_load(idt_entries, 255);
	INTERRUPT_ENABLE();

	interrupt_set_handler(0x80, test_handler);

	INTERRUPT_RAISE(0x80);

    terminal_init(&termbuf);
    print_line("Test line 1");
    print_line("Test line 2");

    if (mbinfo->flags & MULTIBOOT_INFO_FLAG_MMAP)
    {
        print_line("Memory map present.");
    }

    if (mbinfo->flags & MULTIBOOT_INFO_FLAG_AOUT_SYM)
    {
        print_line("a.out symbols present");
    }

    if (mbinfo->flags & MULTIBOOT_INFO_FLAG_ELF_SYM)
    {
        print_line("ELF symbols present.");
    }

    rustmain();

	while(1)
		asm volatile("hlt"); // Busy loop
}
