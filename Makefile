CC = i686-elf-gcc
CXX = i686-elf-g++

CFLAGS = -std=gnu99 -nostdlib -ffreestanding -O2 -Wall -Wextra -pedantic
LDFLAGS = -T linker.ld -lgcc
NASMFLAGS = -f elf

CCOMPILE = $(CC) $(CFLAGS) -c
CLINK = $(CC) $(CFLAGS) $(LDFLAGS)
ASM = nasm $(NASMFLAGS)

.PHONY: all
all: os.bin

os.bin: boot.o kernel.o console.o
	$(CLINK) boot.o kernel.o console.o -o os.bin

boot.o: boot.s
	$(ASM) boot.s -o boot.o

console.o: console.c console.h
	$(CCOMPILE) console.c -o console.o

kernel.o: kernel.c console.h port.h
	$(CCOMPILE) kernel.c -o kernel.o

.PHONY: clean
clean:
	rm *.o os.bin