#!/usr/bin/env sh

set -e

cd loader
cargo build
cd ../kernel
cargo build
cd ..

mkdir -p out/iso/boot/grub
cp grub.cfg out/iso/boot/grub
cp loader/target/i686-unknown-none/debug/loader out/iso/boot
cp kernel/target/x86_64-unknown-none/debug/kernel out/iso/boot
grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso
