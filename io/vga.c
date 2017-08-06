#include "vga.h"

#include <stddef.h>

static volatile uint16_t *vmem = 0xb8000;

void vga_clear()
{
	for(unsigned int i = 0; i < 80 * 25; ++i)
		vmem[i] = 0;
}

uint8_t vga_make_color(vga_color_t fg, vga_color_t bg)
{
	return (0xF0 & (uint8_t) bg) | (0x0F & (uint8_t) fg);
}
