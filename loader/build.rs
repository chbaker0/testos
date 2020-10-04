use nasm_rs::Build;

fn main() -> Result<(), String> {
    Build::new().file("boot.nasm").flag("-f elf32").compile("libboot")?;
    println!("cargo:rustc-link-lib=libboot");
    Ok(())
}
