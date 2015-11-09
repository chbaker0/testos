#include "pic.h"

#include "port.h"

#define PIC_MASTER_COMMAND_PORT 0x20
#define PIC_MASTER_DATA_PORT 0x21
#define PIC_SLAVE_COMMAND_PORT 0xA0
#define PIC_SLAVE_DATA_PORT 0xA1

void pic_remap(uint8_t irq0_offset, uint8_t irq8_offset)
{
	unsigned char mask1, mask2;

	mask1 = port_read_8(PIC_MASTER_DATA_PORT);
	mask2 = port_read_8(PIC_SLAVE_DATA_PORT);

	port_write_8(PIC_MASTER_COMMAND_PORT, 0x11);
	port_wait();
	port_write_8(PIC_SLAVE_COMMAND_PORT, 0x11);
	port_wait();
	port_write_8(PIC_MASTER_DATA_PORT, irq0_offset);
	port_wait();
	port_write_8(PIC_SLAVE_DATA_PORT, irq8_offset);
	port_wait();
	port_write_8(PIC_MASTER_DATA_PORT, 4);
	port_wait();
	port_write_8(PIC_SLAVE_DATA_PORT, 2);
	port_wait();
	port_write_8(PIC_MASTER_DATA_PORT, 1);
	port_wait();
	port_write_8(PIC_SLAVE_DATA_PORT, 1);
	port_wait();

	port_write_8(PIC_MASTER_DATA_PORT, mask1);
	port_write_8(PIC_SLAVE_DATA_PORT, mask2);
}
