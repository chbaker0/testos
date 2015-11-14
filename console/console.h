#ifndef _CONSOLE_H_
#define _CONSOLE_H_

#include <stdint.h>

typedef enum console_color
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
} console_color_t;

extern unsigned int console_width, console_height;

void console_init();
void console_clear();
uint8_t console_make_color(console_color_t fg, console_color_t bg);

#endif // _CONSOLE_H_
