[BITS 32]

SECTION .text
	
extern __interrupt_handlers
	
%macro handler 1

__isr_%1:
	pushad
	mov eax, __interrupt_handlers + (%1) * 4
	test eax, eax
	jz .no_handler
	cld
	call [eax]
.no_handler:
	popad
	iret

%endmacro

%assign i 0
%rep 256

	handler i

%assign i i+1
%endrep

SECTION .data

global __isr_trampolines
__isr_trampolines:
%assign i 0
%rep 256

	dd __isr_%[i]

%assign i i+1
%endrep

