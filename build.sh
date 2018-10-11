#!/usr/bin/env sh

set -e

# Build kernel crate for our custom 64-bit target.
cd kernel
cargo +nightly xbuild --target ../targets/x86_64-unknown-none.json
cd ..

# Build loader crate for our custom 32-bit target.
cd loader
cargo +nightly xbuild --target ../targets/i686-unknown-none.json
cd ..

# Assemble boot.nasm which contains the multiboot header for the second-stage bootloader and
nasm -f elf32 -o out/boot.nasm.o boot.nasm

# Link kernel image from cargo-built image using linker script kernel.ld.
x86_64-elf-ld -g -T kernel.ld -z max-page-size=0x1000 --gc-sections -o out/kernel kernel/target/x86_64-unknown-none/debug/libkernel.a
objcopy --only-keep-debug out/kernel out/kernel.sym
objcopy --strip-debug out/kernel

# Link loader image from cargo-built image using linker script loader.ld.
i686-elf-ld -g -T loader.ld -z max-page-size=0x1000 --gc-sections -o out/loader out/boot.nasm.o loader/target/i686-unknown-none/debug/libloader.a
objcopy --only-keep-debug out/loader out/loader.sym
objcopy --strip-debug out/loader

# Create bootable ISO with grub.
mkdir -p out/iso/boot/grub
cp grub.cfg out/iso/boot/grub
cp out/kernel out/iso/boot
cp out/loader out/iso/boot
grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso
