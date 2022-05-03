[BITS 32]

SECTION .bootstrap.text

extern kernel_entry

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

    ; Set up top-level entries for identity and higher-half mapping.
    mov eax, PDPT
    or eax, PAGE_BITS
    mov [PML4T], eax
    mov [PML4T+510], eax

    ; Only need one PDPT entry since it represents an entire GB
    mov eax, PDT
    or eax, PAGE_BITS
    mov [PDPT], eax

    ; Set up 2MB pages in PDT.
    xor ecx, ecx
    .pdt_loop:
    ; PDT entries already are init to LARGE_PAGE_BITS; simply or the phys addr
    ; in to the entry.
    mov eax, ecx
    shl eax, 21
    or eax, LARGE_PAGE_BITS
    mov [PDT + 8*ecx], eax

    ; On to the next one
    inc ecx
    cmp ecx, 512
    jl .pdt_loop

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
    jmp 0x8:long_mode

    .hang:
    hlt
    jmp .hang

[BITS 64]
long_mode:
    ; Call main entry point with multiboot info pointer as argument.
    mov edi, [multiboot_ptr]
    jmp .hang
    jmp kernel_entry

    .hang:
    hlt
    jmp .hang

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

SECTION .bootstrap.data

multiboot_ptr: dq 0

; Bootstrapping paging tables. On boot linear addresses = physical addresses.
; We map 512 * 2 MB pages to map the first 1GB of physical memory.
;
; We actually map this 1GB two times:
;   1. Into linear address range 0-1GB (identity map)
;   2. Into linear address range (-2GB) - (-1GB) (higher half kernel base)
;
; (1) corresponds to PML4T[0], (2) to PML4T[510]

PAGE_BITS equ 0000_0011b        ; Normal caching, supervisor only, writable, present.
LARGE_PAGE_BITS equ 0100_0011b    ; Same as above, but page size bit is on


SECTION .bootstrap.data.page_tables
align 4096
PML4T:
    times 512 dq 0

align 4096
PDPT:
    times 512 dq 0

align 4096
PDT:
    times 512 dq 0

SECTION .bootstrap.bss

init_stack_bottom: resb 4096*32
init_stack_top:
