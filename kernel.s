	.file	"kernel.c"
	.globl	flat_gdt
	.bss
	.align 16
	.type	flat_gdt, @object
	.size	flat_gdt, 24
flat_gdt:
	.zero	24
	.comm	idt_entries,2048,32
	.section	.rodata
.LC0:
	.string	"Bye"
	.text
	.globl	panic
	.type	panic, @function
panic:
.LFB6:
	.cfi_startproc
	pushq	%rbp
	.cfi_def_cfa_offset 16
	.cfi_offset 6, -16
	movq	%rsp, %rbp
	.cfi_def_cfa_register 6
	movl	$.LC0, %edi
	call	console_write_line
#APP
# 17 "kernel.c" 1
	cli
# 0 "" 2
#NO_APP
.L2:
#APP
# 19 "kernel.c" 1
	hlt
# 0 "" 2
#NO_APP
	jmp	.L2
	.cfi_endproc
.LFE6:
	.size	panic, .-panic
	.section	.rodata
.LC1:
	.string	"Hello world from a kernel!"
.LC2:
	.string	"This is just a test."
	.align 8
.LC3:
	.ascii	"ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGH"
	.ascii	"IJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOP"
	.ascii	"QRSTUVWXYZABCD"
	.string	"EFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ"
.LC4:
	.string	"Test number 2"
.LC5:
	.string	"Test number 3"
.LC6:
	.string	"Test scroll"
	.text
	.globl	kmain
	.type	kmain, @function
kmain:
.LFB7:
	.cfi_startproc
	pushq	%rbp
	.cfi_def_cfa_offset 16
	.cfi_offset 6, -16
	movq	%rsp, %rbp
	.cfi_def_cfa_register 6
	pushq	%rbx
	subq	$24, %rsp
	.cfi_offset 3, -24
	movl	$12, %r8d
	movl	$0, %ecx
	movl	$138, %edx
	movl	$1048575, %esi
	movl	$0, %edi
	call	gdt_make_entry
	movq	%rax, flat_gdt+8(%rip)
	movl	$12, %r8d
	movl	$0, %ecx
	movl	$130, %edx
	movl	$1048575, %esi
	movl	$0, %edi
	call	gdt_make_entry
	movq	%rax, flat_gdt+16(%rip)
	movl	$3, %esi
	movl	$flat_gdt, %edi
	call	gdt_load
	movl	$16, %esi
	movl	$8, %edi
	call	helpers_reload_all_segments
	movl	$0, -20(%rbp)
	jmp	.L4
.L5:
	movl	-20(%rbp), %eax
	movzbl	%al, %eax
	movl	%eax, %edi
	call	interrupt_get_trampoline_addr
	movq	%rax, -32(%rbp)
	movl	-20(%rbp), %eax
	movzbl	%al, %eax
	movl	$panic, %esi
	movl	%eax, %edi
	call	interrupt_set_handler
	movq	-32(%rbp), %rax
	movl	-20(%rbp), %ebx
	movl	$1, %r8d
	movl	$0, %ecx
	movl	$14, %edx
	movl	$8, %esi
	movl	%eax, %edi
	call	idt_make_entry
	movq	%rax, idt_entries(,%rbx,8)
	addl	$1, -20(%rbp)
.L4:
	cmpl	$255, -20(%rbp)
	jbe	.L5
	movl	$0, %eax
	call	console_init
	movl	$.LC1, %edi
	call	console_write_line
	movl	$.LC2, %edi
	call	console_write_line
	movl	$.LC3, %edi
	call	console_write_line
	movl	$3, %esi
	movl	$2, %edi
	call	console_advance_cursor
	movl	$.LC4, %edi
	call	console_write_line
	movl	$.LC5, %edi
	call	console_write_line
	movl	$2, %edi
	call	console_scroll
	movl	$.LC6, %edi
	call	console_write_line
#APP
# 56 "kernel.c" 1
	int 0x80
# 0 "" 2
#NO_APP
	nop
	addq	$24, %rsp
	popq	%rbx
	popq	%rbp
	.cfi_def_cfa 7, 8
	ret
	.cfi_endproc
.LFE7:
	.size	kmain, .-kmain
	.ident	"GCC: (GNU) 5.2.0"
	.section	.note.GNU-stack,"",@progbits
