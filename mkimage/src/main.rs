use buildutil::*;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[cfg(feature = "rust-bootloader")]
use bootloader_locator::locate_bootloader;
use cargo_metadata::Message;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    kernel_image: PathBuf,
}

#[cfg(feature = "rust-bootloader")]
fn main() -> anyhow::Result<()> {
    let cwd = env::current_dir()?;
    let args = Args::parse();
    let cargo = env::var("CARGO").unwrap();

    let kernel_manifest = cwd.join("Cargo.toml");
    let kernel_image = cwd.join(args.kernel_image);
    let target_dir = cwd.join("target");
    let out_dir = cwd.join("out");

    let bootloader_root = locate_bootloader("bootloader")
        .map_err(|e| anyhow::Error::new(e))?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("could not get parent of bootloader manifest"))?
        .to_owned();
    run_and_check(
        Command::new(cargo)
            .current_dir(bootloader_root)
            .arg("builder")
            .arg("--kernel-manifest")
            .arg(kernel_manifest)
            .arg("--kernel-binary")
            .arg(kernel_image)
            .arg("--target-dir")
            .arg(target_dir)
            .arg("--out-dir")
            .arg(out_dir),
    )?;
    Ok(())
}

#[cfg(not(feature = "rust-bootloader"))]
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cargo = env::var("CARGO").unwrap();

    println!("Building bootloader...");
    let mut build_invocation = Command::new(cargo)
        .arg("build")
        .arg("--package")
        .arg("loader")
        .arg("--target")
        .arg("targets/i686-unknown-none.json")
        .arg("-Zbuild-std=core,compiler_builtins")
        .arg("-Zbuild-std-features=compiler-builtins-mem")
        .arg("--message-format=json")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut loader_binary = None;
    for message in Message::parse_stream(std::io::BufReader::new(
        build_invocation.stdout.as_mut().unwrap(),
    )) {
        match message.unwrap() {
            Message::CompilerArtifact(artifact) => {
                if artifact.executable == None {
                    continue;
                }
                assert_eq!(loader_binary, None);
                loader_binary = artifact.executable;
            }
            _ => (),
        }
    }

    assert!(build_invocation.wait().unwrap().success());

    let loader_binary = loader_binary.expect("loader not found!");
    println!("Loader binary at {}", loader_binary);

    println!("Building image from {}...", args.kernel_image.display());

    // mkdir -p out/iso/boot/grub
    // cp grub.cfg out/iso/boot/grub
    // cp loader/target/i686-unknown-none/$OUT_PREFIX/loader out/iso/boot
    // cp kernel/target/x86_64-unknown-none/$OUT_PREFIX/kernel out/iso/boot
    // grub-mkrescue -o out/kernel.iso -d /usr/lib/grub/i386-pc out/iso

    fs::create_dir_all("out/iso/boot/grub").unwrap();
    fs::copy("grub.cfg", "out/iso/boot/grub/grub.cfg").unwrap();
    fs::copy(loader_binary, "out/iso/boot/loader").unwrap();
    fs::copy(args.kernel_image, "out/iso/boot/kernel").unwrap();

    run_and_check(
        Command::new("grub-mkrescue")
            .arg("-o")
            .arg("out/kernel.iso")
            .arg("-d")
            .arg("/usr/lib/grub/i386-pc")
            .arg("out/iso"),
    )?;

    Ok(())
}
