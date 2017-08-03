#ifndef _TERMINAL_H_INCLUDED_
#define _TERMINAL_H_INCLUDED_

#include <stdint.h>

#define TERMINAL_HEIGHT 1024
#define TERMINAL_WIDTH 80

struct terminal_buffer
{
    uint32_t bottom_line;
    // Circular array of lines.
    char buf[TERMINAL_WIDTH * TERMINAL_HEIGHT];
};

void terminal_init(struct terminal_buffer *tb);
void terminal_write_line(struct terminal_buffer *tb, const char *str);

#endif // _TERMINAL_H_INCLUDED_
