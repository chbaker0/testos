#ifndef _PORT_H_
#define _PORT_H_

#include <stdint.h>

inline uint8_t port_read_8(uint16_t port)
{
	uint8_t result;
	asm volatile ("inb %1, %0" : "=a" (result) : "d" (port));
	return result;
}
inline uint16_t port_read_16(uint16_t port)
{
	uint16_t result;
	asm volatile ("inw %1, %0" : "=a" (result) : "d" (port));
	return result;
}
inline uint32_t port_read_32(uint16_t port)
{
	uint32_t result;
	asm volatile ("inl %1, %0" : "=a" (result) : "d" (port));
	return result;
}

inline void port_write_8(uint16_t port, uint8_t val)
{
	asm volatile ("outb %1, %0" : : "d" (port), "a" (val));
}
inline void port_write_16(uint16_t port, uint16_t val)
{
	asm volatile ("outw %1, %0" : : "d" (port), "a" (val));
}
inline void port_write_32(uint16_t port, uint32_t val)
{
	asm volatile ("outl %1, %0" : : "d" (port), "a" (val));
}

#endif // _PORT_H_