#include "console.h"

#include <stddef.h>

static const size_t CONSOLE_WIDTH = 80, CONSOLE_HEIGHT = 25;

static size_t console_row, console_column;
static uint8_t console_color;
static volatile uint16_t *console_buffer;

static uint16_t vga_entry(char c, uint8_t color)
{
    return (uint16_t) c | ((uint16_t) color) << 8U;
}

void console_set_color(console_color_t fg, console_color_t bg)
{
    console_color = (0xF0 & (uint8_t) bg) | (0x0F & (uint8_t) fg);
}

void console_clear()
{
    for(size_t i = 0; i < CONSOLE_WIDTH * CONSOLE_HEIGHT; ++i)
        console_buffer[i] = vga_entry(' ', console_color);
}

void console_init()
{
    console_buffer = (uint16_t*) 0xB8000;
    console_row = console_column = 0;
    console_set_color(CONSOLE_COLOR_LIGHT_GREY, CONSOLE_COLOR_BLACK);
    console_clear();
}

void console_scroll(unsigned int lines)
{
    if(lines >= CONSOLE_HEIGHT)
    {
        console_clear();
        console_row = 0;
    }
    else
    {
        size_t copy_count = lines * CONSOLE_WIDTH;
        size_t copy_read = (CONSOLE_HEIGHT - lines) * CONSOLE_WIDTH;
        
        for(size_t i = 0; i < copy_count; ++i)
            console_buffer[i] = console_buffer[copy_read + i];
        
        for(size_t i = copy_read; i < CONSOLE_WIDTH * CONSOLE_HEIGHT; ++i)
            console_buffer[i] = vga_entry(' ', console_color);
    }
    
    console_row = console_row >= lines ? console_row - lines : 0;
}

static void console_advance_row(unsigned int rows)
{
    console_row += rows;
    if(rows >= CONSOLE_HEIGHT)
    {
        console_scroll(console_row - CONSOLE_HEIGHT + 1);
        console_row = CONSOLE_HEIGHT - 1;
    }
}
static unsigned int console_advance_column(unsigned int columns)
{
    size_t new_column = (console_column + columns) % CONSOLE_WIDTH;
    size_t retval = (console_column + columns) / CONSOLE_WIDTH;
    console_column = new_column;
    return retval;
}

void console_advance_cursor(unsigned int rows, unsigned int columns)
{
    unsigned int row_off = console_advance_column(columns);
    console_advance_row(rows + row_off);
}

void console_carriage_return()
{
    console_column = 0;
}

void console_new_line()
{
    console_advance_row(1);
    console_column = 0;
}

void console_put_char(char c)
{
    console_buffer[console_row * CONSOLE_WIDTH + console_column] = vga_entry(c, console_color);
    if(++console_column == CONSOLE_WIDTH)
        console_column = 0, ++console_row;
    if(console_row == CONSOLE_HEIGHT)
        console_scroll(1);
}

void console_write_line(const char *str)
{
    while(*str != 0)
        console_put_char(*str++);
    console_new_line();
}
