[BITS 32]

SECTION .bss
gdtr:
	dw 0
	dd 0
	
SECTION .text

global gdt_load
gdt_load:
	mov eax, [esp + 4]
	mov [gdtr + 2], eax
	mov ax, [esp + 8]
	mov [gdtr], ax
	lgdt [gdtr]
	ret
