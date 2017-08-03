#include <stddef.h>

#include "terminal.h"

static uint16_t buffer[TERMINAL_WIDTH * TERMINAL_HEIGHT] = {0};
static unsigned int bottom = 0, head = 0;
static vga_color_t cur_color = 0;

void terminal_init()
{
    cur_color = vga_make_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK);
	bottom = 24;
}

void terminal_set_color(vga_color_t color)
{
	cur_color = color;
}

static void clear_bottom()
{
	for(unsigned int i = 0; i < TERMINAL_WIDTH; ++i)
		buffer[bottom * TERMINAL_WIDTH + i] = 0;
}

static void scroll()
{
	if(++bottom == TERMINAL_HEIGHT)
		bottom = 0;
	for(unsigned int i = 0; i < TERMINAL_WIDTH; ++i)
		buffer[bottom * TERMINAL_WIDTH + i] = 0;
}

static void draw()
{
	if(bottom >= 24)
	{
		struct screen_pos off = {0, 0};
		struct screen_pos size = {80, 25};
		vga_write_rect(buffer + (24 - bottom) * TERMINAL_WIDTH, &off, &size);
	}
	else
	{
		struct screen_pos off = {0, 0};
		struct screen_pos size = {80, 24 - bottom};
		vga_write_rect(buffer + (TERMINAL_HEIGHT - bottom - 1) * TERMINAL_WIDTH,
					   &off, &size);
		off.y = size.y;
		size.y = 25 - size.y;
		vga_write_rect(buffer + bottom * TERMINAL_WIDTH, &off, &size);
	}
}

// Writes string to bottom of terminal. Assumes input string is no
// longer than TERMINAL_WIDTH.
static void write_line_impl(const char *str, size_t str_length)
{
    clear_bottom();
    for (size_t i = 0; i < str_length; ++i)
    {
        buffer[bottom * TERMINAL_WIDTH + i] =
            (uint16_t) str[i] + ((uint16_t) cur_color << 8U);
    }
}

void terminal_write_line(const char *str)
{
    size_t str_length = 0;
    const char *cur = str;
	while (*cur != 0)
    {
        ++str_length;
        ++cur;
    }

    size_t num_lines = (str_length+TERMINAL_WIDTH-1) / TERMINAL_WIDTH;
    size_t cur_ndx = 0;
    for (size_t l = 0; l < num_lines; ++l)
    {
        size_t line_length = (str_length - cur_ndx) < 80 ? (str_length - cur_ndx) : 80;
        write_line_impl(str + cur_ndx, line_length);
        scroll();
    }

	draw();
}
