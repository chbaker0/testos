#include "vga.h"

#include <stddef.h>

static volatile uint16_t *vmem = 0xb8000;

void vga_clear()
{
	for(unsigned int i = 0; i < 80 * 25; ++i)
		vmem[i] = 0;
}

void vga_display_terminal(struct terminal_buffer *term)
{
    vga_clear();
    const uint8_t color = vga_make_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK);

    uint32_t top_line;
    if (term->bottom_line >= 25)
    {
        top_line = term->bottom_line - 25;
    }
    else
    {
        top_line = TERMINAL_HEIGHT - 25 + term->bottom_line;
    }

    for (uint32_t line_ndx = 0; line_ndx < 25; ++line_ndx)
    {
        const uint32_t term_line = (line_ndx + top_line) % TERMINAL_HEIGHT;

        for (size_t i = 0; i < 80; ++i)
        {
            const uint8_t term_char = term->buf[term_line*TERMINAL_WIDTH + i];
            vmem[line_ndx*80 + i] = term_char + ((uint16_t) color << 8U);
        }
    }
}

uint8_t vga_make_color(vga_color_t fg, vga_color_t bg)
{
	return (0xF0 & (uint8_t) bg) | (0x0F & (uint8_t) fg);
}
