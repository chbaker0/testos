use buildutil::*;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    kernel_image: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Building image from {}...", args.kernel_image.display());

    // mkdir -p out/iso/boot/grub
    // cp grub.cfg out/iso/boot/grub
    // cp loader/target/i686-unknown-none/$OUT_PREFIX/loader out/iso/boot
    // cp kernel/target/x86_64-unknown-none/$OUT_PREFIX/kernel out/iso/boot
    // grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso

    fs::create_dir_all("out/iso/boot/grub").unwrap();
    fs::copy("grub.cfg", "out/iso/boot/grub/grub.cfg").unwrap();
    fs::copy(args.kernel_image, "out/iso/boot/kernel").unwrap();

    if cfg!(feature = "grub-mkrescue") {
        run_and_check(
            Command::new("grub-mkrescue")
                .arg("-o")
                .arg("out/kernel.iso")
                .arg("-d")
                .arg("/usr/lib/grub/i386-pc")
                .arg("out/iso"),
        )?;
    } else {
        run_and_check(Command::new("xorriso").args(&[
            "-as",
            "mkisofs",
            "-graft-points",
            "-b",
            "boot/grub/i386-pc/eltorito.img",
            "-no-emul-boot",
            "-boot-load-size",
            "4",
            "-boot-info-table",
            "--grub2-boot-info",
            "--grub2-mbr",
            "third_party/boot_hybrid.img",
            "--protective-msdos-label",
            "-o",
            "out/kernel.iso",
            "-r",
            "third_party/grub-image",
            "--sort-weight",
            "0",
            "/",
            "--sort-weight",
            "1",
            "/boot",
            "out/iso",
        ]))?
    }

    Ok(())
}
