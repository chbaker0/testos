#ifndef _VGA_H_
#define _VGA_H_

#include <stdint.h>

struct screen_pos
{
	unsigned int x, y;
};

typedef enum vga_color
{
	CONSOLE_COLOR_BLACK = 0,
	CONSOLE_COLOR_BLUE,
	CONSOLE_COLOR_GREEN,
	CONSOLE_COLOR_CYAN,
	CONSOLE_COLOR_RED,
	CONSOLE_COLOR_MAGENTA,
	CONSOLE_COLOR_BROWN,
	CONSOLE_COLOR_LIGHT_GREY,
	CONSOLE_COLOR_DARK_GREY,
	CONSOLE_COLOR_LIGHT_BLUE,
	CONSOLE_COLOR_LIGHT_GREEN,
	CONSOLE_COLOR_LIGHT_CYAN,
	CONSOLE_COLOR_LIGHT_RED,
	CONSOLE_COLOR_LIGHT_MAGENTA,
	CONSOLE_COLOR_LIGHT_BROWN,
	CONSOLE_COLOR_WHITE
} vga_color_t;

void vga_clear();
void vga_write_rect(const uint16_t *buf, const struct screen_pos *off, const struct screen_pos *size);
uint8_t vga_make_color(vga_color_t fg, vga_color_t bg);

#endif // _VGA_H_
