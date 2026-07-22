# Vendored OVMF UEFI firmware

QEMU needs UEFI firmware to boot the loader (`out/esp/efi/boot/bootx64.efi`).
That firmware is vendored here so the build is hermetic — `make-image.sh` and
`run-qemu.sh` read these files directly and never download anything, which lets
the image build and boot in offline / network-restricted environments (e.g.
cloud CI sandboxes that block third-party GitHub release downloads).

## Files

| Path            | Role                                                              |
| --------------- | ---------------------------------------------------------------- |
| `x64/code.fd`   | UEFI firmware executable (OVMF `CODE`), mounted read-only pflash. |
| `x64/vars.fd`   | UEFI variable-store template (OVMF `VARS`), mounted read-only.    |

`run-qemu.sh` attaches both as `if=pflash,...,readonly=on`, so the var store is
a pristine template and nothing here is mutated at runtime.

## Provenance of the current version

- **Upstream:** [`rust-osdev/ovmf-prebuilt`][repo] GitHub release, which
  repackages OVMF built from [tianocore/EDK2][edk2].
- **Release tag:** `edk2-stable202511-r1`
- **Tarball SHA-256 (verified on download):**
  `79841c5dcac6d4bb71ead5edb6ca2a251237330be3c0b166bdc8a8fec0ce760d`
- **Architecture:** `x64` (X64 / `x86_64`).

[repo]: https://github.com/rust-osdev/ovmf-prebuilt
[edk2]: https://github.com/tianocore/edk2/tree/master/OvmfPkg

## License

OVMF is part of EDK2 and is distributed under **BSD-2-Clause-Patent**, which
permits redistribution — including committing these binaries into this
repository.

## Updating

The pinned version lives in one place: the `TAG` constant in
[`fetch-prebuilts/src/main.rs`](../../fetch-prebuilts/src/main.rs). To bump it:

1. Edit `TAG` (and update the tag/SHA-256 in this file to match).
2. Run the updater from a machine with network access:

   ```
   cargo run -p fetch-prebuilts
   ```

   It downloads + hash-verifies the release into the gitignored `target/ovmf`
   scratch dir, then copies `code.fd` / `vars.fd` into `x64/` here (leaving this
   README untouched).
3. Commit the updated `third_party/ovmf/x64/*.fd` and this README together.
