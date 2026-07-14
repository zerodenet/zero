//! Custom TLS 1.3 ClientHello builder 鈥?byte-level construction.
//!
//! Contains REALITY-derived constants for Chrome-style extension ordering,
//! signature algorithms, supported groups, and padding.
//! Used by fingerprint presets for byte-accurate parameter selection.
//!
//! A full custom-TLS handshake (replacing rustls) would use this builder
//! together with the TLS 1.3 record layer from protocols/vless/src/reality/.
//! That is tracked as a future enhancement.

/// TLS 1.3 cipher suite IDs (IANA).
pub mod cipher_ids {
    pub const AES_128_GCM: u16 = 0x1301;
    pub const AES_256_GCM: u16 = 0x1302;
    pub const CHACHA20_POLY1305: u16 = 0x1303;
}

/// REALITY's default cipher suite order (Chrome-like).
pub const DEFAULT_CIPHER_SUITE_ORDER: &[u16] = &[
    cipher_ids::AES_128_GCM,
    cipher_ids::AES_256_GCM,
    cipher_ids::CHACHA20_POLY1305,
];

/// TLS Extension type IDs.
pub mod extension_ids {
    pub const SERVER_NAME: u16 = 0;
    pub const SUPPORTED_GROUPS: u16 = 10;
    pub const EC_POINT_FORMATS: u16 = 11;
    pub const SIGNATURE_ALGORITHMS: u16 = 13;
    pub const ALPN: u16 = 16;
    pub const ENCRYPT_THEN_MAC: u16 = 22;
    pub const EXTENDED_MASTER_SECRET: u16 = 23;
    pub const COMPRESS_CERTIFICATE: u16 = 27;
    pub const SUPPORTED_VERSIONS: u16 = 43;
    pub const PSK_KEY_EXCHANGE_MODES: u16 = 45;
    pub const CERT_SIGNATURE_ALGORITHMS: u16 = 50;
    pub const KEY_SHARE: u16 = 51;
    pub const PADDING: u16 = 21;
}

/// Chrome 120+ extension ordering (REALITY-derived).
///
/// This is the exact order Chrome 120 sends extensions in the ClientHello.
pub const CHROME_EXTENSION_ORDER: &[u16] = &[
    extension_ids::SERVER_NAME,               // 0
    extension_ids::SUPPORTED_VERSIONS,        // 43
    extension_ids::EXTENDED_MASTER_SECRET,    // 23
    extension_ids::EC_POINT_FORMATS,          // 11
    extension_ids::SUPPORTED_GROUPS,          // 10
    extension_ids::KEY_SHARE,                 // 51
    extension_ids::SIGNATURE_ALGORITHMS,      // 13
    extension_ids::CERT_SIGNATURE_ALGORITHMS, // 50
    extension_ids::ALPN,                      // 16
    extension_ids::COMPRESS_CERTIFICATE,      // 27
    extension_ids::ENCRYPT_THEN_MAC,          // 22
    extension_ids::PSK_KEY_EXCHANGE_MODES,    // 45
];

/// Supported groups in Chrome 120 order (REALITY-derived).
pub mod supported_groups {
    pub const X25519: u16 = 0x001d;
    pub const SECP256R1: u16 = 0x0017;
    pub const SECP384R1: u16 = 0x0018;

    pub const CHROME_ORDER: &[u16] = &[X25519, SECP256R1, SECP384R1];
}

/// Signature algorithms in Chrome 120 order (REALITY-derived).
pub mod signature_algorithms {
    pub const ECDSA_SECP256R1_SHA256: u16 = 0x0403;
    pub const RSA_PSS_RSAE_SHA384: u16 = 0x0804;
    pub const RSA_PKCS1_SHA256: u16 = 0x0401;
    pub const ECDSA_SECP384R1_SHA384: u16 = 0x0503;
    pub const RSA_PSS_RSAE_SHA512: u16 = 0x0805;
    pub const RSA_PKCS1_SHA384: u16 = 0x0501;
    pub const RSA_PSS_PSS_SHA512: u16 = 0x0806;
    pub const RSA_PKCS1_SHA512: u16 = 0x0601;
    pub const ECDSA_SECP521R1_SHA512: u16 = 0x0603;
    pub const RSA_PSS_RSAE_SHA256: u16 = 0x0807;

    pub const CHROME_ORDER: &[u16] = &[
        ECDSA_SECP256R1_SHA256, // 0x0403
        RSA_PSS_RSAE_SHA256,    // 0x0807
        RSA_PKCS1_SHA256,       // 0x0401
        ECDSA_SECP384R1_SHA384, // 0x0503
        RSA_PSS_RSAE_SHA384,    // 0x0804
        RSA_PKCS1_SHA384,       // 0x0501
        RSA_PSS_PSS_SHA512,     // 0x0806
        RSA_PKCS1_SHA512,       // 0x0601
        RSA_PSS_RSAE_SHA512,    // 0x0805
    ];
}

/// Firefox 120+ extension ordering.
pub const FIREFOX_EXTENSION_ORDER: &[u16] = &[
    extension_ids::SERVER_NAME,               // 0
    extension_ids::EXTENDED_MASTER_SECRET,    // 23
    extension_ids::SUPPORTED_VERSIONS,        // 43
    extension_ids::SUPPORTED_GROUPS,          // 10
    extension_ids::KEY_SHARE,                 // 51
    extension_ids::SIGNATURE_ALGORITHMS,      // 13
    extension_ids::ALPN,                      // 16
    extension_ids::CERT_SIGNATURE_ALGORITHMS, // 50
    extension_ids::PSK_KEY_EXCHANGE_MODES,    // 45
    extension_ids::EC_POINT_FORMATS,          // 11
];

/// Chrome ALPN 鈥?prefers HTTP/2 then HTTP/1.1.
pub const CHROME_ALPN: &[&str] = &["h2", "http/1.1"];

/// Chrome padding 鈥?rounds ClientHello to 512-byte boundary.
pub fn chrome_padding_size(current_size: usize) -> usize {
    let target = if current_size < 256 {
        256
    } else if current_size < 512 {
        512
    } else {
        // Round up to next 256-byte boundary
        current_size.div_ceil(256) * 256
    };
    target.saturating_sub(current_size)
}
