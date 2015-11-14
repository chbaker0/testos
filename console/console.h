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

void console_init();
void console_set_color(console_color_t fg, console_color_t bg);
void console_clear();
void console_advance_cursor(unsigned int rows, unsigned int columns);
void console_carriage_return();
void console_new_line();
void console_put_char(char c);
void console_write_line(const char *str);
void console_scroll(unsigned int lines);

#endif // _CONSOLE_H_