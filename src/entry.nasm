[BITS 32]

SECTION .bootstrap.text

global _start
_start:
    mov byte [0xb8000], 'F'
    mov byte [0xb8002], 'U'
    mov byte [0xb8004], 'C'
    mov byte [0xb8006], 'K'

    mov esp, init_stack_top

    .hang:
    hlt
    jmp .hang

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

SECTION .bootstrap.data

; Bootstrapping paging tables. On boot linear addresses = physical addresses.
; We map 512 * 2 MB pages

PAGE_BITS equ 0000_0011b        ; Normal caching, supervisor only, writable, present.
LARGE_PAGE_BITS equ 0100_0011    ; Same as above, but page size bit is on

align 4096
PML4T:
PML4E:
    times 512 dq PAGE_BITS

align 4096
PDPT:
    times 512 dq PAGE_BITS

align 4096
PDT:
    times 512 dq LARGE_PAGE_BITS

SECTION .bootstrap.bss

init_stack_bottom: resb 4096*32
init_stack_top:
