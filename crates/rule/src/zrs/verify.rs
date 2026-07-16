use std::collections::BTreeMap;

use fst::Streamer;

use crate::normalize;

use super::layout::*;
use super::ZrsError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerifyMode {
    #[default]
    Structure,
    Semantic,
    FullChecksum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSetMetadata {
    pub major_version: u16,
    pub minor_version: u16,
    pub display_name: Option<String>,
    pub file_size: u64,
    pub body_checksum: u32,
    pub counts: RuleTypeCounts,
    pub section_bytes: RuleSectionBytes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleTypeCounts {
    pub domain_exact: u64,
    pub domain_suffix: u64,
    pub domain_keyword: u32,
    pub ipv4_ranges: u32,
    pub ipv6_ranges: u32,
}

impl RuleTypeCounts {
    pub fn entry_count(self) -> u64 {
        self.domain_exact
            + self.domain_suffix
            + u64::from(self.domain_keyword)
            + u64::from(self.ipv4_ranges)
            + u64::from(self.ipv6_ranges)
    }

    pub fn is_empty(self) -> bool {
        self.entry_count() == 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleSectionBytes {
    pub domain_exact: u64,
    pub domain_suffix: u64,
    pub domain_keyword: u64,
    pub ipv4_ranges: u64,
    pub ipv6_ranges: u64,
}

impl RuleSectionBytes {
    pub fn index_bytes(self) -> u64 {
        self.domain_exact
            + self.domain_suffix
            + self.domain_keyword
            + self.ipv4_ranges
            + self.ipv6_ranges
    }
}

impl RuleSetMetadata {
    pub fn entry_count(&self) -> u64 {
        self.counts.entry_count()
    }

    pub fn index_bytes(&self) -> u64 {
        self.section_bytes.index_bytes()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub metadata: RuleSetMetadata,
    pub exact: Section,
    pub suffix: Section,
    pub keyword: Section,
    pub ipv4: Section,
    pub ipv6: Section,
}

pub fn verify(bytes: &[u8], mode: VerifyMode) -> Result<RuleSetMetadata, ZrsError> {
    Ok(verify_layout(bytes, mode)?.metadata)
}

pub(crate) fn verify_layout(bytes: &[u8], mode: VerifyMode) -> Result<Layout, ZrsError> {
    limit("file size", bytes.len() as u64, MAX_FILE_SIZE)?;
    if bytes.len() < HEADER_SIZE {
        return Err(ZrsError::Truncated);
    }
    if bytes[0..4] != MAGIC {
        return Err(ZrsError::InvalidMagic);
    }
    let major = read_u16(bytes, 4);
    let minor = read_u16(bytes, 6);
    if major != MAJOR_VERSION || minor > MINOR_VERSION {
        return Err(ZrsError::UnsupportedVersion { major, minor });
    }
    if read_u16(bytes, 8) as usize != HEADER_SIZE {
        return Err(ZrsError::InvalidHeader("unexpected header size"));
    }
    if bytes[12..16].iter().any(|byte| *byte != 0)
        || bytes[28..32].iter().any(|byte| *byte != 0)
        || bytes[96..128].iter().any(|byte| *byte != 0)
    {
        return Err(ZrsError::InvalidHeader(
            "reserved header bytes are not zero",
        ));
    }
    let count = read_u16(bytes, 10) as usize;
    limit("section count", count as u64, MAX_SECTION_COUNT as u64)?;
    let directory_end = HEADER_SIZE
        .checked_add(
            count
                .checked_mul(SECTION_ENTRY_SIZE)
                .ok_or(ZrsError::FileTooLarge)?,
        )
        .ok_or(ZrsError::FileTooLarge)?;
    if directory_end > bytes.len() || read_u64(bytes, 16) != bytes.len() as u64 {
        return Err(ZrsError::InvalidHeader("invalid file or directory length"));
    }
    if mode == VerifyMode::FullChecksum
        && read_u32(bytes, 24) != crc32fast::hash(&bytes[HEADER_SIZE..])
    {
        return Err(ZrsError::ChecksumMismatch);
    }
    let display_name = decode_name(&bytes[32..96])?;
    let mut sections = BTreeMap::new();
    let mut ranges = Vec::with_capacity(count);
    for index in 0..count {
        let entry = HEADER_SIZE + index * SECTION_ENTRY_SIZE;
        let kind = read_u16(bytes, entry);
        let encoding = read_u16(bytes, entry + 2);
        let flags = read_u32(bytes, entry + 4);
        if flags & !SECTION_FLAG_REQUIRED != 0 {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "unknown section flags",
            });
        }
        let known = matches!(kind, KIND_DOMAIN_EXACT..=KIND_IPV6_RANGE);
        if !known && flags & SECTION_FLAG_REQUIRED != 0 {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "unknown required section",
            });
        }
        let offset =
            usize::try_from(read_u64(bytes, entry + 8)).map_err(|_| ZrsError::FileTooLarge)?;
        let length =
            usize::try_from(read_u64(bytes, entry + 16)).map_err(|_| ZrsError::FileTooLarge)?;
        limit("section size", length as u64, MAX_SECTION_SIZE)?;
        let end = offset.checked_add(length).ok_or(ZrsError::FileTooLarge)?;
        if offset % 8 != 0 || offset < align_8(directory_end) || end > bytes.len() {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "invalid alignment or bounds",
            });
        }
        let section = Section {
            kind,
            encoding,
            offset,
            length,
        };
        if sections.insert(kind, section).is_some() {
            return Err(ZrsError::DuplicateSection(kind));
        }
        ranges.push((offset, end));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(ZrsError::InvalidHeader("overlapping sections"));
    }

    let exact = required(&sections, KIND_DOMAIN_EXACT, ENCODING_FST_SET_V1)?;
    let suffix = required(&sections, KIND_DOMAIN_SUFFIX, ENCODING_FST_SET_V1)?;
    let keyword = required(&sections, KIND_DOMAIN_KEYWORD, ENCODING_STRING_TABLE_V1)?;
    let ipv4 = required(&sections, KIND_IPV4_RANGE, ENCODING_IPV4_RANGE_V1)?;
    let ipv6 = required(&sections, KIND_IPV6_RANGE, ENCODING_IPV6_RANGE_V1)?;
    verify_domain_fst(
        slice(bytes, exact),
        exact.kind,
        mode != VerifyMode::Structure,
    )?;
    verify_domain_fst(
        slice(bytes, suffix),
        suffix.kind,
        mode != VerifyMode::Structure,
    )?;
    verify_strings(
        slice(bytes, keyword),
        keyword.kind,
        mode != VerifyMode::Structure,
    )?;
    verify_ipv4_ranges(slice(bytes, ipv4), ipv4.kind, mode != VerifyMode::Structure)?;
    verify_ipv6_ranges(slice(bytes, ipv6), ipv6.kind, mode != VerifyMode::Structure)?;
    let exact_count = fst::Set::new(slice(bytes, exact))
        .map_err(ZrsError::InvalidFst)?
        .len() as u64;
    let suffix_count = fst::Set::new(slice(bytes, suffix))
        .map_err(ZrsError::InvalidFst)?
        .len() as u64;
    let keyword_count = read_u32(slice(bytes, keyword), 0);
    let ipv4_count = read_u32(slice(bytes, ipv4), 0);
    let ipv6_count = read_u32(slice(bytes, ipv6), 0);
    for (resource, count) in [
        ("domain exact entries", exact_count),
        ("domain suffix entries", suffix_count),
        ("domain keyword entries", u64::from(keyword_count)),
        ("IPv4 range entries", u64::from(ipv4_count)),
        ("IPv6 range entries", u64::from(ipv6_count)),
    ] {
        limit(resource, count, MAX_ENTRIES_PER_SECTION)?;
    }
    let metadata = RuleSetMetadata {
        major_version: major,
        minor_version: minor,
        display_name,
        file_size: bytes.len() as u64,
        body_checksum: read_u32(bytes, 24),
        counts: RuleTypeCounts {
            domain_exact: exact_count,
            domain_suffix: suffix_count,
            domain_keyword: keyword_count,
            ipv4_ranges: ipv4_count,
            ipv6_ranges: ipv6_count,
        },
        section_bytes: RuleSectionBytes {
            domain_exact: exact.length as u64,
            domain_suffix: suffix.length as u64,
            domain_keyword: keyword.length as u64,
            ipv4_ranges: ipv4.length as u64,
            ipv6_ranges: ipv6.length as u64,
        },
    };
    Ok(Layout {
        metadata,
        exact,
        suffix,
        keyword,
        ipv4,
        ipv6,
    })
}

fn limit(resource: &'static str, actual: u64, maximum: u64) -> Result<(), ZrsError> {
    if actual > maximum {
        return Err(ZrsError::ResourceLimit {
            resource,
            actual,
            maximum,
        });
    }
    Ok(())
}

pub(crate) fn slice(bytes: &[u8], section: Section) -> &[u8] {
    &bytes[section.offset..section.offset + section.length]
}

fn required(
    sections: &BTreeMap<u16, Section>,
    kind: u16,
    encoding: u16,
) -> Result<Section, ZrsError> {
    let section = *sections.get(&kind).ok_or(ZrsError::MissingSection(kind))?;
    if section.encoding != encoding {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "unsupported encoding",
        });
    }
    Ok(section)
}

fn decode_name(bytes: &[u8]) -> Result<Option<String>, ZrsError> {
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(ZrsError::InvalidHeader(
            "display name is not NUL terminated",
        ))?;
    if bytes[length + 1..].iter().any(|byte| *byte != 0) {
        return Err(ZrsError::InvalidHeader("display name padding is not zero"));
    }
    if length == 0 {
        return Ok(None);
    }
    Ok(Some(
        std::str::from_utf8(&bytes[..length])
            .map_err(|_| ZrsError::InvalidUtf8)?
            .to_owned(),
    ))
}

fn verify_strings(bytes: &[u8], kind: u16, semantic: bool) -> Result<(), ZrsError> {
    if bytes.len() < 16 {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "truncated string table",
        });
    }
    if read_u32(bytes, 4) != 0 {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "reserved string table bytes are not zero",
        });
    }
    let count = read_u32(bytes, 0) as usize;
    let data = 8_usize
        .checked_add((count + 1).checked_mul(8).ok_or(ZrsError::FileTooLarge)?)
        .ok_or(ZrsError::FileTooLarge)?;
    if data > bytes.len() {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "truncated string offsets",
        });
    }
    let body_len = bytes.len() - data;
    let mut previous = 0;
    let mut offsets = Vec::with_capacity(count + 1);
    for index in 0..=count {
        let offset =
            usize::try_from(read_u64(bytes, 8 + index * 8)).map_err(|_| ZrsError::FileTooLarge)?;
        if offset < previous || offset > body_len {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "invalid string offset",
            });
        }
        previous = offset;
        offsets.push(offset);
    }
    if previous != body_len {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "string table does not cover body",
        });
    }
    for pair in offsets.windows(2) {
        let value = std::str::from_utf8(&bytes[data + pair[0]..data + pair[1]])
            .map_err(|_| ZrsError::InvalidUtf8)?;
        if semantic && normalize::keyword(value).as_deref() != Ok(value) {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "non-canonical keyword",
            });
        }
    }
    Ok(())
}

fn verify_range_length(bytes: &[u8], width: usize, kind: u16) -> Result<usize, ZrsError> {
    if bytes.len() < 8 {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "truncated range table",
        });
    }
    if read_u32(bytes, 4) != 0 {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "reserved range table bytes are not zero",
        });
    }
    let count = read_u32(bytes, 0) as usize;
    if 8_usize
        .checked_add(count.checked_mul(width).ok_or(ZrsError::FileTooLarge)?)
        .ok_or(ZrsError::FileTooLarge)?
        != bytes.len()
    {
        return Err(ZrsError::InvalidSection {
            kind,
            reason: "invalid range table length",
        });
    }
    Ok(count)
}

fn verify_domain_fst(bytes: &[u8], kind: u16, semantic: bool) -> Result<(), ZrsError> {
    let set = fst::Set::new(bytes).map_err(ZrsError::InvalidFst)?;
    if !semantic {
        return Ok(());
    }
    let mut stream = set.stream();
    while let Some(value) = stream.next() {
        let value = std::str::from_utf8(value).map_err(|_| ZrsError::InvalidUtf8)?;
        if normalize::domain(value).as_deref() != Ok(value) {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "non-canonical domain",
            });
        }
    }
    Ok(())
}

fn verify_ipv4_ranges(bytes: &[u8], kind: u16, semantic: bool) -> Result<(), ZrsError> {
    let count = verify_range_length(bytes, 8, kind)?;
    if !semantic {
        return Ok(());
    }
    let mut previous_end = None;
    for index in 0..count {
        let start = read_u32(bytes, 8 + index * 8);
        let end = read_u32(bytes, 12 + index * 8);
        if start > end || previous_end.is_some_and(|previous| start <= previous) {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "unsorted or overlapping ranges",
            });
        }
        previous_end = Some(end);
    }
    Ok(())
}

fn verify_ipv6_ranges(bytes: &[u8], kind: u16, semantic: bool) -> Result<(), ZrsError> {
    let count = verify_range_length(bytes, 32, kind)?;
    if !semantic {
        return Ok(());
    }
    let read = |offset| {
        u128::from_le_bytes(
            bytes[offset..offset + 16]
                .try_into()
                .expect("validated bounds"),
        )
    };
    let mut previous_end = None;
    for index in 0..count {
        let start = read(8 + index * 32);
        let end = read(24 + index * 32);
        if start > end || previous_end.is_some_and(|previous| start <= previous) {
            return Err(ZrsError::InvalidSection {
                kind,
                reason: "unsorted or overlapping ranges",
            });
        }
        previous_end = Some(end);
    }
    Ok(())
}
