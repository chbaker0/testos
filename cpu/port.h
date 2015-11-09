#ifndef _PORT_H_
#define _PORT_H_

#include <stdint.h>

uint8_t port_read_8(uint16_t port);
uint16_t port_read_16(uint16_t port);
uint32_t port_read_32(uint16_t port);

void port_write_8(uint16_t port, uint8_t val);
void port_write_16(uint16_t port, uint16_t val);
void port_write_32(uint16_t port, uint32_t val);

void port_wait();

#endif // _PORT_H_
