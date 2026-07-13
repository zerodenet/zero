use alloc::string::String;

use zero_core::Error;

pub fn parse_uuid(input: &str) -> Result<[u8; 16], Error> {
    let input = input.trim();
    let mut compact = [0_u8; 32];
    let mut offset = 0;

    for (index, byte) in input.bytes().enumerate() {
        if byte == b'-' {
            if !matches!(index, 8 | 13 | 18 | 23) || input.len() != 36 {
                return Err(Error::Config("VLESS UUID is not canonical"));
            }
            continue;
        }
        if offset >= compact.len() {
            return Err(Error::Config("VLESS UUID has too many hex digits"));
        }
        if hex_nibble(byte).is_none() {
            return Err(Error::Config("VLESS UUID contains non-hex digits"));
        }
        compact[offset] = byte;
        offset += 1;
    }

    if offset != compact.len() {
        return Err(Error::Config("VLESS UUID must contain 32 hex digits"));
    }

    let mut uuid = [0_u8; 16];
    for i in 0..16 {
        let high = hex_nibble(compact[i * 2]).expect("hex digit checked");
        let low = hex_nibble(compact[i * 2 + 1]).expect("hex digit checked");
        uuid[i] = (high << 4) | low;
    }
    Ok(uuid)
}

pub fn format_uuid(id: &[u8; 16]) -> String {
    let mut out = String::with_capacity(36);
    for (index, byte) in id.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(hex_char(byte >> 4));
        out.push(hex_char(byte & 0x0f));
    }
    out
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + value - 10),
        _ => unreachable!("nibble value"),
    }
}
