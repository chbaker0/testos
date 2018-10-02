#!/usr/bin/env sh

set -e

cd kernel
cargo +nightly xbuild --target ../targets/x86_64-unknown-none.json
cd ..

cd loader
cargo +nightly xbuild --target ../targets/i686-unknown-none.json
cd ..

nasm -f elf32 -o out/boot.nasm.o boot.nasm

x86_64-elf-ld -g -T kernel.ld -z max-page-size=0x1000 --gc-sections -o out/kernel kernel/target/x86_64-unknown-none/debug/libkernel.a
i686-elf-ld -g -T loader.ld -z max-page-size=0x1000 --gc-sections -o out/loader out/boot.nasm.o loader/target/i686-unknown-none/debug/libloader.a

mkdir -p out/iso/boot/grub
cp out/kernel out/iso/boot
cp out/loader out/iso/boot
grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso
