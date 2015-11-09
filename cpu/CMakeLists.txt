add_library(cpu OBJECT port.c gdt.c gdt.nasm idt.c idt.nasm interrupt.nasm interrupt.c apic.c helpers.nasm)
