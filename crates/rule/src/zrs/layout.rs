pub const MAGIC: [u8; 4] = *b"ZRS!";
pub const MAJOR_VERSION: u16 = 0;
pub const MINOR_VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 128;
pub const SECTION_ENTRY_SIZE: usize = 24;
pub const SECTION_COUNT: usize = 5;
pub const MAX_SECTION_COUNT: usize = 64;
pub const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024;
pub const MAX_SECTION_SIZE: u64 = 512 * 1024 * 1024;
pub const MAX_ENTRIES_PER_SECTION: u64 = 4_000_000;

pub const KIND_DOMAIN_EXACT: u16 = 1;
pub const KIND_DOMAIN_SUFFIX: u16 = 2;
pub const KIND_DOMAIN_KEYWORD: u16 = 3;
pub const KIND_IPV4_RANGE: u16 = 4;
pub const KIND_IPV6_RANGE: u16 = 5;

pub const ENCODING_FST_SET_V1: u16 = 1;
pub const ENCODING_STRING_TABLE_V1: u16 = 2;
pub const ENCODING_IPV4_RANGE_V1: u16 = 3;
pub const ENCODING_IPV6_RANGE_V1: u16 = 4;
pub const SECTION_FLAG_REQUIRED: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Section {
    pub kind: u16,
    pub encoding: u16,
    pub offset: usize,
    pub length: usize,
}

pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated bounds"),
    )
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated bounds"),
    )
}

pub(crate) fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("validated bounds"),
    )
}

pub(crate) fn align_8(value: usize) -> usize {
    (value + 7) & !7
}
