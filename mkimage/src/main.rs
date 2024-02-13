use buildutil::*;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use cargo_metadata::Message;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    kernel_image: PathBuf,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    // Build init binary:
    let mut init_build_command = Command::new(env::var("CARGO")?)
        .args(&["ibuild", "--message-format=json-render-diagnostics"])
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let mut init_bin: Option<PathBuf> = None;
    for message in cargo_metadata::Message::parse_stream(std::io::BufReader::new(
        init_build_command.stdout.take().unwrap(),
    )) {
        let message = message?;
        match message {
            Message::CompilerArtifact(artifact) => {
                if let Some(ref exe) = artifact.executable {
                    assert_eq!(init_bin, None, "other artifact {:?}", artifact);
                    init_bin = Some(exe.as_std_path().to_path_buf());
                }
            }
            Message::BuildFinished(m) => assert!(m.success),
            _ => (),
        }
    }

    assert!(init_build_command.wait()?.success());
    let init_bin = init_bin.unwrap();

    println!("Building image from {}...", args.kernel_image.display());

    // mkdir -p out/iso/boot/grub
    // cp grub.cfg out/iso/boot/grub
    // cp loader/target/i686-unknown-none/$OUT_PREFIX/loader out/iso/boot
    // cp kernel/target/x86_64-unknown-none/$OUT_PREFIX/kernel out/iso/boot
    // grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso

    fs::create_dir_all("out/iso/boot/grub").unwrap();
    fs::copy("grub.cfg", "out/iso/boot/grub/grub.cfg").unwrap();
    fs::copy(args.kernel_image, "out/iso/boot/kernel").unwrap();
    fs::copy(init_bin, "out/iso/boot/init").unwrap();

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
