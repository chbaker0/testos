use core::option::Option;

pub enum SectionType {
    pub Null,
    pub ProgBits,
    pub SymTab,
    pub StrTab,
    pub RelA,
    pub Hash,
    pub Dynamic,
    pub Note,
    pub NoBits,
    pub Rel,
    pub ShLib,
    pub DynSym,
}

pub struct SectionHeader {
    pub name_ndx: u32,
    pub typ: SectionType,
    pub addr: u32,
    pub offset: u32,
    pub size: u32,
    pub addralign: u32,
    pub entsize: u32,
}

#[repr(C, packed)]
pub struct SectionHeaderRaw {
    pub name: u32,
    pub typ: u32,
    pub flags: u32,
    pub addr: u32,
    pub offset: u32,
    pub size: u32,
    pub link: u32,
    pub info: u32,
    pub addralign: u32,
    pub entsize: u32,
}

pub struct SectionHeaderIterator {
    base: *const u8,
    entry_size: usize,
    num_entries: usize,
}

impl Iterator for SectionHeaderIterator {
    type Item = SectionHeader;
    fn next(&mut self) -> Option<Self::Item> {
        let raw_header =
    }
}

pub fn get_section_header_iterator(table: &[SectionHeader],
