#!/usr/bin/env bash

source .env
set -eux

# OVMF UEFI firmware is vendored under third_party/ovmf (see its README) so the
# build is hermetic and does NOT download anything. To (re)generate the vendored
# firmware, run `cargo run -p fetch-prebuilts` (needs network) and commit it.
efi_code="${TESTOS_QEMU_EFI_CODE:-third_party/ovmf/x64/code.fd}"
efi_vars="${TESTOS_QEMU_EFI_VARS:-third_party/ovmf/x64/vars.fd}"
if [[ ! -f "$efi_code" || ! -f "$efi_vars" ]]; then
    set +x
    echo "error: OVMF firmware not found ($efi_code / $efi_vars)." >&2
    echo "The vendored firmware is missing. Regenerate it with:" >&2
    echo "    cargo run -p fetch-prebuilts   # downloads into third_party/ovmf" >&2
    echo "then commit third_party/ovmf, or point TESTOS_QEMU_EFI_CODE/_VARS at" >&2
    echo "an existing firmware pair (e.g. a distro OVMF under /usr/share)." >&2
    exit 1
fi

cargo +nightly build -p loader --target x86_64-unknown-uefi
cargo +nightly kbuild
cargo +nightly ibuild

mkdir -p out/esp/efi/boot
cp target/x86_64-unknown-uefi/debug/loader.efi out/esp/efi/boot/bootx64.efi
cp target/x86_64-unknown-none/debug/kernel out/esp/testos
cp target/x86_64-unknown-testos/debug/init out/esp/init
