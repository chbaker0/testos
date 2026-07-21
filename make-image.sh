#!/usr/bin/env bash

source .env
set -eux

cargo +nightly run -p fetch-prebuilts
cargo +nightly build -p loader --target x86_64-unknown-uefi
cargo +nightly kbuild
cargo +nightly ibuild

mkdir -p out/esp/efi/boot
cp target/x86_64-unknown-uefi/debug/loader.efi out/esp/efi/boot/bootx64.efi
cp target/x86_64-unknown-none/debug/kernel out/esp/testos
cp target/x86_64-unknown-testos/debug/init out/esp/init
