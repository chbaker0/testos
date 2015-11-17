#ifndef _TERMINAL_H_INCLUDED_
#define _TERMINAL_H_INCLUDED_

#include <stdint.h>
#include "vga.h"

#define TERMINAL_HEIGHT 1024
#define TERMINAL_WIDTH 80

void terminal_init();
void terminal_set_color(vga_color_t color);
void terminal_write_line(const char *str);

#endif // _TERMINAL_H_INCLUDED_
