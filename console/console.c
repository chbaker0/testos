#include "console.h"

#include <stddef.h>

unsigned int console_width, console_height;
static volatile uint16_t *vmem;

void console_init()
{
	// Asume VGA text mode
	console_width = 80;
	console_height = 25;
    vmem = (uint16_t*) 0xB8000;
    console_clear();
}

void console_clear()
{
	for(unsigned int i = 0; i < console_width * console_height; ++i)
		vmem[i] = 0;
}

uint8_t console_make_color(console_color_t fg, console_color_t bg)
{
	return (0xF0 & (uint8_t) bg) | (0x0F & (uint8_t) fg);
}
