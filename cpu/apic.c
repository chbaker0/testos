#include "apic.h"

#include "port.h"

#define LOCAL_APIC_DEFAULT_BASE 0xFEE00000

// Register offsets
#define LOCAL_APIC_REGISTER_ID 0x20
#define LOCAL_APIC_REGISTER_VERSION 0x30
#define LOCAL_APIC_REGISTER_TPR 0x80
#define LOCAL_APIC_REGISTER_APR 0x90
#define LOCAL_APIC_REGISTER_PPR 0xA0
#define LOCAL_APIC_REGISTER_EOI 0xB0
#define LOCAL_APIC_REGISTER_RRD 0xC0
#define LOCAL_APIC_REGISTER_LOGICAL_DEST 0xD0
#define LOCAL_APIC_REGISTER_DEST_FORMAT 0xE0
#define LOCAL_APIC_REGISTER_SPURIOUS_VECTOR 0xF0
// TODO: Fill in the rest of these registers
#define LOCAL_APIC_REGISTER_ICR_0_31 0x300
#define LOCAL_APIC_REGISTER_ICR_32_63 0x310

// Register bits
#define LOCAL_APIC_SPURIOUS_VECTOR_BITS 0xFF
#define LOCAL_APIC_SOFTWARE_ENABLE_BIT 0x100

static char * apic_base;

static void write_reg(uintptr_t offset, uint32_t val)
{
	*(uint32_t*)(apic_base + offset) = val;
}

static uint32_t read_reg(uintptr_t offset)
{
	return *(uint32_t*)(apic_base + offset);
}

void local_apic_init(uint8_t spurious)
{
	// First disable the PIC
	port_write_8(0xA1, 0xFF);
	port_write_8(0x21, 0xFF);

	apic_base = (char *) LOCAL_APIC_DEFAULT_BASE;

	write_reg(LOCAL_APIC_REGISTER_SPURIOUS_VECTOR,
			  read_reg(LOCAL_APIC_REGISTER_SPURIOUS_VECTOR)
			  | LOCAL_APIC_SOFTWARE_ENABLE_BIT
			  | spurious);
}
