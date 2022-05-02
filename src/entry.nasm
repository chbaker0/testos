[BITS 32]

; SECTION .bss

; init_stack_bottom: resb 4096*32
; init_stack_top:

SECTION .bootstrap

global _start
_start:
    mov byte [0xb8000], 'F'
    mov byte [0xb8002], 'U'
    mov byte [0xb8004], 'C'
    mov byte [0xb8006], 'K'

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
