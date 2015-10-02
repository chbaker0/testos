#ifndef _INTERRUPT_H_INCLUDED_
#define _INTERRUPT_H_INCLUDED_

#include <stdint.h>

typedef void (*interrupt_handler)(void);

uintptr_t interrupt_get_trampoline_addr(uint8_t i);
interrupt_handler interrupt_get_handler(uint8_t i);
void interrupt_set_handler(uint8_t i, interrupt_handler h);

#endif // _INTERRUPT_H_INCLUDED_
