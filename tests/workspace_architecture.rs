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

#[test]
fn protocol_crates_depend_only_on_runtime_neutral_workspace_crates() {
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
            "zero-engine",
            "zero-platform-tokio",
            "zero-proxy",
            "zero-router",
            "zero-transport",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must not depend on runtime crate `{forbidden}`",
                manifest.display()
            );
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
        "vmess::parse_uuid",
        "vmess::VmessCipher::from_name",
        "shadowsocks::validation::validate_cipher",
        "shadowsocks::validation::validate_password",
        "socks5::validate_credential_part",
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
fn engine_runtime_domains_do_not_regrow_in_the_facade() {
    let runtime = read(&workspace_root().join("crates/engine/src/runtime.rs"));
    for implementation in [
        "pub fn reload_config",
        "pub fn subscribe_reload",
        "pub fn set_selector_target",
        "pub fn update_urltest_state",
        "pub fn events_snapshot",
        "pub fn update_sink_status",
    ] {
        assert!(
            !runtime.contains(implementation),
            "engine runtime facade must not re-own `{implementation}`"
        );
    }
    for domain in ["configuration", "policy", "observability"] {
        assert!(
            runtime.contains(&format!("mod {domain};")),
            "engine runtime must delegate the `{domain}` domain"
        );
    }
}
