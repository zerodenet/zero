use fst::SetBuilder;

use crate::CompiledRuleSet;

use super::layout::*;
use super::ZrsError;

pub fn encode(rule_set: &CompiledRuleSet) -> Result<Vec<u8>, ZrsError> {
    validate_entry_counts(rule_set)?;
    let sections = [
        (
            KIND_DOMAIN_EXACT,
            ENCODING_FST_SET_V1,
            encode_fst(rule_set.domain_exact())?,
        ),
        (
            KIND_DOMAIN_SUFFIX,
            ENCODING_FST_SET_V1,
            encode_fst(rule_set.domain_suffix())?,
        ),
        (
            KIND_DOMAIN_KEYWORD,
            ENCODING_STRING_TABLE_V1,
            encode_strings(rule_set.domain_keyword())?,
        ),
        (
            KIND_IPV4_RANGE,
            ENCODING_IPV4_RANGE_V1,
            encode_ipv4(rule_set),
        ),
        (
            KIND_IPV6_RANGE,
            ENCODING_IPV6_RANGE_V1,
            encode_ipv6(rule_set),
        ),
    ];
    let directory_end = HEADER_SIZE + SECTION_COUNT * SECTION_ENTRY_SIZE;
    let mut output = vec![0_u8; align_8(directory_end)];
    let mut entries = Vec::with_capacity(SECTION_COUNT);
    for (kind, encoding, body) in sections {
        if body.len() as u64 > MAX_SECTION_SIZE {
            return Err(ZrsError::ResourceLimit {
                resource: "section size",
                actual: body.len() as u64,
                maximum: MAX_SECTION_SIZE,
            });
        }
        output.resize(align_8(output.len()), 0);
        let offset = output.len();
        output.extend_from_slice(&body);
        entries.push((kind, encoding, offset, body.len()));
    }

    output[0..4].copy_from_slice(&MAGIC);
    output[4..6].copy_from_slice(&MAJOR_VERSION.to_le_bytes());
    output[6..8].copy_from_slice(&MINOR_VERSION.to_le_bytes());
    output[8..10].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
    output[10..12].copy_from_slice(&(SECTION_COUNT as u16).to_le_bytes());
    let file_length = output.len() as u64;
    output[16..24].copy_from_slice(&file_length.to_le_bytes());
    if let Some(name) = rule_set.display_name() {
        output[32..32 + name.len()].copy_from_slice(name.as_bytes());
    }
    for (index, (kind, encoding, offset, length)) in entries.into_iter().enumerate() {
        let entry = HEADER_SIZE + index * SECTION_ENTRY_SIZE;
        output[entry..entry + 2].copy_from_slice(&kind.to_le_bytes());
        output[entry + 2..entry + 4].copy_from_slice(&encoding.to_le_bytes());
        output[entry + 4..entry + 8].copy_from_slice(&SECTION_FLAG_REQUIRED.to_le_bytes());
        output[entry + 8..entry + 16].copy_from_slice(&(offset as u64).to_le_bytes());
        output[entry + 16..entry + 24].copy_from_slice(&(length as u64).to_le_bytes());
    }
    let checksum = crc32fast::hash(&output[HEADER_SIZE..]);
    output[24..28].copy_from_slice(&checksum.to_le_bytes());
    if output.len() as u64 > MAX_FILE_SIZE {
        return Err(ZrsError::ResourceLimit {
            resource: "file size",
            actual: output.len() as u64,
            maximum: MAX_FILE_SIZE,
        });
    }
    Ok(output)
}

fn validate_entry_counts(rule_set: &CompiledRuleSet) -> Result<(), ZrsError> {
    for (resource, count) in [
        ("domain exact entries", rule_set.domain_exact().len()),
        ("domain suffix entries", rule_set.domain_suffix().len()),
        ("domain keyword entries", rule_set.domain_keyword().len()),
        ("IPv4 range entries", rule_set.ipv4_ranges().len()),
        ("IPv6 range entries", rule_set.ipv6_ranges().len()),
    ] {
        if count as u64 > MAX_ENTRIES_PER_SECTION {
            return Err(ZrsError::ResourceLimit {
                resource,
                actual: count as u64,
                maximum: MAX_ENTRIES_PER_SECTION,
            });
        }
    }
    limit_source_bytes("domain exact source bytes", rule_set.domain_exact())?;
    limit_source_bytes("domain suffix source bytes", rule_set.domain_suffix())?;
    let keyword_bytes = rule_set
        .domain_keyword()
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(value.len()))
        .ok_or(ZrsError::FileTooLarge)?;
    let keyword_size = 8_usize
        .checked_add(
            (rule_set.domain_keyword().len() + 1)
                .checked_mul(8)
                .ok_or(ZrsError::FileTooLarge)?,
        )
        .and_then(|size| size.checked_add(keyword_bytes))
        .ok_or(ZrsError::FileTooLarge)?;
    limit_encoded_bytes("domain keyword encoded bytes", keyword_size)?;
    limit_encoded_bytes(
        "IPv4 range encoded bytes",
        8_usize
            .checked_add(
                rule_set
                    .ipv4_ranges()
                    .len()
                    .checked_mul(8)
                    .ok_or(ZrsError::FileTooLarge)?,
            )
            .ok_or(ZrsError::FileTooLarge)?,
    )?;
    limit_encoded_bytes(
        "IPv6 range encoded bytes",
        8_usize
            .checked_add(
                rule_set
                    .ipv6_ranges()
                    .len()
                    .checked_mul(32)
                    .ok_or(ZrsError::FileTooLarge)?,
            )
            .ok_or(ZrsError::FileTooLarge)?,
    )?;
    Ok(())
}

fn limit_source_bytes(resource: &'static str, values: &[String]) -> Result<(), ZrsError> {
    let bytes = values
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(value.len()))
        .ok_or(ZrsError::FileTooLarge)?;
    limit_encoded_bytes(resource, bytes)
}

fn limit_encoded_bytes(resource: &'static str, bytes: usize) -> Result<(), ZrsError> {
    if bytes as u64 > MAX_SECTION_SIZE {
        return Err(ZrsError::ResourceLimit {
            resource,
            actual: bytes as u64,
            maximum: MAX_SECTION_SIZE,
        });
    }
    Ok(())
}

fn encode_fst(values: &[String]) -> Result<Vec<u8>, ZrsError> {
    let mut builder = SetBuilder::memory();
    for value in values {
        builder.insert(value).map_err(ZrsError::FstBuild)?;
    }
    builder.into_inner().map_err(ZrsError::FstBuild)
}

fn encode_strings(values: &[String]) -> Result<Vec<u8>, ZrsError> {
    let count = u32::try_from(values.len()).map_err(|_| ZrsError::TooManyEntries)?;
    let index_size = 8_usize
        .checked_add(
            (values.len() + 1)
                .checked_mul(8)
                .ok_or(ZrsError::FileTooLarge)?,
        )
        .ok_or(ZrsError::FileTooLarge)?;
    let mut output = vec![0_u8; index_size];
    output[0..4].copy_from_slice(&count.to_le_bytes());
    let mut cursor = 0_u64;
    for (index, value) in values.iter().enumerate() {
        output[8 + index * 8..16 + index * 8].copy_from_slice(&cursor.to_le_bytes());
        output.extend_from_slice(value.as_bytes());
        cursor = cursor
            .checked_add(value.len() as u64)
            .ok_or(ZrsError::FileTooLarge)?;
    }
    let end = 8 + values.len() * 8;
    output[end..end + 8].copy_from_slice(&cursor.to_le_bytes());
    Ok(output)
}

fn encode_ipv4(rule_set: &CompiledRuleSet) -> Vec<u8> {
    let mut output = Vec::with_capacity(8 + rule_set.ipv4_ranges().len() * 8);
    output.extend_from_slice(&(rule_set.ipv4_ranges().len() as u32).to_le_bytes());
    output.extend_from_slice(&[0; 4]);
    for range in rule_set.ipv4_ranges() {
        output.extend_from_slice(&range.start.to_le_bytes());
        output.extend_from_slice(&range.end.to_le_bytes());
    }
    output
}

fn encode_ipv6(rule_set: &CompiledRuleSet) -> Vec<u8> {
    let mut output = Vec::with_capacity(8 + rule_set.ipv6_ranges().len() * 32);
    output.extend_from_slice(&(rule_set.ipv6_ranges().len() as u32).to_le_bytes());
    output.extend_from_slice(&[0; 4]);
    for range in rule_set.ipv6_ranges() {
        output.extend_from_slice(&range.start.to_le_bytes());
        output.extend_from_slice(&range.end.to_le_bytes());
    }
    output
}
