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

align 4096
PML4T:
    dq 0b0_00000000000_0000000000000000000000000000000000000000_0000_00_000011
    times 511 dq 0
align 4096
PDPT:
    dq 0b0_00000000000_0000000000000000000000000000000000000000_0000_00_000011
    times 511 dq 0
align 4096
PDT:
    dq 0b0_00000000000_0000000000000000000000000000000000000000_0000_00_000011
    dq 0b0_00000000000_0000000000000000000000000000000000000000_0000_00_000011
    times 511 dq 0
align 4096
PT:
    times 1024 dq 0b0_00000000000_0000000000000000000000000000000000000000_0000_00_000011

extern kmain

global _start
_start:
    ; Copy multiboot info structure
    mov ecx, 116
    mov esi, ebx
    mov edi, multiboot_info
    rep movsb

    ; Identity map first 4 MiB

    mov eax, [PML4T]
    or eax, PDPT
    mov [PML4T], eax

    mov eax, [PDPT]
    or eax, PDT
    mov [PDPT], eax

    mov eax, [PDT]
    or eax, PT
    mov [PDT], eax

    mov eax, [PDT+8]
    or eax, PT+4096
    mov [PDT+8], eax

    mov ecx, 0
.ptloop:
    mov edx, ecx
    shl edx, 3
    mov eax, [PT+edx]
    mov ebx, ecx
    shl ebx, 12
    or eax, ebx
    mov [PT+edx], eax
    add ecx, 1
    cmp ecx, 1024
    jne .ptloop

    mov eax, PML4T
    mov cr3, eax

    mov eax, cr4
    or eax, 0b100000
    mov cr4, eax

    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    mov eax, cr0
    or eax, 0b1000_0000_0000_0000_0000_0000_0000_0000
    mov cr0, eax

    lgdt [GDT.pointer]
    jmp 0x8:_kernel_handoff


[bits 64]

_kernel_handoff:
    mov byte [0xb8000], 'A'
    ; mov esp, stack_top

    ; push ebx
    ; call kmain

    cli

    .hang:
    hlt
    jmp .hang
