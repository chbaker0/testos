//! Refresh the vendored OVMF firmware under `third_party/ovmf`.
//!
//! This is a manual maintenance tool, NOT part of the normal build: it needs
//! network access to download the prebuilt from the `rust-osdev/ovmf-prebuilt`
//! GitHub release. Run it (from a networked machine) whenever you bump `TAG`,
//! then commit the updated `third_party/ovmf/x64/*.fd`. The regular build reads
//! the committed copy and never touches the network.

use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use std::fs;
use std::path::Path;

/// Pinned OVMF release. Bump this together with `third_party/ovmf/README.md`.
const TAG: &Source = &Source::EDK2_STABLE202511_R1;

/// Committed firmware directory consumed by make-image.sh / run-qemu.sh.
const VENDOR_DIR: &str = "third_party/ovmf/x64";

fn main() {
    // Download + hash-verify into a gitignored scratch dir. The crate wipes its
    // output dir on every fetch, so we deliberately do NOT point it at the
    // committed vendor dir (that would delete README.md alongside the blobs).
    let prebuilt =
        Prebuilt::fetch(TAG.clone(), "target/ovmf").expect("failed to fetch OVMF prebuilt");

    let code = prebuilt.get_file(Arch::X64, FileType::Code);
    let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

    fs::create_dir_all(VENDOR_DIR).expect("create vendor dir");
    for (src, name) in [(code, "code.fd"), (vars, "vars.fd")] {
        let dst = Path::new(VENDOR_DIR).join(name);
        fs::copy(&src, &dst)
            .unwrap_or_else(|e| panic!("copy {} -> {}: {e}", src.display(), dst.display()));
        println!("vendored {}", dst.display());
    }
}
