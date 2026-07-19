use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};

const TAG: &'static Source = &Source::EDK2_STABLE202511_R1;

fn main() {
    let prebuilt = Prebuilt::fetch(TAG.clone(), "target/ovmf").expect("failed to update prebuilt");

    prebuilt.get_file(Arch::X64, FileType::Code);
    prebuilt.get_file(Arch::X64, FileType::Vars);
}
