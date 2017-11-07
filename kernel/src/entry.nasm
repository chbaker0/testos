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

global context_init_asm
context_init_asm:
    ; Parameters: new context's stack pointer, entry point
    ; Returns: modified stack pointer

    ; Put entry point on stack
    mov qword [rdi-8], rsi
    sub rdi, 8

    ; Set saved registers to 0
    mov qword [rdi-8], 0        ; rbx
    mov qword [rdi-16], 0       ; rbp
    mov qword [rdi-24], 0       ; r12
    mov qword [rdi-32], 0       ; r13
    mov qword [rdi-40], 0       ; r14
    mov qword [rdi-48], 0       ; r15
    mov qword [rdi-56], 0b1000000000000000000010 ; rflags

    sub rdi, 56
    mov rax, rdi

    ret

global context_switch_asm
context_switch_asm:
    ; Parameters: stack pointer, pointer to location to store old stack pointer

    ; Save current context's registers
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    pushf

    ; Switch stacks
    mov [rsi], rsp
    mov rsp, rdi

    ; Restore new context's registers
    popf
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    ret
