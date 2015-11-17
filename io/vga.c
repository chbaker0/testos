#include "vga.h"

#include <stddef.h>

static volatile uint16_t *vmem = 0xb8000;

void vga_clear()
{
	for(unsigned int i = 0; i < 80 * 25; ++i)
		vmem[i] = 0;
}

void vga_write_rect(const uint16_t *buf, const struct screen_pos *off, const struct screen_pos *size)
{
	for(unsigned int i = 0; i < size->y && i < 25 - size->y + 1; ++i)
	{
		for(unsigned int j = 0; j < size->x && j < 80 - size->x + 1; ++j)
		{
			vmem[(i + off->y) * 80 + j + off->x]
				= buf[i * size->y + j];
		}
	}
}

uint8_t vga_make_color(vga_color_t fg, vga_color_t bg)
{
	return (0xF0 & (uint8_t) bg) | (0x0F & (uint8_t) fg);
}
