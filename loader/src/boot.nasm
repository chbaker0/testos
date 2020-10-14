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

SECTION .bss
stack_bottom: resb 4096 * 128
stack_top:

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

    ; Set up stack
    mov esp, stack_top
    push dword 0
    push dword 0
    mov ebp, esp

    ; Pass multiboot info structure
    push ebx
    call loader_main

    cli

    .hang:
    hlt
    jmp .hang

global kernel_handoff
kernel_handoff:
    ; Args: page_table_addr, kernel_entry_addr

    push ebp
    mov ebp, esp

    ; Set page table address.
    mov eax, [ebp + 8]
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

    ; Get the entry point address and jump to it.
    mov eax, [ebp + 12]
    jmp rax

    .hang:
    hlt
    jmp .hang
