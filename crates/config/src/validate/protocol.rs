use std::collections::HashSet;

use crate::{
    ConfigError, InboundProtocolConfig, InboundRealityConfig, OutboundProtocolConfig,
    RealityConfig, Socks5UserConfig, VlessUserConfig, VmessUserConfig,
};

pub(super) fn validate_inbound_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<(), ConfigError> {
    match protocol {
        InboundProtocolConfig::Socks5 { users } => validate_socks5_users("socks5 inbound", users),
        InboundProtocolConfig::Mixed { socks5_users } => {
            validate_socks5_users("mixed inbound socks5", socks5_users)
        }
        InboundProtocolConfig::HttpConnect => Ok(()),
        InboundProtocolConfig::Vless {
            users,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            fallback: _,
            quic,
            split_http: _,
        } => {
            validate_vless_users(users)?;
            if let Some(tls) = tls {
                validate_inbound_optional_non_empty("vless tls.cert_path", &tls.cert_path)?;
                validate_inbound_optional_non_empty("vless tls.key_path", &tls.key_path)?;
            }
            if let Some(reality) = reality {
                validate_vless_inbound_reality(reality)?;
            }
            if tls.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "`vless` inbound cannot set both `tls` and `reality`".to_owned(),
                ));
            }
            if ws.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "`vless` inbound `reality` supports raw TCP only, not `ws`".to_owned(),
                ));
            }
            if grpc.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "`vless` inbound `reality` supports raw TCP only, not `grpc`".to_owned(),
                ));
            }
            if h2.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "`vless` inbound `reality` supports raw TCP only, not `h2`".to_owned(),
                ));
            }
            if http_upgrade.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "`vless` inbound `reality` supports raw TCP only, not `http_upgrade`"
                        .to_owned(),
                ));
            }
            if quic.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "`vless` inbound `reality` supports raw TCP only, not `quic`".to_owned(),
                ));
            }
            if let Some(ws) = ws {
                validate_inbound_optional_non_empty("vless ws.path", &ws.path)?;
                validate_inbound_ws_headers("vless ws.headers", &ws.headers)?;
            }
            if let Some(grpc) = grpc {
                for name in &grpc.service_names {
                    validate_inbound_optional_non_empty("vless grpc.service_names", name)?;
                }
            }
            if let Some(h2) = h2 {
                validate_inbound_optional_non_empty("vless h2.path", &h2.path)?;
            }
            if let Some(http_upgrade) = http_upgrade {
                validate_inbound_optional_non_empty("vless http_upgrade.path", &http_upgrade.path)?;
            }
            if let Some(quic) = quic {
                if let Some(cert_path) = &quic.cert_path {
                    validate_inbound_optional_non_empty("vless quic.cert_path", cert_path)?;
                }
                if let Some(key_path) = &quic.key_path {
                    validate_inbound_optional_non_empty("vless quic.key_path", key_path)?;
                }
            }
            Ok(())
        }
        InboundProtocolConfig::Hysteria2 {
            password,
            cert_path,
            key_path,
            ..
        } => {
            validate_inbound_optional_non_empty("hysteria2 password", password)?;
            if cert_path.is_some() != key_path.is_some() {
                return Err(ConfigError::InvalidInbound(
                    "hysteria2 tls requires both cert_path and key_path, or neither".to_owned(),
                ));
            }
            if let Some(cert_path) = cert_path {
                validate_inbound_optional_non_empty("hysteria2 cert_path", cert_path)?;
            }
            if let Some(key_path) = key_path {
                validate_inbound_optional_non_empty("hysteria2 key_path", key_path)?;
            }
            Ok(())
        }
        InboundProtocolConfig::Shadowsocks {
            password, cipher, ..
        } => {
            validate_inbound_optional_non_empty("shadowsocks password", password)?;
            validate_shadowsocks_cipher("inbound", cipher)?;
            Ok(())
        }
        InboundProtocolConfig::Trojan {
            password,
            sni: _,
            tls: _,
            ..
        } => {
            validate_inbound_optional_non_empty("trojan password", password)?;
            Ok(())
        }
        InboundProtocolConfig::Vmess { users, .. } => {
            validate_vmess_users(users)?;
            Ok(())
        }
        InboundProtocolConfig::Direct { .. } => Ok(()),
        InboundProtocolConfig::Mieru { users } => {
            if users.is_empty() {
                return Err(ConfigError::InvalidInbound(
                    "`mieru` inbound requires at least one user".to_owned(),
                ));
            }
            Ok(())
        }
    }
}

pub(super) fn validate_outbound_protocol(
    protocol: &OutboundProtocolConfig,
) -> Result<(), ConfigError> {
    match protocol {
        OutboundProtocolConfig::Socks5 {
            username, password, ..
        } => validate_socks5_outbound_auth(username.as_deref(), password.as_deref()),
        OutboundProtocolConfig::Vless {
            server,
            port,
            id,
            flow: _,
            mux_concurrency: _,
            mux_idle_timeout_secs: _,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            quic,
            split_http: _,
        } => {
            validate_outbound_endpoint("vless", server, *port)?;
            validate_uuid_literal(id).map_err(|message| {
                ConfigError::InvalidOutbound(format!("`vless` outbound `id` {message}"))
            })?;
            if let Some(tls) = tls {
                if let Some(server_name) = &tls.server_name {
                    validate_outbound_optional_non_empty("vless tls.server_name", server_name)?;
                }
                if let Some(ca_cert_path) = &tls.ca_cert_path {
                    validate_outbound_optional_non_empty("vless tls.ca_cert_path", ca_cert_path)?;
                }
            }
            if let Some(reality) = reality {
                validate_vless_reality(reality)?;
            }
            if tls.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidOutbound(
                    "`vless` outbound cannot set both `tls` and `reality`".to_owned(),
                ));
            }
            if ws.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidOutbound(
                    "`vless` outbound `reality` supports raw TCP only, not `ws`".to_owned(),
                ));
            }
            if grpc.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidOutbound(
                    "`vless` outbound `reality` supports raw TCP only, not `grpc`".to_owned(),
                ));
            }
            if h2.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidOutbound(
                    "`vless` outbound `reality` supports raw TCP only, not `h2`".to_owned(),
                ));
            }
            if http_upgrade.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidOutbound(
                    "`vless` outbound `reality` supports raw TCP only, not `http_upgrade`"
                        .to_owned(),
                ));
            }
            if quic.is_some() && reality.is_some() {
                return Err(ConfigError::InvalidOutbound(
                    "`vless` outbound `reality` supports raw TCP only, not `quic`".to_owned(),
                ));
            }
            if let Some(ws) = ws {
                validate_outbound_optional_non_empty("vless ws.path", &ws.path)?;
                validate_outbound_ws_headers("vless ws.headers", &ws.headers)?;
            }
            if let Some(grpc) = grpc {
                for name in &grpc.service_names {
                    validate_outbound_optional_non_empty("vless grpc.service_names", name)?;
                }
            }
            if let Some(h2) = h2 {
                validate_outbound_optional_non_empty("vless h2.path", &h2.path)?;
            }
            if let Some(http_upgrade) = http_upgrade {
                validate_outbound_optional_non_empty(
                    "vless http_upgrade.path",
                    &http_upgrade.path,
                )?;
            }
            if let Some(quic) = quic {
                if let Some(server_name) = &quic.server_name {
                    validate_outbound_optional_non_empty("vless quic.server_name", server_name)?;
                }
            }
            Ok(())
        }
        OutboundProtocolConfig::Hysteria2 { server, port, .. } => {
            validate_outbound_endpoint("hysteria2", server, *port)?;
            Ok(())
        }
        OutboundProtocolConfig::Shadowsocks {
            server,
            port,
            password: _,
            cipher,
        } => {
            validate_outbound_endpoint("shadowsocks", server, *port)?;
            validate_shadowsocks_cipher("outbound", cipher)?;
            Ok(())
        }
        OutboundProtocolConfig::Trojan { server, port, .. } => {
            validate_outbound_endpoint("trojan", server, *port)?;
            Ok(())
        }
        OutboundProtocolConfig::Vmess {
            server,
            port,
            id,
            cipher: _,
            tls: _,
            ws: _,
            grpc: _,
        } => {
            validate_outbound_endpoint("vmess", server, *port)?;
            validate_uuid_literal(id)
                .map_err(|m| ConfigError::InvalidOutbound(format!("`vmess` outbound `id` {m}")))?;
            Ok(())
        }
        OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => Ok(()),
        OutboundProtocolConfig::Mieru {
            server,
            port,
            username: _,
            password: _,
        } => {
            validate_outbound_endpoint("mieru", server, *port)?;
            Ok(())
        }
    }
}

fn validate_vless_users(users: &[VlessUserConfig]) -> Result<(), ConfigError> {
    if users.is_empty() {
        return Err(ConfigError::InvalidInbound(
            "`vless` inbound requires at least one user".to_owned(),
        ));
    }

    let mut seen = HashSet::new();
    for user in users {
        validate_uuid_literal(&user.id).map_err(|message| {
            ConfigError::InvalidInbound(format!("`vless` inbound user `id` {message}"))
        })?;

        if !seen.insert(normalize_uuid_key(&user.id)) {
            return Err(ConfigError::InvalidInbound(
                "`vless` inbound contains duplicate user id".to_owned(),
            ));
        }

        if let Some(credential_id) = &user.credential_id {
            validate_inbound_optional_non_empty("vless credential_id", credential_id)?;
        }
        if let Some(principal_key) = &user.principal_key {
            validate_inbound_optional_non_empty("vless principal_key", principal_key)?;
        }
    }

    Ok(())
}

fn validate_vmess_users(users: &[VmessUserConfig]) -> Result<(), ConfigError> {
    if users.is_empty() {
        return Err(ConfigError::InvalidInbound(
            "`vmess` inbound requires at least one user".to_owned(),
        ));
    }

    let valid_ciphers = ["aes-128-gcm", "aes-256-gcm", "chacha20-poly1305"];
    for user in users {
        validate_uuid_literal(&user.id).map_err(|message| {
            ConfigError::InvalidInbound(format!("`vmess` inbound user `id` {message}"))
        })?;

        if !valid_ciphers.contains(&user.cipher.as_str()) {
            return Err(ConfigError::InvalidInbound(format!(
                "`vmess` inbound cipher `{}` is not valid; expected one of: {}",
                user.cipher,
                valid_ciphers.join(", ")
            )));
        }
    }

    Ok(())
}

fn validate_vless_inbound_reality(reality: &InboundRealityConfig) -> Result<(), ConfigError> {
    validate_inbound_optional_non_empty("vless reality.private_key", &reality.private_key)?;
    if !matches!(base64url_decoded_len(&reality.private_key), Some(32)) {
        return Err(ConfigError::InvalidInbound(
            "`vless` inbound `reality.private_key` must be a 32-byte base64url value without padding"
                .to_owned(),
        ));
    }

    for short_id in &reality.short_ids {
        validate_short_id("inbound", short_id).map_err(ConfigError::InvalidInbound)?;
    }

    if let Some(server_name) = &reality.server_name {
        validate_inbound_optional_non_empty("vless reality.server_name", server_name)?;
    }

    validate_reality_cipher_suites("inbound", &reality.cipher_suites)
        .map_err(ConfigError::InvalidInbound)
}

fn validate_vless_reality(reality: &RealityConfig) -> Result<(), ConfigError> {
    validate_outbound_optional_non_empty("vless reality.public_key", &reality.public_key)?;
    if !matches!(base64url_decoded_len(&reality.public_key), Some(32)) {
        return Err(ConfigError::InvalidOutbound(
            "`vless` outbound `reality.public_key` must be a 32-byte base64url value without padding"
                .to_owned(),
        ));
    }

    validate_short_id("outbound", &reality.short_id).map_err(ConfigError::InvalidOutbound)?;

    if let Some(server_name) = &reality.server_name {
        validate_outbound_optional_non_empty("vless reality.server_name", server_name)?;
    }

    validate_reality_cipher_suites("outbound", &reality.cipher_suites)
        .map_err(ConfigError::InvalidOutbound)
}

fn validate_short_id(kind: &str, short_id: &str) -> Result<(), String> {
    if short_id.len() > 16 {
        return Err(format!(
            "`vless` {kind} `reality.short_id` must be at most 16 hex characters"
        ));
    }
    if !short_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "`vless` {kind} `reality.short_id` must contain only hex digits"
        ));
    }
    Ok(())
}

fn validate_reality_cipher_suites(kind: &str, cipher_suites: &[String]) -> Result<(), String> {
    for cipher_suite in cipher_suites {
        match cipher_suite.as_str() {
            "TLS_AES_128_GCM_SHA256"
            | "TLS_AES_256_GCM_SHA384"
            | "TLS_CHACHA20_POLY1305_SHA256" => {}
            _ => {
                return Err(format!(
                    "`vless` {kind} `reality.cipher_suites` contains unsupported suite `{cipher_suite}`"
                ));
            }
        }
    }

    Ok(())
}

fn base64url_decoded_len(value: &str) -> Option<usize> {
    let mut bits = 0usize;
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => bits += 6,
            _ => return None,
        }
    }

    Some(bits / 8)
}

fn validate_outbound_endpoint(
    protocol: &'static str,
    server: &str,
    port: u16,
) -> Result<(), ConfigError> {
    if server.trim().is_empty() {
        return Err(ConfigError::InvalidOutbound(format!(
            "`{protocol}` outbound requires a non-empty `server`"
        )));
    }

    if port == 0 {
        return Err(ConfigError::InvalidOutbound(format!(
            "`{protocol}` outbound `port` must be greater than 0"
        )));
    }

    Ok(())
}

fn validate_socks5_users(
    scope: &'static str,
    users: &[Socks5UserConfig],
) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();

    for user in users {
        validate_socks5_credential_part(scope, "username", &user.username)?;
        validate_socks5_credential_part(scope, "password", &user.password)?;
        if !seen.insert(user.username.as_str()) {
            return Err(ConfigError::InvalidInbound(format!(
                "`{scope}` contains duplicate username `{}`",
                user.username
            )));
        }
    }

    Ok(())
}

fn validate_socks5_outbound_auth(
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(), ConfigError> {
    match (username, password) {
        (None, None) => Ok(()),
        (Some(username), Some(password)) => {
            validate_socks5_outbound_credential_part("username", username)?;
            validate_socks5_outbound_credential_part("password", password)
        }
        _ => Err(ConfigError::InvalidOutbound(
            "`socks5` outbound requires both `username` and `password`, or neither".to_owned(),
        )),
    }
}

fn validate_socks5_outbound_credential_part(
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    let len = value.len();
    if len == 0 {
        return Err(ConfigError::InvalidOutbound(format!(
            "`socks5` outbound `{field}` must not be empty"
        )));
    }

    if len > u8::MAX as usize {
        return Err(ConfigError::InvalidOutbound(format!(
            "`socks5` outbound `{field}` must be at most 255 bytes"
        )));
    }

    Ok(())
}

fn validate_socks5_credential_part(
    scope: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    let len = value.len();
    if len == 0 {
        return Err(ConfigError::InvalidInbound(format!(
            "`{scope}` `{field}` must not be empty"
        )));
    }

    if len > u8::MAX as usize {
        return Err(ConfigError::InvalidInbound(format!(
            "`{scope}` `{field}` must be at most 255 bytes"
        )));
    }

    Ok(())
}

fn validate_inbound_optional_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::InvalidInbound(format!(
            "`{field}` must not be empty"
        )));
    }

    Ok(())
}

fn validate_outbound_optional_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::InvalidOutbound(format!(
            "`{field}` must not be empty"
        )));
    }

    Ok(())
}

fn validate_inbound_ws_headers(
    field: &'static str,
    headers: &std::collections::HashMap<String, String>,
) -> Result<(), ConfigError> {
    for key in headers.keys() {
        let key_lower = key.to_lowercase();
        if is_reserved_ws_header(&key_lower) {
            return Err(ConfigError::InvalidInbound(format!(
                "`{field}` contains reserved header `{key}` which is managed by WebSocket handshake",
            )));
        }
    }

    Ok(())
}

fn validate_outbound_ws_headers(
    field: &'static str,
    headers: &std::collections::HashMap<String, String>,
) -> Result<(), ConfigError> {
    for key in headers.keys() {
        let key_lower = key.to_lowercase();
        if is_reserved_ws_header(&key_lower) {
            return Err(ConfigError::InvalidOutbound(format!(
                "`{field}` contains reserved header `{key}` which is managed by WebSocket handshake",
            )));
        }
    }

    Ok(())
}

fn is_reserved_ws_header(header: &str) -> bool {
    const RESERVED_HEADERS: &[&str] = &[
        "host",
        "connection",
        "upgrade",
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-protocol",
        "sec-websocket-extensions",
        "sec-websocket-accept",
    ];
    RESERVED_HEADERS.contains(&header)
}

fn validate_uuid_literal(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    let mut digits = 0;

    for (index, byte) in value.bytes().enumerate() {
        if byte == b'-' {
            if value.len() != 36 || !matches!(index, 8 | 13 | 18 | 23) {
                return Err("must be a canonical UUID or 32 hex digits");
            }
            continue;
        }

        if !byte.is_ascii_hexdigit() {
            return Err("must contain only hex digits");
        }

        digits += 1;
    }

    if digits == 32 {
        Ok(())
    } else {
        Err("must contain 32 hex digits")
    }
}

fn normalize_uuid_key(value: &str) -> String {
    value
        .bytes()
        .filter(|byte| *byte != b'-')
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect()
}

fn validate_shadowsocks_cipher(kind: &'static str, cipher: &str) -> Result<(), ConfigError> {
    const VALID_CIPHERS: &[&str] = &[
        "aes-128-gcm",
        "aes-256-gcm",
        "chacha20-ietf-poly1305",
        "2022-blake3-aes-128-gcm",
        "2022-blake3-aes-256-gcm",
    ];
    if !VALID_CIPHERS.iter().any(|c| *c == cipher) {
        return Err(match kind {
            "inbound" => ConfigError::InvalidInbound(format!(
                "`shadowsocks` {kind} cipher `{cipher}` is not valid; expected one of: {}",
                VALID_CIPHERS.join(", ")
            )),
            _ => ConfigError::InvalidOutbound(format!(
                "`shadowsocks` {kind} cipher `{cipher}` is not valid; expected one of: {}",
                VALID_CIPHERS.join(", ")
            )),
        });
    }
    Ok(())
}
