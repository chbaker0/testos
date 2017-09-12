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
stack_bottom: resb 4096 * 32
stack_top:

SECTION .bss
multiboot_info:
    resb 128

SECTION .text

GDT:
.null_descriptor:
    dq 0
.code_segment_descriptor:
    ; Limit 15:0
    dw 0xFFFF
    ; Base 15:0
    dw 0
    ; Base 23:16
    db 0
    ; Access
    db 0b10011010
    ; Flags and limit 19:16
    db 0b00101111
    ; Base 31:24
    db 0
.data_segment_descriptor:
    ; Limit 15:0
    dw 0xFFFF
    ; Base 15:0
    dw 0
    ; Base 23:16
    db 0
    ; Access
    db 0b10011010
    ; Flags and limit 19:16
    db 0b00001111
    ; Base 31:24
    db 0
.pointer:
    dw .pointer - GDT - 1
    dq GDT

extern loader_entry

global _start
_start:
    mov byte [0xb8000], 'A'

    push ebx
    call loader_entry

    cli

    .hang:
    hlt
    jmp .hang
