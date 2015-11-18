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

void terminal_write_line(const char *str)
{
	while(*str != 0)
	{
		buffer[bottom * TERMINAL_WIDTH + head] = (uint16_t) *str++ + ((uint16_t) cur_color << 8U);
	    if(++head == TERMINAL_WIDTH)
		{
			head = 0;
		    scroll();
		}
	}
	scroll();
	draw();
}
