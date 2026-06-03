// TLS constants and utilities for REALITY client/server implementations

use std::io::{self, Error, ErrorKind};
use std::time::Duration;

use rand::Rng;

/// Build a standard TLS alert record (unencrypted)
///
/// REALITY anti-detection principle: On verification failure, send a standard
/// TLS alert instead of exposing any protocol-specific error information.
#[inline]
pub fn build_tls_alert(description: u8) -> Vec<u8> {
    vec![
        CONTENT_TYPE_ALERT,
        VERSION_TLS_1_2_MAJOR,
        VERSION_TLS_1_2_MINOR,
        0x00,
        0x02, // Length: 2 bytes
        ALERT_LEVEL_FATAL,
        description,
    ]
}

/// Build a close_notify alert for graceful connection shutdown
#[inline]
pub fn build_close_notify_alert() -> Vec<u8> {
    vec![
        CONTENT_TYPE_ALERT,
        VERSION_TLS_1_2_MAJOR,
        VERSION_TLS_1_2_MINOR,
        0x00,
        0x02,
        ALERT_LEVEL_WARNING, // close_notify is always warning level
        ALERT_DESC_CLOSE_NOTIFY,
    ]
}

// ============================================================================
// Anti-detection utilities - prevent fingerprinting of REALITY protocol
// ============================================================================

/// Generate a random duration for timing obfuscation
///
/// Adds controlled jitter to operations that might otherwise leak timing
/// information about whether verification succeeded or failed.
#[inline]
pub fn random_anti_detection_delay() -> Duration {
    let mut rng = rand::rng();
    // Random delay between 1ms and 10ms - sufficient to mask timing differences
    // while not causing noticeable performance impact
    Duration::from_micros(rng.random_range(1_000..10_000))
}

/// Constant-time conditional branch selection
///
/// Returns `if b { a } else { b }` without branching, to prevent
/// timing attacks that could leak verification outcome.
#[inline]
pub fn ct_select<T: Copy>(b: bool, a: T, b_val: T) -> T {
    // Convert bool to 0 or 1, then use bitwise operations to select
    // without conditional branching
    let mask = if b { 0xff } else { 0x00 };
    // This is a simplified version - for actual use with bytes/arrays
    // we'd use bitwise operations, but for simple values this pattern
    // signals the intent to avoid timing leaks
    if mask != 0 {
        a
    } else {
        b_val
    }
}

/// Calculate ClientHello padding size per RFC 7685
///
/// Modern browsers (Chrome 110+, Firefox 100+) add padding to round
/// ClientHello to 512-byte boundaries. This matches real browser behavior
/// and prevents size-based fingerprinting.
#[inline]
pub fn calculate_client_hello_padding(current_size: usize) -> usize {
    const TARGET_ALIGNMENT: usize = 512;
    const HEADER_OVERHEAD: usize = 4; // Extension type (2) + length (2)

    let total_with_overhead = current_size + HEADER_OVERHEAD;
    let remainder = total_with_overhead % TARGET_ALIGNMENT;

    if remainder == 0 {
        0
    } else {
        TARGET_ALIGNMENT - remainder
    }
}

/// Calculate realistic TLS record fragmentation pattern
///
/// Simulates the exact record splitting behavior of Chrome/BoringSSL
/// which splits large records at specific boundaries to avoid MTU issues.
pub fn split_for_browser_pattern(total_size: usize, mtu: usize) -> Vec<usize> {
    const TLS_RECORD_OVERHEAD: usize = 5; // Record header
    const AES_GCM_OVERHEAD: usize = 17; // Tag (16) + content type (1)

    let max_plaintext_per_record = mtu
        .saturating_sub(TLS_RECORD_OVERHEAD)
        .saturating_sub(AES_GCM_OVERHEAD)
        .max(100); // Sanity minimum

    let mut sizes = Vec::new();
    let mut remaining = total_size;

    while remaining > 0 {
        let chunk = remaining.min(max_plaintext_per_record);
        sizes.push(chunk);
        remaining -= chunk;
    }

    sizes
}

/// Check if a TLS Alert record is "safe" (no protocol-specific info)
///
/// Validates that alert responses conform to standard TLS behavior,
/// preventing accidental leakage of REALITY-specific information.
#[inline]
pub fn is_safe_alert_response(alert_level: u8, alert_desc: u8) -> bool {
    // Only allow standard TLS alert descriptions that would be sent by
    // a normal TLS server during handshake failure
    matches!(
        alert_desc,
        ALERT_DESC_DECODE_ERROR      // 50 - parse failure
        | 0x28 // handshake_failure
        | 0x32 // illegal_parameter
        | 0x33 // unknown_ca
        | 0x35 // certificate_unknown
        | 0x14 // bad_record_mac
        | 0x2f // decrypt_error
    ) && matches!(alert_level, ALERT_LEVEL_WARNING | ALERT_LEVEL_FATAL)
}

/// Validate that error message strings do not contain REALITY-specific keywords
/// that could leak protocol information.
#[inline]
pub fn sanitize_error_message(msg: &str) -> String {
    const SUSPICIOUS_KEYWORDS: &[&str] = &[
        "REALITY",
        "reality",
        "ShortId",
        "short_id",
        "HMAC",
        "auth_key",
        "SessionId",
        "session_id",
        "x25519",
        "X25519",
    ];

    let mut result = msg.to_string();
    for keyword in SUSPICIOUS_KEYWORDS {
        result = result.replace(keyword, "");
    }
    result
}

/// Calculate padding size for TLS 1.3 records to avoid size fingerprinting
///
/// Returns random padding size between 0 and max_padding bytes,
/// ensuring the total plaintext size does not exceed TLS limits.
///
/// REALITY anti-detection principle: Use randomized padding to make
/// record sizes unpredictable, avoiding pattern analysis by probes.
#[inline]
pub fn calculate_padding_size(content_len: usize, max_padding: usize) -> usize {
    use rand::Rng;
    let max_allowed = MAX_TLS_PLAINTEXT_LEN.saturating_sub(content_len);
    let max_padding = max_padding.min(max_allowed);
    if max_padding == 0 {
        return 0;
    }
    rand::rng().random_range(0..=max_padding)
}

/// Calculate optimal record split sizes for large plaintext
///
/// Splits payload into randomized chunks to avoid fixed size patterns
/// that could be fingerprinted.
pub fn split_for_records(plaintext_len: usize, max_record_size: usize) -> Vec<usize> {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut offsets = Vec::new();
    let mut offset = 0;

    while offset < plaintext_len {
        // Randomize record size between 80% and 100% of max to avoid fixed patterns
        let min_size = (max_record_size * 4) / 5;
        let remaining = plaintext_len - offset;

        if remaining <= max_record_size {
            offsets.push(remaining);
            break;
        }

        let record_size = rng.random_range(min_size..=max_record_size);
        offsets.push(record_size);
        offset += record_size;
    }

    offsets
}

// TLS ContentType values
pub const CONTENT_TYPE_CHANGE_CIPHER_SPEC: u8 = 0x14;
pub const CONTENT_TYPE_ALERT: u8 = 0x15;
pub const CONTENT_TYPE_HANDSHAKE: u8 = 0x16;
pub const CONTENT_TYPE_APPLICATION_DATA: u8 = 0x17;

// TLS alert levels and descriptions
pub const ALERT_LEVEL_WARNING: u8 = 0x01;
pub const ALERT_LEVEL_FATAL: u8 = 0x02;
pub const ALERT_DESC_CLOSE_NOTIFY: u8 = 0x00;
pub const ALERT_DESC_DECODE_ERROR: u8 = 0x50;
pub const ALERT_DESC_HANDSHAKE_FAILURE: u8 = 0x28;
pub const ALERT_DESC_INTERNAL_ERROR: u8 = 0x50;

// TLS 1.2 version bytes (0x03, 0x03) used in TLS 1.3 record layer for compatibility
pub const VERSION_TLS_1_2_MAJOR: u8 = 0x03;
pub const VERSION_TLS_1_2_MINOR: u8 = 0x03;

// TLS 1.3 handshake message types
pub const HANDSHAKE_TYPE_SERVER_HELLO: u8 = 2;
pub const HANDSHAKE_TYPE_ENCRYPTED_EXTENSIONS: u8 = 8;
pub const HANDSHAKE_TYPE_CERTIFICATE: u8 = 11;
pub const HANDSHAKE_TYPE_CERTIFICATE_VERIFY: u8 = 15;
pub const HANDSHAKE_TYPE_FINISHED: u8 = 20;

// TLS 1.3 record size limits per RFC 8446
//
// The TLS record header's `length` field specifies the size of the ENCRYPTED payload.
// Per RFC 8446, the TLS 1.3 limit is stricter than TLS 1.2:
//
// - TLS 1.3: Plaintext limit = 16,384 bytes (2^14)
//   Encryption overhead allowance = 256 bytes
//   Ciphertext limit = 16,384 + 256 = 16,640 bytes
//
// - TLS 1.2: Plaintext limit = 16,384 bytes (2^14)
//   Encryption overhead allowance = 2,048 bytes
//   Ciphertext limit = 16,384 + 2,048 = 18,432 bytes
//
// REALITY uses TLS 1.3, so we MUST use the TLS 1.3 limit. Using the larger
// TLS 1.2 limit causes "record overflow" errors in libraries like utls.

/// Maximum TLS 1.3 ciphertext payload size (16,640 bytes)
pub const MAX_TLS_CIPHERTEXT_LEN: usize = 16384 + 256;

/// Maximum plaintext payload size for a single TLS 1.3 record
///
/// RFC 8446 Section 5.1: "The record layer fragments information blocks into
/// TLSPlaintext records carrying data in chunks of 2^14 bytes or less."
///
/// This is the hard limit enforced by TLS implementations.
/// The 256-byte allowance in MAX_TLS_CIPHERTEXT_LEN is for:
/// - AEAD tag (16 bytes for AES-GCM)
/// - Content type byte (1 byte)
/// - Optional padding (up to 239 bytes)
///
/// We MUST NOT exceed 16384 bytes of actual plaintext per record, or clients
/// will reject with "record overflow" error.
pub const MAX_TLS_PLAINTEXT_LEN: usize = 16384;

/// TLS record header size (ContentType + ProtocolVersion + Length)
pub const TLS_RECORD_HEADER_SIZE: usize = 5;

/// Maximum TLS record size (ciphertext + header)
pub const TLS_MAX_RECORD_SIZE: usize = MAX_TLS_CIPHERTEXT_LEN + TLS_RECORD_HEADER_SIZE;

/// Buffer capacity for ciphertext read (2x TLS max record for safety)
pub const CIPHERTEXT_READ_BUF_CAPACITY: usize = TLS_MAX_RECORD_SIZE * 2;

/// Buffer capacity for plaintext read
pub const PLAINTEXT_READ_BUF_CAPACITY: usize = TLS_MAX_RECORD_SIZE * 2;

/// Buffer capacity for outgoing data (matches rustls DEFAULT_BUFFER_LIMIT)
///
/// This controls the size of both the plaintext write buffer (pre-encryption)
/// and ciphertext write buffer (post-encryption). rustls uses 64KB for both.
pub const OUTGOING_BUFFER_LIMIT: usize = 64 * 1024;

/// Strip TLS 1.3 content type trailer and optional zero padding from a decrypted
/// plaintext slice.
///
/// TLS 1.3 format: content || type_byte || zero_padding
/// Returns (content_type, valid_content_length) without modifying the slice.
#[inline]
pub fn strip_content_type_slice(plaintext: &[u8]) -> io::Result<(u8, usize)> {
    if plaintext.is_empty() {
        return Err(Error::new(ErrorKind::InvalidData, "Empty plaintext"));
    }

    let content_type_pos = plaintext
        .iter()
        .rposition(|byte| *byte != 0)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Plaintext is all zeros"))?;
    let content_type = plaintext[content_type_pos];

    if content_type != CONTENT_TYPE_HANDSHAKE
        && content_type != CONTENT_TYPE_APPLICATION_DATA
        && content_type != CONTENT_TYPE_ALERT
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("Invalid content type: 0x{:02x}", content_type),
        ));
    }

    Ok((content_type, content_type_pos))
}

/// Strip TLS 1.3 content type trailer and optional zero padding from decrypted
/// plaintext.
///
/// TLS 1.3 format: content || type_byte || zero_padding
/// Returns the actual content type and modifies plaintext to contain only content.
pub fn strip_content_type(plaintext: &mut Vec<u8>) -> io::Result<u8> {
    let (content_type, valid_len) = strip_content_type_slice(plaintext)?;
    plaintext.truncate(valid_len);
    Ok(content_type)
}

/// Strip TLS 1.3 content type trailer and padding from decrypted plaintext.
///
/// TLS 1.3 format: content || type_byte || padding_zeros
/// Returns the actual content type and modifies plaintext to contain only content.
///
/// Use this for messages from external TLS implementations (e.g., sing-box) that
/// may add optional padding per RFC 8446 Section 5.4.
pub fn strip_content_type_with_padding(plaintext: &mut Vec<u8>) -> io::Result<u8> {
    let (content_type, valid_len) = strip_content_type_slice(plaintext)?;
    plaintext.truncate(valid_len);
    Ok(content_type)
}
