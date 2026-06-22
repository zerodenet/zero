use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zero_config::{
    EventSinkConfig, InboundProtocolConfig, LoadBalanceStrategy, ModeConfig, OutboundGroupKind,
    OutboundProtocolConfig, RouteActionConfig, RuleConditionConfig, RuntimeConfig,
};

#[test]
fn parses_config_into_adts() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "socks-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "socks5" }
                },
                {
                    "tag": "http-in",
                    "listen": { "address": "127.0.0.1", "port": 8080 },
                    "protocol": { "type": "http_connect" }
                }
            ],
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "block",
                    "protocol": { "type": "block" }
                },
                {
                    "tag": "chain",
                    "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["chain", "direct"],
                    "selected": "chain"
                }
            ],
            "runtime": {
                "udp_upstream_idle_timeout_seconds": 12
            },
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [
                    {
                        "condition": {
                            "type": "or",
                            "items": [
                                { "type": "domain", "values": ["blocked.example"] },
                                { "type": "ip", "values": ["10.0.0.0/8"] }
                            ]
                        },
                        "action": { "type": "route", "outbound": "block" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.inbounds[0].protocol,
        InboundProtocolConfig::Socks5 { .. }
    ));
    assert!(matches!(
        config.inbounds[1].protocol,
        InboundProtocolConfig::HttpConnect
    ));
    assert!(matches!(
        config.outbounds[0].protocol,
        OutboundProtocolConfig::Direct
    ));
    assert!(matches!(
        config.outbounds[1].protocol,
        OutboundProtocolConfig::Block
    ));
    assert!(matches!(
        config.outbounds[2].protocol,
        OutboundProtocolConfig::Socks5 { .. }
    ));
    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::Selector { .. }
    ));
    assert_eq!(config.runtime.udp_upstream_idle_timeout_seconds, 12);
    assert!(matches!(config.mode, ModeConfig::Global { .. }));
    assert!(matches!(
        config.route.final_action,
        RouteActionConfig::Direct
    ));
    assert!(matches!(
        config.route.rules[0].condition,
        RuleConditionConfig::Or { .. }
    ));
}

#[test]
fn parses_vless_inbound_and_outbound_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-in",
                    "listen": { "address": "127.0.0.1", "port": 1082 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            {
                                "id": "11111111-2222-3333-4444-555555555555",
                                "credential_id": "node-user-1",
                                "principal_key": "user:10001"
                            }
                        ]
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "vless-chain",
                    "protocol": {
                        "type": "vless",
                        "server": "127.0.0.1",
                        "port": 2081,
                        "id": "11111111-2222-3333-4444-555555555555"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vless-chain" }
            }
        }"#,
    )
    .expect("config should parse");

    match &config.inbounds[0].protocol {
        InboundProtocolConfig::Vless { users, .. } => {
            assert_eq!(users[0].credential_id.as_deref(), Some("node-user-1"));
            assert_eq!(users[0].principal_key.as_deref(), Some("user:10001"));
        }
        _ => panic!("expected vless inbound"),
    }
    assert_eq!(config.inbounds[0].protocol.vless_users().len(), 1);
    assert!(matches!(
        config.outbounds[0].protocol,
        OutboundProtocolConfig::Vless { .. }
    ));
}

#[test]
fn parses_vmess_inbound_and_outbound_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vmess-in",
                    "listen": { "address": "127.0.0.1", "port": 1082 },
                    "protocol": {
                        "type": "vmess",
                        "users": [
                            {
                                "id": "11111111-2222-3333-4444-555555555555",
                                "cipher": "chacha20-poly1305",
                                "credential_id": "node-user-1",
                                "principal_key": "user:10001"
                            }
                        ],
                        "tls": {
                            "cert_path": "certs/server.crt",
                            "key_path": "certs/server.key"
                        }
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "vmess-chain",
                    "protocol": {
                        "type": "vmess",
                        "server": "example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "cipher": "chacha20-poly1305",
                        "tls": {
                            "server_name": "example.com",
                            "ca_cert_path": "certs/ca.pem"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vmess-chain" }
            }
        }"#,
    )
    .expect("vmess config should parse");

    assert!(matches!(
        config.inbounds[0].protocol,
        InboundProtocolConfig::Vmess { .. }
    ));
    assert!(matches!(
        config.outbounds[0].protocol,
        OutboundProtocolConfig::Vmess { .. }
    ));
}

#[test]
fn rejects_vmess_inbound_without_tls() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vmess-in",
                    "listen": { "address": "127.0.0.1", "port": 1082 },
                    "protocol": {
                        "type": "vmess",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ]
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("vmess inbound without tls should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));
}

#[test]
fn normalizes_vmess_cipher_auto_to_aead_baseline() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vmess-in",
                    "listen": { "address": "127.0.0.1", "port": 1082 },
                    "protocol": {
                        "type": "vmess",
                        "users": [
                            {
                                "id": "11111111-2222-3333-4444-555555555555",
                                "cipher": "auto"
                            }
                        ],
                        "tls": {
                            "cert_path": "certs/server.crt",
                            "key_path": "certs/server.key"
                        }
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "vmess-chain",
                    "protocol": {
                        "type": "vmess",
                        "server": "example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "cipher": "auto"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vmess-chain" }
            }
        }"#,
    )
    .expect("vmess cipher auto should normalize");

    match &config.inbounds[0].protocol {
        InboundProtocolConfig::Vmess { users, .. } => {
            assert_eq!(users[0].cipher, "aes-128-gcm");
        }
        _ => panic!("expected vmess inbound"),
    }

    match &config.outbounds[0].protocol {
        OutboundProtocolConfig::Vmess { cipher, .. } => {
            assert_eq!(cipher, "aes-128-gcm");
        }
        _ => panic!("expected vmess outbound"),
    }
}

#[test]
fn rejects_unknown_vmess_cipher() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "vmess-chain",
                    "protocol": {
                        "type": "vmess",
                        "server": "example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "cipher": "bogus"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vmess-chain" }
            }
        }"#,
    )
    .expect_err("unsupported vmess cipher should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));
}

#[test]
fn rejects_vmess_ws_and_grpc_together() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vmess-in",
                    "listen": { "address": "127.0.0.1", "port": 1082 },
                    "protocol": {
                        "type": "vmess",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "tls": {
                            "cert_path": "certs/server.crt",
                            "key_path": "certs/server.key"
                        },
                        "ws": { "path": "/vmess" },
                        "grpc": { "service_names": ["zero.vmess"] }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("vmess inbound ws and grpc together should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));
}

#[test]
fn parses_vless_tls_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-tls-in",
                    "listen": { "address": "127.0.0.1", "port": 8443 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "tls": {
                            "cert_path": "certs/fullchain.pem",
                            "key_path": "certs/privkey.pem"
                        }
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "vless-tls-chain",
                    "protocol": {
                        "type": "vless",
                        "server": "example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "tls": {
                            "server_name": "edge.example.com",
                            "ca_cert_path": "certs/ca.pem"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vless-tls-chain" }
            }
        }"#,
    )
    .expect("config should parse");

    let inbound_tls = config.inbounds[0]
        .protocol
        .vless_tls()
        .expect("vless inbound tls");
    assert_eq!(inbound_tls.cert_path, "certs/fullchain.pem");
    assert_eq!(inbound_tls.key_path, "certs/privkey.pem");

    match &config.outbounds[0].protocol {
        OutboundProtocolConfig::Vless { tls, .. } => {
            let tls = tls.as_ref().expect("vless outbound tls");
            assert_eq!(tls.server_name.as_deref(), Some("edge.example.com"));
            assert_eq!(tls.ca_cert_path.as_deref(), Some("certs/ca.pem"));
        }
        _ => panic!("expected vless outbound"),
    }
}

#[test]
fn parses_vless_reality_outbound_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "vless-reality-chain",
                    "protocol": {
                        "type": "vless",
                        "server": "edge.example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "reality": {
                            "public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                            "short_id": "0123456789abcdef",
                            "server_name": "www.cloudflare.com",
                            "cipher_suites": [
                                "TLS_AES_128_GCM_SHA256",
                                "TLS_CHACHA20_POLY1305_SHA256"
                            ]
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vless-reality-chain" }
            }
        }"#,
    )
    .expect("config should parse");

    match &config.outbounds[0].protocol {
        OutboundProtocolConfig::Vless { reality, .. } => {
            let reality = reality.as_ref().expect("vless outbound reality");
            assert_eq!(reality.server_name.as_deref(), Some("www.cloudflare.com"));
            assert_eq!(reality.short_id, "0123456789abcdef");
            assert_eq!(reality.cipher_suites.len(), 2);
        }
        _ => panic!("expected vless outbound"),
    }
}

#[test]
fn parses_vless_reality_inbound_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-reality-in",
                    "listen": { "address": "127.0.0.1", "port": 8443 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "reality": {
                            "private_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                            "short_ids": ["0123456789abcdef"],
                            "server_name": "www.cloudflare.com",
                            "cipher_suites": ["TLS_AES_128_GCM_SHA256"]
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    let reality = config.inbounds[0]
        .protocol
        .vless_reality()
        .expect("vless inbound reality");
    assert_eq!(reality.short_ids, vec!["0123456789abcdef"]);
    assert_eq!(reality.server_name.as_deref(), Some("www.cloudflare.com"));
}

#[test]
fn rejects_invalid_vless_reality_config() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "vless-reality-chain",
                    "protocol": {
                        "type": "vless",
                        "server": "edge.example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "reality": {
                            "public_key": "bad",
                            "short_id": "0123456789abcdef00"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vless-reality-chain" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));
}

#[test]
fn rejects_invalid_vless_inbound_reality_config() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-reality-in",
                    "listen": { "address": "127.0.0.1", "port": 8443 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "reality": {
                            "private_key": "invalid",
                            "short_ids": ["not-hex"]
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));
}

#[test]
fn rejects_vless_reality_with_tls_or_ws() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "vless-reality-chain",
                    "protocol": {
                        "type": "vless",
                        "server": "edge.example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "tls": {},
                        "reality": {
                            "public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vless-reality-chain" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));

    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "vless-reality-chain",
                    "protocol": {
                        "type": "vless",
                        "server": "edge.example.com",
                        "port": 443,
                        "id": "11111111-2222-3333-4444-555555555555",
                        "ws": { "path": "/vless" },
                        "reality": {
                            "public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "vless-reality-chain" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));
}

#[test]
fn rejects_vless_inbound_reality_with_tls_or_ws() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-reality-in",
                    "listen": { "address": "127.0.0.1", "port": 8443 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "tls": {
                            "cert_path": "cert.pem",
                            "key_path": "key.pem"
                        },
                        "reality": {
                            "private_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));

    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-reality-in",
                    "listen": { "address": "127.0.0.1", "port": 8443 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "ws": { "path": "/vless" },
                        "reality": {
                            "private_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));
}

#[test]
fn rejects_empty_vless_tls_paths() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-tls-in",
                    "listen": { "address": "127.0.0.1", "port": 8443 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "11111111-2222-3333-4444-555555555555" }
                        ],
                        "tls": {
                            "cert_path": "",
                            "key_path": "certs/privkey.pem"
                        }
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));
}

#[test]
fn rejects_invalid_vless_uuid() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "vless-in",
                    "listen": { "address": "127.0.0.1", "port": 1082 },
                    "protocol": {
                        "type": "vless",
                        "users": [
                            { "id": "not-a-uuid" }
                        ]
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidInbound(_)));
}

#[test]
fn accepts_all_shadowsocks_supported_ciphers() {
    const CIPHERS: &[&str] = &[
        "aes-128-gcm",
        "aes-256-gcm",
        "chacha20-ietf-poly1305",
        "2022-blake3-aes-128-gcm",
        "2022-blake3-aes-256-gcm",
        "2022-blake3-chacha20-poly1305",
    ];

    for cipher in CIPHERS {
        let password = shadowsocks_password_for_cipher(cipher);
        let config = RuntimeConfig::parse(&format!(
            r#"{{
                "inbounds": [
                    {{
                        "tag": "ss-in",
                        "listen": {{ "address": "127.0.0.1", "port": 8388 }},
                        "protocol": {{
                            "type": "shadowsocks",
                            "password": "{password}",
                            "cipher": "{cipher}"
                        }}
                    }}
                ],
                "outbounds": [
                    {{
                        "tag": "ss-out",
                        "protocol": {{
                            "type": "shadowsocks",
                            "server": "127.0.0.1",
                            "port": 8389,
                            "password": "{password}",
                            "cipher": "{cipher}"
                        }}
                    }}
                ],
                "route": {{
                    "rules": [],
                    "final": {{ "type": "route", "outbound": "ss-out" }}
                }}
            }}"#
        ))
        .expect("shadowsocks cipher should parse");

        match &config.inbounds[0].protocol {
            InboundProtocolConfig::Shadowsocks { cipher: parsed, .. } => assert_eq!(parsed, cipher),
            _ => panic!("expected shadowsocks inbound"),
        }
        match &config.outbounds[0].protocol {
            OutboundProtocolConfig::Shadowsocks { cipher: parsed, .. } => {
                assert_eq!(parsed, cipher)
            }
            _ => panic!("expected shadowsocks outbound"),
        }
    }
}

fn shadowsocks_password_for_cipher(cipher: &str) -> &'static str {
    match cipher {
        "2022-blake3-aes-128-gcm" => "MDEyMzQ1Njc4OWFiY2RlZg==",
        "2022-blake3-aes-256-gcm" | "2022-blake3-chacha20-poly1305" => {
            "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY="
        }
        _ => "secret",
    }
}

#[test]
fn rejects_invalid_shadowsocks_cipher_and_empty_outbound_password() {
    let cipher_error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "ss-in",
                    "listen": { "address": "127.0.0.1", "port": 8388 },
                    "protocol": {
                        "type": "shadowsocks",
                        "password": "secret",
                        "cipher": "unsupported"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("invalid shadowsocks cipher should fail");

    assert!(matches!(
        cipher_error,
        zero_config::ConfigError::InvalidInbound(_)
    ));

    let password_error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "ss-out",
                    "protocol": {
                        "type": "shadowsocks",
                        "server": "127.0.0.1",
                        "port": 8389,
                        "password": ""
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "ss-out" }
            }
        }"#,
    )
    .expect_err("empty shadowsocks outbound password should fail");

    assert!(matches!(
        password_error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));
}

#[test]
fn rejects_invalid_shadowsocks_2022_password_key_material() {
    let inbound_error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "ss-in",
                    "listen": { "address": "127.0.0.1", "port": 8388 },
                    "protocol": {
                        "type": "shadowsocks",
                        "password": "secret",
                        "cipher": "2022-blake3-aes-128-gcm"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("invalid shadowsocks 2022 password should fail");

    assert!(matches!(
        inbound_error,
        zero_config::ConfigError::InvalidInbound(_)
    ));

    let outbound_error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "ss-out",
                    "protocol": {
                        "type": "shadowsocks",
                        "server": "127.0.0.1",
                        "port": 8389,
                        "password": "MDEyMzQ1Njc4OWFiY2RlZg==",
                        "cipher": "2022-blake3-aes-256-gcm"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "ss-out" }
            }
        }"#,
    )
    .expect_err("wrong shadowsocks 2022 password length should fail");

    assert!(matches!(
        outbound_error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));
}

#[test]
fn parses_api_event_sinks_and_control_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "api": {
                "event_sinks": [
                    {
                        "tag": "panel",
                        "type": "webhook",
                        "url": "https://panel.example.com/api/zero/events",
                        "events": ["flow.completed", "engine.warning"],
                        "source_id": "edge-01",
                        "api_key_env": "ZERO_PANEL_API_KEY"
                    },
                    {
                        "tag": "local-events",
                        "type": "jsonl",
                        "path": "zero-events.jsonl",
                        "events": ["flow.completed"]
                    }
                ],
                "control": {
                    "enabled": true,
                    "listen": { "address": "127.0.0.1", "port": 9090 },
                    "api_key_env": "ZERO_NODE_API_KEY"
                }
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.api.event_sinks.len(), 2);
    let EventSinkConfig::Webhook {
        tag,
        url,
        events,
        source_id,
        api_key_env,
        ..
    } = &config.api.event_sinks[0]
    else {
        panic!("expected webhook sink");
    };
    assert_eq!(tag, "panel");
    assert_eq!(url, "https://panel.example.com/api/zero/events");
    assert_eq!(events, &["flow.completed", "engine.warning"]);
    assert_eq!(source_id.as_deref(), Some("edge-01"));
    assert_eq!(api_key_env.as_deref(), Some("ZERO_PANEL_API_KEY"));

    assert!(config.api.control.enabled);
    assert_eq!(
        config.api.control.listen.as_ref().expect("listen").port,
        9090
    );
}

#[test]
fn rejects_unknown_api_event_type() {
    let error = RuntimeConfig::parse(
        r#"{
            "api": {
                "event_sinks": [
                    {
                        "tag": "panel",
                        "type": "webhook",
                        "url": "https://panel.example.com/api/zero/events",
                        "events": ["panel.user.changed"],
                        "api_key": "secret"
                    }
                ]
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("unknown event type should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidApi(_)));
}

#[test]
fn rejects_insecure_webhook_without_explicit_opt_in() {
    let error = RuntimeConfig::parse(
        r#"{
            "api": {
                "event_sinks": [
                    {
                        "tag": "panel",
                        "type": "webhook",
                        "url": "http://127.0.0.1:9000/events",
                        "events": ["flow.completed"],
                        "api_key": "secret"
                    }
                ]
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("http webhook should require allow_insecure");

    assert!(matches!(error, zero_config::ConfigError::InvalidApi(_)));
}

#[test]
fn runtime_idle_timeout_defaults_to_thirty_seconds() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.runtime.udp_upstream_idle_timeout_seconds, 30);
}

#[test]
fn rejects_zero_udp_upstream_idle_timeout() {
    let error = RuntimeConfig::parse(
        r#"{
            "runtime": {
                "udp_upstream_idle_timeout_seconds": 0
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidRuntime(_)));
}

#[test]
fn rejects_undefined_outbound_reference() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "missing" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::UndefinedRouteTargetTag { .. }
    ));
}

#[test]
fn rejects_removed_protocol_and_action_aliases() {
    let protocol_error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "http-in",
                    "listen": { "address": "127.0.0.1", "port": 8080 },
                    "protocol": { "type": "http" }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("http alias should be rejected");

    assert!(matches!(
        protocol_error,
        zero_config::ConfigError::ParseConfig(_)
    ));

    let action_error = RuntimeConfig::parse(
        r#"{
            "route": {
                "rules": [],
                "final": { "type": "block" }
            }
        }"#,
    )
    .expect_err("block action alias should be rejected");

    assert!(matches!(
        action_error,
        zero_config::ConfigError::ParseConfig(_)
    ));
}

#[test]
fn accepts_mixed_inbound_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "mixed-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "mixed" }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.inbounds[0].protocol,
        InboundProtocolConfig::Mixed { .. }
    ));
}

#[test]
fn parses_socks5_inbound_and_outbound_auth() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "socks-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": {
                        "type": "socks5",
                        "users": [
                            { "username": "alice", "password": "secret" }
                        ]
                    }
                },
                {
                    "tag": "mixed-in",
                    "listen": { "address": "127.0.0.1", "port": 1081 },
                    "protocol": {
                        "type": "mixed",
                        "socks5_users": [
                            { "password": "mixed-secret" }
                        ]
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "chain",
                    "protocol": {
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080,
                        "password": "upstream-secret"
                    }
                },
                {
                    "tag": "no-auth-chain",
                    "protocol": {
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2081
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "chain" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.inbounds[0].protocol.socks5_users()[0].username,
        "alice"
    );
    assert_eq!(
        config.inbounds[1].protocol.socks5_users()[0].username,
        "mixed-secret"
    );
    assert_eq!(
        config.inbounds[1].protocol.socks5_users()[0].password,
        "mixed-secret"
    );
    match &config.outbounds[0].protocol {
        OutboundProtocolConfig::Socks5 {
            username, password, ..
        } => {
            assert_eq!(username.as_deref(), Some("upstream-secret"));
            assert_eq!(password.as_deref(), Some("upstream-secret"));
        }
        _ => panic!("expected socks5 outbound"),
    }
    match &config.outbounds[1].protocol {
        OutboundProtocolConfig::Socks5 {
            username, password, ..
        } => {
            assert_eq!(username, &None);
            assert_eq!(password, &None);
        }
        _ => panic!("expected socks5 outbound"),
    }
}

#[test]
fn parses_mieru_username_defaults_from_password() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "mieru-in",
                    "listen": { "address": "127.0.0.1", "port": 2998 },
                    "protocol": {
                        "type": "mieru",
                        "users": [
                            { "password": "inbound-secret" }
                        ]
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "mieru-node",
                    "protocol": {
                        "type": "mieru",
                        "server": "example.com",
                        "port": 2999,
                        "password": "318149df-2bab-4a35-9de1-870f3e410598"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "mieru-node" }
            }
        }"#,
    )
    .expect("config should parse");

    match &config.outbounds[0].protocol {
        OutboundProtocolConfig::Mieru {
            username, password, ..
        } => {
            assert_eq!(
                username.as_deref(),
                Some("318149df-2bab-4a35-9de1-870f3e410598")
            );
            assert_eq!(password, "318149df-2bab-4a35-9de1-870f3e410598");
        }
        _ => panic!("expected mieru outbound"),
    }
    match &config.inbounds[0].protocol {
        InboundProtocolConfig::Mieru { users } => {
            assert_eq!(users[0].username, "inbound-secret");
            assert_eq!(users[0].password, "inbound-secret");
        }
        _ => panic!("expected mieru inbound"),
    }
}

#[test]
fn rejects_partial_socks5_outbound_auth() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "chain",
                    "protocol": {
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080,
                        "username": "upstream"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "chain" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutbound(_)
    ));
}

#[test]
fn rejects_duplicate_inbound_listen_endpoint() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "socks-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "socks5" }
                },
                {
                    "tag": "http-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "http_connect" }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::DuplicateInboundListen { .. }
    ));
}

#[test]
fn parses_utf8_bom_prefixed_json() {
    let config = RuntimeConfig::parse(
        "\u{feff}{\n  \"inbounds\": [],\n  \"route\": { \"rules\": [], \"final\": { \"type\": \"direct\" } }\n}",
    )
    .expect("config with utf-8 bom should parse");

    assert!(config.inbounds.is_empty());
    assert!(matches!(
        config.route.final_action,
        RouteActionConfig::Direct
    ));
}

#[test]
fn selector_group_requires_defined_member_outbounds() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["missing"]
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutboundGroup(_)
    ));
}

#[test]
fn global_mode_accepts_selector_group_target() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["direct"]
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(config.mode, ModeConfig::Global { .. }));
}

#[test]
fn accepts_fallback_group_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "chain",
                    "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "fallback",
                    "outbounds": ["chain", "direct"]
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::Fallback { .. }
    ));
}

#[test]
fn accepts_urltest_group_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "chain",
                    "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "url_test",
                    "outbounds": ["chain", "direct"],
                    "url": "http://127.0.0.1:8081/",
                    "interval_seconds": 15
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::UrlTest { .. }
    ));
}

#[test]
fn accepts_loadbalance_group_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "chain", "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 } }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "load_balance",
                    "outbounds": ["chain", "direct"],
                    "strategy": "round_robin"
                }
            ],
            "mode": { "type": "global", "outbound": "proxy" },
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("config should parse");

    let OutboundGroupKind::LoadBalance {
        outbounds,
        default,
        strategy,
    } = &config.outbound_groups[0].group
    else {
        panic!("expected loadbalance group");
    };
    assert_eq!(outbounds.len(), 2);
    assert!(default.is_none());
    assert!(matches!(strategy, LoadBalanceStrategy::RoundRobin));
}

#[test]
fn loadbalance_group_defaults_to_round_robin_strategy() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "s5", "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 } }
            ],
            "outbound_groups": [
                {
                    "tag": "lb",
                    "type": "load_balance",
                    "outbounds": ["s5", "direct"]
                }
            ],
            "mode": { "type": "global", "outbound": "lb" },
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("config should parse");

    let OutboundGroupKind::LoadBalance { strategy, .. } = &config.outbound_groups[0].group else {
        panic!("expected loadbalance group");
    };
    assert!(matches!(strategy, LoadBalanceStrategy::RoundRobin));
}

#[test]
fn accepts_loadbalance_random_strategy() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "s5", "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 } }
            ],
            "outbound_groups": [
                {
                    "tag": "lb",
                    "type": "load_balance",
                    "outbounds": ["s5", "direct"],
                    "strategy": "random"
                }
            ],
            "mode": { "type": "global", "outbound": "lb" },
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("config should parse");

    let OutboundGroupKind::LoadBalance { strategy, .. } = &config.outbound_groups[0].group else {
        panic!("expected loadbalance group");
    };
    assert!(matches!(strategy, LoadBalanceStrategy::Random));
}

#[test]
fn loadbalance_group_with_default() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "s5", "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 } }
            ],
            "outbound_groups": [
                {
                    "tag": "lb",
                    "type": "load_balance",
                    "outbounds": ["s5", "direct"],
                    "default": "direct"
                }
            ],
            "mode": { "type": "global", "outbound": "lb" },
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.outbound_groups[0].active_outbound(), Some("direct"));
}

#[test]
fn loadbalance_group_requires_defined_member_outbounds() {
    let result = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } }
            ],
            "outbound_groups": [
                {
                    "tag": "lb",
                    "type": "load_balance",
                    "outbounds": ["missing", "direct"]
                }
            ],
            "mode": { "type": "global", "outbound": "lb" },
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    );
    assert!(result.is_err());
}

#[test]
fn accepts_group_member_referencing_another_group() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "block",
                    "protocol": { "type": "block" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "fallback-proxy",
                    "type": "fallback",
                    "outbounds": ["block", "direct"]
                },
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["fallback-proxy", "direct"],
                    "selected": "fallback-proxy"
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "reject" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.outbound_groups.len(), 2);
    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::Fallback { .. }
    ));
    assert!(matches!(
        config.outbound_groups[1].group,
        OutboundGroupKind::Selector { .. }
    ));
}

#[test]
fn rejects_group_reference_cycle() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "group-a",
                    "type": "selector",
                    "outbounds": ["group-b"],
                    "selected": "group-b"
                },
                {
                    "tag": "group-b",
                    "type": "fallback",
                    "outbounds": ["group-a"]
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "group-a"
            },
            "route": {
                "rules": [],
                "final": { "type": "reject" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutboundGroup(_)
    ));
}

#[test]
fn loads_rule_set_from_relative_file_path() {
    let project_dir = temp_test_dir("config-rule-set-relative");
    let rules_dir = project_dir.join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(rules_dir.join("ads.txt"), "blocked.example\n.ads.local\n").expect("write rules");

    let config_path = project_dir.join("config.json");
    fs::write(
        &config_path,
        r#"{
            "outbounds": [
                { "tag": "block", "protocol": { "type": "block" } }
            ],
            "route": {
                "rule_sets": [
                    {
                        "tag": "ads",
                        "type": "file",
                        "path": "rules/ads.txt",
                        "format": "domain_list"
                    }
                ],
                "rules": [
                    {
                        "condition": { "type": "rule_set", "tag": "ads" },
                        "action": { "type": "route", "outbound": "block" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("write config");

    let config = RuntimeConfig::load_from_path(&config_path).expect("load config");

    assert_eq!(config.source_dir(), Some(project_dir.as_path()));
    assert!(matches!(
        config.route.rules[0].condition,
        RuleConditionConfig::RuleSet { .. }
    ));

    cleanup_temp_dir(&project_dir);
}

#[test]
fn rejects_undefined_rule_set_reference() {
    let error = RuntimeConfig::parse(
        r#"{
            "route": {
                "rules": [
                    {
                        "condition": { "type": "rule_set", "tag": "ads" },
                        "action": { "type": "direct" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::UndefinedRuleSetTag { .. }
    ));
}

#[test]
fn parses_inbound_route_condition() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "hk-in",
                    "listen": { "address": "127.0.0.1", "port": 7891 },
                    "protocol": { "type": "mixed" }
                }
            ],
            "outbounds": [
                { "tag": "hk-out", "protocol": { "type": "direct" } }
            ],
            "route": {
                "rules": [
                    {
                        "condition": { "type": "inbound", "values": ["hk-in"] },
                        "action": { "type": "route", "outbound": "hk-out" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.route.rules[0].condition,
        RuleConditionConfig::Inbound { .. }
    ));
}

#[test]
fn rejects_undefined_inbound_route_condition_reference() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "hk-in",
                    "listen": { "address": "127.0.0.1", "port": 7891 },
                    "protocol": { "type": "mixed" }
                }
            ],
            "route": {
                "rules": [
                    {
                        "condition": { "type": "inbound", "values": ["missing-in"] },
                        "action": { "type": "direct" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidRuleCondition(_)
    ));
    assert!(error.to_string().contains("missing-in"));
}

#[test]
fn rejects_invalid_cidr_rule_set_entry() {
    let project_dir = temp_test_dir("config-rule-set-invalid-cidr");
    let rules_dir = project_dir.join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(rules_dir.join("lan.txt"), "10.0.0.0/8\nnot-a-cidr\n").expect("write rules");

    let config_path = project_dir.join("config.json");
    fs::write(
        &config_path,
        r#"{
            "route": {
                "rule_sets": [
                    {
                        "tag": "lan",
                        "type": "file",
                        "path": "rules/lan.txt",
                        "format": "cidr_list"
                    }
                ],
                "rules": [
                    {
                        "condition": { "type": "rule_set", "tag": "lan" },
                        "action": { "type": "direct" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("write config");

    let error = RuntimeConfig::load_from_path(&config_path).expect_err("config should fail");
    assert!(matches!(error, zero_config::ConfigError::InvalidRuleSet(_)));

    cleanup_temp_dir(&project_dir);
}

fn temp_test_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{nonce}"));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn cleanup_temp_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}
