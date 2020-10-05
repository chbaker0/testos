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
    db 0b10010010
    ; Flags and limit 19:16
    db 0b00001111
    ; Base 31:24
    db 0
.pointer:
    dw .pointer - GDT - 1
    dq GDT

extern loader_main

global _start
_start:
    mov byte [0xb8000], 'Z'

    ; Pass multiboot info structure
    push ebx
    call loader_main

    cli

    .hang:
    hlt
    jmp .hang

global kernel_handoff
kernel_handoff:
    push ebp
    mov ebp, esp

    ; Set page table address.
    mov eax, [ebp + 12]
    mov eax, [eax]
    mov cr3, eax

    ; Enable PAE
    mov eax, cr4
    or eax, 0b100000
    mov cr4, eax

    ; Enable long mode
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    lgdt [GDT.pointer]
    mov ax, 0x10
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    jmp 0x8:long_mode

[bits 64]
long_mode:
    xchg bx, bx
    mov edi, [ebp + 8]
    mov edi, [edi]
    mov esi, [ebp + 20]
    mov eax, [ebp + 16]
    mov rax, [eax]
    jmp rax

    .hang
    hlt
    jmp .hang
