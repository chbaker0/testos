[bits 64]

section .bss
global kernel_stack_begin
global kernel_stack_end
kernel_stack_begin:
    alignb 4096
    resb 1024*1024
kernel_stack_end:

section .text

hang:
    hlt
    jmp hang

extern kinit
global kentry
kentry:
    mov rsp, kernel_stack_end
    mov rbp, rsp

    call kinit

    jmp hang

global switch_stacks
switch_stacks:
    ; Parameters: function pointer, stack pointer
    mov rbp, rsi
    mov rsp, rsi
    call rdi
