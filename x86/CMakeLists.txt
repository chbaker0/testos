add_library(x86 OBJECT gdt.c gdt.nasm idt.c idt.nasm interrupt.nasm interrupt.c helpers.nasm boot.nasm)
