[BITS 32]

; Multiboot header
MAGIC equ 0x1BADB002
FLAGS equ 0000_0000_0000_0000_0000_0000_0000_0111b

SECTION .multiboot
	; Required fields
	dd MAGIC
	dd FLAGS
	dd -(MAGIC + FLAGS)
	; Load addresses (unused here)
	dd 0
	dd 0
	dd 0
	dd 0
	dd 0
	; Video mode info
	dd 1
	dd 132
	dd 60
	dd 0

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
