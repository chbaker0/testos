use std::env;
use std::path::Path;
use std::process::Command;

fn run_and_check(cmd: &mut Command) {
    let status = cmd.status().unwrap();
    assert!(status.success());
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    run_and_check(
        Command::new("nasm")
            .args(&["src/boot.nasm", "-f", "elf32", "-o"])
            .arg(&format!("{}/boot.o", out_dir)),
    );
    run_and_check(
        Command::new("ar")
            .args(&["crus", "libboot.a", "boot.o"])
            .current_dir(&Path::new(&out_dir)),
    );

    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib=boot");
}
