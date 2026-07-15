//! Workspace-level dependency direction tests.
//!
//! These assertions lock ownership boundaries rather than file layouts.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_rust_sources(root, &mut sources);
    sources
}

fn collect_rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(root).unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn transport_does_not_project_engine_outbound_leaves() {
    let transport = workspace_root().join("crates/transport/src");
    for path in rust_sources(&transport) {
        let source = read(&path);
        for forbidden in [
            "ResolvedLeafOutbound",
            "ProtocolTransportLeafResolver",
            "prepare_transport_bridge_leaf",
            "resolve_transport_leaf",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must remain independent of engine leaf projection `{forbidden}`",
                path.display()
            );
        }
    }
}

fn dependency_line_is_optional(manifest: &str, dependency: &str) -> bool {
    manifest.lines().any(|line| {
        line.contains(dependency)
            && line.contains('=')
            && line.contains('{')
            && line.contains("optional = true")
    })
}

fn manifest_line<'a>(manifest: &'a str, prefix: &str) -> &'a str {
    manifest
        .lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .unwrap_or_else(|| panic!("missing manifest line `{prefix}`"))
}

#[test]
fn protocol_crates_keep_runtime_support_deps_optional_and_out_of_config_space() {
    let protocols = workspace_root().join("protocols");
    for entry in fs::read_dir(protocols).expect("read protocols") {
        let manifest = entry.expect("protocol crate").path().join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let source = read(&manifest);
        for forbidden in [
            "zero-api",
            "zero-config",
            "zero-dns",
            "zero-proxy",
            "zero-router",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must not depend on non-protocol boundary crate `{forbidden}`",
                manifest.display()
            );
        }
        for runtime_support in ["zero-engine", "zero-platform-tokio", "zero-transport"] {
            if source.contains(runtime_support) {
                assert!(
                    source.contains("runtime = ["),
                    "{} must isolate `{runtime_support}` behind a runtime feature",
                    manifest.display()
                );
                assert!(
                    dependency_line_is_optional(&source, runtime_support),
                    "{} must keep `{runtime_support}` optional",
                    manifest.display()
                );
            }
        }
    }
}

#[test]
fn foundational_crates_do_not_depend_on_runtime_or_integration_crates() {
    let boundaries = [
        (
            "crates/traits/Cargo.toml",
            ["zero-core", "zero-engine", "zero-proxy", "zero-transport"],
        ),
        (
            "crates/core/Cargo.toml",
            ["zero-config", "zero-engine", "zero-proxy", "zero-transport"],
        ),
        (
            "crates/router/Cargo.toml",
            ["zero-config", "zero-engine", "zero-proxy", "zero-transport"],
        ),
        (
            "crates/stack/Cargo.toml",
            ["zero-config", "zero-engine", "zero-proxy", "zero-transport"],
        ),
    ];
    for (relative, forbidden) in boundaries {
        let manifest = workspace_root().join(relative);
        let source = read(&manifest);
        for dependency in forbidden {
            assert!(
                !source.contains(dependency),
                "{} must not depend on `{dependency}`",
                manifest.display()
            );
        }
    }
}

#[test]
fn protocol_projection_is_confined_to_proxy_adapters() {
    let proxy = workspace_root().join("crates/proxy/src");
    for path in rust_sources(&proxy) {
        let source = read(&path);
        if !source.contains("ResolvedLeafOutbound::") {
            continue;
        }
        let relative = path.strip_prefix(&proxy).expect("proxy-relative path");
        let allowed = relative.starts_with("adapters")
            || relative.starts_with("protocol_registry")
            || relative.starts_with("inventory/tests");
        assert!(
            allowed,
            "{} must not project concrete engine leaves outside adapter/registry integration",
            path.display()
        );
    }
}

#[test]
fn config_delegates_protocol_private_value_validation() {
    let validator = read(&workspace_root().join("crates/config/src/validate/protocol.rs"));
    for delegated in [
        "vless::parse_uuid",
        "vless::validation::validate_reality_key",
        "vless::validation::validate_reality_short_id",
        "vless::validation::validate_reality_cipher_suites",
        "vless::validation::validate_xhttp_mode",
    ] {
        assert!(
            validator.contains(delegated),
            "config must delegate protocol-private validation through `{delegated}`"
        );
    }
    assert!(
        validator.contains("vmess::parse_uuid"),
        "config must delegate VMess UUID validation through the protocol-owned validator"
    );
    assert!(
        validator.contains("vmess::VmessCipher::from_name"),
        "config must delegate VMess cipher validation through the protocol-owned validator"
    );
    assert!(
        validator.contains("socks5::validate_credential_part"),
        "config must delegate SOCKS5 credential validation through the protocol-owned validator"
    );
    assert!(
        validator.contains("shadowsocks::validation::validate_cipher"),
        "config must delegate Shadowsocks cipher validation through the protocol-owned validator"
    );
    assert!(
        validator.contains("shadowsocks::validation::validate_password"),
        "config must delegate Shadowsocks password validation through the protocol-owned validator"
    );
    for duplicate in [
        "fn validate_uuid_literal",
        "fn shadowsocks_2022_key_len",
        "fn decode_shadowsocks_2022_key_len",
    ] {
        assert!(
            !validator.contains(duplicate),
            "config must not recreate protocol validator `{duplicate}`"
        );
    }
}

#[test]
fn config_protocol_dependencies_use_validation_features() {
    let manifest = read(&workspace_root().join("crates/config/Cargo.toml"));
    for dependency in ["socks5", "shadowsocks", "vmess", "vless"] {
        let line = manifest_line(&manifest, dependency);
        assert!(
            line.contains("default-features = false"),
            "config dependency `{dependency}` must disable protocol default features"
        );
        assert!(
            line.contains("validation"),
            "config dependency `{dependency}` must depend on the protocol validation surface"
        );
    }

    let shadowsocks = manifest_line(&manifest, "shadowsocks");
    assert!(
        shadowsocks.contains("blake3"),
        "config Shadowsocks dependency must keep the `blake3` validation surface enabled"
    );
}

#[test]
fn engine_runtime_domains_do_not_regrow_in_the_facade() {
    let runtime = read(&workspace_root().join("crates/engine/src/runtime.rs"));
    for implementation in [
        "pub fn reload_config",
        "pub fn subscribe_reload",
        "pub fn set_selector_target",
        "pub fn update_urltest_state",
        "pub fn events_snapshot",
        "pub fn update_sink_status",
        "pub fn dns_lookup",
        "pub fn probe_target",
        "pub fn prepare_session",
        "pub fn finish_session",
    ] {
        assert!(
            !runtime.contains(implementation),
            "engine runtime facade must not re-own `{implementation}`"
        );
    }
    for domain in [
        "configuration",
        "diagnostics",
        "observability",
        "policy",
        "session",
    ] {
        assert!(
            runtime.contains(&format!("mod {domain};")),
            "engine runtime must delegate the `{domain}` domain"
        );
    }
}

#[test]
fn generic_transport_carriers_do_not_depend_on_protocol_crates() {
    let transport = workspace_root().join("crates/transport/src");
    for path in rust_sources(&transport) {
        let source = read(&path);
        for protocol in [
            "hysteria2",
            "mieru",
            "shadowsocks",
            "socks5",
            "trojan",
            "vless",
            "vmess",
        ] {
            assert!(
                !source.contains(&format!("use {protocol}::")),
                "generic carrier {} must not import protocol crate `{protocol}`",
                path.display()
            );
        }
    }
}

#[test]
fn transport_does_not_hardcode_protocol_service_or_alpn_defaults() {
    let transport = workspace_root().join("crates/transport/src");
    for path in rust_sources(&transport) {
        let source = read(&path);
        for forbidden in [
            "/v2ray.core.proxy.vless.encap.GrpcService/Tun",
            "b\"hysteria2\".to_vec()",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must not own protocol default `{forbidden}`",
                path.display()
            );
        }
    }
}

#[test]
fn generic_config_models_do_not_hardcode_protocol_transport_defaults() {
    let transport_model = read(&workspace_root().join("crates/config/src/model/transport.rs"));
    for forbidden in [
        "/v2ray.core.proxy.vless.encap.GrpcService/Tun",
        "default_grpc_service_names",
    ] {
        assert!(
            !transport_model.contains(forbidden),
            "generic config transport model must not own protocol default `{forbidden}`"
        );
    }
}

#[test]
fn transport_does_not_depend_on_config_protocol_adts() {
    let transport = workspace_root().join("crates/transport/src");
    for path in rust_sources(&transport) {
        let source = read(&path);
        if !source.contains("zero_config::") && !source.contains("use zero_config") {
            continue;
        }
        panic!(
            "{} must not import zero-config from transport",
            path.display()
        );
    }

    let manifest = read(&workspace_root().join("crates/transport/Cargo.toml"));
    assert!(
        !manifest.contains("zero-config"),
        "crates/transport/Cargo.toml must not depend on zero-config"
    );
    for protocol in [
        "hysteria2",
        "mieru",
        "shadowsocks",
        "socks5",
        "trojan",
        "vless",
        "vmess",
    ] {
        assert!(
            !manifest.contains(&format!("name = \"{protocol}\""))
                && !manifest.contains(&format!("path = \"../../protocols/{protocol}\"")),
            "crates/transport/Cargo.toml must not depend on protocol crate `{protocol}`"
        );
    }
}

#[test]
fn stack_keeps_packet_parsing_separate_from_connection_lifecycle() {
    let stack = workspace_root().join("crates/stack/src");
    let packet = read(&stack.join("packet.rs"));
    for forbidden in ["tokio::", "TcpState", "UserTcpStack", "mpsc::"] {
        assert!(
            !packet.contains(forbidden),
            "packet parser must not own `{forbidden}` lifecycle state"
        );
    }
    let tcp = read(&stack.join("tcp.rs"));
    assert!(
        tcp.contains("crate::packet"),
        "TCP lifecycle must consume the packet parser boundary"
    );
    assert!(
        !tcp.contains("fn parse_ip("),
        "TCP lifecycle must not recreate IP parsing"
    );
}

#[test]
fn root_process_entrypoint_delegates_command_execution() {
    let main = read(&workspace_root().join("src/main.rs"));
    assert!(main.contains("application::execute"));
    for request in [
        "method: \"mode.set\"",
        "method: \"tun.start\"",
        "method: \"tun.stop\"",
        "IpcRequest",
        "ProxyHandle",
        "EngineHandle",
        "spawn_ipc_server",
        "spawn_http_server",
        "spawn_push_connector",
        "spawn_event_dispatcher",
        "wait_for_shutdown_signal",
        "status_server_spec",
    ] {
        assert!(
            !main.contains(request),
            "process entrypoint must not own application runtime responsibility `{request}`"
        );
    }
    for implementation in [
        "async fn main",
        "async fn try_main",
        "fn init_tracing_from_config",
    ] {
        assert!(
            main.contains(implementation),
            "process entrypoint must retain `{implementation}`"
        );
    }
}
