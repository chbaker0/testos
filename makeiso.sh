#!/usr/bin/env sh

mkdir -p out/iso/boot/grub
cp out/kernel.bin out/iso/boot
cp grub.cfg out/iso/boot/grub
grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso
