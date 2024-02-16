#!/usr/bin/env bash

source .env
set -xu

qemu-system-x86_64 -enable-kvm \
    -drive if=pflash,format=raw,readonly=on,file=${TESTOS_QEMU_EFI_CODE} \
    -drive if=pflash,format=raw,readonly=on,file=${TESTOS_QEMU_EFI_VARS} \
    -drive format=raw,file=fat:rw:out/esp
