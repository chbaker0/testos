pub enum ElfType {
    None,
    Rel,
    Exec,
    Dyn,
    Core,
}

#[repr(C, packed)]
pub struct ElfHeaderRaw {
    ident: [u8; 16],
    typ: u16,
    machine: u16,
    version: u32,
    entry: u64,
    phoff: u64,
    shoff: u64,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

pub enum SectionType {
    Null,
    ProgBits,
    SymTab,
    StrTab,
    RelA,
    Hash,
    Dynamic,
    Note,
    NoBits,
    Rel,
    ShLib,
    DynSym,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SectionHeaderRaw {
    pub name: u32,
    pub typ: u32,
    pub flags: u64,
    pub addr: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub addralign: u64,
    pub entsize: u64,
}

pub unsafe fn get_section_header(base: *const u8, entry_size: usize, ndx: usize) -> SectionHeaderRaw {
    let ptr = base.offset((entry_size * ndx) as isize) as *const SectionHeaderRaw;
    *ptr
}
