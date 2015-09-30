[BITS 32]

extern __interrupt_handlers
	
%macro handler 1

__isr_%1:
	pushad
	cld
	call [__interrupt_handlers + (%1) * 4]
	popad
	iret

%endmacro

%assign i 0
%rep 256

	handler i

%assign i i+1
%endrep
