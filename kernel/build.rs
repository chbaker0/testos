use nasm_rs::Build;

fn main() -> Result<(), String> {
    Build::new()
        .file("src/entry.nasm")
        .flag("-f elf64")
        .compile("libentry")?;
    println!("cargo:rustc-link-lib=libentry");
    Ok(())
}
