[BITS 32]

; Multiboot header
MAGIC equ 0x1BADB002
FLAGS equ 0000_0000_0000_0000_0000_0000_0000_0111

SECTION .multiboot
dd MAGIC
dd FLAGS
dd -(MAGIC + FLAGS) 

SECTION .boot_stack nobits
stack_bottom: resb 16384
stack_top:

SECTION .text

extern kmain

global _start
_start:
    mov esp, stack_top
    
    call kmain
    
    cli

    .hang:
    hlt
    jmp .hang
