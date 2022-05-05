[BITS 32]

SECTION .bootstrap.text

global _start
_start:
    mov byte [0xb8000], 'F'
    mov byte [0xb8002], 'U'
    mov byte [0xb8004], 'C'
    mov byte [0xb8006], 'K'

    ; Magic number check
    cmp eax, 0x36d76289
    jne .hang

    mov byte [0xb8000], 'D'

    ; Save MB2 structure
    mov [multiboot_ptr], ebx
    ; Use bootstrap stack
    mov esp, init_stack_top
    mov ebp, esp

    ; Set up top-level entries for identity and higher-half mapping
    mov eax, PDPT_LOWER
    or eax, PAGE_BITS
    mov [PML4_LOWER], eax
    mov eax, PDPT_HIGHER
    or eax, PAGE_BITS
    mov [PML4_HIGHER], eax

    ; Map first GB in PDPT_LOWER
    mov eax, PDT
    or eax, PAGE_BITS
    mov [PDPT_LOWER], eax

    ; Map second-to-last GB in PDPT_HIGHER
    mov eax, PDT
    or eax, PAGE_BITS
    mov [PDPT_HIGHER+510*8], eax

    ; Make 512 PDT entries, each pointing to one of our PT
    xor ecx, ecx
    .pdt_loop:
    ; Each level-1 page table is 512 * 8 = 4096 bytes. Each PDT entry points to
    ; a successive L1 table. 2^12 = 4096, so PT[i] is at PT + i*2^12.
    mov eax, ecx
    shl eax, 12
    add eax, PT
    or eax, PAGE_BITS
    mov [PDT + 8*ecx], eax

    inc ecx
    cmp ecx, 512
    jl .pdt_loop

    ; Make 512 PT entries in each of the 512 L1 page tables. We are mapping a
    ; contiguous block of physical memory so we can do it in one shot.
    xor ecx, ecx
    .pt_loop:
    mov eax, ecx
    shl eax, 12
    or eax, PAGE_BITS
    mov [PT + 8*ecx], eax

    inc ecx
    cmp ecx, 512*512
    jl .pt_loop

    ;
    ; Begin handoff
    ;

    ; Set page root pointer
    mov eax, PML4T
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

    ; Enable paging
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

    ; We have to do the jump in two stages: we are jumping from the first GB of
    ; memory to the second-to-last, plus we need to do a "far jump" to load our
    ; new GDT segment. x86 instruction encoding doesn't let us do the segment
    ; jump and a 64-bit absolute jump at the same time.
    ;
    ; So first, load our code segment and officially switch to 64-bit mode.
    jmp 0x8:long_mode_trampoline

    .hang:
    hlt
    jmp .hang

[BITS 64]
long_mode_trampoline:
    ; Now we can simply get the absolute address of our higher-half entry point
    ; and jump to it.
    mov rax, long_mode
    jmp rax

[BITS 32]

; extern kernel_entry

; global _start
; _start:
;     ; Args: boot_info_addr [rdi]

;     mov byte [0xb8000], 'Z'

;     mov rsp, init_stack_top
;     call kernel_entry

;     mov byte [0xb8000], '?'

; .hang:
;     hlt
;     jmp .hang

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

PAGE_BITS equ 0000_0011b        ; Normal caching, supervisor only, writable, present.
LARGE_PAGE_BITS equ 0100_0011b    ; Same as above, but page size bit is on

SECTION .bootstrap.data

multiboot_ptr: dq 0

; Bootstrapping paging tables. On boot linear addresses = physical addresses.
; We map 512 * 2 MB pages to map the first 1GB of physical memory.
;
; We actually map this 1GB two times:
;   1. Into linear address range 0-1GB (identity map)
;   2. Into linear address range (-2GB) - (-1GB) (higher half kernel base)

SECTION .bootstrap.bss

init_stack_bottom: resb 4096*32
init_stack_top:

align 4096
; The lower entry corresponds to the lower 256 TB of memory, and the upper to
; the upper 256 TB.
PML4T:
PML4_LOWER:
    resq 1
PML4_UNUSED:
    resq 510
PML4_HIGHER:
    resq 1

; Kernel is mapped to two base addresses: 0 and -2G (beginning at last two GB
; of linear addresses). Each entry corresponds to 1G of linear space. We use two
; tables, corresponding to our two non-zero entries in the PML4T.
align 4096
PDPT_LOWER:
    resq 512
PDPT_HIGHER:
    resq 512

align 4096
; Both the lower and upper half spaces map to the same thing: the first 1GB of
; physical memory. One PDT maps 1 GB of memory, so we can re-use the same table.
PDT:
    resq 512

; Just like above, we reuse the same PT.
align 4096
PT:
    resq 512*512


[BITS 64]
SECTION .text

extern KERNEL_BASE
extern kernel_entry

long_mode:
    ; Now we are in 64-bit mode and running in the higher half. Hand control to
    ; the Rust entry point.

    mov byte [0xb8000], 'L'

    ; "Call" with multiboot info pointer as argument. kernel_entry does not
    ; return. Note that our multiboot_ptr is linked in the bottom half; we must
    ; offset it with the kernel base address.
    mov edi, [multiboot_ptr]
    add rdi, KERNEL_BASE
    jmp kernel_entry

    .hang:
    hlt
    jmp .hang
