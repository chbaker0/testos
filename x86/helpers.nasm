[BITS 32]

SECTION .text:

global helpers_reload_all_segments
helpers_reload_all_segments:
	xchg bx, bx
	xor ecx, ecx
	xor eax, eax
	mov cx, word [esp + 8] 			; New data segments
	mov ax, word [esp + 4] 			; New code segment
	mov es, cx
	mov ds, cx
	mov es, cx
	mov fs, cx
	mov gs, cx
	mov ss, cx
	sub esp, 6
	mov dword [esp], .reload_cs
	mov [esp + 4], ax
	jmp far [esp]
.reload_cs:
	add esp, 6
	ret
