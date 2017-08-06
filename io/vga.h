#ifndef _VGA_H_
#define _VGA_H_

#include <stdint.h>

#include "core/terminal.h"

struct screen_pos
{
	unsigned int x, y;
};

typedef enum vga_color
{
	VGA_COLOR_BLACK = 0,
	VGA_COLOR_BLUE,
	VGA_COLOR_GREEN,
	VGA_COLOR_CYAN,
	VGA_COLOR_RED,
	VGA_COLOR_MAGENTA,
	VGA_COLOR_BROWN,
	VGA_COLOR_LIGHT_GREY,
	VGA_COLOR_DARK_GREY,
	VGA_COLOR_LIGHT_BLUE,
	VGA_COLOR_LIGHT_GREEN,
	VGA_COLOR_LIGHT_CYAN,
	VGA_COLOR_LIGHT_RED,
	VGA_COLOR_LIGHT_MAGENTA,
	VGA_COLOR_LIGHT_BROWN,
	VGA_COLOR_WHITE
} vga_color_t;

void vga_clear();
uint8_t vga_make_color(vga_color_t fg, vga_color_t bg);

#endif // _VGA_H_
