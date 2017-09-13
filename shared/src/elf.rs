pub enum ElfType {
    None,
    Rel,
    Exec,
    Dyn,
    Core,
}

#[repr(C, packed)]
pub struct ElfHeaderRaw {
    pub ident: [u8; 16],
    pub typ: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub phoff: u64,
    pub shoff: u64,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

pub enum SegmentType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    Phrd,
    Tls,
}

pub const PROGRAM_FLAG_X: u32 = 1;
pub const PROGRAM_FLAG_W: u32 = 2;
pub const PROGRAM_FLAG_R: u32 = 4;

#[repr(C, packed)]
pub struct ProgramHeaderRaw {
    pub typ: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub align: u64,
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
