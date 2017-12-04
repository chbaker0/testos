ifdef NDEBUG
	CARGO_BUILD_FLAGS = --release
else
	CARGO_BUILD_FLAGS =
endif

.PHONY: all
all: out/kernel.iso

.PHONY: clean
clean:
	rm out/boot.nasm.o out/rustsrc.a out/kernel.bin out/kernel.iso

out/boot.nasm.o: boot.nasm
	nasm -f elf32 -o $@  $^

out/kernel.a: FORCE
	cd kernel; RUST_TARGET_PATH=`pwd`/../targets xargo +nightly rustc $(CARGO_BUILD_FLAGS) --target x86_64-unknown-none -- --emit link=../out/kernel.a

out/loader.a: FORCE
	cd loader; RUST_TARGET_PATH=`pwd`/../targets xargo +nightly rustc $(CARGO_BUILD_FLAGS) --target i686-unknown-none -- --emit link=../out/loader.a

out/kernel.bin: kernel.ld out/kernel.a
	x86_64-elf-gcc -g -mcmodel=kernel -T kernel.ld -z max-page-size=0x1000 -Wl,--gc-sections -nostdlib -lgcc -o $@ out/kernel.a out/acpica.a
	objcopy --only-keep-debug out/kernel.bin out/kernel.sym
	objcopy --strip-debug out/kernel.bin

out/loader.bin: loader.ld out/boot.nasm.o out/loader.a
	i686-elf-gcc -g -T loader.ld -z max-page-size=0x1000 -Wl,--gc-sections -nostdlib -lgcc -o $@ out/boot.nasm.o out/loader.a
	objcopy --only-keep-debug out/loader.bin out/loader.sym
	objcopy --strip-debug out/loader.bin

out/kernel.iso: out/kernel.bin out/loader.bin grub.cfg
	mkdir -p out/iso/boot/grub
	cp out/loader.bin out/iso/boot
	cp out/kernel.bin out/iso/boot
	cp grub.cfg out/iso/boot/grub
	grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso

ACPICA_SOURCES := $(wildcard acpica/*.c)
ACPICA_OBJECTS := $(patsubst acpica/%.c, out/acpica/%.o, $(ACPICA_SOURCES))

out/acpica/%.o: acpica/%.c
	x86_64-elf-gcc -c -o $@ -Iacpica $<

out/acpica.a: $(ACPICA_OBJECTS)
	x86_64-elf-ar rcs out/acpica.a $^

.PHONY: FORCE
FORCE:
