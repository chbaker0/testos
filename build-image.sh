#!/usr/bin/env sh

set -e

if [ "$RELEASE" = "1" ]
then
    OUT_PREFIX="release"
    CARGO_ARGS="--release"
else
    OUT_PREFIX="debug"
    CARGO_ARGS=""
fi

cd loader
cargo build $CARGO_ARGS
cd ../kernel
cargo build $CARGO_ARGS
cd ..

mkdir -p out/iso/boot/grub
cp grub.cfg out/iso/boot/grub
cp target/i686-unknown-none/$OUT_PREFIX/loader out/iso/boot
cp target/x86_64-unknown-none/$OUT_PREFIX/kernel out/iso/boot
grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso
