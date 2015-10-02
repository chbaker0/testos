#include "interrupt.h"

interrupt_handler __interrupt_handlers[256] = {0};
extern uintptr_t __isr_trampolines[256];

uintptr_t interrupt_get_trampoline_addr(uint8_t i)
{
	return __isr_trampolines[i];
}

interrupt_handler interrupt_get_handler(uint8_t i)
{
	return __interrupt_handlers[i];
}

void interrupt_set_handler(uint8_t i, interrupt_handler h)
{
	__interrupt_handlers[i] = h;
}
