#ifndef _PIC_H_INCLUDED_
#define _PIC_H_INCLUDED_

#include <stdint.h>

void pic_remap(uint8_t irq0_offset, uint8_t irq8_offset);
void pic_eoi(uint8_t irq);

#endif // _PIC_H_INCLUDED_
