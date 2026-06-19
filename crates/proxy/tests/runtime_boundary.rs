use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("proxy crate parent")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = manifest_dir().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn rust_sources_under(relative: &str) -> Vec<PathBuf> {
    let root = manifest_dir().join(relative);
    let mut pending = vec![root];
    let mut files = Vec::new();

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).unwrap_or_else(|error| {
            panic!("read dir {}: {error}", path.display());
        }) {
            let entry = entry.expect("read dir entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    files
}

fn relative(path: &Path) -> String {
    path.strip_prefix(manifest_dir())
        .expect("path under manifest dir")
        .to_string_lossy()
        .replace('\\', "/")
}

fn assert_src_pattern_confined(
    pattern: &str,
    allowed_exact: &[&str],
    allowed_prefixes: &[&str],
    reason: &str,
) {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if !content.contains(pattern) {
            continue;
        }
        let allowed = allowed_exact.iter().any(|item| *item == source)
            || allowed_prefixes.iter().any(|prefix| source.starts_with(prefix));
        assert!(
            allowed,
            "{source} should not contain `{pattern}`; {reason}"
        );
    }
}

#[test]
fn ordinary_udp_inbounds_submit_packets_through_udp_pipe() {
    for source in [
        "src/runtime/udp_associate.rs",
        "src/inbound/vless/udp_session.rs",
        "src/inbound/trojan.rs",
        "src/inbound/shadowsocks.rs",
        "src/inbound/hysteria2.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("UdpPipe::new") && content.contains("UdpPipeInput"),
            "{source} should submit inbound UDP packets through UdpPipe"
        );
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call the UDP dispatch state machine directly"
        );
    }
}

#[test]
fn udp_dispatch_entry_is_only_called_by_udp_pipe() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if source == "src/runtime/pipe.rs" {
            continue;
        }
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call UdpDispatch::dispatch directly"
        );
    }
}

#[test]
fn ordinary_tcp_inbounds_use_tcp_pipe_for_route_execution() {
    let lifecycle = read("src/runtime/inbound_protocol.rs");
    assert!(
        lifecycle.contains("TcpPipe::new") && lifecycle.contains("TcpPipeInput"),
        "serve_inbound should route ordinary TCP sessions through TcpPipe"
    );

    let vless = read("src/inbound/vless/mux.rs");
    assert!(
        vless.contains("TcpPipe::new") && vless.contains("TcpPipeInput"),
        "VLESS MUX sub-streams should route through TcpPipe"
    );
    assert!(
        !vless.contains("dispatch_tcp_outbound"),
        "VLESS inbound should not bypass TcpPipe through TCP outbound helpers"
    );
}

#[test]
fn tcp_outbound_resolution_helper_stays_inside_tcp_dispatch() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if source == "src/runtime/tcp_dispatch.rs" {
            continue;
        }
        assert!(
            !content.contains("dispatch_tcp_outbound"),
            "{source} should not call the TCP outbound helper directly"
        );
    }
}

#[test]
fn protocol_config_variant_matching_is_confined_to_adapters_and_protocol_entrypoints() {
    assert_src_pattern_confined(
        "InboundProtocolConfig::",
        &[
            "src/protocol_adapter.rs",
            "src/runtime.rs",
            "src/inbound/direct.rs",
            "src/inbound/hysteria2.rs",
            "src/inbound/mieru.rs",
            "src/inbound/shadowsocks.rs",
            "src/inbound/trojan.rs",
            "src/inbound/vmess/listener.rs",
        ],
        &["src/adapters/"],
        "protocol config variant matching should stay inside adapters, the mixed special-case, or protocol-owned inbound entrypoints",
    );
}

#[test]
fn resolved_outbound_variant_matching_is_confined_to_adapters_and_neutral_orchestration() {
    assert_src_pattern_confined(
        "ResolvedLeafOutbound::",
        &[
            "src/protocol_adapter.rs",
            "src/runtime/orchestration.rs",
        ],
        &["src/adapters/"],
        "resolved outbound variant matching should stay inside adapters or neutral runtime classification helpers",
    );
}

#[test]
fn protocol_crates_do_not_depend_on_proxy_runtime_layers() {
    let protocols_root = repo_root().join("protocols");
    let forbidden = [
        "zero-proxy",
        "zero-engine",
        "zero-config",
        "zero-dns",
        "zero-platform-tokio",
        "zero-transport",
        "zero-tun",
        "zero-stack",
    ];

    for entry in fs::read_dir(&protocols_root).expect("read protocols dir") {
        let entry = entry.expect("protocol entry");
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let content = fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("read {}: {error}", manifest.display()));
        for crate_name in forbidden {
            assert!(
                !content.contains(crate_name),
                "{} should not depend on forbidden runtime crate `{crate_name}`",
                manifest.display()
            );
        }
    }
}
