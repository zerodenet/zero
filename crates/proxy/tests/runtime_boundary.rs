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
        "src/protocol_runtime/socks5_udp_associate/dispatch.rs",
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
            "src/protocol_adapter/registry/tests.rs",
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
fn runtime_control_handle_lives_outside_runtime_root() {
    let runtime = read("src/runtime.rs");
    let handle = read("src/runtime/handle.rs");

    for forbidden in [
        "struct ProxyHandle",
        "impl zero_api::QueryService for ProxyHandle",
        "impl zero_api::CommandService for ProxyHandle",
        "impl zero_api::EventSource for ProxyHandle",
        "fn parse_ip_address",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "src/runtime.rs should keep control-plane handle details in src/runtime/handle.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct ProxyHandle",
        "impl zero_api::QueryService for ProxyHandle",
        "impl zero_api::CommandService for ProxyHandle",
        "impl zero_api::EventSource for ProxyHandle",
        "fn parse_ip_address",
    ] {
        assert!(
            handle.contains(required),
            "runtime control-plane handle details should live in src/runtime/handle.rs; missing `{required}`"
        );
    }
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
            "src/protocol_adapter/registry/tests.rs",
            "src/runtime/orchestration.rs",
        ],
        &["src/adapters/"],
        "resolved outbound variant matching should stay inside adapters or neutral runtime classification helpers",
    );
}

#[test]
fn adapter_roots_keep_udp_runtime_details_in_udp_modules() {
    let cases: &[(&str, &[&str])] = &[
        (
            "direct",
            &[
                "resolve_target_addr",
                "send_direct_packet",
                "UdpFlowOutbound::Direct",
                "resolve_udp_target",
                "udp_direct_send",
            ],
        ),
        (
            "hysteria2",
            &[
                "PacketPathCarrierDescriptor {",
                "UdpPacketPathCarrier::Hysteria2",
                "build_hysteria2_packet_path",
                "Hysteria2UdpFlowRequest",
                "UdpFlowOutbound::Hysteria2",
            ],
        ),
        (
            "mieru",
            &[
                "MieruUdpRelayFlow",
                "MieruUdpFlowRequest",
                "start_mieru_udp_relay_flow",
                "UdpFlowOutbound::Mieru",
            ],
        ),
        (
            "shadowsocks",
            &[
                "PacketPathCarrierDescriptor {",
                "UdpPacketPathCarrier::Shadowsocks",
                "build_shadowsocks_packet_path",
                "UdpDatagramSource {",
                "ShadowsocksUdpFlow",
                "start_shadowsocks_udp_flow",
                "UdpFlowOutbound::Shadowsocks",
            ],
        ),
        (
            "socks5",
            &[
                "PacketPathCarrierDescriptor {",
                "UdpPacketPathCarrier::Socks5",
                "build_socks5_packet_path",
                "Socks5UdpSend",
                "UdpFlowOutbound::Socks5",
            ],
        ),
        (
            "trojan",
            &[
                "TrojanUdpFlowRequest",
                "TrojanUdpRelayFlowRequest",
                "UdpFlowOutbound::Trojan",
            ],
        ),
        (
            "vless",
            &[
                "VlessUdpFlow",
                "VlessUdpRelayFinalHop",
                "VlessUdpRelayTwoStream",
                "open_udp_stream",
                "encode_udp_packet",
                "dispatch_tcp_relay_prefix",
                "start_vless_udp_",
            ],
        ),
        (
            "vmess",
            &[
                "VmessUdpFlow",
                "VmessUdpRelayFlow",
                "start_vmess_udp_flow",
                "start_vmess_udp_relay_flow",
            ],
        ),
    ];

    for (adapter_name, forbidden_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let udp = manifest_dir().join(format!("src/adapters/{adapter_name}/udp.rs"));

        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep UDP runtime details in src/adapters/{adapter_name}/udp.rs; found `{forbidden}`"
            );
        }
        assert!(
            udp.exists(),
            "{adapter_name} adapter UDP runtime details should live in src/adapters/{adapter_name}/udp.rs"
        );
    }
}

#[test]
fn adapter_root_does_not_import_protocol_udp_request_types() {
    let adapters = read("src/adapters/mod.rs");

    for forbidden in [
        "ShadowsocksUdpFlow",
        "VlessUdpFlow",
        "VlessUdpRelayFinalHop",
        "VlessUdpRelayTwoStream",
        "VmessUdpFlow",
        "VmessUdpRelayFlow",
        "MieruUdpRelayFlow",
    ] {
        assert!(
            !adapters.contains(forbidden),
            "src/adapters/mod.rs should not globally import protocol UDP request type `{forbidden}`"
        );
    }
}

#[test]
fn adapter_roots_keep_tcp_runtime_details_in_tcp_modules() {
    let cases: &[(&str, &[&str])] = &[
        (
            "direct",
            &[
                ".direct_connector()\n            .connect(",
                "connect_direct",
                "EstablishedTcpOutbound::Direct",
            ],
        ),
        (
            "hysteria2",
            &[
                "crate::outbound::hysteria2::connect_tcp",
                "connect_upstream_hysteria2",
                "EstablishedTcpOutbound::Hysteria2",
            ],
        ),
        (
            "mieru",
            &[
                "crate::outbound::mieru::connect_tcp",
                "crate::outbound::mieru::apply_tcp_hop",
                "connect_upstream_mieru",
                "EstablishedTcpOutbound::Mieru",
            ],
        ),
        (
            "shadowsocks",
            &[
                "crate::outbound::shadowsocks::connect_tcp",
                "crate::outbound::shadowsocks::apply_tcp_hop",
                "connect_upstream_shadowsocks",
                "EstablishedTcpOutbound::Shadowsocks",
            ],
        ),
        (
            "socks5",
            &[
                "crate::outbound::socks5::connect_tcp",
                "crate::outbound::socks5::apply_tcp_hop",
                "connect_upstream_socks5",
                "EstablishedTcpOutbound::Socks5",
            ],
        ),
        (
            "trojan",
            &[
                "crate::outbound::trojan::connect_tcp",
                "crate::outbound::trojan::apply_tcp_hop",
                "connect_upstream_trojan",
                "EstablishedTcpOutbound::Trojan",
            ],
        ),
        (
            "vless",
            &[
                "crate::outbound::vless::connect_tcp",
                "crate::outbound::vless::apply_tcp_hop",
                "connect_upstream_vless",
                "EstablishedTcpOutbound::Vless",
            ],
        ),
        (
            "vmess",
            &[
                "crate::outbound::vmess::connect_tcp",
                "crate::outbound::vmess::apply_tcp_hop",
                "connect_upstream_vmess",
                "EstablishedTcpOutbound::Vmess",
            ],
        ),
    ];

    for (adapter_name, forbidden_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let tcp = manifest_dir().join(format!("src/adapters/{adapter_name}/tcp.rs"));

        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep TCP runtime details in src/adapters/{adapter_name}/tcp.rs; found `{forbidden}`"
            );
        }
        assert!(
            tcp.exists(),
            "{adapter_name} adapter TCP runtime details should live in src/adapters/{adapter_name}/tcp.rs"
        );
    }
}

#[test]
fn adapter_roots_keep_inbound_runtime_details_in_inbound_modules() {
    let cases: &[(&str, &[&str])] = &[
        (
            "direct",
            &["run_direct_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "http_connect",
            &["run_http_connect_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "hysteria2",
            &[
                "QuicInbound::bind",
                "run_hysteria2_listener_with_bound",
                "cert_path",
                "key_path",
            ],
        ),
        (
            "mieru",
            &["run_mieru_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "mixed",
            &["run_mixed_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "shadowsocks",
            &["run_shadowsocks_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "socks5",
            &["run_socks5_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "trojan",
            &["run_trojan_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "vless",
            &[
                "QuicInbound::bind",
                "zero_platform_tokio::TokioListener::bind",
                "run_vless_listener_with_bound",
                "quic.cert_path",
            ],
        ),
        (
            "vmess",
            &["run_vmess_listener_with_bound", "bound.into_tcp()"],
        ),
    ];

    for (adapter_name, forbidden_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let inbound = manifest_dir().join(format!("src/adapters/{adapter_name}/inbound.rs"));

        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep inbound runtime details in src/adapters/{adapter_name}/inbound.rs; found `{forbidden}`"
            );
        }
        assert!(
            inbound.exists(),
            "{adapter_name} adapter inbound runtime details should live in src/adapters/{adapter_name}/inbound.rs"
        );
    }
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
    let root = read("src/protocol_runtime/socks5_udp.rs");
    let active = read("src/protocol_runtime/socks5_udp/active.rs");
    let model = read("src/protocol_runtime/socks5_udp/model.rs");
    let send_source = read("src/protocol_runtime/socks5_udp/send.rs");
    let send = manifest_dir().join("src/protocol_runtime/socks5_udp/send.rs");
    let runtime = manifest_dir().join("src/protocol_runtime/socks5_udp/runtime.rs");
    let packet_path = manifest_dir().join("src/protocol_runtime/socks5_udp/packet_path.rs");

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

    for forbidden in [
        "Socks5UdpRelay",
        "ActiveUpstreamSocks5UdpAssociation",
        "UpstreamAssociationCloseReason",
        "send_socks5_udp_packet",
        "ensure_socks5_udp_association",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_runtime/socks5_udp.rs should stay a thin facade; found `{forbidden}`"
        );
    }

    assert!(
        active.contains("struct ActiveUpstreamSocks5UdpAssociation")
            && active.contains("Socks5UdpRelay"),
        "SOCKS5 UDP active association should live in protocol_runtime/socks5_udp/active.rs"
    );
    assert!(
        model.contains("enum UpstreamAssociationCloseReason")
            && model.contains("struct Socks5UdpAssociation"),
        "SOCKS5 UDP association model should live in protocol_runtime/socks5_udp/model.rs"
    );
    assert!(
        send_source.contains("async fn send_socks5_udp_packet")
            && send_source.contains("async fn ensure_socks5_udp_association"),
        "SOCKS5 UDP send orchestration should live in protocol_runtime/socks5_udp/send.rs"
    );
    assert!(
        send.exists() && runtime.exists() && packet_path.exists(),
        "SOCKS5 UDP runtime should be split into send.rs, runtime.rs, and packet_path.rs"
    );
}

#[test]
fn vless_udp_state_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vless_udp.rs");
    let model = read("src/protocol_runtime/vless_udp/model.rs");

    for forbidden in [
        "struct VlessUdpUpstream",
        "struct VlessUdpTransport",
        "struct VlessUdpStartFlow",
        "struct VlessUdpRelayTwoStream",
        "struct VlessUdpRelayFinalHop",
        "struct VlessUdpUpstreamRequest",
    ] {
        assert!(
            !root.contains(forbidden),
            "vless_udp.rs should keep state/request models in vless_udp/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VlessUdpUpstream",
        "struct VlessUdpTransport",
        "struct VlessUdpStartFlow",
        "struct VlessUdpRelayTwoStream",
        "struct VlessUdpRelayFinalHop",
        "struct VlessUdpUpstreamRequest",
    ] {
        assert!(
            model.contains(required),
            "VLESS UDP state/request model should live in vless_udp/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn vmess_udp_state_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vmess_udp.rs");
    let model = read("src/protocol_runtime/vmess_udp/model.rs");

    for forbidden in [
        "struct VmessUdpUpstream",
        "struct VmessUdpTransport",
        "struct VmessUdpStartFlow",
        "struct VmessUdpRelayFlow",
        "struct VmessUdpUpstreamRequest",
    ] {
        assert!(
            !root.contains(forbidden),
            "vmess_udp.rs should keep state/request models in vmess_udp/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VmessUdpUpstream",
        "struct VmessUdpTransport",
        "struct VmessUdpStartFlow",
        "struct VmessUdpRelayFlow",
        "struct VmessUdpUpstreamRequest",
    ] {
        assert!(
            model.contains(required),
            "VMess UDP state/request model should live in vmess_udp/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn vmess_mux_pool_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vmess_mux_pool.rs");
    let model = read("src/protocol_runtime/vmess_mux_pool/model.rs");

    for forbidden in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxConn",
        "struct VmessMuxOpenRequest",
        "struct VmessMuxConnectionPool",
    ] {
        assert!(
            !root.contains(forbidden),
            "vmess_mux_pool.rs should keep pool/request models in vmess_mux_pool/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxConn",
        "struct VmessMuxOpenRequest",
        "struct VmessMuxConnectionPool",
    ] {
        assert!(
            model.contains(required),
            "VMess MUX pool model should live in vmess_mux_pool/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn vless_mux_pool_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vless_mux_pool.rs");
    let model = read("src/protocol_runtime/vless_mux_pool/model.rs");

    for forbidden in ["struct MuxConnectionPool", "struct VlessMuxOpenRequest"] {
        assert!(
            !root.contains(forbidden),
            "vless_mux_pool.rs should keep proxy-layer pool/request models in vless_mux_pool/model.rs; found `{forbidden}`"
        );
    }

    for required in ["struct MuxConnectionPool", "struct VlessMuxOpenRequest"] {
        assert!(
            model.contains(required),
            "VLESS MUX pool model should live in vless_mux_pool/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn inbound_vmess_mux_task_model_lives_outside_mux_root() {
    let root = read("src/inbound/vmess/mux.rs");
    let model = read("src/inbound/vmess/model.rs");

    for forbidden in [
        "struct VmessMuxTcpStreamTask",
        "struct VmessMuxUdpStreamTask",
    ] {
        assert!(
            !root.contains(forbidden),
            "inbound/vmess/mux.rs should keep MUX task models in inbound/vmess/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VmessMuxTcpStreamTask",
        "struct VmessMuxUdpStreamTask",
    ] {
        assert!(
            model.contains(required),
            "VMess inbound MUX task model should live in inbound/vmess/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn inbound_vless_mux_task_model_lives_outside_mux_root() {
    let root = read("src/inbound/vless/mux.rs");
    let model = read("src/inbound/vless/model.rs");

    assert!(
        !root.contains("struct VlessMuxUdpStreamTask"),
        "inbound/vless/mux.rs should keep MUX task models in inbound/vless/model.rs"
    );

    assert!(
        model.contains("struct VlessMuxUdpStreamTask"),
        "VLESS inbound MUX task model should live in inbound/vless/model.rs"
    );
}

#[test]
fn mieru_client_stream_model_lives_outside_inbound_root() {
    let root = read("src/inbound/mieru.rs");
    let model = read("src/inbound/mieru/model.rs");

    for forbidden in [
        "struct MieruClientStream",
        "impl AsyncRead for MieruClientStream",
        "impl AsyncWrite for MieruClientStream",
    ] {
        assert!(
            !root.contains(forbidden),
            "inbound/mieru.rs should keep client stream state in inbound/mieru/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct MieruClientStream",
        "impl AsyncRead for MieruClientStream",
        "impl AsyncWrite for MieruClientStream",
    ] {
        assert!(
            model.contains(required),
            "Mieru client stream state should live in inbound/mieru/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn socks5_udp_send_details_stay_out_of_udp_dispatch() {
    let dispatch = read("src/runtime/udp_dispatch/socks5_flow.rs");

    for forbidden in [
        "struct Socks5UdpSend",
        "Socks5UdpAssociation {",
        "send_socks5_udp_packet",
        "UpstreamAssociationCloseReason::Dropped",
        "log_udp_upstream_association_dropped",
        "record_udp_upstream_send_failure",
    ] {
        assert!(
            !dispatch.contains(forbidden),
            "runtime UDP dispatch should delegate SOCKS5 UDP send details to protocol_runtime; found `{forbidden}`"
        );
    }
}

#[test]
fn socks5_udp_association_close_details_stay_out_of_udp_associate_loop() {
    let associate = read("src/protocol_runtime/socks5_udp_associate.rs");

    for forbidden in [
        "UpstreamAssociationCloseReason",
        ".close(",
        ".outbound_tag()",
        ".upstream_endpoint()",
        ".take_socks5_upstream()",
        ".socks5_upstream()",
    ] {
        assert!(
            !associate.contains(forbidden),
            "SOCKS5 UDP associate loop should use dispatch/runtime facades instead of association internals; found `{forbidden}`"
        );
    }
}

#[test]
fn socks5_udp_associate_loop_delegates_dispatch_and_direct_response_framing() {
    let associate = read("src/protocol_runtime/socks5_udp_associate.rs");
    let chain_response = read("src/protocol_runtime/socks5_udp_associate/chain_response.rs");
    let cleanup = read("src/protocol_runtime/socks5_udp_associate/cleanup.rs");
    let dispatch = read("src/protocol_runtime/socks5_udp_associate/dispatch.rs");
    let direct_response = read("src/protocol_runtime/socks5_udp_associate/direct_response.rs");
    let idle_timeout = read("src/protocol_runtime/socks5_udp_associate/idle_timeout.rs");
    let relay_socket = read("src/protocol_runtime/socks5_udp_associate/relay_socket.rs");
    let setup = read("src/protocol_runtime/socks5_udp_associate/setup.rs");
    let upstream_response = read("src/protocol_runtime/socks5_udp_associate/upstream_response.rs");

    for forbidden in [
        "UdpPipeInput",
        "ProtocolType::Socks5",
        "DnsResolver",
        ".resolver.resolve(",
        "async fn dispatch_packet",
        "async fn forward_direct_udp_response",
        "async fn forward_chain_response",
        "socks5::build_udp_packet(&address_from_socket_addr",
        "direct_response_session_id",
        "record_session_outbound_rx",
        "record_session_inbound_tx",
        "failed to forward direct UDP response",
        "socks5::build_udp_packet(target",
        "failed to send UDP chain response to client",
        "failed to build SOCKS5 UDP chain response",
        "chain upstream read error",
        "chain response task panicked",
        "async fn handle_upstream_response",
        "socks5_upstream_view",
        "upstream_response_session_id",
        "record_udp_upstream_recv_failure",
        "log_udp_upstream_association_dropped",
        "failed to attribute upstream UDP response",
        "async fn handle_idle_timeout",
        "fn handle_idle_timeout",
        "drop_socks5_idle",
        "log_udp_upstream_association_idle_timeout",
        "async fn handle_relay_packet",
        "client_udp_addr.is_none",
        "failed to process UDP packet",
        "dropping udp packet from unexpected sender",
        "Socks5Reply::Succeeded",
        "send_response_with_bound",
        "bind_addr(SocketAddr::new",
        "socks5 udp association ready",
        "drain_traffic",
        "finish_all",
        "log_completed_udp_flow",
    ] {
        assert!(
            !associate.contains(forbidden),
            "SOCKS5 UDP associate loop should delegate dispatch/direct response details; found `{forbidden}`"
        );
    }

    assert!(
        dispatch.contains("async fn dispatch_packet")
            && dispatch.contains("UdpPipeInput")
            && dispatch.contains("ProtocolType::Socks5")
            && dispatch.contains(".resolver.resolve("),
        "SOCKS5 UDP packet dispatch should live in socks5_udp_associate/dispatch.rs"
    );
    assert!(
        direct_response.contains("async fn forward_direct_udp_response")
            && direct_response.contains("async fn forward_relay_socket_response")
            && direct_response.contains("async fn forward_dispatch_socket_response")
            && direct_response.contains("direct_response_session_id")
            && direct_response.contains("socks5::build_udp_packet"),
        "SOCKS5 UDP direct response metering and framing should live in socks5_udp_associate/direct_response.rs"
    );
    assert!(
        chain_response.contains("async fn handle_chain_result")
            && chain_response.contains("pub(super) struct ChainResponseRequest")
            && chain_response.contains("struct ForwardChainResponseRequest")
            && chain_response.contains("socks5::build_udp_packet(request.target")
            && chain_response.contains("failed to send UDP chain response to client")
            && chain_response.contains("chain response task panicked"),
        "SOCKS5 UDP chain response result handling and framing should live in socks5_udp_associate/chain_response.rs"
    );
    assert!(
        upstream_response.contains("async fn handle_upstream_response")
            && upstream_response.contains("socks5_upstream_view")
            && upstream_response.contains("upstream_response_session_id")
            && upstream_response.contains("record_udp_upstream_recv_failure")
            && upstream_response.contains("failed to attribute upstream UDP response"),
        "SOCKS5 UDP upstream response attribution and cleanup should live in socks5_udp_associate/upstream_response.rs"
    );
    assert!(
        idle_timeout.contains("fn handle_idle_timeout")
            && idle_timeout.contains("drop_socks5_idle")
            && idle_timeout.contains("log_udp_upstream_association_idle_timeout"),
        "SOCKS5 UDP idle timeout cleanup should live in socks5_udp_associate/idle_timeout.rs"
    );
    assert!(
        relay_socket.contains("async fn handle_relay_packet")
            && relay_socket.contains("pub(super) struct RelayPacketRequest")
            && relay_socket.contains("client_udp_addr.is_none")
            && relay_socket.contains("failed to process UDP packet")
            && relay_socket.contains("dropping udp packet from unexpected sender"),
        "SOCKS5 UDP relay socket packet classification should live in socks5_udp_associate/relay_socket.rs"
    );
    assert!(
        setup.contains("async fn setup_association")
            && setup.contains("Socks5Reply::Succeeded")
            && setup.contains("send_response_with_bound")
            && setup.contains("bind_addr(SocketAddr::new")
            && setup.contains("socks5 udp association ready")
            && setup.contains("drain_traffic"),
        "SOCKS5 UDP associate bind/response setup should live in socks5_udp_associate/setup.rs"
    );
    assert!(
        cleanup.contains("fn finish_dispatch")
            && cleanup.contains("finish_all")
            && cleanup.contains("log_completed_udp_flow"),
        "SOCKS5 UDP associate cleanup should live in socks5_udp_associate/cleanup.rs"
    );
}

#[test]
fn udp_dispatch_poll_refs_does_not_expose_socks5_association_type() {
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");

    for forbidden in [
        "Option<&crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation>",
        "self.socks5.upstream()",
    ] {
        assert!(
            !lifecycle.contains(forbidden),
            "UdpDispatch poll refs should expose Socks5UdpRuntime facade, not SOCKS5 association internals; found `{forbidden}`"
        );
    }
    assert!(
        lifecycle.contains("&crate::protocol_runtime::socks5_udp::Socks5UdpRuntime"),
        "UdpDispatch poll refs should expose the SOCKS5 runtime facade"
    );
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
fn udp_flow_helpers_do_not_depend_on_protocol_runtime() {
    let content = read("src/runtime/udp_flow/helpers.rs");

    assert!(
        !content.contains("protocol_runtime::"),
        "src/runtime/udp_flow/helpers.rs should stay protocol-neutral"
    );
}

#[test]
fn udp_packet_path_carrier_snapshot_lives_with_protocol_runtime() {
    let runtime = read("src/runtime/udp_flow/sessions.rs");
    let protocol_runtime = read("src/protocol_runtime/udp/packet_path_snapshot.rs");

    assert!(
        !runtime.contains("enum UdpPacketPathCarrier"),
        "UdpPacketPathCarrier should not be declared in generic runtime UDP flow state"
    );
    assert!(
        protocol_runtime.contains("enum UdpPacketPathCarrier"),
        "protocol_runtime::udp should own UdpPacketPathCarrier"
    );
}

#[test]
fn udp_flow_outbound_snapshot_is_not_declared_in_session_bookkeeping() {
    let sessions = read("src/runtime/udp_flow/sessions.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");

    assert!(
        !sessions.contains("enum UdpFlowOutbound"),
        "UdpFlowOutbound should not be declared in generic UDP session bookkeeping"
    );
    assert!(
        outbound.contains("enum UdpFlowOutbound"),
        "runtime::udp_flow::outbound should own UdpFlowOutbound"
    );
}

#[test]
fn udp_session_bookkeeping_does_not_match_protocol_outbound_variants() {
    let content = read("src/runtime/udp_flow/sessions.rs");

    for forbidden in [
        "UdpFlowOutbound::Shadowsocks",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_flow/sessions.rs should not match protocol UDP outbound variant `{forbidden}`"
        );
    }
}

#[test]
fn generic_udp_dispatch_does_not_contain_protocol_manager_modules() {
    let forbidden = [
        "protocol_flows.rs",
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
    assert!(
        content.contains("socks5: Socks5UdpRuntime"),
        "UdpDispatch should keep SOCKS5 UDP association state behind Socks5UdpRuntime"
    );

    for forbidden in [
        "socks5_upstream:",
        "socks5_idle_deadline:",
        "ActiveUpstreamSocks5UdpAssociation",
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
fn udp_dispatch_internal_state_fields_are_not_crate_public() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for field in [
        "inbound_tag",
        "flows",
        "direct_socket",
        "socks5",
        "protocol_state",
        "chain_tasks",
    ] {
        assert!(
            !content.contains(&format!("pub(crate) {field}:")),
            "UdpDispatch field `{field}` should stay private behind methods"
        );
    }
}

#[test]
fn protocol_udp_flow_requests_do_not_extend_udp_dispatch() {
    let content = read("src/protocol_runtime/udp/flows.rs");

    for forbidden in [
        "impl UdpDispatch",
        "use crate::runtime::udp_dispatch::UdpDispatch",
    ] {
        assert!(
            !content.contains(forbidden),
            "protocol_runtime::udp::flows should define request types, not extend runtime dispatcher; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_start_logic_is_split_by_protocol_family() {
    let root = manifest_dir().join("src/protocol_runtime/udp");

    assert!(
        !root.join("start.rs").exists(),
        "protocol UDP start logic should live under src/protocol_runtime/udp/start/, not in a monolithic start.rs"
    );

    for path in [
        "start/mod.rs",
        "start/datagram.rs",
        "start/mieru.rs",
        "start/trojan.rs",
        "start/vless.rs",
        "start/vmess.rs",
    ] {
        assert!(
            root.join(path).exists(),
            "protocol UDP start logic should keep protocol-family module `{path}`"
        );
    }
}

#[test]
fn protocol_udp_datagram_start_keeps_trojan_and_mieru_in_protocol_modules() {
    let datagram = read("src/protocol_runtime/udp/start/datagram.rs");
    let trojan = manifest_dir().join("src/protocol_runtime/udp/start/trojan.rs");
    let mieru = manifest_dir().join("src/protocol_runtime/udp/start/mieru.rs");

    for forbidden in [
        "TrojanUdpFlowRequest",
        "TrojanUdpRelayFlowRequest",
        "MieruUdpFlowRequest",
        "start_mieru_udp_relay_flow",
        "TrojanSendExisting",
        "MieruSendExisting",
    ] {
        assert!(
            !datagram.contains(forbidden),
            "start/datagram.rs should keep Trojan and Mieru start facades in protocol modules; found `{forbidden}`"
        );
    }
    assert!(
        trojan.exists(),
        "Trojan UDP start facade should live in start/trojan.rs"
    );
    assert!(
        mieru.exists(),
        "Mieru UDP start facade should live in start/mieru.rs"
    );
}

#[test]
fn udp_dispatch_does_not_keep_protocol_start_wrappers() {
    let root = manifest_dir().join("src/runtime/udp_dispatch");

    assert!(
        !root.join("protocol_start.rs").exists(),
        "runtime UDP protocol start wrappers should not live beside udp_dispatch root"
    );
    assert!(
        !root.join("start/protocol.rs").exists(),
        "runtime UDP dispatch should not expose protocol-named start wrappers; adapters should call protocol_runtime::udp state directly"
    );

    for path in rust_sources_under("src/runtime/udp_dispatch") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "ShadowsocksUdpFlow",
            "MieruUdpRelayFlow",
            "VlessUdpFlow",
            "VlessUdpRelayFinalHop",
            "VlessUdpRelayTwoStream",
            "VmessUdpFlow",
            "VmessUdpRelayFlow",
            "start_shadowsocks_udp_flow",
            "Hysteria2UdpFlowRequest",
            "TrojanUdpFlowRequest",
            "TrojanUdpRelayFlowRequest",
            "MieruUdpFlowRequest",
            "start_mieru_udp_relay_flow",
            "start_vless_udp_flow",
            "start_vless_udp_relay_two_stream",
            "start_vless_udp_relay_final_hop",
            "start_vmess_udp_flow",
            "start_vmess_udp_relay_flow",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not expose protocol UDP start wrapper `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_registry_tests_live_outside_logic_file() {
    let registry = read("src/protocol_adapter/registry.rs");
    let tests = manifest_dir().join("src/protocol_adapter/registry/tests.rs");

    assert!(
        !registry.contains("mod tests {"),
        "protocol registry tests should live in src/protocol_adapter/registry/tests.rs"
    );
    assert!(
        tests.exists(),
        "protocol registry boundary tests should stay in a sibling tests module"
    );
}

#[test]
fn protocol_registry_build_lives_outside_adapters_root() {
    let adapters = read("src/adapters/mod.rs");
    let registry = read("src/protocol_adapter/registry.rs");
    let inventory = read("src/inventory.rs");

    assert!(
        !adapters.contains("build_registry"),
        "src/adapters/mod.rs should not own registry construction"
    );
    assert!(
        registry.contains("pub(crate) fn build() -> Self"),
        "src/protocol_adapter/registry.rs should own registry construction"
    );
    assert!(
        inventory.contains("ProtocolRegistry::build()"),
        "src/inventory.rs should build the registry through protocol_adapter::registry"
    );
}

#[test]
fn udp_dispatch_does_not_reexport_protocol_runtime_udp_types() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for forbidden in [
        "pub(crate) use crate::protocol_runtime::udp::",
        "pub(crate) use crate::protocol_runtime::socks5_udp::",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/mod.rs should not re-export protocol-runtime UDP types; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_state_manager_fields_are_not_crate_public() {
    let content = read("src/protocol_runtime/udp/state.rs");

    for field in [
        "vless",
        "vmess",
        "shadowsocks",
        "packet_path",
        "trojan",
        "mieru",
        "hysteria2",
    ] {
        assert!(
            !content.contains(&format!("pub(crate) {field}:")),
            "ProtocolUdpState manager field `{field}` should not be crate-public"
        );
    }
}

#[test]
fn udp_dispatch_cached_flow_fast_path_delegates_to_protocol_state() {
    let content = read("src/runtime/udp_dispatch/dispatch.rs");

    assert!(
        content.contains("send_existing_cached_flow"),
        "UDP dispatch should delegate cached protocol flow handling to ProtocolUdpState"
    );

    let normalized = content.replace("\r\n", "\n");
    for forbidden in [
        ".protocol_state\n            .vless",
        ".protocol_state\n            .vmess",
    ] {
        assert!(
            !normalized.contains(forbidden),
            "src/runtime/udp_dispatch/dispatch.rs should not reach into protocol manager `{forbidden}` directly"
        );
    }
}

#[test]
fn udp_relay_start_delegates_packet_path_chain_to_protocol_state() {
    let content = read("src/runtime/udp_dispatch/start/relay.rs");

    assert!(
        content.contains("send_packet_path_chain"),
        "UDP relay start should delegate packet-path manager work to ProtocolUdpState"
    );
    assert!(
        !content.contains(".packet_path"),
        "src/runtime/udp_dispatch/start/relay.rs should not reach into packet_path manager directly"
    );
    assert!(
        !content.contains("UdpFlowOutbound::"),
        "src/runtime/udp_dispatch/start/relay.rs should not construct UDP flow outbound variants directly"
    );
}

#[test]
fn udp_forward_stays_protocol_neutral_and_does_not_construct_peer_types() {
    let content = read("src/runtime/udp_dispatch/forward.rs");

    assert!(
        content.contains("forward_existing_protocol_flow"),
        "src/runtime/udp_dispatch/forward.rs should delegate protocol manager forwarding to ProtocolUdpState"
    );

    for forbidden in [
        "SsUdpPeer",
        "H2UdpPeer",
        "TrojanUdpPeer",
        "MieruUdpPeer",
        "UdpPeerEndpoint",
        ".packet_path",
        ".shadowsocks",
        ".hysteria2",
        ".trojan",
        ".mieru",
        "UdpFlowOutbound::Direct",
        "UdpFlowOutbound::Socks5",
        "UdpFlowOutbound::Shadowsocks",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/forward.rs should not construct protocol peer types; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_existing_flow_forward_lives_outside_state_root() {
    let state = read("src/protocol_runtime/udp/state.rs");
    let forward = manifest_dir().join("src/protocol_runtime/udp/state/forward.rs");

    for forbidden in [
        "fn forward_existing_protocol_flow",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
        "UdpFlowOutbound::Direct",
        "UdpFlowOutbound::Socks5",
        "udp_protocol_forward",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/protocol_runtime/udp/state.rs should keep existing-flow forwarding details in state/forward.rs; found `{forbidden}`"
        );
    }
    assert!(
        forward.exists(),
        "existing UDP protocol-flow forwarding should live in protocol_runtime/udp/state/forward.rs"
    );
}

#[test]
fn protocol_udp_existing_flow_handlers_live_outside_forward_dispatch() {
    let forward = read("src/protocol_runtime/udp/state/forward.rs");
    let root = manifest_dir().join("src/protocol_runtime/udp/state/forward");

    for forbidden in [
        "SsSendExisting",
        "H2SendExisting",
        "TrojanSendExisting",
        "MieruSendExisting",
        "UdpFlowContext",
        "UdpPacketRef",
        ".send_with_snapshot(",
    ] {
        assert!(
            !forward.contains(forbidden),
            "state/forward.rs should dispatch existing UDP flows and keep protocol handlers in state/forward/*.rs; found `{forbidden}`"
        );
    }
    for path in ["shadowsocks.rs", "hysteria2.rs", "trojan.rs", "mieru.rs"] {
        assert!(
            root.join(path).exists(),
            "existing UDP protocol-flow handler should live in state/forward/{path}"
        );
    }
}

#[test]
fn protocol_udp_cached_flow_fast_path_lives_outside_state_root() {
    let state = read("src/protocol_runtime/udp/state.rs");
    let cached = manifest_dir().join("src/protocol_runtime/udp/state/cached.rs");

    for forbidden in [
        "fn send_existing_cached_flow",
        ".vless\n            .send_existing",
        ".vmess\n            .send_existing",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/protocol_runtime/udp/state.rs should keep cached-flow forwarding details in state/cached.rs; found `{forbidden}`"
        );
    }
    assert!(
        cached.exists(),
        "cached UDP flow forwarding should live in protocol_runtime/udp/state/cached.rs"
    );
}

#[test]
fn protocol_udp_packet_path_facade_lives_outside_state_root() {
    let state = read("src/protocol_runtime/udp/state.rs");
    let packet_path = manifest_dir().join("src/protocol_runtime/udp/state/packet_path.rs");

    for forbidden in [
        "fn datagram_chain_flow_outbound",
        "fn send_packet_path_chain",
        "UdpFlowOutbound::Shadowsocks",
        "packet_path_carrier",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/protocol_runtime/udp/state.rs should keep packet-path facade details in state/packet_path.rs; found `{forbidden}`"
        );
    }
    assert!(
        packet_path.exists(),
        "UDP packet-path facade should live in protocol_runtime/udp/state/packet_path.rs"
    );
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
fn packet_path_traits_are_grouped_by_responsibility() {
    let facade = read("src/protocol_runtime/udp/packet_path_traits.rs");
    let root = manifest_dir().join("src/protocol_runtime/udp/packet_path_traits");

    for forbidden in [
        "trait PacketPathCarrier",
        "struct PacketPathCarrierDescriptor",
        "struct UdpDatagramSource",
        "type ChainTask =",
        "struct UdpFlowContext",
        "struct UdpPacketRef",
        "struct SsUdpPeer",
        "struct H2UdpPeer",
        "struct TrojanUdpPeer",
        "struct MieruUdpPeer",
    ] {
        assert!(
            !facade.contains(forbidden),
            "packet_path_traits.rs should stay a facade and keep grouped definitions in packet_path_traits/*.rs; found `{forbidden}`"
        );
    }
    for path in ["carrier.rs", "context.rs", "peer.rs"] {
        assert!(
            root.join(path).exists(),
            "packet-path trait/helper definitions should keep grouped module packet_path_traits/{path}"
        );
    }
}

#[test]
fn packet_path_carriers_live_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let carriers = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/carriers.rs");

    for forbidden in ["struct ShadowsocksPacketPath", "struct Hysteria2PacketPath"] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep concrete carrier implementations in carriers.rs; found `{forbidden}`"
        );
    }
    assert!(
        carriers.exists(),
        "packet-path carrier implementations should live in packet_path_chain/carriers.rs"
    );
}

#[test]
fn packet_path_protocol_carriers_live_outside_carrier_facade() {
    let facade = read("src/protocol_runtime/udp/packet_path_chain/carriers.rs");
    let shadowsocks = manifest_dir()
        .join("src/protocol_runtime/udp/packet_path_chain/carriers/shadowsocks_carrier.rs");
    let hysteria2 = manifest_dir()
        .join("src/protocol_runtime/udp/packet_path_chain/carriers/hysteria2_carrier.rs");

    for forbidden in [
        "struct ShadowsocksPacketPath",
        "struct Hysteria2PacketPath",
        "ShadowsocksDatagramCodec",
        "Hysteria2UdpPacketTarget",
        "connect_raw",
    ] {
        assert!(
            !facade.contains(forbidden),
            "packet_path_chain/carriers.rs should keep concrete carrier internals in protocol files; found `{forbidden}`"
        );
    }
    assert!(
        shadowsocks.exists(),
        "Shadowsocks packet-path carrier should live in carriers/shadowsocks_carrier.rs"
    );
    assert!(
        hysteria2.exists(),
        "Hysteria2 packet-path carrier should live in carriers/hysteria2_carrier.rs"
    );
}

#[test]
fn packet_path_response_bridge_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/bridge.rs");

    for forbidden in ["async fn recv_loop", "fn remove_waiter"] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep response bridge internals in bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "packet-path response bridge should live in packet_path_chain/bridge.rs"
    );
}

#[test]
fn packet_path_key_model_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let key = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/key.rs");

    for forbidden in [
        "struct PathKey",
        "carrier_key: carrier.cache_key().to_owned()",
        "datagram_password: datagram_password.to_owned()",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep key construction details in packet_path_chain/key.rs; found `{forbidden}`"
        );
    }
    assert!(
        key.exists(),
        "packet-path key model should live in packet_path_chain/key.rs"
    );
}

#[test]
fn packet_path_entry_model_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let model = read("src/protocol_runtime/udp/packet_path_chain/model.rs");

    for forbidden in [
        "struct Entry",
        "struct EntryCandidate",
        "fn key(&self) -> PathKey",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep entry model details in packet_path_chain/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct Entry",
        "struct EntryCandidate",
        "fn key(&self) -> PathKey",
    ] {
        assert!(
            model.contains(required),
            "packet-path entry model should live in packet_path_chain/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn packet_path_entry_build_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let entry = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/entry.rs");

    for forbidden in [
        "udp_packet_path_carrier_descriptor",
        "udp_datagram_source",
        "ShadowsocksDatagramCodec",
        "tokio::spawn(recv_loop",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep entry build details in packet_path_chain/entry.rs; found `{forbidden}`"
        );
    }
    assert!(
        entry.exists(),
        "packet-path entry build logic should live in packet_path_chain/entry.rs"
    );
}

#[test]
fn packet_path_diagnostics_live_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let diagnostics =
        manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/diagnostics.rs");

    for forbidden in ["fn carrier_upstream", "orchestration::endpoint"] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep diagnostics helpers in packet_path_chain/diagnostics.rs; found `{forbidden}`"
        );
    }
    assert!(
        diagnostics.exists(),
        "packet-path diagnostics helpers should live in packet_path_chain/diagnostics.rs"
    );
}

#[test]
fn packet_path_snapshot_lookup_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let snapshot = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/snapshot.rs");

    for forbidden in [
        "PathKey::from_snapshot",
        "packet_path_carrier_dropped",
        "cached packet-path carrier not found",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep snapshot lookup details in packet_path_chain/snapshot.rs; found `{forbidden}`"
        );
    }
    assert!(
        snapshot.exists(),
        "packet-path snapshot lookup should live in packet_path_chain/snapshot.rs"
    );
}

#[test]
fn packet_path_snapshot_send_uses_request_model() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let forward = read("src/protocol_runtime/udp/state/forward/shadowsocks.rs");

    assert!(
        manager.contains("struct SendWithSnapshotRequest")
            && manager.contains("request: SendWithSnapshotRequest<'_>"),
        "packet-path snapshot send should use a request model"
    );
    assert!(
        forward.contains("SendWithSnapshotRequest {"),
        "packet-path snapshot forward path should pass SendWithSnapshotRequest"
    );
}

#[test]
fn feature_gated_udp_manager_modules_do_not_embed_disabled_stubs() {
    for source in [
        "src/protocol_runtime/udp/h2_manager.rs",
        "src/protocol_runtime/udp/mieru_manager.rs",
        "src/protocol_runtime/udp/trojan_manager.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains("#[cfg(not(feature ="),
            "{source} should not mix enabled manager logic with disabled-feature stubs"
        );
    }
}

#[test]
fn trojan_udp_socket_wrappers_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let socket = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/socket.rs");

    for forbidden in ["struct ReadOnlySocket", "struct WriteOnlySocket"] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep stream socket adapters in trojan_manager/socket.rs; found `{forbidden}`"
        );
    }
    assert!(
        socket.exists(),
        "Trojan UDP socket wrappers should live in trojan_manager/socket.rs"
    );
}

#[test]
fn trojan_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/bridge.rs");

    for forbidden in [
        "broadcast::channel",
        "recv_tx.subscribe",
        "fn spawn_bridge",
        "trojan upstream closed",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep response bridge details in trojan_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Trojan UDP response bridge should live in trojan_manager/bridge.rs"
    );
}

#[test]
fn trojan_udp_tls_connect_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let connect = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/connect.rs");

    for forbidden in [
        "ClientTlsConfig",
        "connect_tls_upstream",
        "connect_tls_stream",
        ".connect_host(",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep TLS connect details in trojan_manager/connect.rs; found `{forbidden}`"
        );
    }
    assert!(
        connect.exists(),
        "Trojan UDP TLS connect helpers should live in trojan_manager/connect.rs"
    );
}

#[test]
fn trojan_udp_packet_stream_tasks_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let stream = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/stream.rs");

    for forbidden in [
        "MeteredStream",
        "UdpPacketStreamFraming",
        "write_udp_packet",
        "read_udp_packet",
        "tokio::io::split",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep packet stream task details in trojan_manager/stream.rs; found `{forbidden}`"
        );
    }
    assert!(
        stream.exists(),
        "Trojan UDP packet stream tasks should live in trojan_manager/stream.rs"
    );
}

#[test]
fn mieru_udp_packet_codec_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let codec = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/codec.rs");

    for forbidden in [
        "UdpPacketFraming",
        "MieruUdpAssociatePacket",
        "fn encode_associate_packet",
        "fn decode_associate_packet",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep associate packet codec details in mieru_manager/codec.rs; found `{forbidden}`"
        );
    }
    assert!(
        codec.exists(),
        "Mieru UDP packet codec should live in mieru_manager/codec.rs"
    );
}

#[test]
fn mieru_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/bridge.rs");

    for forbidden in [
        "type RecvItem",
        "broadcast::channel",
        "recv_tx.subscribe",
        "fn spawn_bridge",
        "mieru upstream closed",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep response bridge details in mieru_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Mieru UDP response bridge should live in mieru_manager/bridge.rs"
    );
}

#[test]
fn mieru_udp_connect_handshake_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let connect = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/connect.rs");

    for forbidden in [
        "MieruOutbound::connect",
        ".connect_host(",
        "ASSOCIATE",
        "encrypt_client_data(&assoc_req)",
        "mieru udp assoc",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep connect and UDP associate handshake details in mieru_manager/connect.rs; found `{forbidden}`"
        );
    }
    assert!(
        connect.exists(),
        "Mieru UDP connect helpers should live in mieru_manager/connect.rs"
    );
}

#[test]
fn mieru_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/model.rs");

    for forbidden in [
        "enum MieruKey",
        "struct MieruEntry",
        "struct MieruSendExisting",
        "struct MieruRelayExisting",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep state/request models in mieru_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Mieru UDP state/request models should live in mieru_manager/model.rs"
    );
}

#[test]
fn mieru_udp_establish_logic_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let establish = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/establish.rs");

    for forbidden in [
        "fn establish_direct",
        "fn establish_packet_stream",
        "connect::direct_stream",
        "connect::establish_udp_associate",
        "spawn_packet_stream",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep UDP establish glue in mieru_manager/establish.rs; found `{forbidden}`"
        );
    }
    assert!(
        establish.exists(),
        "Mieru UDP establish glue should live in mieru_manager/establish.rs"
    );
}

#[test]
fn mieru_udp_send_orchestration_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let send = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/send.rs");

    for forbidden in [
        "async fn send(",
        "fn send_relay",
        "send_existing(",
        "send_relay_existing(",
        "mieru_relay_upstream",
        "mieru_establish",
        "mieru_relay_establish",
        "UdpFlowContext",
        "UdpPacketRef",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep send orchestration in mieru_manager/send.rs; found `{forbidden}`"
        );
    }
    assert!(
        send.exists(),
        "Mieru UDP send orchestration should live in mieru_manager/send.rs"
    );
}

#[test]
fn trojan_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/model.rs");

    for forbidden in [
        "enum TrojanKey",
        "struct TrojanEntry",
        "struct TrojanSendExisting",
        "struct TrojanRelaySend",
        "struct TrojanRelayExisting",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep state/request models in trojan_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Trojan UDP state/request models should live in trojan_manager/model.rs"
    );
}

#[test]
fn trojan_udp_establish_logic_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let establish = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/establish.rs");

    for forbidden in [
        "fn establish_direct",
        "fn establish_over_relay_stream",
        "fn establish_packet_stream",
        "connect::direct_tls_stream",
        "connect::relay_tls_stream",
        "spawn_packet_stream",
        "TrojanUdpPacket {",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep UDP establish glue in trojan_manager/establish.rs; found `{forbidden}`"
        );
    }
    assert!(
        establish.exists(),
        "Trojan UDP establish glue should live in trojan_manager/establish.rs"
    );
}

#[test]
fn trojan_udp_send_orchestration_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let send = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/send.rs");

    for forbidden in [
        "async fn send(",
        "fn send_relay",
        "send_existing(",
        "send_relay_existing(",
        "trojan_relay_upstream",
        "trojan_establish",
        "trojan_relay_establish",
        "UdpFlowContext",
        "UdpPacketRef",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep send orchestration in trojan_manager/send.rs; found `{forbidden}`"
        );
    }
    assert!(
        send.exists(),
        "Trojan UDP send orchestration should live in trojan_manager/send.rs"
    );
}

#[test]
fn mieru_udp_packet_stream_tasks_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let stream = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/stream.rs");

    for forbidden in [
        "tokio::io::split",
        "encrypt_client_data(&payload)",
        "decrypt_server_data_with_consumed(&raw)",
        "parse_udp_packet",
        "AsyncReadExt",
        "AsyncWriteExt",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep packet stream task details in mieru_manager/stream.rs; found `{forbidden}`"
        );
    }
    assert!(
        stream.exists(),
        "Mieru UDP packet stream tasks should live in mieru_manager/stream.rs"
    );
}

#[test]
fn h2_udp_datagram_codec_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let codec = manifest_dir().join("src/protocol_runtime/udp/h2_manager/codec.rs");

    for forbidden in [
        "UdpDatagramFraming",
        "Hysteria2UdpPacketTarget",
        "fn decode_packet",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep datagram codec details in h2_manager/codec.rs; found `{forbidden}`"
        );
    }
    assert!(
        codec.exists(),
        "Hysteria2 UDP datagram codec should live in h2_manager/codec.rs"
    );
}

#[test]
fn h2_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/h2_manager/bridge.rs");

    for forbidden in [
        "type RecvItem",
        "broadcast::channel",
        "recv_tx.subscribe",
        "h2 upstream closed",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep response bridge details in h2_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Hysteria2 UDP response bridge should live in h2_manager/bridge.rs"
    );
}

#[test]
fn h2_udp_packet_stream_tasks_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let stream = manifest_dir().join("src/protocol_runtime/udp/h2_manager/stream.rs");

    for forbidden in [
        "Hysteria2Connector",
        "connect_raw",
        "send_datagram",
        "read_datagram",
        "tokio::spawn",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep QUIC packet stream task details in h2_manager/stream.rs; found `{forbidden}`"
        );
    }
    assert!(
        stream.exists(),
        "Hysteria2 UDP packet stream tasks should live in h2_manager/stream.rs"
    );
}

#[test]
fn h2_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/h2_manager/model.rs");

    for forbidden in ["struct H2Entry", "struct H2SendExisting", "struct H2Key"] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep state/request models in h2_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Hysteria2 UDP state/request models should live in h2_manager/model.rs"
    );
}

#[test]
fn h2_udp_model_details_live_outside_manager_root() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let model = read("src/protocol_runtime/udp/h2_manager/model.rs");

    for forbidden in ["struct H2Entry", "struct H2SendExisting", "struct H2Key"] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep model details in h2_manager/model.rs; found `{forbidden}`"
        );
    }

    for required in ["struct H2Entry", "struct H2SendExisting", "struct H2Key"] {
        assert!(
            model.contains(required),
            "h2_manager model details should live in h2_manager/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn h2_udp_send_orchestration_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let send = manifest_dir().join("src/protocol_runtime/udp/h2_manager/send.rs");

    for forbidden in [
        "async fn send(",
        "pub(crate) async fn send_existing",
        "H2Key::from_peer",
        "h2_udp_packet",
        "h2_establish",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep send orchestration in h2_manager/send.rs; found `{forbidden}`"
        );
    }
    assert!(
        send.exists(),
        "Hysteria2 UDP send orchestration should live in h2_manager/send.rs"
    );
}

#[test]
fn h2_udp_establish_logic_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let establish = manifest_dir().join("src/protocol_runtime/udp/h2_manager/establish.rs");

    for forbidden in [
        "async fn establish",
        "stream::establish",
        "spawn_response_bridge",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep establish glue in h2_manager/establish.rs; found `{forbidden}`"
        );
    }
    assert!(
        establish.exists(),
        "Hysteria2 UDP establish glue should live in h2_manager/establish.rs"
    );
}

#[test]
fn shadowsocks_udp_datagram_codec_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let codec = manifest_dir().join("src/protocol_runtime/udp/ss_manager/codec.rs");

    for forbidden in [
        "UdpDatagramFraming",
        "ShadowsocksUdpPacketTarget",
        "ShadowsocksUdpDecodeContext",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep datagram codec details in ss_manager/codec.rs; found `{forbidden}`"
        );
    }
    assert!(
        codec.exists(),
        "Shadowsocks UDP datagram codec should live in ss_manager/codec.rs"
    );
}

#[test]
fn shadowsocks_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/ss_manager/bridge.rs");

    for forbidden in [
        "oneshot::channel",
        "VecDeque",
        "struct SsResponseWaiter",
        "fn remove_waiter",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep response waiter bridge details in ss_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Shadowsocks UDP response bridge should live in ss_manager/bridge.rs"
    );
}

#[test]
fn shadowsocks_udp_socket_runtime_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let socket = manifest_dir().join("src/protocol_runtime/udp/ss_manager/socket.rs");

    for forbidden in [
        "UdpSocket::bind",
        "from_std",
        "fn recv_loop",
        "tokio::spawn(Self::recv_loop",
        "shadowsocks udp recv loop stopped",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep socket runtime details in ss_manager/socket.rs; found `{forbidden}`"
        );
    }
    assert!(
        socket.exists(),
        "Shadowsocks UDP socket runtime should live in ss_manager/socket.rs"
    );
}

#[test]
fn shadowsocks_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/ss_manager/model.rs");

    for forbidden in [
        "struct SsUpstream",
        "struct SsSendExisting",
        "struct SsKey",
        "format!(\"{cipher_kind:?}\")",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep state/request models in ss_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Shadowsocks UDP state/request models should live in ss_manager/model.rs"
    );
}

#[test]
fn shadowsocks_udp_entry_cache_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let entry = manifest_dir().join("src/protocol_runtime/udp/ss_manager/entry.rs");

    for forbidden in [
        "fn ensure_entry",
        "SsKey::new",
        "socket::bind_for_target",
        "BridgeWaiters::new",
        "socket::spawn_recv_loop",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep entry/cache construction in ss_manager/entry.rs; found `{forbidden}`"
        );
    }
    assert!(
        entry.exists(),
        "Shadowsocks UDP entry/cache construction should live in ss_manager/entry.rs"
    );
}

#[test]
fn adapters_do_not_reach_into_udp_dispatch_manager_fields() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            ".protocol_state",
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

#[test]
fn adapters_do_not_import_protocol_udp_types_through_runtime_dispatch() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "crate::runtime::udp_dispatch::PacketPathCarrier",
            "crate::runtime::udp_dispatch::PacketPathCarrierDescriptor",
            "crate::runtime::udp_dispatch::UdpDatagramSource",
            "crate::runtime::udp_dispatch::build_socks5_packet_path",
            "crate::runtime::udp_dispatch::build_shadowsocks_packet_path",
            "crate::runtime::udp_dispatch::build_hysteria2_packet_path",
            "crate::runtime::udp_dispatch::ShadowsocksUdpFlow",
            "crate::runtime::udp_dispatch::MieruUdpRelayFlow",
            "crate::runtime::udp_dispatch::VlessUdpFlow",
            "crate::runtime::udp_dispatch::VmessUdpFlow",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should import protocol UDP type directly from protocol_runtime, not `{forbidden}`"
            );
        }
    }
}
