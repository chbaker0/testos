#include <stddef.h>

#include "terminal.h"

void terminal_buffer_init(struct terminal_buffer *tb)
{
    tb->bottom_line = 24;

    for (size_t i = 0; i < TERMINAL_WIDTH * TERMINAL_HEIGHT; ++i)
    {
        tb->buf[i] = ' ';
    }
}

// Writes string to bottom of terminal. Assumes input string is no
// longer than TERMINAL_WIDTH.
static void write_line_impl(struct terminal_buffer *tb, const char *str, size_t str_length)
{
    // Clear bottom line of buffer.
    for(unsigned int i = 0; i < TERMINAL_WIDTH; ++i)
		tb->buf[tb->bottom_line * TERMINAL_WIDTH + i] = 0;

    // Copy string.
    for (size_t i = 0; i < str_length; ++i)
    {
        tb->buf[tb->bottom_line * TERMINAL_WIDTH + i] = str[i];
    }
}

void terminal_write_line(struct terminal_buffer *tb, const char *str)
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
        // Write next 80 characters (or rest of string).
        size_t line_length = (str_length - cur_ndx) < 80 ? (str_length - cur_ndx) : 80;
        write_line_impl(tb, str + cur_ndx, line_length);
        cur_ndx += line_length;

        // Scroll and clear bottom line.
        tb->bottom_line = (tb->bottom_line + 1) % TERMINAL_HEIGHT;
        for(unsigned int i = 0; i < TERMINAL_WIDTH; ++i)
            tb->buf[tb->bottom_line * TERMINAL_WIDTH + i] = 0;
    }
}
