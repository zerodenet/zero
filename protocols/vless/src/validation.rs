use alloc::format;
use alloc::string::String;

use base64::Engine;

pub fn validate_reality_key(value: &str) -> Result<(), &'static str> {
    if value.contains('=') {
        return Err("must be base64url without padding");
    }
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "must be valid base64url without padding")?;
    if decoded.len() != 32 {
        return Err("must decode to exactly 32 bytes");
    }
    Ok(())
}

pub fn validate_reality_short_id(short_id: &str) -> Result<(), &'static str> {
    if short_id.len() > 16 {
        return Err("must be at most 16 hex characters");
    }
    if !short_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("must contain only hex digits");
    }
    Ok(())
}

pub fn validate_reality_cipher_suites(cipher_suites: &[String]) -> Result<(), String> {
    for cipher_suite in cipher_suites {
        match cipher_suite.as_str() {
            "TLS_AES_128_GCM_SHA256"
            | "TLS_AES_256_GCM_SHA384"
            | "TLS_CHACHA20_POLY1305_SHA256" => {}
            _ => return Err(format!("unsupported cipher suite `{cipher_suite}`")),
        }
    }
    Ok(())
}

pub fn validate_xhttp_mode(mode: &str) -> Result<(), String> {
    match mode {
        "" | "auto" | "packet-up" | "stream-up" | "stream-one" => Ok(()),
        other => Err(format!(
            "mode `{other}` is not one of: auto, packet-up, stream-up, stream-one"
        )),
    }
}
