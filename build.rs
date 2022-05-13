use buildutil::*;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use multiboot2_header::*;

pub fn generate_mb2_header() -> Vec<u8> {
    let mut builder = Multiboot2HeaderBuilder::new(HeaderTagISA::I386);
    builder = builder.console_tag(ConsoleHeaderTag::new(
        HeaderTagFlag::Required,
        ConsoleHeaderTagFlags::ConsoleRequired,
    ));

    let mut mbi_builder = InformationRequestHeaderTagBuilder::new(HeaderTagFlag::Required);
    mbi_builder = mbi_builder.add_irs(&[MbiTagType::Mmap, MbiTagType::AcpiV2, MbiTagType::End]);
    builder = builder.information_request_tag(mbi_builder);

    builder.build()
}

fn main() -> anyhow::Result<()> {
    let out_dir = PathBuf::from_str(&env::var("OUT_DIR")?)?;

    let mb2_header_bin = "mb2_header";
    fs::write(out_dir.join(&mb2_header_bin), generate_mb2_header())?;

    let mb2_header_elf = "mb2_header.o";

    run_and_check(Command::new("objcopy").current_dir(&out_dir).args(&[
        "-Ibinary",
        "-Oelf64-x86-64",
        // "--binary-architecture=x64",
        "--rename-section",
        ".data=.text.mb2_header",
        mb2_header_bin,
        mb2_header_elf,
    ]))?;

    run_and_check(Command::new("ar").current_dir(&out_dir).args(&[
        "crus",
        "libmb2_header.a",
        mb2_header_elf,
    ]))?;

    println!("cargo:rustc-link-search={}", out_dir.to_str().unwrap());
    println!("cargo:rustc-link-lib=mb2_header");

    let debug_flags = if env::var("PROFILE")? == "debug" {
        ["-F", "dwarf", "-g"].as_slice()
    } else {
        [].as_slice()
    };

    run_and_check(
        Command::new("nasm")
            .args(debug_flags)
            .args(&["-f", "elf64", "-o"])
            .arg(&format!("{}/entry.o", out_dir.to_str().unwrap()))
            .arg("src/entry.nasm"),
    )?;

    run_and_check(Command::new("ar").current_dir(&out_dir).args(&[
        "crus",
        "libentry.a",
        "entry.o",
    ]))?;

    println!("cargo:rustc-link-lib=entry");

    Ok(())
}
