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
            || allowed_prefixes
                .iter()
                .any(|prefix| source.starts_with(prefix));
        assert!(allowed, "{source} should not contain `{pattern}`; {reason}");
    }
}

#[test]
fn ordinary_udp_inbounds_submit_packets_through_udp_pipe() {
    for source in [
        "src/protocol_runtime/socks5_udp_associate.rs",
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
            "src/protocol_adapter/registry.rs",
            "src/inbound/direct.rs",
            "src/inbound/hysteria2.rs",
            "src/inbound/mieru.rs",
            "src/inbound/shadowsocks.rs",
            "src/inbound/trojan.rs",
            "src/inbound/vmess/listener.rs",
        ],
        &["src/adapters/"],
        "protocol config variant matching should stay inside adapters or protocol-owned inbound entrypoints",
    );
}

#[test]
fn runtime_does_not_match_protocol_config_variants() {
    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("InboundProtocolConfig::"),
            "{source} should not match inbound protocol config variants"
        );
    }

    let runtime = read("src/runtime.rs");
    assert!(
        !runtime.contains("InboundProtocolConfig::"),
        "src/runtime.rs should dispatch inbound lifecycle through ProtocolAdapter"
    );
}

#[test]
fn proxy_does_not_own_protocol_listener_entrypoints() {
    for path in rust_sources_under("src/inbound") {
        let source = relative(&path);
        let content = fs::read_to_string(&path)
            .expect("read rust source")
            .replace("\r\n", "\n");

        assert!(
            !content.contains("_listener_with_bound(\n")
                || !content.contains("impl Proxy {\n    pub(crate) async fn run_"),
            "{source} should expose run_*_listener_with_bound as module functions, not Proxy methods"
        );
    }
}

#[test]
fn resolved_outbound_variant_matching_is_confined_to_adapters_and_neutral_orchestration() {
    assert_src_pattern_confined(
        "ResolvedLeafOutbound::",
        &[
            "src/protocol_adapter.rs",
            "src/protocol_adapter/registry.rs",
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

#[test]
fn generic_udp_dispatch_does_not_encode_protocol_packets_directly() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for forbidden in [
        "proxy.protocols.vless_outbound",
        "proxy.protocols.vmess_outbound",
        "VlessUdpPacketTarget",
        "VmessUdpPacketTarget",
        "VlessOutbound as UdpPacketFraming",
        "VmessOutbound as UdpPacketFraming",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/mod.rs should stay protocol-neutral; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_inventory_keeps_protocol_instances_private() {
    let content = read("src/inventory.rs");

    for forbidden in [
        "pub socks5_inbound:",
        "pub socks5_outbound:",
        "pub http_connect_inbound:",
        "pub vless_inbound:",
        "pub vless_outbound:",
        "pub hysteria2_inbound:",
        "pub hysteria2_outbound:",
        "pub shadowsocks_inbound:",
        "pub shadowsocks_outbound:",
        "pub trojan_inbound:",
        "pub trojan_outbound:",
        "pub vmess_inbound:",
        "pub vmess_outbound:",
        "pub(crate) direct_outbound:",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/inventory.rs should keep protocol instances private; found `{forbidden}`"
        );
    }

    for required in [
        "fn direct_connector(&self)",
        "fn socks5_inbound_protocol(&self)",
        "fn http_connect_inbound_protocol(&self)",
        "fn vless_inbound_protocol(&self)",
        "fn vless_outbound_protocol(&self)",
        "fn shadowsocks_outbound_protocol(&self)",
        "fn trojan_outbound_protocol(&self)",
        "fn vmess_outbound_protocol(&self)",
    ] {
        assert!(
            content.contains(required),
            "src/inventory.rs should expose controlled protocol accessors; missing `{required}`"
        );
    }
}

#[test]
fn socks5_udp_association_runtime_state_stays_out_of_outbound_module() {
    let outbound = read("src/outbound/socks5.rs");
    let runtime = read("src/protocol_runtime/socks5_udp.rs");

    for forbidden in [
        "ActiveUpstreamSocks5UdpAssociation",
        "Socks5UdpAssociation",
        "UpstreamAssociationCloseReason",
        "send_socks5_udp_packet",
        "ensure_socks5_udp_association",
        "Socks5UdpRelay",
    ] {
        assert!(
            !outbound.contains(forbidden),
            "src/outbound/socks5.rs should stay focused on TCP handshake; found `{forbidden}`"
        );
    }

    for required in [
        "pub struct ActiveUpstreamSocks5UdpAssociation",
        "pub struct Socks5UdpAssociation",
        "pub enum UpstreamAssociationCloseReason",
        "pub async fn send_socks5_udp_packet",
    ] {
        assert!(
            runtime.contains(required),
            "src/protocol_runtime/socks5_udp.rs should own SOCKS5 UDP runtime state; missing `{required}`"
        );
    }
}

#[test]
fn generic_runtime_root_does_not_import_protocol_crates_directly() {
    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);

        let content = fs::read_to_string(&path).expect("read rust source");
        for protocol_crate in [
            "use socks5::",
            "use vless::",
            "use vmess::",
            "use shadowsocks::",
            "use trojan::",
            "use hysteria2::",
            "use mieru::",
        ] {
            assert!(
                !content.contains(protocol_crate),
                "{source} should not import protocol crate `{protocol_crate}` directly; move protocol state to src/protocol_runtime"
            );
        }
    }
}

#[test]
fn generic_runtime_udp_state_uses_protocol_neutral_module_name() {
    let runtime_root = manifest_dir().join("src/runtime");

    for forbidden in ["udp_associate.rs", "udp_associate"] {
        let path = runtime_root.join(forbidden);
        assert!(
            !path.exists(),
            "src/runtime/{forbidden} should stay protocol-neutral as src/runtime/udp_flow"
        );
    }
}

#[test]
fn generic_udp_dispatch_does_not_contain_protocol_manager_modules() {
    let forbidden = [
        "h2_manager.rs",
        "mieru_manager.rs",
        "packet_path_chain.rs",
        "packet_path_traits.rs",
        "ss_manager.rs",
        "trojan_manager.rs",
    ];

    for file_name in forbidden {
        let path = manifest_dir()
            .join("src/runtime/udp_dispatch")
            .join(file_name);
        assert!(
            !path.exists(),
            "src/runtime/udp_dispatch/{file_name} should live under src/protocol_runtime/udp"
        );
    }
}

#[test]
fn udp_dispatch_keeps_protocol_managers_in_protocol_runtime_state() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    assert!(
        content.contains("protocol_state: ProtocolUdpState"),
        "UdpDispatch should keep protocol-specific managers behind ProtocolUdpState"
    );

    for forbidden in [
        "vless_manager:",
        "vmess_manager:",
        "ss_manager:",
        "packet_path_manager:",
        "trojan_manager:",
        "mieru_manager:",
        "h2_manager:",
    ] {
        assert!(
            !content.contains(forbidden),
            "UdpDispatch should not declare protocol manager field `{forbidden}` directly"
        );
    }
}

#[test]
fn udp_forward_stays_protocol_neutral_and_does_not_construct_peer_types() {
    let content = read("src/runtime/udp_dispatch/forward.rs");

    for forbidden in [
        "SsUdpPeer",
        "H2UdpPeer",
        "TrojanUdpPeer",
        "MieruUdpPeer",
        "UdpPeerEndpoint",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/forward.rs should not construct protocol peer types; found `{forbidden}`"
        );
    }
}

#[test]
fn adapters_do_not_construct_udp_dispatch_peer_helpers() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "SsUdpPeer",
            "H2UdpPeer",
            "TrojanUdpPeer",
            "MieruUdpPeer",
            "UdpFlowContext",
            "UdpPacketRef",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not construct udp-dispatch peer helper `{forbidden}`"
            );
        }
    }
}

#[test]
fn packet_path_chain_does_not_own_socks5_runtime_state() {
    let content = read("src/protocol_runtime/udp/packet_path_chain.rs");

    for forbidden in [
        "ActiveUpstreamSocks5UdpAssociation",
        "Socks5PacketPath",
        "socks5::parse_udp_packet",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/protocol_runtime/udp/packet_path_chain.rs should stay generic; found `{forbidden}`"
        );
    }
}

#[test]
fn adapters_do_not_reach_into_udp_dispatch_manager_fields() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            ".ss_manager",
            ".h2_manager",
            ".trojan_manager",
            ".mieru_manager",
            ".vless_manager",
            ".vmess_manager",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not reach into udp-dispatch manager field `{forbidden}`"
            );
        }
    }
}
