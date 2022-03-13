[BITS 64]

SECTION .bss

init_stack_bottom: resb 4096*32
init_stack_top:

SECTION .text

extern kernel_entry

global _start
_start:
    ; Args: boot_info_addr [rdi]

    mov byte [0xb8000], 'Z'

    mov rsp, init_stack_top
    call kernel_entry

    mov byte [0xb8000], '?'

.hang:
    hlt
    jmp .hang
