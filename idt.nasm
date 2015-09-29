[BITS 32]

SECTION .text

global idt_load
idt_load:
	mov eax, [esp + 4]
	mov bx, [esp + 8]
	shl bx, 3
	sub esp, 6
	mov [esp], bx
	mov [esp + 2], eax
	lidt [esp]
	add esp, 6
	ret
