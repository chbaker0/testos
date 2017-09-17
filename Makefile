.PHONY: all
all: out/kernel.iso

.PHONY: clean
clean:
	rm out/boot.nasm.o out/rustsrc.a out/kernel.bin out/kernel.iso

out/boot.nasm.o: boot.nasm
	nasm -f elf64 -o $@  $^

out/kernel.a: FORCE
	cd kernel; RUST_TARGET_PATH=`pwd`/../targets xargo +nightly rustc --target x86_64-unknown-none -- --emit link=../out/kernel.a

out/kernel.bin: linker.ld out/boot.nasm.o out/kernel.a
	x86_64-elf-gcc -mcmodel=kernel -T linker.ld -z max-page-size=0x1000 -Wl,--gc-sections -nostdlib -lgcc -o $@ out/boot.nasm.o out/rustsrc.a

out/kernel.iso: out/kernel.bin grub.cfg
	mkdir -p out/iso/boot/grub
	cp out/kernel.bin out/iso/boot
	cp grub.cfg out/iso/boot/grub
	grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso

.PHONY: FORCE
FORCE:
