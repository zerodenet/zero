use std::collections::HashSet;

pub(crate) fn domain(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return Err("domain is empty".to_owned());
    }

    let ascii = idna::domain_to_ascii(trimmed).map_err(|error| error.to_string())?;
    let normalized = ascii.to_ascii_lowercase();
    if normalized.len() > 253 {
        return Err("domain exceeds 253 ASCII bytes".to_owned());
    }
    if normalized
        .split('.')
        .any(|label| label.is_empty() || label.len() > 63)
    {
        return Err("domain contains an empty label or a label longer than 63 bytes".to_owned());
    }

    Ok(normalized)
}

pub(crate) fn is_covered_by_suffix(domain: &str, suffixes: &HashSet<String>) -> bool {
    let mut candidate = domain;
    loop {
        if suffixes.contains(candidate) {
            return true;
        }
        let Some(dot) = candidate.find('.') else {
            return false;
        };
        candidate = &candidate[dot + 1..];
    }
}

pub(crate) fn label_count(domain: &str) -> usize {
    domain.bytes().filter(|byte| *byte == b'.').count() + 1
}

pub(crate) fn keyword(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("keyword is empty".to_owned());
    }
    if trimmed.len() > crate::MAX_RULE_VALUE_BYTES {
        return Err(format!(
            "keyword exceeds {} bytes",
            crate::MAX_RULE_VALUE_BYTES
        ));
    }
    if trimmed.as_bytes().contains(&0) {
        return Err("keyword contains a NUL byte".to_owned());
    }
    if !trimmed.is_ascii() {
        return Err("keyword must contain ASCII characters only".to_owned());
    }
    Ok(trimmed.to_ascii_lowercase())
}
