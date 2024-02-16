#!/usr/bin/env bash

source .env
set -xu

cargo +nightly build -p loader --target x86_64-unknown-uefi

mkdir -p out/esp/efi/boot
cp target/x86_64-unknown-uefi/debug/loader.efi out/esp/efi/boot/bootx64.efi
