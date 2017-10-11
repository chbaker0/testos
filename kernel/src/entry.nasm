[bits 64]

section .bss
global kernel_stack_begin
global kernel_stack_end
kernel_stack_begin:
    alignb 4096
    resb 1024*1024
kernel_stack_end:

section .text

extern kinit
global kentry
kentry:
    mov rsp, kernel_stack_end
    mov rbp, rsp

    call kinit

.hang:
    hlt
    jmp .hang
