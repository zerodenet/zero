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

fn impl_block(source: &str, type_name: &str) -> String {
    let normalized = source.replace("\r\n", "\n");
    let needle = format!("impl {type_name} {{");
    let start = normalized
        .find(&needle)
        .unwrap_or_else(|| panic!("missing impl block for {type_name}"));
    let body_start = start + needle.len();
    let mut depth = 1usize;
    for (offset, ch) in normalized[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return normalized[start..body_start + offset + 1].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("unterminated impl block for {type_name}")
}

fn struct_block<'a>(source: &'a str, type_name: &str) -> &'a str {
    let needle = format!("pub struct {type_name}");
    source
        .split(&needle)
        .nth(1)
        .and_then(|content| content.split(&format!("impl {type_name}")).next())
        .unwrap_or_else(|| panic!("missing struct block for {type_name}"))
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
fn proxy_production_sources_do_not_keep_todo_markers() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for marker in ["TODO", "FIXME"] {
            assert!(
                !content.contains(marker),
                "{source} should not keep unresolved `{marker}` markers in production code"
            );
        }
    }
}

#[test]
fn protocol_identity_parsing_is_confined_to_adapters() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        if source.starts_with("src/adapters/") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "parse_uuid",
            "VmessCipher::from_name",
            "CipherKind::from_str",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not parse protocol identity/cipher config outside adapters; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn runtime_protocol_runtime_references_are_confined_to_facades() {
    let allowed_exact = [
        "src/runtime/udp_dispatch/mod.rs",
        "src/runtime/udp_dispatch/lifecycle.rs",
        "src/runtime/udp_dispatch/managed.rs",
        "src/runtime/udp_dispatch/start/relay.rs",
    ];

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read runtime source");
        if !content.contains("crate::protocol_runtime::") {
            continue;
        }
        assert!(
            allowed_exact.iter().any(|allowed| *allowed == source),
            "{source} should not reference protocol_runtime directly; add a narrow facade or extend this allow-list with a boundary test"
        );
    }
}

#[test]
fn ordinary_udp_inbounds_submit_packets_through_udp_pipe() {
    for source in [
        "src/inbound/socks5/udp_associate/dispatch.rs",
        "src/inbound/vless/udp_session.rs",
        "src/inbound/trojan.rs",
        "src/inbound/shadowsocks/udp.rs",
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
fn inbound_udp_glue_does_not_name_protocol_private_packet_models() {
    for path in rust_sources_under("src/inbound") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read inbound source");

        for forbidden in [
            "InboundUdpCodec",
            "InboundUdpDispatchParts",
            "InboundUdpResponseTarget",
            "InboundUdpPacket",
            "InboundUdpResponse",
            "decode_inbound_udp",
            "encode_inbound_udp",
            "read_inbound_udp",
            "write_inbound_udp",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should use protocol-owned UDP sessions instead of naming protocol-private inbound UDP model `{forbidden}`"
            );
        }
    }
}

#[test]
fn inbound_udp_response_accounting_uses_runtime_helpers() {
    let helper = read("src/runtime/udp_flow/helpers.rs");
    assert!(
        helper.contains("fn record_udp_inbound_response_rx")
            && helper.contains("fn record_udp_inbound_response_tx")
            && helper.contains("fn udp_response_session_id")
            && helper.contains("record_session_outbound_rx")
            && helper.contains("record_session_inbound_tx")
            && helper.contains("session_id_by_target"),
        "neutral UDP inbound response accounting should live in runtime/udp_flow helpers"
    );

    for source in [
        "src/inbound/vless/udp_session.rs",
        "src/inbound/vless/mux.rs",
        "src/inbound/vmess/mux.rs",
        "src/inbound/trojan.rs",
        "src/inbound/mieru.rs",
        "src/inbound/socks5/udp_associate/direct_response.rs",
        "src/inbound/socks5/udp_associate/chain_response.rs",
        "src/inbound/socks5/udp_associate/upstream_response.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("record_udp_inbound_response_rx")
                && content.contains("record_udp_inbound_response_tx")
                && !content.contains("record_session_outbound_rx")
                && !content.contains("record_session_inbound_tx"),
            "{source} should use neutral UDP inbound response accounting helpers"
        );
        assert!(
            !content.contains("session_id_by_target"),
            "{source} should use udp_response_session_id instead of querying dispatch response sessions directly"
        );
    }

    let hysteria2 = read("src/inbound/hysteria2.rs");
    let datagram_loop = hysteria2
        .split("async fn hysteria2_datagram_loop")
        .nth(1)
        .expect("hysteria2 datagram loop");
    assert!(
        datagram_loop.contains("record_udp_inbound_response_rx")
            && datagram_loop.contains("record_udp_inbound_response_tx")
            && datagram_loop.contains("udp_response_session_id")
            && !datagram_loop.contains("record_session_outbound_rx")
            && !datagram_loop.contains("record_session_inbound_tx")
            && !datagram_loop.contains("session_id_by_target"),
        "Hysteria2 datagram loop should use neutral UDP response accounting helpers without affecting TCP stream relay accounting"
    );
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
    let vmess = read("src/inbound/vmess/mux.rs");
    assert!(
        vless.contains("TcpPipe::new") && vless.contains("TcpPipeInput"),
        "VLESS MUX sub-streams should route through TcpPipe"
    );
    assert!(
        !vless.contains("dispatch_tcp_outbound"),
        "VLESS inbound should not bypass TcpPipe through TCP outbound helpers"
    );
    assert!(
        vmess.contains("TcpPipe::new")
            && vmess.contains("TcpPipeInput")
            && !vmess.contains("dispatch_tcp("),
        "VMess MUX sub-streams should route through TcpPipe"
    );
}

#[test]
fn vless_inbound_mux_frame_detail_lives_in_protocol_crate() {
    let inbound = read("src/inbound/vless/mux.rs");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vless/src/mux.rs"))
        .expect("read protocols/vless/src/mux.rs");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vless/src/lib.rs"))
        .expect("read protocols/vless/src/lib.rs");

    for forbidden in [
        "encode_new_stream_response",
        "parse_new_stream",
        "MUX_STATUS_",
        "NETWORK_TCP",
        "NETWORK_UDP",
        "STATUS_NEW",
        "STATUS_KEEP",
        "STATUS_END",
        "STATUS_KEEP_ALIVE",
        "let mut next_id",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "VLESS inbound mux should delegate protocol MUX frame/state detail to protocols/vless; found `{forbidden}`"
        );
    }

    for required in [
        "VlessInboundMuxSession",
        "VlessInboundMuxAction",
        "VlessInboundMuxWriter",
        "read_inbound_action",
        "accept_inbound_stream",
        "reject_inbound_stream",
        "send_inbound_stream_payload",
        "end_inbound_stream",
    ] {
        assert!(
            inbound.contains(required),
            "VLESS inbound mux should consume protocol-owned semantic MUX server APIs; missing `{required}`"
        );
        assert!(
            protocol_mux.contains(required),
            "protocols/vless should own VLESS semantic MUX server API `{required}`"
        );
    }
    for forbidden in [
        ".next_action(",
        ".accept_stream(",
        ".reject_stream(",
        ".send_data(",
        ".end_stream(",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "VLESS inbound mux glue should not call low-level MUX frame operations directly; found `{forbidden}`"
        );
    }
    for required in [
        "pub async fn next_action",
        "pub async fn accept_stream",
        "pub async fn reject_stream",
        "pub async fn send_data",
        "pub async fn send_inbound_stream_data",
        "pub async fn send_inbound_stream_payload",
        "pub async fn end_stream",
        "pub fn end_inbound_stream",
    ] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vless should keep low-level MUX frame operation `{required}`"
        );
    }
    assert!(
        protocol_mux.contains("pub fn into_session(self) -> Result<Session, Error>")
            && protocol_mux.contains("ProtocolType::Vless")
            && protocol_mux.contains("impl From<MuxServerEvent> for VlessInboundMuxAction")
            && inbound.contains("VlessInboundMuxAction::OpenStream")
            && !inbound.contains("target.into_session()")
            && !inbound.contains("MuxNetwork")
            && !inbound.contains("zero_core::Session::new"),
        "VLESS inbound mux target to Session conversion should be protocol-owned and exposed as an action"
    );
    for forbidden in [
        "MuxServerEvent",
        ".next_event(",
        "MuxServer::",
        ".recv_event(",
        ".write_new_stream_accepted(",
        ".write_new_stream_rejected(",
        ".write_data(",
        ".write_end(",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "VLESS inbound mux should use VlessInboundMuxSession instead of low-level MUX server API `{forbidden}`"
        );
    }
    assert!(
        inbound.contains("mux.send_inbound_stream_payload(&mut client, sid, &payload)")
            && !inbound.contains("mux.send_inbound_stream_data(&mut client, sid, &payload)")
            && !inbound.contains("mux.end_inbound_stream(&mut client, sid)")
            && protocol_mux.contains("if payload.is_empty()")
            && protocol_mux.contains("self.end_inbound_stream(stream, sid).await")
            && protocol_mux.contains("self.send_inbound_stream_data(stream, sid, payload).await"),
        "VLESS inbound mux downstream payload to DATA/END frame selection should live in protocols/vless"
    );
    for private_root_item in [
        "encode_frame",
        "encode_new_stream",
        "encode_new_stream_response",
        "encode_data_frame",
        "encode_udp_data_frame",
        "encode_end_frame",
        "encode_keepalive",
        "parse_new_stream",
        "parse_new_stream_response",
        "parse_udp_target_from_keep",
        "MuxFrame",
        "MuxNetwork",
        "MuxTarget",
        "MuxServerEvent",
        "MUX_FRAME_HEADER_LEN",
        "MUX_MAX_PAYLOAD",
        "MUX_NETWORK_TCP",
        "MUX_NETWORK_UDP",
        "MUX_STATUS_FAIL",
        "MUX_STATUS_OK",
        "MUX_STREAM_NEW",
        "NETWORK_TCP",
        "NETWORK_UDP",
        "OPTION_DATA",
        "STATUS_END",
        "STATUS_KEEP",
        "STATUS_KEEP_ALIVE",
        "STATUS_NEW",
    ] {
        assert!(
            protocol_mux.contains(private_root_item) && !protocol_lib.contains(private_root_item),
            "VLESS low-level mux detail `{private_root_item}` should stay under vless::mux instead of the crate root"
        );
    }
    for private_root_item in [
        "MuxClient",
        "MuxClientStream",
        "MuxServer",
        "VlessInboundMuxAction",
        "VlessInboundMuxSession",
        "VlessInboundMuxWriter",
    ] {
        assert!(
            protocol_mux.contains(private_root_item) && !protocol_lib.contains(private_root_item),
            "VLESS MUX API `{private_root_item}` should stay under vless::mux instead of the crate root"
        );
    }
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
            "src/protocol_registry/mod.rs",
            "src/protocol_registry/registry/mod.rs",
            "src/protocol_registry/registry/metadata.rs",
            "src/protocol_registry/registry/tests/mod.rs",
            "src/protocol_registry/registry/tests/fixtures.rs",
        ],
        &["src/adapters/"],
        "protocol config variant matching should stay inside adapters or protocol-owned inbound entrypoints",
    );
}

#[test]
fn outbound_config_variant_matching_is_confined_to_adapters_and_registry() {
    assert_src_pattern_confined(
        "OutboundProtocolConfig::",
        &[
            "src/protocol_registry/registry/mod.rs",
            "src/protocol_registry/registry/support.rs",
        ],
        &["src/adapters/"],
        "outbound config variant matching should stay inside adapters or protocol registry feature helpers",
    );
}

#[test]
fn direct_udp_helpers_do_not_live_in_outbound_facade() {
    assert!(
        !manifest_dir().join("src/outbound").exists(),
        "src/outbound should not remain as an empty compatibility facade"
    );
    assert!(
        !manifest_dir().join("src/outbound/direct.rs").exists(),
        "src/outbound/direct.rs should not be kept as an empty compatibility facade"
    );

    let helpers = read("src/runtime/udp_helpers.rs");
    let adapter = read("src/adapters/direct/udp.rs");
    assert!(
        helpers.contains("resolve_udp_target") && helpers.contains("send_direct_udp_packet"),
        "direct UDP target resolution and sending should live in runtime::udp_helpers"
    );
    assert!(
        !helpers.contains("outbound/direct.rs"),
        "runtime::udp_helpers should not keep historical references to removed outbound direct facades"
    );
    assert!(
        adapter.contains("resolve_udp_target") && adapter.contains("send_direct_packet"),
        "direct adapter UDP module should call runtime helpers through UdpDispatch"
    );
}

#[test]
fn outbound_protocol_helpers_are_crate_private() {
    let outbound_root = manifest_dir().join("src/outbound");

    assert!(
        !outbound_root.exists(),
        "protocol outbound helpers should live in adapter/protocol-owned modules, not src/outbound"
    );
}

#[test]
fn outbound_root_is_facade_only() {
    assert!(
        !manifest_dir().join("src/outbound/mod.rs").exists(),
        "src/outbound/mod.rs should be deleted once protocol-named outbound glue has moved into adapters"
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
        "src/runtime.rs should dispatch inbound lifecycle through ProtocolRegistry"
    );
}

#[test]
fn runtime_does_not_resolve_inbound_adapter_objects() {
    let listeners = read("src/runtime/listeners.rs");
    let inventory_inbound = read("src/inventory/inbound.rs");

    for forbidden in ["find_inbound", "adapter.spawn_inbound"] {
        assert!(
            !listeners.contains(forbidden),
            "src/runtime/listeners.rs should ask ProtocolInventory to spawn inbounds without resolving adapter objects; found `{forbidden}`"
        );
    }
    assert!(
        inventory_inbound.contains("pub(crate) fn spawn_inbound(")
            && inventory_inbound.contains("self.registry.find_inbound(&inbound.protocol)?")
            && inventory_inbound.contains("InboundListenerCapability::spawn_inbound("),
        "src/inventory/inbound.rs should own inbound adapter resolution and spawn dispatch"
    );
}

#[test]
fn tcp_runtime_does_not_resolve_outbound_adapter_objects() {
    let tcp_dispatch = read("src/runtime/tcp_dispatch.rs");
    let inventory_tcp = read("src/inventory/tcp.rs");

    for forbidden in ["find_outbound_leaf", ".connect_tcp(", ".apply_relay_hop("] {
        assert!(
            !tcp_dispatch.contains(forbidden),
            "src/runtime/tcp_dispatch.rs should ask ProtocolInventory to drive TCP adapters without resolving adapter objects; found `{forbidden}`"
        );
    }
    assert!(
        inventory_tcp.contains("pub(crate) async fn connect_tcp_leaf(")
            && inventory_tcp.contains("TcpOutboundCapability::connect_tcp(")
            && inventory_tcp.contains("pub(crate) async fn apply_tcp_relay_hop(")
            && inventory_tcp.contains("TcpOutboundCapability::apply_relay_hop("),
        "src/inventory/tcp.rs should own TCP outbound adapter resolution and dispatch"
    );
}

#[test]
fn tcp_runtime_does_not_match_protocol_outbound_results() {
    let tcp_dispatch = read("src/runtime/tcp_dispatch.rs");
    let tcp_outbound = read("src/transport/tcp_outbound.rs");

    for forbidden in [
        "EstablishedTcpOutbound::Socks5",
        "EstablishedTcpOutbound::Vless",
        "EstablishedTcpOutbound::Hysteria2",
        "EstablishedTcpOutbound::Shadowsocks",
        "EstablishedTcpOutbound::Trojan",
        "EstablishedTcpOutbound::Vmess",
        "EstablishedTcpOutbound::Mieru",
    ] {
        assert!(
            !tcp_dispatch.contains(forbidden),
            "src/runtime/tcp_dispatch.rs should not unpack protocol TCP outbound variants; found `{forbidden}`"
        );
        assert!(
            tcp_outbound.contains(forbidden),
            "src/transport/tcp_outbound.rs should own TCP outbound result normalization for `{forbidden}`"
        );
    }

    assert!(
        tcp_dispatch.contains(".into_relay_stream()"),
        "src/runtime/tcp_dispatch.rs should ask EstablishedTcpOutbound for a neutral relay stream"
    );
}

#[test]
fn udp_single_hop_runtime_does_not_resolve_outbound_adapter_objects() {
    let udp_start = read("src/runtime/udp_dispatch/start/mod.rs");
    let inventory_udp_leaf = read("src/inventory/udp/leaf.rs");

    for forbidden in ["find_outbound_leaf", ".start_udp_flow("] {
        assert!(
            !udp_start.contains(forbidden),
            "src/runtime/udp_dispatch/start/mod.rs should ask ProtocolInventory to drive single-hop UDP adapters without resolving adapter objects; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_leaf.contains("pub(crate) async fn start_udp_leaf_flow(")
            && inventory_udp_leaf.contains("UdpFlowCapability::start_udp_flow("),
        "src/inventory/udp/leaf.rs should own single-hop UDP adapter resolution and dispatch"
    );
}

#[test]
fn udp_relay_runtime_does_not_resolve_final_hop_adapter_objects() {
    let relay = read("src/runtime/udp_dispatch/start/relay.rs");
    let inventory_udp_relay = read("src/inventory/udp/relay.rs");

    for forbidden in [
        "adapter.udp_relay_needs_two_streams(",
        "adapter.start_udp_relay_two_stream(",
        "adapter.start_udp_relay_final_hop(",
        "find_outbound_leaf(chain.last()",
    ] {
        assert!(
            !relay.contains(forbidden),
            "src/runtime/udp_dispatch/start/relay.rs should ask ProtocolInventory to drive UDP relay final-hop adapters; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_relay.contains("pub(crate) fn udp_relay_needs_two_streams(")
            && inventory_udp_relay.contains("pub(crate) async fn start_udp_relay_two_stream(")
            && inventory_udp_relay.contains("pub(crate) async fn start_udp_relay_final_hop("),
        "src/inventory/udp/relay.rs should own UDP relay final-hop adapter resolution and dispatch"
    );
}

#[test]
fn udp_relay_runtime_does_not_resolve_packet_path_pair_adapters() {
    let relay = read("src/runtime/udp_dispatch/start/relay.rs");
    let inventory_udp_packet_path = read("src/inventory/udp/packet_path.rs");

    for forbidden in [
        "carrier_adapter",
        "datagram_adapter",
        "udp_packet_path_carrier_descriptor(",
        "udp_datagram_source(",
    ] {
        assert!(
            !relay.contains(forbidden),
            "src/runtime/udp_dispatch/start/relay.rs should ask ProtocolInventory to classify packet-path pairs; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_packet_path.contains("pub(crate) fn udp_packet_path_pair")
            && inventory_udp_packet_path
                .contains("UdpPacketPathCapability::udp_packet_path_carrier_descriptor")
            && inventory_udp_packet_path.contains("UdpPacketPathCapability::udp_datagram_source")
            && inventory_udp_packet_path.contains("PacketPathFlowBinding::new"),
        "src/inventory/udp/packet_path.rs should own packet-path pair adapter probing"
    );
    assert!(
        relay.contains("flow_binding")
            && !relay.contains("flow_snapshot")
            && !relay.contains("packet_path_carrier"),
        "UDP relay start should treat packet-path pair output as one neutral binding"
    );
}

#[test]
fn packet_path_dispatch_is_not_feature_gated_by_datagram_protocol() {
    for source in [
        "src/protocol_registry/capability.rs",
        "src/inventory/udp/packet_path.rs",
        "src/runtime/udp_dispatch/start/relay.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains(r#"#[cfg(feature = "shadowsocks")]"#),
            "{source} should expose generic packet-path dispatch independently of the current datagram protocol feature"
        );
    }
}

#[test]
fn packet_path_entry_does_not_resolve_adapter_objects() {
    let entry = read("src/runtime/udp_flow/packet_path_chain/entry.rs");
    let inventory_udp_packet_path = read("src/inventory/udp/packet_path.rs");

    for forbidden in [
        "find_outbound_leaf",
        "carrier_adapter",
        "datagram_adapter",
        "udp_packet_path_carrier_descriptor(",
        "udp_datagram_source(",
        ".build_udp_packet_path(",
    ] {
        assert!(
            !entry.contains(forbidden),
            "packet_path_chain/entry.rs should ask ProtocolInventory to resolve packet-path adapters; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_packet_path.contains("pub(crate) fn resolve_udp_packet_path_candidate")
            && inventory_udp_packet_path
                .contains("pub(crate) async fn build_udp_packet_path_carrier")
            && inventory_udp_packet_path
                .contains("UdpPacketPathCapability::build_udp_packet_path("),
        "src/inventory/udp/packet_path.rs should own packet-path carrier adapter resolution"
    );
}

#[test]
fn mixed_inbound_is_adapter_owned_not_runtime_special_case() {
    let adapter = read("src/adapters/mixed/inbound.rs");
    assert!(
        adapter.contains("run_mixed_listener_with_bound") && adapter.contains("bound.into_tcp()"),
        "MixedAdapter should own mixed listener spawn through the adapter inbound module"
    );

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "InboundProtocolConfig::Mixed",
            "run_mixed_listener_with_bound",
            "MixedAdapter",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not special-case mixed inbound; found `{forbidden}`"
            );
        }
    }
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
fn running_proxy_handle_lives_outside_runtime_root() {
    let runtime = read("src/runtime.rs");
    let running = read("src/runtime/running.rs");

    for forbidden in [
        "pub struct RunningProxy",
        "impl RunningProxy",
        "impl Deref for RunningProxy",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "src/runtime.rs should keep RunningProxy handle details in src/runtime/running.rs; found `{forbidden}`"
        );
        assert!(
            running.contains(forbidden),
            "src/runtime/running.rs should own RunningProxy handle detail `{forbidden}`"
        );
    }

    assert!(
        runtime.contains("mod running;") && runtime.contains("pub use running::RunningProxy;"),
        "src/runtime.rs should expose RunningProxy through the runtime/running.rs module"
    );
}

#[test]
fn runtime_reload_bridge_lives_outside_runtime_root() {
    let runtime = read("src/runtime.rs");
    let reload = read("src/runtime/reload.rs");

    for forbidden in [
        "spawn_blocking",
        "recv_timeout",
        "unbounded_channel",
        "RecvTimeoutError",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "src/runtime.rs should keep reload bridge details in src/runtime/reload.rs; found `{forbidden}`"
        );
        assert!(
            reload.contains(forbidden),
            "src/runtime/reload.rs should own reload bridge detail `{forbidden}`"
        );
    }

    assert!(
        runtime.contains("mod reload;")
            && runtime.contains("reload::subscribe_reload_bridge(self.engine.subscribe_reload())"),
        "src/runtime.rs should subscribe to reloads through runtime/reload.rs"
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
fn inbound_root_is_facade_only() {
    let root = read("src/inbound/mod.rs");

    for expected in [
        "pub(crate) mod direct;",
        "mod http_connect;",
        "pub(crate) mod hysteria2;",
        "pub(crate) mod mieru;",
        "mod mixed;",
        "pub(crate) mod shadowsocks;",
        "mod socks5;",
        "mod system;",
        "pub(crate) mod trojan;",
        "mod tun;",
        "pub(crate) mod vless;",
        "pub(crate) mod vmess;",
        "pub(crate) use direct::run_direct_listener_with_bound;",
        "pub(crate) use http_connect::run_http_connect_listener_with_bound;",
        "pub(crate) use hysteria2::run_hysteria2_listener_with_bound;",
        "pub(crate) use mieru::run_mieru_listener_with_bound;",
        "pub(crate) use mixed::run_mixed_listener_with_bound;",
        "pub(crate) use shadowsocks::run_shadowsocks_listener_with_bound;",
        "pub(crate) use socks5::run_socks5_listener_with_bound;",
        "pub(crate) use trojan::run_trojan_listener_with_bound;",
        "pub(crate) use vless::run_vless_listener_with_bound;",
        "pub(crate) use vmess::run_vmess_listener_with_bound;",
    ] {
        assert!(
            root.contains(expected),
            "src/inbound/mod.rs should expose inbound facade item `{expected}`"
        );
    }

    for line in root.lines().map(str::trim) {
        let allowed = line.is_empty()
            || line.starts_with("#[cfg(")
            || line.starts_with("mod ")
            || line.starts_with("pub(crate) mod ")
            || line.starts_with("pub(crate) use ");
        assert!(
            allowed,
            "src/inbound/mod.rs should only declare inbound modules and re-export listener entrypoints; found `{line}`"
        );
    }

    for forbidden in [
        "async fn",
        "fn ",
        "impl ",
        "match ",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "ProtocolRegistry",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inbound/mod.rs should remain a facade over inbound listener modules; found `{forbidden}`"
        );
    }
}

#[test]
fn resolved_outbound_variant_matching_is_confined_to_adapters_and_registry() {
    assert_src_pattern_confined(
        "ResolvedLeafOutbound::",
        &[
            "src/protocol_registry/mod.rs",
            "src/protocol_registry/registry/mod.rs",
            "src/protocol_registry/registry/outbound.rs",
            "src/protocol_registry/registry/tests/mod.rs",
            "src/protocol_registry/registry/tests/fixtures.rs",
            "src/protocol_registry/registry/tests/outbound.rs",
        ],
        &["src/adapters/"],
        "resolved outbound variant matching should stay inside adapters or protocol registry dispatch helpers",
    );
}

#[test]
fn block_outbound_leaf_is_registry_kernel_exception_not_adapter_protocol() {
    let outbound = read("src/protocol_registry/registry/outbound.rs");
    assert!(
        outbound.contains("ResolvedLeafOutbound::Block")
            && outbound.contains("TcpPathCategory::Block"),
        "ProtocolRegistry outbound dispatch should own the kernel-level Block leaf classification"
    );

    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("ResolvedLeafOutbound::Block")
                && !content.contains("TcpPathCategory::Block"),
            "{source} should not model block as an adapter-owned protocol"
        );
    }

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("ResolvedLeafOutbound::Block"),
            "{source} should get block facts from ProtocolRegistry::outbound_leaf_runtime"
        );
    }
}

#[test]
fn runtime_uses_registry_for_outbound_leaf_runtime_facts() {
    let orchestration = read("src/runtime/orchestration.rs");
    for forbidden in [
        "ResolvedLeafOutbound::",
        "fn health_tag",
        "fn endpoint",
        "fn kernel_leaf_tag",
        "fn tcp_path_category",
    ] {
        assert!(
            !orchestration.contains(forbidden),
            "runtime/orchestration.rs should only define neutral fact types, not classify outbound leaf variants; found `{forbidden}`"
        );
    }

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("ResolvedLeafOutbound::"),
            "{source} should use ProtocolRegistry::outbound_leaf_runtime instead of matching outbound leaf variants"
        );
    }
}

#[test]
fn direct_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/direct.rs");
    let adapter = read("src/adapters/direct/inbound.rs");

    assert!(
        inbound.contains("struct DirectInboundRequest")
            && inbound.contains("request: DirectInboundRequest"),
        "direct inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Direct"),
        "direct inbound entrypoint should not parse direct config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Direct")
            && adapter.contains("DirectInboundRequest"),
        "direct adapter should extract direct config and pass DirectInboundRequest"
    );
}

#[test]
fn socks5_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/socks5.rs");
    let mixed = read("src/inbound/mixed.rs");
    let adapter = read("src/adapters/socks5/inbound.rs");
    let mixed_adapter = read("src/adapters/mixed/inbound.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/socks5/src/inbound.rs"))
        .expect("read socks5 protocol inbound source");

    assert!(
        inbound.contains("struct Socks5InboundRequest")
            && inbound.contains("request: Socks5InboundRequest")
            && adapter.contains("InboundProtocolConfig::Socks5")
            && adapter.contains("Socks5InboundRequest"),
        "SOCKS5 inbound listener should receive an adapter-built request model"
    );
    assert!(
        mixed.contains("struct MixedInboundRequest")
            && mixed.contains("request: MixedInboundRequest")
            && mixed_adapter.contains("InboundProtocolConfig::Mixed")
            && mixed_adapter.contains("MixedInboundRequest"),
        "mixed inbound listener should receive an adapter-built request model"
    );
    assert!(
        protocol_inbound.contains("pub struct ConfiguredSocks5PasswordAuth")
            && protocol_inbound.contains("impl Socks5PasswordAuth for ConfiguredSocks5PasswordAuth")
            && protocol_inbound.contains("pub struct ConfiguredSocks5User")
            && protocol_inbound.contains("pub fn from_config_parts")
            && inbound.contains("auth: socks5::ConfiguredSocks5PasswordAuth")
            && mixed.contains("socks5_auth: socks5::ConfiguredSocks5PasswordAuth")
            && adapter.contains("fn socks5_password_auth_from_users")
            && adapter.contains("socks5::ConfiguredSocks5PasswordAuth::from_config_parts")
            && adapter.contains("socks5_password_auth_from_users(users)")
            && mixed_adapter.contains("use crate::adapters::socks5::socks5_password_auth_from_users")
            && mixed_adapter.contains("socks5_password_auth_from_users(socks5_users)")
            && !mixed_adapter.contains("socks5::ConfiguredSocks5PasswordAuth::from_config_parts")
            && !adapter.contains("socks5::ConfiguredSocks5User::new")
            && !mixed_adapter.contains("socks5::ConfiguredSocks5User::new")
            && !adapter.contains("fn socks5_users_from_config")
            && !mixed_adapter.contains("fn socks5_users_from_config")
            && !inbound.contains("Socks5UserConfig")
            && !mixed.contains("Socks5UserConfig")
            && !inbound.contains("impl Socks5PasswordAuth")
            && !inbound.contains("handler.users")
            && !mixed.contains("handler.users")
            && !mixed.contains("socks5_h.users")
            && !inbound.contains("socks5_users()")
            && !mixed.contains("socks5_users()"),
        "SOCKS5 inbound auth lookup should live in protocols/socks5 while proxy listeners hold only a protocol-owned auth profile"
    );
    assert!(
        inbound.contains(".send_success_response(")
            && inbound.contains(".send_blocked_response(")
            && inbound.contains(".send_upstream_failure_response(")
            && !inbound.contains("Socks5Reply"),
        "SOCKS5 inbound TCP response reply selection should stay behind protocol-owned semantic response methods"
    );
    assert!(
        protocol_inbound.contains("pub async fn send_success_response")
            && protocol_inbound.contains("pub async fn send_blocked_response")
            && protocol_inbound.contains("pub async fn send_upstream_failure_response")
            && protocol_inbound.contains("Socks5Reply::Succeeded")
            && protocol_inbound.contains("Socks5Reply::ConnectionNotAllowed")
            && protocol_inbound.contains("Socks5Reply::HostUnreachable"),
        "protocols/socks5 should own concrete SOCKS5 reply selection for common inbound TCP outcomes"
    );
}

#[test]
fn mieru_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/mieru.rs");
    let adapter = read("src/adapters/mieru/inbound.rs");

    assert!(
        inbound.contains("struct MieruInboundRequest")
            && inbound.contains("request: MieruInboundRequest"),
        "Mieru inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Mieru"),
        "Mieru inbound entrypoint should not parse Mieru config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Mieru") && adapter.contains("MieruInboundRequest"),
        "Mieru adapter should extract Mieru config and pass MieruInboundRequest"
    );
    assert!(
        inbound.contains("pub(crate) profile: MieruInboundProfile")
            && inbound.contains("profile: MieruInboundProfile")
            && inbound.contains(".profile")
            && inbound.contains(".accept_request(&self.mieru_inbound, &mut metered)")
            && inbound.contains("self.profile.inbound_auth()")
            && !inbound.contains("pub(crate) users: Vec<(String, String)>")
            && !inbound.contains("users: Vec<(String, String)>")
            && !inbound.contains("accept_request(&mut metered, &self.users)")
            && adapter.contains("MieruInboundProfile::from_config_parts")
            && !adapter.contains(".collect::<Vec<_>>()")
            && !adapter.contains("MieruInboundProfile::from_config(profile)"),
        "Mieru inbound listener should receive a protocol-owned profile instead of raw user/password tuples"
    );
}

#[test]
fn shadowsocks_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/shadowsocks.rs");
    let udp = read("src/inbound/shadowsocks/udp.rs");
    let adapter = read("src/adapters/shadowsocks/inbound.rs");

    assert!(
        inbound.contains("struct ShadowsocksInboundRequest")
            && inbound.contains("request: ShadowsocksInboundRequest"),
        "Shadowsocks inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Shadowsocks"),
        "Shadowsocks inbound entrypoint should not parse Shadowsocks config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Shadowsocks")
            && adapter.contains("ShadowsocksInboundRequest"),
        "Shadowsocks adapter should extract Shadowsocks config and pass ShadowsocksInboundRequest"
    );
    assert!(
        inbound.contains("pub(crate) profile: ShadowsocksInboundProfile")
            && !inbound.contains("pub(crate) cipher: CipherKind")
            && !inbound.contains("pub(crate) password: String")
            && !inbound.contains("CipherKind::from_str"),
        "Shadowsocks inbound listener should receive a protocol-owned profile, not raw cipher/password"
    );
    assert!(
        inbound.contains("ShadowsocksInboundTcpState")
            && inbound.contains("profile.tcp_state()")
            && inbound.contains("tcp_state.check_accept_replay(&accept)")
            && !inbound.contains("ReplaySaltPool")
            && !inbound.contains("request_salt")
            && !inbound.contains("is_2022()"),
        "Shadowsocks inbound listener should delegate TCP replay state and salt checks to the protocol crate"
    );
    assert!(
        adapter.contains("ShadowsocksInboundProfile::from_config_parts")
            && !adapter.contains("CipherKind::from_str"),
        "Shadowsocks adapter should delegate inbound profile validation to the protocol crate"
    );
    assert!(
        !inbound.contains("#[allow(clippy::too_many_lines)]"),
        "Shadowsocks inbound listener should stay small enough without a too_many_lines allowance"
    );
    assert!(
        !inbound.contains("async fn ss_udp_relay_loop")
            && !inbound.contains("struct SsProtocolResponse"),
        "Shadowsocks UDP relay details should live outside the listener entrypoint"
    );
    assert!(
        udp.contains("async fn ss_udp_relay_loop")
            && !udp.contains("struct SsProtocolResponse")
            && udp.contains(".send_proxy_session_response_to_sender_tokio")
            && !udp.contains("response_datagram_for_proxy_session")
            && !udp.contains("address_from_socket_addr(sender)")
            && udp.contains("UdpPipe::new"),
        "Shadowsocks UDP relay should live in src/inbound/shadowsocks/udp.rs, route through UdpPipe, and delegate response framing to protocols/shadowsocks"
    );
    assert!(
        udp.contains("ShadowsocksInboundProfile")
            && udp.contains("profile.udp_session()")
            && udp.contains("profile.inbound_auth()")
            && !udp.contains("profile.principal_key()")
            && !udp.contains("CipherKind")
            && !udp.contains("password: &str")
            && !udp.contains("ShadowsocksInboundUdpCodec::new"),
        "Shadowsocks UDP relay should delegate protocol-private UDP session/auth construction to the profile"
    );
}

#[test]
fn inbound_auth_identity_stays_in_protocol_crates() {
    let shadowsocks_inbound = read("src/inbound/shadowsocks.rs");
    let shadowsocks_udp = read("src/inbound/shadowsocks/udp.rs");
    let trojan_inbound = read("src/inbound/trojan.rs");
    let mieru_inbound = read("src/inbound/mieru.rs");

    let shadowsocks_protocol =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/inbound.rs"))
            .expect("read shadowsocks protocol inbound source");
    let trojan_protocol = fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
        .expect("read trojan protocol inbound source");
    let mieru_protocol = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");

    for (source_name, source, required) in [
        (
            "src/inbound/shadowsocks.rs",
            shadowsocks_inbound.as_str(),
            "profile.inbound_auth()",
        ),
        (
            "src/inbound/shadowsocks/udp.rs",
            shadowsocks_udp.as_str(),
            "profile.inbound_auth()",
        ),
        (
            "src/inbound/trojan.rs",
            trojan_inbound.as_str(),
            "self.profile.inbound_auth()",
        ),
        (
            "src/inbound/mieru.rs",
            mieru_inbound.as_str(),
            "self.profile.inbound_auth()",
        ),
    ] {
        assert!(
            source.contains(required)
                && !source.contains("SessionAuth::new(\"shadowsocks\")")
                && !source.contains("SessionAuth::new(\"trojan\")")
                && !source.contains("SessionAuth::new(\"mieru\")")
                && !source.contains("principal_key = Some"),
            "{source_name} should apply protocol-built inbound auth instead of constructing protocol identity in proxy"
        );
    }

    assert!(
        shadowsocks_protocol.contains("pub fn inbound_auth(&self) -> SessionAuth")
            && trojan_protocol.contains("pub fn inbound_auth(&self) -> SessionAuth")
            && mieru_protocol.contains("pub fn inbound_auth(&self) -> SessionAuth"),
        "protocol crates should own their inbound auth identity construction"
    );
}

#[test]
fn stream_udp_inbound_direct_response_target_conversion_is_protocol_owned() {
    let trojan_inbound = read("src/inbound/trojan.rs");
    let mieru_inbound = read("src/inbound/mieru.rs");
    let hysteria2_inbound = read("src/inbound/hysteria2.rs");
    let vless_udp_inbound = read("src/inbound/vless/udp_session.rs");
    let vless_mux_inbound = read("src/inbound/vless/mux.rs");
    let vmess_mux_inbound = read("src/inbound/vmess/mux.rs");
    let trojan_protocol = fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
        .expect("read trojan protocol inbound source");
    let mieru_protocol = fs::read_to_string(repo_root().join("protocols/mieru/src/udp.rs"))
        .expect("read mieru protocol udp source");
    let hysteria2_protocol = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let vless_protocol = fs::read_to_string(repo_root().join("protocols/vless/src/shared.rs"))
        .expect("read vless protocol shared source");
    let vmess_protocol = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read vmess protocol udp source");

    assert!(
        trojan_inbound.contains("write_response_to_socket_addr_tokio")
            && !trojan_inbound.contains("socket_addr_to_ip(sender)")
            && trojan_protocol.contains("pub async fn write_response_to_socket_addr_tokio")
            && trojan_protocol.contains("fn address_from_socket_addr"),
        "Trojan inbound UDP direct response target conversion should live behind the protocol UDP session"
    );
    assert!(
        mieru_inbound.contains("write_response_for_sender_tokio")
            && !mieru_inbound.contains("address_from_socket_addr(sender)")
            && mieru_protocol.contains("pub async fn write_response_for_sender_tokio")
            && mieru_protocol.contains("fn address_from_socket_addr"),
        "Mieru inbound UDP direct response target conversion should live behind the protocol UDP session"
    );
    assert!(
        hysteria2_inbound.contains("send_response_to_socket_addr")
            && !hysteria2_inbound.contains("socket_addr_to_ip(sender)")
            && hysteria2_protocol.contains("pub fn send_response_to_socket_addr")
            && hysteria2_protocol.contains("fn address_from_socket_addr"),
        "Hysteria2 inbound UDP direct response target conversion should live behind the protocol UDP session"
    );
    assert!(
        vless_udp_inbound.contains("write_response_to_socket_addr_tokio")
            && vless_mux_inbound.contains("send_mux_response_to_socket_addr")
            && !vless_udp_inbound.contains("socket_addr_to_ip(sender)")
            && !vless_mux_inbound.contains("socket_addr_to_ip(sender)")
            && vless_protocol.contains("pub async fn write_response_to_socket_addr_tokio")
            && vless_protocol.contains("pub fn send_mux_response_to_socket_addr")
            && vless_protocol.contains("fn address_from_socket_addr"),
        "VLESS inbound UDP direct response target conversion should live behind the protocol UDP session"
    );
    assert!(
        vmess_mux_inbound.contains("write_response_to_socket_addr_tokio")
            && vmess_mux_inbound.contains("write_mux_response_to_socket_addr")
            && !vmess_mux_inbound.contains("socket_addr_to_ip(sender)")
            && vmess_protocol.contains("pub async fn write_response_to_socket_addr_tokio")
            && vmess_protocol.contains("pub fn write_mux_response_to_socket_addr")
            && vmess_protocol.contains("fn address_from_socket_addr"),
        "VMess inbound UDP direct response target conversion should live behind the protocol UDP session"
    );
}

#[test]
fn trojan_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/trojan.rs");
    let adapter = read("src/adapters/trojan/inbound.rs");

    assert!(
        inbound.contains("struct TrojanInboundRequest")
            && inbound.contains("request: TrojanInboundRequest"),
        "Trojan inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Trojan"),
        "Trojan inbound entrypoint should not parse Trojan config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Trojan")
            && adapter.contains("TrojanInboundRequest"),
        "Trojan adapter should extract Trojan config and pass TrojanInboundRequest"
    );
    assert!(
        inbound.contains("pub(crate) profile: TrojanInboundProfile")
            && inbound.contains("pub(crate) tls_acceptor: crate::transport::TlsAcceptor")
            && inbound.contains("profile: TrojanInboundProfile")
            && inbound.contains("self.profile.accept(self.trojan_inbound, &mut sock)")
            && inbound.contains("self.profile.inbound_auth()")
            && !inbound.contains("pub(crate) password: String")
            && !inbound.contains("password: String")
            && !inbound.contains("pub(crate) tls: Option<zero_config::TlsConfig>")
            && !inbound.contains("build_tls_acceptor")
            && !inbound.contains("zero_config::TlsConfig")
            && !inbound.contains("std::slice::from_ref(&self.password)")
            && adapter.contains("TrojanInboundProfile::from_config_parts")
            && adapter.contains("crate::transport::build_tls_acceptor")
            && adapter.contains("tls_acceptor")
            && !adapter.contains("password.clone(), tls.clone()"),
        "Trojan inbound listener should receive protocol-owned profile plus adapter-built TLS acceptor instead of raw password/TLS config"
    );
}

#[test]
fn vmess_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/vmess/listener.rs");
    let model = read("src/inbound/vmess/model.rs");
    let root = read("src/inbound/vmess/mod.rs");
    let transport = read("src/inbound/vmess/transport.rs");
    let adapter = read("src/adapters/vmess/inbound.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vmess/src/inbound.rs"))
        .expect("read vmess protocol inbound source");

    assert!(
        model.contains("struct VmessInboundRequest")
            && inbound.contains("request: VmessInboundRequest"),
        "VMess inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Vmess")
            && !root.contains("InboundProtocolConfig::Vmess"),
        "VMess inbound entrypoint should not parse VMess config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Vmess") && adapter.contains("VmessInboundRequest"),
        "VMess adapter should extract VMess config and pass VmessInboundRequest"
    );
    for forbidden in [
        "parse_uuid",
        "VmessCipher::from_name",
        "vmess unknown cipher",
    ] {
        assert!(
            !inbound.contains(forbidden) && !model.contains(forbidden),
            "VMess inbound listener/model should receive adapter-parsed users; found `{forbidden}`"
        );
        assert!(
            !adapter.contains(forbidden) && protocol_inbound.contains(forbidden),
            "VMess user parsing detail `{forbidden}` should live in protocols/vmess"
        );
    }
    assert!(
        adapter.contains("vmess::VmessInboundProfile::from_config_parts")
            && !adapter.contains("vmess::VmessUser::from_config")
            && protocol_inbound.contains("pub fn from_config")
            && protocol_inbound.contains("pub fn from_config_parts"),
        "VMess adapter should ask protocols/vmess to build parsed inbound profiles"
    );
    assert!(
        protocol_inbound.contains("pub struct VmessInboundProfile")
            && protocol_inbound.contains("pub fn from_users(users: Vec<VmessUser>) -> Self")
            && protocol_inbound.contains("pub async fn accept_tcp<S: AsyncSocket>")
            && model.contains("pub(crate) profile: vmess::VmessInboundProfile")
            && model.contains("pub(crate) tls_acceptor: crate::transport::TlsAcceptor")
            && root.contains("profile: VmessInboundProfile")
            && root.contains(".profile")
            && root.contains(".accept_tcp(self.vmess_inbound, &mut sock)")
            && inbound.contains("profile.is_empty()")
            && !model.contains("tls: Option<Box<zero_config::TlsConfig>>")
            && !inbound.contains("build_tls_acceptor")
            && !inbound.contains("zero_config::TlsConfig")
            && adapter.contains("crate::transport::build_tls_acceptor")
            && adapter.contains("tls_acceptor")
            && !adapter.contains("vmess::VmessInboundProfile::from_users")
            && !model.contains("users: Vec<vmess::VmessUser>")
            && !root.contains("users: Vec<VmessUser>")
            && !root.contains("handler.users")
            && !transport.contains("handler.users"),
        "VMess inbound should carry a protocol-owned profile instead of proxy-owned user vectors"
    );
    assert!(
        !inbound.contains("VmessUserConfig") && !model.contains("VmessUserConfig"),
        "VMess inbound listener/model should not carry raw config user records"
    );
}

#[test]
fn vless_inbound_users_are_adapter_parsed() {
    let listener = read("src/inbound/vless/listener.rs");
    let model = read("src/inbound/vless/model.rs");
    let session = read("src/inbound/vless/session.rs");
    let helpers = read("src/inbound/vless/helpers.rs");
    let adapter = read("src/adapters/vless/inbound.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read vless protocol inbound source");

    for forbidden in [
        "VlessUserConfig",
        "parse_uuid",
        "parse_flow",
        "vless_users()",
        "vless_reality()",
        "vless_tls()",
        "vless_ws()",
        "vless_grpc()",
        "vless_h2()",
        "vless_http_upgrade()",
        "vless_split_http()",
        "vless_fallback()",
        "InboundRealityConfig",
        "RealityServerOptions",
        "private_key:",
        "short_ids:",
        "cipher_suites:",
    ] {
        assert!(
            !listener.contains(forbidden)
                && !session.contains(forbidden)
                && !helpers.contains(forbidden),
            "VLESS inbound listener/session/helpers should receive adapter-parsed protocol values; found `{forbidden}`"
        );
    }
    for required in [
        "parse_inbound_profile",
        "VlessInboundProfile::from_config_parts",
    ] {
        assert!(
            adapter.contains(required),
            "VLESS adapter inbound module should ask protocols/vless for parsed users through `{required}`"
        );
    }
    for private_detail in ["parse_uuid", "parse_flow"] {
        assert!(
            !adapter.contains(private_detail) && protocol_inbound.contains(private_detail),
            "VLESS user construction detail `{private_detail}` should live in protocols/vless"
        );
    }
    assert!(
        !adapter.contains("vless::VlessUser {")
            && !adapter.contains("VlessConfiguredUser::from_config")
            && !adapter.contains("VlessInboundProfile::from_users")
            && protocol_inbound.contains("pub fn from_config")
            && protocol_inbound.contains("pub fn from_config_parts")
            && protocol_inbound.contains("VlessUser::from_config"),
        "VLESS adapter should not hand-build protocol users"
    );
    assert!(
        !helpers.contains("ConfiguredVlessUser")
            && !helpers.contains("VlessUserStore")
            && !listener.contains("VlessConfiguredUser")
            && !session.contains("VlessConfiguredUser")
            && !session.contains("VlessConfiguredUsers::new")
            && model.contains("profile: vless::VlessInboundProfile")
            && session.contains("profile: vless::VlessInboundProfile")
            && session.contains(".accept_tcp_with_auth_and_id(vless::VlessInbound, &mut metered)")
            && protocol_inbound.contains("pub struct VlessInboundProfile")
            && protocol_inbound
                .contains("pub fn from_users(users: Vec<VlessConfiguredUser>) -> Self")
            && protocol_inbound.contains("let auth = VlessConfiguredUsers::new(&self.users)")
            && protocol_inbound.contains("pub struct VlessConfiguredUsers")
            && protocol_inbound.contains("impl VlessUserStore for VlessConfiguredUsers")
            && protocol_inbound.contains("user.user.clone()"),
        "VLESS user store should live in protocols/vless, not proxy inbound helpers"
    );
    assert!(
        model.contains("struct VlessInboundRequest")
            && model.contains("reality: Option<vless::VlessRealityServerProfile>")
            && listener.contains("request: VlessInboundRequest")
            && listener.contains("let VlessInboundRequest")
            && model.contains("tls_acceptor: Option<crate::transport::TlsAcceptor>")
            && model.contains("ws: Option<Box<zero_config::WebSocketConfig>>")
            && model.contains("grpc: Option<Box<zero_config::GrpcConfig>>")
            && model.contains("h2: Option<Box<zero_config::H2Config>>")
            && model.contains("http_upgrade: Option<Box<zero_config::HttpUpgradeConfig>>")
            && model.contains("split_http: Option<Box<zero_config::SplitHttpConfig>>")
            && model.contains("fallback: Option<Box<zero_config::FallbackConfig>>")
            && adapter.contains("parse_transport_config")
            && adapter.contains("parse_reality_profile")
            && adapter.contains("crate::transport::build_tls_acceptor")
            && adapter.contains("tls_acceptor")
            && adapter.contains("VlessRealityServerProfile::from_config_parts")
            && !adapter.contains("VlessRealityServerProfile::new")
            && !listener.contains("build_tls_acceptor")
            && !listener.contains("zero_config::TlsConfig")
            && protocol_inbound.contains("pub struct VlessInboundProfile"),
        "VLESS inbound request model should live in inbound/vless/model.rs"
    );
    let transport_split_http =
        fs::read_to_string(repo_root().join("crates/transport/src/split_http.rs"))
            .expect("read transport split_http source");
    assert!(
        session.contains("crate::transport::accept_xhttp_inbound")
            && !session.contains("XhttpMode::parse")
            && !session.contains("accept_xhttp_stream_one")
            && !session.contains("accept_split_http")
            && transport_split_http.contains("pub async fn accept_xhttp_inbound")
            && transport_split_http.contains("XhttpMode::parse(&config.mode)"),
        "VLESS inbound session glue should delegate XHTTP mode selection to the transport layer"
    );
    let protocol_reality =
        fs::read_to_string(repo_root().join("protocols/vless/src/reality/stream.rs"))
            .expect("read vless reality stream source");
    assert!(
        helpers.contains("profile.upgrade_server(stream).await")
            && protocol_reality.contains("pub struct VlessRealityServerProfile")
            && protocol_reality.contains("pub fn from_config_parts")
            && protocol_reality.contains("pub async fn upgrade_server")
            && protocol_reality.contains("RealityServerOptions")
            && protocol_reality.contains("private_key: &self.private_key")
            && protocol_reality.contains("short_ids: &self.short_ids")
            && protocol_reality.contains("cipher_suites: &self.cipher_suites"),
        "VLESS Reality server option construction should live in protocols/vless"
    );
}

#[test]
fn hysteria2_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/hysteria2.rs");
    let adapter = read("src/adapters/hysteria2/inbound.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let protocol_dispatch_parts = struct_block(&protocol_udp, "Hysteria2InboundUdpDispatchParts");
    let protocol_inbound =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/inbound.rs"))
            .expect("read hysteria2 protocol inbound source");
    let protocol_shared = fs::read_to_string(repo_root().join("protocols/hysteria2/src/shared.rs"))
        .expect("read hysteria2 protocol shared source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/hysteria2/src/lib.rs"))
        .expect("read hysteria2 protocol lib source");

    assert!(
        inbound.contains("struct Hysteria2InboundRequest")
            && inbound.contains("request: Hysteria2InboundRequest"),
        "Hysteria2 inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Hysteria2"),
        "Hysteria2 inbound entrypoint should not parse Hysteria2 config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Hysteria2")
            && adapter.contains("Hysteria2InboundRequest"),
        "Hysteria2 adapter should extract Hysteria2 config and pass Hysteria2InboundRequest"
    );
    assert!(
        inbound.contains("pub(crate) profile: Hysteria2InboundProfile")
            && !inbound.contains("pub(crate) up_bps: Option<u64>")
            && !inbound.contains("pub(crate) down_bps: Option<u64>")
            && !inbound.contains("pub(crate) password: String")
            && !adapter.contains("up_bps")
            && !adapter.contains("down_bps")
            && adapter.contains("Hysteria2InboundProfile::from_config_parts"),
        "Hysteria2 inbound listener should receive only protocol-owned profile data, not raw password or unused rate-limit config"
    );
    for forbidden in [
        "parse_auth_frame",
        "verify_hmac",
        "authenticate_client(&salt",
        "auth_error_response",
        "auth_ok_response",
        "export_keying_material",
        "b\"hysteria2 auth\"",
        "build_auth_error",
        "build_auth_ok",
        "build_connect_error",
        "build_connect_ok",
        "parse_tcp_connect_header",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "Hysteria2 inbound should delegate private auth/connect framing to the protocol crate; found `{forbidden}`"
        );
    }
    assert!(
        inbound.contains("profile\n            .authenticate_quic_connection(&conn, &mut auth_stream)")
            && inbound.contains("Hysteria2Inbound.accept_tcp_stream(&mut stream).await")
            && inbound.contains(".send_connect_ok(client)")
            && inbound.contains(".send_connect_error(client, \"blocked\")")
            && inbound.contains(".send_connect_error(client, \"outbound failed\")")
            && !inbound.contains("connect_ok_response()")
            && !inbound.contains("connect_error_response(")
            && !inbound.contains("AsyncSocket::write_all")
            && protocol_inbound.contains("pub async fn authenticate_quic_connection")
            && protocol_inbound.contains("conn.export_keying_material")
            && protocol_inbound.contains("pub async fn authenticate_connection")
            && protocol_inbound.contains("pub async fn accept_tcp_stream")
            && protocol_inbound.contains("pub async fn send_connect_ok")
            && protocol_inbound.contains("pub async fn send_connect_error")
            && protocol_inbound.contains("self.authenticate_client(salt")
            && protocol_inbound.contains("self.auth_error_response")
            && protocol_inbound.contains("self.auth_ok_response")
            && protocol_inbound.contains("self.connect_ok_response()")
            && protocol_inbound.contains("self.connect_error_response(message)")
            && protocol_inbound.contains("self.accept_tcp_connect_header(&header_buf[..n])"),
        "Hysteria2 protocol crate should own auth stream and TCP connect header IO while proxy only orchestrates QUIC tasks"
    );
    for private_helper in [
        "build_auth_error",
        "build_auth_frame",
        "build_auth_ok",
        "build_connect_error",
        "build_connect_ok",
        "build_tcp_connect_header",
        "parse_auth_frame",
        "parse_auth_response",
        "parse_tcp_connect_header",
        "derive_salt",
        "sign_hmac",
        "verify_hmac",
    ] {
        assert!(
            protocol_shared.contains(&format!("pub fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "Hysteria2 low-level shared helper `{private_helper}` should stay off the crate root"
        );
    }
    for forbidden in [
        "build_udp_datagram",
        "parse_udp_datagram",
        "hysteria2::build_udp_datagram",
        "hysteria2::parse_udp_datagram",
        "hysteria2::decode_inbound_udp_datagram",
        "hysteria2::encode_inbound_udp_datagram",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "Hysteria2 inbound should use inbound-specific protocol datagram helpers instead of `{forbidden}`"
        );
    }
    assert!(
        inbound.contains("hysteria2::Hysteria2Inbound.udp_session()")
            && inbound.contains("udp_session.read_dispatch_parts_from_datagram")
            && !inbound.contains("udp_session.read_dispatch_view_from_datagram")
            && !inbound.contains("conn.read_datagram")
            && !inbound.contains("udp_session.decode_dispatch_view")
            && !inbound.contains("view.pipe_parts()")
            && !inbound.contains("view.clone().into_pipe_parts()")
            && !inbound.contains("udp_session.record_proxy_session_for_view")
            && !inbound.contains("parts.request_session_id()")
            && !inbound.contains("request_session_id")
            && inbound.contains("udp_session.record_proxy_session_for_parts(*sid, &parts)")
            && inbound.contains("udp_session.send_response")
            && inbound.contains("udp_session.send_response_to_socket_addr")
            && !inbound.contains("request.into_dispatch_parts()")
            && !inbound.contains("request.request_session_id")
            && !inbound.contains("request.client_session_id")
            && !inbound.contains("parts.target")
            && !inbound.contains("parts.port")
            && !inbound.contains("parts.payload")
            && !inbound.contains("parts.client_session_id")
            && inbound.contains("parts.pipe_parts()")
            && !inbound.contains("parts.into_pipe_parts()")
            && inbound.contains("UdpDispatch::new(&inbound_tag)")
            && inbound.contains("dispatch.poll_refs()")
            && inbound.contains("upstream_udp.recv_response")
            && inbound.contains("wait_for_upstream_idle(socks5_idle)")
            && !inbound.contains("tokio::net::UdpSocket::bind")
            && !inbound.contains("failed to bind UDP socket")
            && !inbound.contains("resolver: Arc<zero_dns::DnsSystem>")
            && !inbound.contains("client_session_id: None")
            && !inbound.contains("request.target().clone()")
            && !inbound.contains("request.payload()")
            && !inbound.contains("Address::Ipv4")
            && !inbound.contains("Address::Ipv6")
            && !inbound.contains("Hysteria2InboundUdpCodec")
            && !inbound.contains("decode_datagram")
            && !inbound.contains("send_datagram")
            && !inbound.contains("h2_flows")
            && !inbound.contains("Hysteria2InboundUdpCodec.encode_datagram")
            && !inbound.contains("conn.send_datagram")
            && protocol_udp.contains("struct Hysteria2InboundUdpCodec")
            && protocol_udp.contains("struct Hysteria2InboundUdpSession")
            && protocol_udp.contains("struct Hysteria2InboundUdpRequest")
            && protocol_udp.contains("struct Hysteria2InboundUdpDispatchParts")
            && !protocol_dispatch_parts.contains("pub request_session_id: u16")
            && !protocol_dispatch_parts.contains("pub target: Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>")
            && protocol_udp.contains("fn into_parts")
            && protocol_udp.contains("fn into_dispatch_parts")
            && protocol_udp.contains("fn pipe_parts")
            && protocol_udp.contains("fn into_pipe_parts")
            && protocol_udp.contains("fn session_id")
            && protocol_udp.contains("fn decode_request")
            && protocol_udp.contains("fn decode_dispatch_parts")
            && protocol_udp.contains("fn read_dispatch_parts_from_datagram")
            && protocol_udp.contains("fn record_proxy_session")
            && protocol_udp.contains("fn record_proxy_session_for_parts")
            && protocol_udp.contains("fn send_response")
            && protocol_udp.contains("fn send_response_to_ip")
            && protocol_udp.contains("fn send_response_to_socket_addr")
            && protocol_udp.contains("fn decode_datagram")
            && protocol_udp.contains("fn encode_datagram")
            && protocol_udp.contains("fn send_datagram"),
        "Hysteria2 inbound should delegate UDP datagram state and framing through the protocol-owned inbound UDP session"
    );
    for private_helper in ["decode_inbound_udp_datagram", "encode_inbound_udp_datagram"] {
        assert!(
            !protocol_udp.contains(&format!("pub fn {private_helper}"))
                && protocol_udp.contains(&format!("fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "Hysteria2 inbound UDP helper `{private_helper}` should stay private to protocols/hysteria2::udp and should not be re-exported"
        );
    }
}

#[test]
fn inbound_root_does_not_reexport_protocol_request_models() {
    let root = read("src/inbound/mod.rs");

    for forbidden in [
        "DirectInboundRequest",
        "Hysteria2InboundRequest",
        "MieruInboundRequest",
        "ShadowsocksInboundRequest",
        "TrojanInboundRequest",
        "ConfiguredVlessUser",
        "VlessInboundRequest",
        "VmessInboundRequest",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inbound/mod.rs should expose listener entrypoints, not protocol request model `{forbidden}`"
        );
    }
}

#[test]
fn protocol_inbound_roots_do_not_define_request_models() {
    for (root, model, request) in [
        (
            "src/inbound/vless/mod.rs",
            "src/inbound/vless/model.rs",
            "VlessInboundRequest",
        ),
        (
            "src/inbound/vmess/mod.rs",
            "src/inbound/vmess/model.rs",
            "VmessInboundRequest",
        ),
    ] {
        let root_content = read(root);
        let model_content = read(model);
        assert!(
            !root_content.contains(&format!("struct {request}")),
            "{root} should not define protocol request model `{request}`"
        );
        assert!(
            model_content.contains(&format!("struct {request}")),
            "{model} should own protocol request model `{request}`"
        );
    }
}

#[test]
fn vless_inbound_root_does_not_reexport_session_models() {
    let root = read("src/inbound/vless/mod.rs");
    let listener = read("src/inbound/vless/listener.rs");

    for forbidden in ["VlessStreamRequest", "VlessStreamTransport"] {
        assert!(
            !root.contains(forbidden),
            "src/inbound/vless/mod.rs should expose listener entrypoints, not session model `{forbidden}`"
        );
        assert!(
            listener.contains("use super::session::{VlessStreamRequest, VlessStreamTransport};"),
            "VLESS listener should import session models from the session module"
        );
    }
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
                "hysteria2_packet_path_carrier_descriptor",
                "build_hysteria2_packet_path",
                "Hysteria2DatagramSend",
                "UdpFlowOutbound::Hysteria2",
            ],
        ),
        (
            "mieru",
            &[
                "MieruDatagramSend",
                "MieruRelaySend",
                "send_mieru_",
                "UdpFlowOutbound::Mieru",
            ],
        ),
        (
            "shadowsocks",
            &[
                "shadowsocks_packet_path_carrier_descriptor",
                "build_shadowsocks_packet_path",
                "shadowsocks_udp_datagram_source",
                "ShadowsocksDatagramSend",
                "send_shadowsocks_datagram",
                "UdpFlowOutbound::Shadowsocks",
            ],
        ),
        (
            "socks5",
            &[
                "socks5_packet_path_carrier_descriptor",
                "build_socks5_packet_path",
                "Socks5UdpSend",
                "UdpFlowOutbound::Socks5",
            ],
        ),
        (
            "trojan",
            &[
                "TrojanDatagramSend",
                "TrojanRelaySend",
                "send_trojan_",
                "UdpFlowOutbound::Trojan",
            ],
        ),
        (
            "vless",
            &[
                "VlessDatagramSend",
                "VlessRelayFinalHopSend",
                "VlessRelayTwoStreamSend",
                "open_udp_stream",
                "encode_udp_packet",
                "dispatch_tcp_relay_prefix",
                "send_vless_",
            ],
        ),
        (
            "vmess",
            &["VmessDatagramSend", "VmessRelaySend", "send_vmess_"],
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
        "TrojanUdpRelayFlowRequest",
    ] {
        assert!(
            !adapters.contains(forbidden),
            "src/adapters/mod.rs should not globally import protocol UDP request type `{forbidden}`"
        );
    }
}

#[test]
fn shadowsocks_udp_root_delegates_packet_path_and_flow_building() {
    let root = read("src/adapters/shadowsocks/udp.rs");
    let packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let flow = read("src/adapters/shadowsocks/udp/flow.rs");

    for required in ["mod packet_path;", "mod flow;"] {
        assert!(
            root.contains(required),
            "src/adapters/shadowsocks/udp.rs should wire `{required}` as protocol-local UDP glue"
        );
    }
    for forbidden in [
        "ShadowsocksUdpFlowConfig::new",
        "packet_path.cache_key()",
        "packet_path.codec()",
        ".packet_path_cache_key()",
        ".packet_path_codec()",
        "ManagedUdpSend {",
        "ManagedUdpFlowResume::new",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/adapters/shadowsocks/udp.rs should be a UDP capability facade and not own `{forbidden}`"
        );
    }
    assert!(
        packet_path.contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !packet_path.contains("ShadowsocksUdpFlowConfig::new")
            && !packet_path.contains(".packet_path_spec()")
            && packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !packet_path.contains("descriptor.cache_key()")
            && !packet_path.contains("descriptor.server()")
            && !packet_path.contains("descriptor.port()")
            && packet_path.contains("udp_packet_path_datagram_source_build_from_config")
            && packet_path.contains("udp_datagram_source_from_build")
            && !packet_path.contains("spec.datagram_source_parts()")
            && !packet_path.contains("datagram.into_parts()")
            && !packet_path.contains("datagram.cache_key()")
            && !packet_path.contains("datagram.codec()")
            && !packet_path.contains("datagram.tag()")
            && !packet_path.contains("datagram.server()")
            && !packet_path.contains("datagram.port()")
            && !packet_path.contains("spec.carrier()")
            && !packet_path.contains("spec.datagram_source()")
            && !packet_path.contains("spec.cache_key()")
            && !packet_path.contains("spec.carrier_cache_key()")
            && !packet_path.contains("spec.datagram_cache_key()")
            && !packet_path.contains("spec.codec()")
            && !packet_path.contains(".packet_path_cache_key()")
            && !packet_path.contains(".packet_path_codec()")
            && flow.contains("shadowsocks::udp::udp_flow_resume_from_config")
            && !flow.contains("ShadowsocksUdpFlowConfig::new")
            && !flow.contains(".flow_resume()")
            && flow.contains("ManagedDatagramStart")
            && flow.contains(".start_tracked_managed_datagram(")
            && !flow.contains("ManagedUdpSend {")
            && !flow.contains("ManagedUdpFlowResume::new"),
        "Shadowsocks packet-path and managed-flow construction should live in explicit protocol-local UDP submodules"
    );
}

#[test]
fn hysteria2_udp_root_delegates_packet_path_and_flow_building() {
    let root = read("src/adapters/hysteria2/udp.rs");
    let packet_path = read("src/adapters/hysteria2/udp/packet_path.rs");
    let flow = read("src/adapters/hysteria2/udp/flow.rs");

    for required in ["mod packet_path;", "mod flow;"] {
        assert!(
            root.contains(required),
            "src/adapters/hysteria2/udp.rs should wire `{required}` as protocol-local UDP glue"
        );
    }
    for forbidden in [
        "Hysteria2UdpFlowConfig::new",
        "packet_path.cache_key()",
        "packet_path.codec()",
        ".packet_path_cache_key()",
        ".packet_path_codec()",
        "ManagedUdpSend {",
        "ManagedUdpFlowResume::new",
        "open_udp_packet_path_connection",
        "open_udp_packet_path_build",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/adapters/hysteria2/udp.rs should be a UDP capability facade and not own `{forbidden}`"
        );
    }
    assert!(
        packet_path.contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !packet_path.contains("Hysteria2UdpFlowConfig::new")
            && !packet_path.contains(".packet_path_spec()")
            && packet_path.contains("udp_packet_path_carrier_build_from_config")
            && packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !packet_path.contains("descriptor.cache_key()")
            && !packet_path.contains("descriptor.server()")
            && !packet_path.contains("descriptor.port()")
            && !packet_path.contains("spec.carrier()")
            && !packet_path.contains("spec.cache_key()")
            && !packet_path.contains("spec.carrier_cache_key()")
            && !packet_path.contains("spec.codec()")
            && !packet_path.contains("build.server()")
            && !packet_path.contains("build.port()")
            && !packet_path.contains("build.connector_profile()")
            && !packet_path.contains("build.codec()")
            && !packet_path.contains(".packet_path_cache_key()")
            && !packet_path.contains(".packet_path_codec()")
            && packet_path.contains("connector::open_udp_packet_path_build")
            && flow.contains("hysteria2::udp::udp_flow_resume_from_config")
            && !flow.contains("Hysteria2UdpFlowConfig::new")
            && !flow.contains(".flow_resume()")
            && flow.contains("ManagedDatagramStart")
            && flow.contains(".start_tracked_managed_datagram(")
            && !flow.contains("ManagedUdpSend {")
            && !flow.contains("ManagedUdpFlowResume::new"),
        "Hysteria2 packet-path and managed-flow construction should live in explicit protocol-local UDP submodules"
    );
}

#[test]
fn stream_udp_roots_delegate_flow_building() {
    for (root_path, flow_path, config, start_bridge) in [
        (
            "src/adapters/trojan/udp.rs",
            "src/adapters/trojan/udp/flow.rs",
            "trojan::udp::udp_flow_resume_from_config",
            ".start_tracked_managed_stream_packet(",
        ),
        (
            "src/adapters/mieru/udp.rs",
            "src/adapters/mieru/udp/flow.rs",
            "mieru::udp::udp_flow_resume_from_config",
            ".start_tracked_managed_stream_packet(",
        ),
        (
            "src/adapters/vless/udp.rs",
            "src/adapters/vless/udp/flow.rs",
            "vless::udp::udp_flow_config_from_config",
            "register_managed_stream_packet_flow",
        ),
        (
            "src/adapters/vmess/udp.rs",
            "src/adapters/vmess/udp/flow.rs",
            "vmess::udp::udp_flow_config_from_config",
            "register_managed_stream_packet_flow",
        ),
    ] {
        let root = read(root_path);
        let flow = read(flow_path);

        assert!(
            root.contains("mod flow;"),
            "{root_path} should wire flow as protocol-local UDP glue"
        );
        for forbidden in [
            ".flow_resume(false)",
            ".flow_resume(true)",
            "ManagedUdpSend {",
            "ManagedUdpFlowResume::new",
            "register_managed_stream_packet_flow",
            "VlessUdpOutboundManager::new",
            "VmessUdpOutboundManager::new",
            "VlessUdpStartFlow {",
            "VmessUdpStartFlow {",
            "VlessUdpRelayFinalHopStart {",
            "VmessUdpRelayFlowStart {",
            "VlessUdpFlowConfig::new",
            "VmessUdpFlowConfig::new",
        ] {
            assert!(
                !root.contains(forbidden),
                "{root_path} should be a UDP capability facade and not own `{forbidden}`"
            );
        }
        assert!(
            flow.contains(config)
                && flow.contains(start_bridge)
                && !flow.contains("ManagedUdpSend {")
                && !flow.contains("ManagedUdpFlowResume::new"),
            "{flow_path} should own stream UDP flow and relay-final-hop resume construction"
        );
    }
}

#[test]
fn socks5_udp_root_delegates_packet_path_and_flow_building() {
    let root = read("src/adapters/socks5/udp.rs");
    let packet_path = read("src/adapters/socks5/udp/packet_path.rs");
    let flow = read("src/adapters/socks5/udp/flow.rs");

    for required in ["mod packet_path;", "mod flow;"] {
        assert!(
            root.contains(required),
            "src/adapters/socks5/udp.rs should wire `{required}` as protocol-local UDP glue"
        );
    }
    for forbidden in [
        "Socks5UdpFlowConfig::new",
        "packet_path.cache_key()",
        ".packet_path_cache_key()",
        ".packet_path_association_config()",
        "ManagedUdpSend {",
        "ManagedUdpFlowResume::new",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/adapters/socks5/udp.rs should be a UDP capability facade and not own `{forbidden}`"
        );
    }
    assert!(
        packet_path.contains("socks5::udp::udp_packet_path_carrier_descriptor_from_config")
            && !packet_path.contains("Socks5UdpFlowConfig::new")
            && !packet_path.contains("packet_path.cache_key()")
            && !packet_path.contains(".packet_path_spec()")
            && packet_path.contains("udp_packet_path_carrier_build_from_config")
            && packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !packet_path.contains("descriptor.cache_key()")
            && !packet_path.contains("descriptor.server()")
            && !packet_path.contains("descriptor.port()")
            && !packet_path.contains("spec.carrier()")
            && !packet_path.contains("spec.cache_key()")
            && !packet_path.contains("spec.carrier_cache_key()")
            && !packet_path.contains("spec.association_target()")
            && !packet_path.contains("into_association_target()")
            && !packet_path.contains(".packet_path_cache_key()")
            && !packet_path.contains("config.association_target()")
            && !packet_path.contains(".packet_path_association_config()")
            && flow.contains("socks5::udp::udp_flow_resume_from_config")
            && !flow.contains("Socks5UdpFlowConfig::new")
            && !flow.contains(".flow_resume()")
            && flow.contains("ManagedRelayStart")
            && flow.contains(".start_tracked_managed_relay(")
            && !flow.contains("ManagedUdpSend {")
            && !flow.contains("ManagedUdpFlowResume::new"),
        "SOCKS5 packet-path and managed-flow construction should live in explicit protocol-local UDP submodules"
    );
}

#[test]
fn adapter_root_is_facade_only() {
    let adapters = read("src/adapters/mod.rs");

    for expected in [
        "mod common;",
        "mod direct;",
        "mod http_connect;",
        "mod hysteria2;",
        "mod mieru;",
        "mod mixed;",
        "mod shadowsocks;",
        "mod socks5;",
        "mod trojan;",
        "mod vless;",
        "mod vmess;",
        "pub(crate) use direct::DirectAdapter;",
        "pub(crate) use hysteria2::udp::managed_datagram_handler as hysteria2_udp_datagram_handler;",
        "pub(crate) use http_connect::HttpConnectAdapter;",
        "pub(crate) use hysteria2::Hysteria2Adapter;",
        "pub(crate) use mieru::udp::managed_stream_handler as mieru_udp_stream_handler;",
        "pub(crate) use mieru::MieruAdapter;",
        "pub(crate) use mixed::MixedAdapter;",
        "pub(crate) use shadowsocks::udp::managed_datagram_handler as shadowsocks_udp_datagram_handler;",
        "pub(crate) use shadowsocks::ShadowsocksAdapter;",
        "pub(crate) use socks5::Socks5Adapter;",
        "pub(crate) use trojan::udp::managed_stream_handler as trojan_udp_stream_handler;",
        "pub(crate) use trojan::TrojanAdapter;",
        "pub(crate) use vless::VlessAdapter;",
        "pub(crate) use vmess::VmessAdapter;",
    ] {
        assert!(
            adapters.contains(expected),
            "src/adapters/mod.rs should expose adapter facade item `{expected}`"
        );
    }

    for line in adapters.lines().map(str::trim) {
        let allowed = line.is_empty()
            || line.starts_with("//!")
            || line.starts_with("#[cfg(")
            || line.starts_with("mod ")
            || line.starts_with("pub(crate) use ");
        assert!(
            allowed,
            "src/adapters/mod.rs should only declare adapter modules and re-export adapter types; found `{line}`"
        );
    }

    for forbidden in [
        "async fn",
        "fn ",
        "impl ",
        "match ",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "ProtocolRegistry",
    ] {
        assert!(
            !adapters.contains(forbidden),
            "src/adapters/mod.rs should remain a facade over concrete adapter modules; found `{forbidden}`"
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
                "super::connector::connect_tcp",
                "connect_upstream_hysteria2",
                "EstablishedTcpOutbound::Hysteria2",
            ],
        ),
        (
            "mieru",
            &["connect_upstream_mieru", "EstablishedTcpOutbound::Mieru"],
        ),
        (
            "shadowsocks",
            &[
                "connect_upstream_shadowsocks",
                "EstablishedTcpOutbound::Shadowsocks",
            ],
        ),
        (
            "socks5",
            &["connect_upstream_socks5", "EstablishedTcpOutbound::Socks5"],
        ),
        (
            "trojan",
            &["connect_upstream_trojan", "EstablishedTcpOutbound::Trojan"],
        ),
        (
            "vless",
            &["connect_upstream_vless", "EstablishedTcpOutbound::Vless"],
        ),
        (
            "vmess",
            &["connect_upstream_vmess", "EstablishedTcpOutbound::Vmess"],
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
fn outbound_tcp_helpers_are_called_only_by_adapter_tcp_modules() {
    let helpers = ["crate::outbound::"];

    for path in rust_sources_under("src") {
        let source = relative(&path);
        if source.starts_with("src/adapters/") && source.ends_with("/tcp.rs") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read rust source");
        for helper in helpers {
            assert!(
                !content.contains(helper),
                "{source} should not call outbound TCP helper `{helper}` directly; dispatch through the owning ProtocolRegistry"
            );
        }
    }
}

#[test]
fn hysteria2_tcp_udp_connect_glue_lives_in_adapter_connector() {
    let outbound = manifest_dir().join("src/outbound/hysteria2.rs");
    let adapter = read("src/adapters/hysteria2.rs");
    let tcp = read("src/adapters/hysteria2/tcp.rs");
    let connector = read("src/adapters/hysteria2/connector.rs");
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let packet_path = read("src/adapters/hysteria2/udp/packet_path.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/outbound.rs"))
            .expect("read hysteria2 protocol outbound source");

    assert!(
        !outbound.exists(),
        "Hysteria2 should not need a protocol-named proxy outbound module; TCP/UDP connect glue lives in adapters/hysteria2/connector.rs"
    );
    assert!(
        adapter.contains("mod connector;")
            && tcp.contains("super::connector::connect_tcp")
            && managed.contains("connector::establish_udp_flow_session")
            && packet_path.contains("connector::open_udp_packet_path_build")
            && connector.contains("struct Hysteria2Connector")
            && connector.contains("open_hysteria2_quic_connection")
            && connector.contains("Hysteria2QuicProfile::from_parts")
            && connector.contains("quic_profile: Hysteria2QuicProfile")
            && !connector.contains("client_fingerprint: Option<String>")
            && connector.contains("hysteria2::Hysteria2OutboundProfile")
            && connector.contains("Hysteria2OutboundProfile::from_config_parts")
            && !connector.contains("password: String")
            && !connector.contains("Hysteria2Outbound\n            .authenticate_connection")
            && !connector.contains("authenticate_with_password")
            && !connector.contains("export_keying_material")
            && connector.contains("connect_raw_with_udp_profile"),
        "Hysteria2 adapter connector should own proxy-local QUIC stream opening while protocols/hysteria2 owns auth and TCP connect framing"
    );
    assert!(
        protocol_outbound.contains("pub async fn authenticate_connection")
            && protocol_outbound.contains("struct Hysteria2OutboundProfile")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("export_keying_material")
            && protocol_outbound.contains("pub async fn establish_tcp_connect")
            && protocol_outbound.contains("self.send_tcp_connect(stream, session).await?")
            && protocol_outbound.contains("self.read_connect_response(stream).await"),
        "protocols/hysteria2 outbound should own connection authentication and TCP connect handshake composition"
    );
}

#[test]
fn trojan_tcp_connect_uses_request_model() {
    let outbound = manifest_dir().join("src/outbound/trojan.rs");
    let adapter = read("src/adapters/trojan/tcp.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    assert!(
        !outbound.exists(),
        "Trojan should not need a protocol-named proxy outbound module; TCP glue lives in adapters/trojan/tcp.rs and protocol handshake lives in protocols/trojan"
    );
    let forbidden = "zero_transport::tls::connect_tls_upstream";
    assert!(
        !adapter.contains(forbidden),
        "Trojan adapter TCP glue should request TLS stream opening through the transport facade; found `{forbidden}`"
    );
    assert!(
        adapter.contains("open_trojan_udp_tls_stream")
            && adapter.contains("trojan_tls_options("),
        "Trojan adapter TCP glue should share the Trojan transport TLS opening path with UDP while keeping TLS opening outside runtime"
    );
    assert!(
        adapter.contains("TrojanTcpOutboundProfile::from_config_parts")
            && adapter.contains("TrojanTcpTlsProfile::from_config_parts")
            && adapter.contains("TrojanTlsProfile::from_parts")
            && !adapter.contains("ClientTlsConfig")
            && !adapter.contains("ClientTlsConfig {")
            && !adapter.contains("trojan_tcp_tls_config(")
            && !adapter.contains("TrojanTcpTunnelTarget::new")
            && !adapter.contains("TrojanTcpTunnelTarget {"),
        "Trojan adapter TCP glue should use protocol-owned outbound/TLS profiles instead of constructing TCP targets or raw TLS config directly"
    );
    assert!(
        protocol_outbound.contains("struct TrojanTcpOutboundProfile")
            && protocol_outbound.contains("pub struct TrojanTcpTlsProfile")
            && protocol_outbound.contains("pub fn server_name(&self) -> Option<&str>")
            && protocol_outbound.contains("pub fn insecure(&self) -> bool")
            && protocol_outbound.contains("pub fn client_fingerprint(&self) -> Option<&str>")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("impl<'a> TrojanTcpTunnelTarget<'a>")
            && protocol_outbound.contains("pub fn new(session: &'a Session, password: &'a str)"),
        "Trojan protocol crate should own TCP target construction and profile-backed handshake/TLS identity"
    );
}

#[test]
fn shadowsocks_tcp_connect_uses_request_model() {
    let outbound = manifest_dir().join("src/outbound/shadowsocks.rs");
    let adapter = read("src/adapters/shadowsocks/tcp.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");

    assert!(
        !outbound.exists(),
        "Shadowsocks should not need a protocol-named proxy outbound module; TCP glue lives in adapters/shadowsocks/tcp.rs and protocol session setup lives in protocols/shadowsocks"
    );
    assert!(
        adapter.contains("async fn connect_tcp(")
            && adapter.contains("async fn apply_tcp_hop(")
            && adapter.contains("ShadowsocksTcpConnectConfig::from_config"),
        "Shadowsocks adapter TCP module should own proxy glue while using protocol-built TCP config"
    );
    assert!(
        !adapter.contains("CipherKind::from_str")
            && !adapter.contains("shadowsocks::CipherKind")
            && adapter.contains("ShadowsocksTcpConnectConfig::from_config")
            && adapter.contains("config: shadowsocks::ShadowsocksTcpConnectConfig")
            && !adapter.contains("cipher: shadowsocks::CipherKind")
            && !adapter.contains("ShadowsocksTcpTarget {")
            && !adapter.contains("ShadowsocksTcpTarget")
            && !adapter.contains("TcpSessionProtocol")
            && !adapter.contains("config.tcp_target(session)")
            && adapter.contains("config.establish_tcp_session(")
            && adapter.contains("config.wrap_outbound_stream(")
            && !adapter.contains("password_bytes()")
            && !adapter.contains("ShadowsocksAeadStream::outbound")
            && protocol_outbound.contains("pub struct ShadowsocksTcpConnectConfig")
            && protocol_outbound.contains("pub fn from_config")
            && protocol_outbound.contains("CipherKind::from_str")
            && protocol_outbound.contains("pub fn tcp_target")
            && protocol_outbound.contains("pub async fn establish_tcp_session")
            && protocol_outbound.contains("pub fn wrap_outbound_stream")
            && protocol_outbound.contains("ShadowsocksAeadStream::outbound"),
        "Shadowsocks TCP adapter should ask protocols/shadowsocks to parse cipher config, establish TCP sessions, and wrap outbound streams"
    );
}

#[test]
fn vmess_tcp_connect_uses_request_model() {
    let outbound = manifest_dir().join("src/outbound/vmess.rs");
    let adapter = read("src/adapters/vmess/tcp.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vmess/src/outbound.rs"))
        .expect("read vmess protocol outbound source");

    assert!(
        !outbound.exists(),
        "VMess should not need a protocol-named proxy outbound module; TCP glue lives in adapters/vmess/tcp.rs and protocol session setup lives in protocols/vmess"
    );
    for forbidden in [
        "parse_uuid",
        "VmessCipher::from_name",
        "vmess unknown cipher",
        "VmessAeadStream::outbound",
        "TcpSessionProtocol",
        "VmessTcpSessionTarget",
        "config.uuid()",
        "config.cipher()",
        "vmess::establish_tcp_outbound_session",
        "vmess::establish_tcp_outbound_stream",
        "vmess::wrap_tcp_outbound_stream",
        "zero_transport::tls::connect_tls_upstream",
        "zero_transport::grpc::connect_grpc",
        "zero_transport::ws::connect_ws",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "VMess outbound TCP helper should receive adapter-parsed identity and transport-built streams; found `{forbidden}`"
        );
    }
    for adapter_owned in [
        "parse_uuid",
        "VmessCipher::from_name",
        "vmess unknown cipher",
    ] {
        assert!(
            !adapter.contains(adapter_owned) && protocol_outbound.contains(adapter_owned),
            "VMess outbound identity parsing detail `{adapter_owned}` should live in protocols/vmess"
        );
    }
    assert!(
        adapter.contains("VmessTcpConnectConfig::from_config")
            && adapter.contains("config: vmess::VmessTcpConnectConfig")
            && adapter.contains("config.mux_pool_identity()")
            && !adapter.contains("config.mux_pool_identity(cipher)")
            && !adapter.contains("VmessMuxIdentity::from_parts")
            && protocol_outbound.contains("pub struct VmessTcpConnectConfig")
            && protocol_outbound.contains("pub fn from_config")
            && protocol_outbound.contains("cipher_name: String")
            && protocol_outbound.contains("cipher_name: cipher.name().to_owned()")
            && protocol_outbound.contains("pub fn mux_pool_identity(&self)"),
        "VMess adapter should ask protocols/vmess to parse TCP identity config and build MUX identity without passing raw cipher strings back into protocol APIs"
    );
    assert!(
        adapter.contains(".establish_tcp_outbound_session(")
            && adapter.contains(".establish_tcp_outbound_stream(")
            && adapter.contains("config.wrap_tcp_outbound_stream(")
            && protocol_outbound.contains("pub async fn establish_tcp_outbound_session")
            && protocol_outbound.contains("pub async fn establish_tcp_outbound_stream")
            && protocol_outbound.contains("pub fn wrap_tcp_outbound_stream"),
        "VMess adapter TCP glue should delegate VMess session and AEAD setup through the protocol-owned TCP config API"
    );
    assert!(
        adapter.contains("crate::transport::build_vmess_outbound_transport")
            && adapter.contains("crate::transport::VmessOutboundTransportRequest")
            && adapter.contains("crate::transport::VmessTransportOptions"),
        "VMess adapter TCP glue should request VMess transport building through zero-transport"
    );
}

#[test]
fn vless_tcp_connect_uses_request_model() {
    let outbound = manifest_dir().join("src/outbound/vless.rs");
    let adapter = read("src/adapters/vless/tcp.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vless/src/outbound.rs"))
        .expect("read vless protocol outbound source");

    assert!(
        !outbound.exists(),
        "VLESS should not need a protocol-named proxy outbound module; TCP glue lives in adapters/vless/tcp.rs and protocol session setup lives in protocols/vless"
    );
    assert!(
        !adapter.contains("#[allow(clippy::too_many_arguments)]"),
        "VLESS TCP connect should not need a too_many_arguments allowance"
    );
    assert!(
        adapter.contains("struct VlessTcpConnect")
            && adapter.contains("request: VlessTcpConnect<'_>"),
        "VLESS adapter TCP module should own the request model"
    );
    assert!(
        !adapter.contains("parse_uuid"),
        "VLESS outbound TCP helper and adapter should receive protocol-parsed identity"
    );
    assert!(
        adapter.contains("VlessTcpConnectConfig::from_config")
            && adapter.contains("config: vless::VlessTcpConnectConfig")
            && adapter.contains("config.should_open_mux_pool_for_tcp()")
            && adapter.contains("config.has_flow()")
            && !adapter.contains("xtls-rprx-vision")
            && !adapter.contains("config.flow().is_some()")
            && adapter.contains("config.mux_pool_identity()")
            && adapter.contains("config.wrap_deferred_response_stream(")
            && !adapter.contains("DeferredVlessResponseStream::new")
            && !adapter.contains("MuxIdentity::from_uuid")
            && protocol_outbound.contains("pub struct VlessTcpConnectConfig")
            && protocol_outbound.contains("pub fn from_config")
            && protocol_outbound.contains("pub fn should_open_mux_pool_for_tcp")
            && protocol_outbound.contains("pub fn has_flow")
            && protocol_outbound.contains("FLOW_XTLS_RPRX_VISION")
            && protocol_outbound.contains("pub fn mux_pool_identity")
            && protocol_outbound.contains("pub fn wrap_deferred_response_stream")
            && protocol_outbound.contains("DeferredVlessResponseStream::new")
            && protocol_outbound.contains("parse_uuid")
            && protocol_outbound.contains("parse_flow"),
        "VLESS adapter should ask protocols/vless to parse outbound identity/flow config, classify flow behavior, build MUX identity, and wrap deferred response streams"
    );
}

#[test]
fn socks5_tcp_adapter_uses_protocol_target_model() {
    let adapter = read("src/adapters/socks5/tcp.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/socks5/src/outbound.rs"))
            .expect("read socks5 protocol outbound source");

    assert!(
        adapter.contains("Socks5TcpOutboundProfile::from_config_parts")
            && adapter.contains(".establish_tcp_tunnel(")
            && !adapter.contains("Socks5TcpTunnelTarget::new")
            && !adapter.contains("Socks5TcpTunnelTarget {")
            && !adapter.contains("Socks5OutboundAuth")
            && !adapter.contains("username.zip"),
        "SOCKS5 TCP adapter should use a protocol-owned outbound profile and avoid constructing tunnel targets directly"
    );
    assert!(
        protocol_outbound.contains("pub struct Socks5TcpOutboundProfile")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("impl<'a> Socks5TcpTunnelTarget<'a>")
            && protocol_outbound.contains("pub fn outbound_auth")
            && protocol_outbound.contains(".zip(password)")
            && protocol_outbound.contains("Socks5OutboundAuth { username, password }"),
        "SOCKS5 protocol crate should own TCP profile, target auth construction, and tunnel establishment details"
    );
}

#[test]
fn mieru_tcp_connect_glue_lives_in_adapter_tcp_module() {
    let outbound = manifest_dir().join("src/outbound/mieru.rs");
    let adapter = read("src/adapters/mieru/tcp.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");

    assert!(
        !outbound.exists(),
        "Mieru should not need a protocol-named proxy outbound module; TCP glue lives in adapters/mieru/tcp.rs and protocol session setup lives in protocols/mieru"
    );
    assert!(
        adapter.contains("async fn connect_tcp(")
            && adapter.contains("async fn apply_tcp_hop(")
            && adapter.contains("MieruTcpOutboundProfile::from_config_parts")
            && adapter.contains(".establish_tcp_tunnel(")
            && !adapter.contains("MieruTcpTunnelTarget::new")
            && !adapter.contains("MieruTcpTunnelTarget {")
            && !adapter.contains("struct MieruTcpStream")
            && !adapter.contains("async fn socks5_connect")
            && !adapter.contains("encrypt_client_data")
            && !adapter.contains("decrypt_server_data_with_consumed")
            && !adapter.contains("TcpSessionProtocol<mieru::MieruTcpTarget>"),
        "Mieru adapter TCP module should use a protocol-owned outbound profile and delegate tunneled session details to protocols/mieru"
    );
    assert!(
        protocol_outbound.contains("pub struct MieruTcpOutboundProfile")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("pub struct MieruTcpStream")
            && protocol_outbound.contains("pub struct MieruTcpTunnelTarget")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("async fn socks5_connect")
            && protocol_outbound.contains("encrypt_client_data")
            && protocol_outbound.contains("decrypt_server_data_with_consumed"),
        "Mieru protocol crate should own TCP encrypted stream and tunneled SOCKS5 connect details"
    );
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
fn adapter_modules_do_not_use_wildcard_parent_imports() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read adapter module");
        assert!(
            !content.contains("use super::*;"),
            "{source} should import its ProtocolRegistry dependencies explicitly"
        );
    }
}

#[test]
fn udp_dispatch_modules_do_not_use_wildcard_parent_imports() {
    for path in rust_sources_under("src/runtime/udp_dispatch") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read udp dispatch module");
        assert!(
            !content.contains("use super::*;"),
            "{source} should import UDP dispatch dependencies explicitly"
        );
    }
}

#[test]
fn protocol_inbound_submodules_do_not_use_wildcard_parent_imports() {
    for root in ["src/inbound/vless", "src/inbound/vmess"] {
        for path in rust_sources_under(root) {
            let source = relative(&path);
            let content = fs::read_to_string(&path).expect("read inbound protocol module");
            assert!(
                !content.contains("use super::*;"),
                "{source} should import protocol inbound dependencies explicitly"
            );
        }
    }
}

#[test]
fn protocol_named_inbound_modules_stay_proxy_glue_not_crypto_implementations() {
    for root in ["src/inbound/vless", "src/inbound/vmess"] {
        for path in rust_sources_under(root) {
            let source = relative(&path);
            let content = fs::read_to_string(&path).expect("read inbound protocol module");

            for forbidden in [
                "use aes",
                "use chacha",
                "use cipher",
                "use hmac",
                "use md5",
                "use ring",
                "use sha",
                "use uuid",
                "Aes128",
                "Aes256",
                "ChaCha20",
                "Hmac",
                "Md5",
                "Sha1",
                "Sha256",
                "Uuid::",
            ] {
                assert!(
                    !content.contains(forbidden),
                    "{source} should stay proxy-side inbound glue and delegate protocol crypto/parsing primitives to protocols/*; found `{forbidden}`"
                );
            }
        }
    }
}

#[test]
fn tcp_inbound_source_address_conversion_lives_in_platform_layer() {
    let platform = fs::read_to_string(repo_root().join("crates/platform/tokio/src/lib.rs"))
        .expect("read zero-platform-tokio source");

    assert!(
        platform.contains("pub fn remote_ip_to_socket_addr")
            && platform.contains("addr.map(|ip| socket_addr_from_ip(ip, 0))")
            && platform.contains("pub fn socket_address_to_socket_addr")
            && platform.contains("socket_addr_from_ip(addr.ip, addr.port)"),
        "zero-platform-tokio should own remote IpAddress to SocketAddr conversion for listener source addresses"
    );

    for source_path in [
        "src/inbound/direct.rs",
        "src/inbound/http_connect.rs",
        "src/inbound/mixed.rs",
        "src/inbound/socks5.rs",
        "src/inbound/shadowsocks.rs",
        "src/inbound/trojan.rs",
        "src/inbound/mieru.rs",
        "src/inbound/vmess/listener.rs",
        "src/inbound/vmess/helpers.rs",
    ] {
        let source = read(source_path);
        assert!(
            !source.contains("fn remote_addr_to_socket")
                && !source.contains("IpAddress::V4")
                && !source.contains("IpAddress::V6")
                && !source.contains("Ipv4Addr::from")
                && !source.contains("Ipv6Addr::from"),
            "{source_path} should not own listener source address conversion"
        );
    }

    for source_path in [
        "src/inbound/direct.rs",
        "src/inbound/http_connect.rs",
        "src/inbound/mixed.rs",
        "src/inbound/socks5.rs",
        "src/inbound/shadowsocks.rs",
        "src/inbound/trojan.rs",
        "src/inbound/mieru.rs",
        "src/inbound/vmess/listener.rs",
    ] {
        let source = read(source_path);
        assert!(
            source.contains("zero_platform_tokio::remote_ip_to_socket_addr"),
            "{source_path} should call the platform listener source address helper"
        );
    }

    for source_path in ["src/inbound/system.rs", "src/inbound/tun.rs"] {
        let source = read(source_path);
        assert!(
            source.contains("zero_platform_tokio::socket_address_to_socket_addr")
                && !source.contains("use std::net::{IpAddr, Ipv4Addr, Ipv6Addr}")
                && !source.contains("IpAddr::V4")
                && !source.contains("IpAddr::V6")
                && !source.contains("Ipv4Addr::from")
                && !source.contains("Ipv6Addr::from"),
            "{source_path} should delegate stack SocketAddress to SocketAddr conversion to zero-platform-tokio"
        );
    }
}

#[test]
fn mieru_inbound_stream_uses_protocol_codec_not_crypto_primitives() {
    for path in rust_sources_under("src/inbound/mieru") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read mieru inbound module");

        for forbidden in [
            "build_data_segment",
            "parse_segment",
            "DataMetadata",
            "MieruCipher",
            "MieruSession",
            "DATA_SERVER_TO_CLIENT",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should wrap mieru protocol codecs instead of owning crypto/framing primitive `{forbidden}`"
            );
        }
    }

    let inbound = read("src/inbound/mieru.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");
    assert!(
        inbound.contains("type MieruClientStream = mieru::MieruInboundStream<TcpRelayStream>")
            && inbound.contains("mieru::MieruInboundStream::new")
            && inbound.contains("client.accept_tunneled_socks5_session().await")
            && !manifest_dir().join("src/inbound/mieru/model.rs").exists(),
        "Mieru proxy inbound should use a protocol-owned data-phase stream wrapper"
    );
    for required in [
        "pub struct MieruInboundStream",
        "impl<S> AsyncRead for MieruInboundStream<S>",
        "impl<S> AsyncWrite for MieruInboundStream<S>",
        "accept_tunneled_socks5_session",
        "read_tunneled_socks5_request",
        "write_tunneled_socks5_success",
        "decrypt_client_data_with_consumed",
        "encrypt_server_data",
    ] {
        assert!(
            protocol_inbound.contains(required),
            "protocols/mieru should own Mieru inbound stream detail `{required}`"
        );
    }
    for forbidden in [
        "MieruInboundDataCodec",
        "decrypt_client_data_with_consumed",
        "encrypt_server_data",
        "async fn socks5_serve",
        "read_exact(&mut head)",
        "bad request version",
        "bad address type",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "Mieru proxy inbound should not hold data-phase codec detail `{forbidden}`"
        );
    }
}

#[test]
fn shadowsocks_udp_inbound_uses_protocol_codec_not_datagram_primitives() {
    for path in rust_sources_under("src/inbound/shadowsocks") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read shadowsocks inbound module");

        for forbidden in [
            "ShadowsocksDatagramCodec",
            "decode_udp_datagram_2022_session",
            "encode_udp_response_2022",
            "ReplayWindow",
            "DatagramCodec",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should wrap Shadowsocks inbound UDP codecs instead of owning datagram primitive `{forbidden}`"
            );
        }
    }

    let udp = read("src/inbound/shadowsocks/udp.rs");
    let protocol_inbound = repo_root().join("protocols/shadowsocks/src/inbound.rs");
    let protocol_inbound =
        fs::read_to_string(protocol_inbound).expect("read shadowsocks protocol inbound source");
    let protocol_shared =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/shared.rs"))
            .expect("read shadowsocks protocol shared source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/shadowsocks/src/lib.rs"))
        .expect("read shadowsocks protocol lib source");
    let inbound_packet_struct = protocol_inbound
        .split("pub struct ShadowsocksInboundUdpPacket")
        .nth(1)
        .and_then(|content| content.split("impl ShadowsocksInboundUdpPacket").next())
        .expect("Shadowsocks inbound UDP packet struct section");
    let inbound_dispatch_struct =
        struct_block(&protocol_inbound, "ShadowsocksInboundUdpDispatchParts");
    let inbound_response_struct = protocol_inbound
        .split("pub struct ShadowsocksInboundUdpResponse")
        .nth(1)
        .and_then(|content| content.split("impl ShadowsocksInboundUdpResponse").next())
        .expect("Shadowsocks inbound UDP response struct section");
    assert!(
        udp.contains("profile.udp_session()")
            && udp.contains("udp_session.decode_dispatch_parts")
            && !udp.contains("request.into_dispatch_parts().into_parts()")
            && udp.contains("udp_session.record_client_session")
            && udp.contains(".send_proxy_session_response_to_client_tokio")
            && !udp.contains("client_sessions")
            && !udp.contains("ss_send_protocol_response")
            && !udp.contains("response_datagram_for_proxy_session")
            && !udp.contains("response_frame_for_proxy_session")
            && !udp.contains("response_target_for_proxy_session")
            && !udp.contains(".response_frame(")
            && !udp.contains("response_target:")
            && !udp.contains("ShadowsocksInboundUdpDispatchParts")
            && !udp.contains("ShadowsocksInboundUdpResponseTarget")
            && !udp.contains("ShadowsocksInboundUdpCodec")
            && !udp.contains(".encode_response(")
            && !udp.contains("udp_session.encode_response_to_client")
            && protocol_inbound.contains("struct ShadowsocksInboundUdpCodec")
            && protocol_inbound.contains("struct ShadowsocksInboundUdpSession")
            && protocol_inbound.contains("fn decode_request")
            && protocol_inbound.contains("fn decode_dispatch_parts")
            && protocol_inbound.contains("struct ShadowsocksInboundUdpResponse")
            && protocol_inbound.contains("struct ShadowsocksInboundUdpDispatchParts")
            && protocol_inbound.contains("fn into_dispatch_parts")
            && protocol_inbound.contains("fn pipe_parts")
            && protocol_inbound.contains("fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>)")
            && !inbound_dispatch_struct.contains("pub target: Address")
            && !inbound_dispatch_struct.contains("pub port: u16")
            && !inbound_dispatch_struct.contains("pub payload: Vec<u8>")
            && !inbound_dispatch_struct.contains("pub client_session_id: Option<u64>")
            && protocol_inbound.contains("struct ShadowsocksInboundUdpResponseTarget")
            && protocol_inbound.contains("fn encode_response_to_client")
            && protocol_inbound.contains("fn response_frame")
            && protocol_inbound.contains("fn response_frame_for_proxy_session")
            && protocol_inbound.contains("fn response_datagram_for_proxy_session")
            && protocol_inbound.contains("fn send_response_to_client_tokio")
            && protocol_inbound.contains("fn send_proxy_session_response_to_client_tokio")
            && protocol_inbound.contains("struct ShadowsocksInboundUdpResponseDatagram")
            && protocol_inbound.contains("proxy_sessions:")
            && protocol_inbound.contains("proxy_clients:")
            && protocol_inbound.contains("fn record_proxy_session")
            && protocol_inbound.contains("fn record_client_session")
            && protocol_inbound.contains("fn response_target_for_proxy_session")
            && !inbound_packet_struct.contains("pub target: Address")
            && !inbound_packet_struct.contains("pub payload: Vec<u8>")
            && !inbound_packet_struct.contains("pub client_session_id: Option<u64>")
            && !inbound_response_struct.contains("pub datagram: Vec<u8>")
            && udp.contains("dispatch_parts.pipe_parts()")
            && !udp.contains("dispatch_parts.into_parts()")
            && !udp.contains("request.into_parts()")
            && !udp.contains("request.target,")
            && !udp.contains("request.payload,")
            && !udp.contains("request.client_session_id,")
            && !udp.contains("client_session_id:")
            && !udp.contains("client_ss_session_ids")
            && !udp.contains("response_target:")
            && !udp.contains("&response_datagram.datagram"),
        "Shadowsocks inbound UDP should delegate protocol datagram logic through protocols/shadowsocks inbound UDP session"
    );
    for private_helper in [
        "derive_udp_packet_key",
        "encode_udp_datagram_2022",
        "encode_udp_response_2022",
        "decode_udp_datagram_2022",
        "decode_udp_datagram_2022_session",
        "aead_encrypt_udp",
        "aead_decrypt_udp",
    ] {
        assert!(
            protocol_shared.contains(&format!("pub(crate) fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "Shadowsocks UDP helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
}

#[test]
fn protocol_crates_do_not_depend_on_proxy_runtime_layers() {
    let protocols_root = repo_root().join("protocols");
    let forbidden = [
        "zero-proxy",
        "zero-api",
        "zero-config",
        "zero-router",
        "zero-engine",
        "zero-logging",
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
        for required in ["zero-core", "zero-traits"] {
            assert!(
                content.contains(required),
                "{} should stay anchored on protocol base crate `{required}`",
                manifest.display()
            );
        }
    }
}

#[test]
fn generic_udp_dispatch_does_not_encode_protocol_packets_directly() {
    let content = read("src/runtime/udp_dispatch/mod.rs");
    let dispatch = read("src/runtime/udp_dispatch/dispatch.rs");
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let types = read("src/runtime/udp_dispatch/types.rs");

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

    for forbidden in ["VlessFlow", "VmessFlow", "vless_handles", "vmess_handles"] {
        for source in [&dispatch, &lifecycle, &types] {
            assert!(
                !source.contains(forbidden),
                "UDP dispatch should use neutral managed-flow state instead of `{forbidden}`"
            );
        }
    }
    assert!(
        !types.contains("ManagedFlow")
            && !dispatch.contains("managed_flows")
            && !dispatch.contains("send_existing_cached_flow"),
        "UDP dispatch should track protocol-managed flows in UdpSessionFlows and avoid cached-manager pre-scans"
    );
}

#[test]
fn http_connect_redirect_response_framing_stays_in_protocol_crate() {
    let inbound = read("src/inbound/http_connect.rs");
    let mixed = read("src/inbound/mixed.rs");
    let protocol_inbound =
        fs::read_to_string(repo_root().join("protocols/http-connect/src/inbound.rs"))
            .expect("read http-connect protocol inbound source");

    assert!(
        inbound.contains("select_redirect_target")
            && !inbound.contains("build_redirect_response")
            && inbound.contains("send_redirect_response")
            && inbound.contains("Some((status, location))")
            && !inbound.contains("HTTP/1.1 {status} Found")
            && !inbound.contains("Location: {location}")
            && protocol_inbound.contains("pub fn redirect_response")
            && protocol_inbound.contains("pub async fn send_redirect_response")
            && protocol_inbound.contains("HTTP/1.1 {status} Found")
            && protocol_inbound.contains("Location: {location}"),
        "HTTP CONNECT redirect wire response framing should live in protocols/http-connect; proxy should only select status/location"
    );
    assert!(
        inbound.contains(".send_success_response(")
            && inbound.contains(".send_blocked_response(")
            && inbound.contains(".send_upstream_failure_response(")
            && inbound.contains(".send_method_not_allowed_response(")
            && inbound.contains(".send_bad_request_response(")
            && mixed.contains(".send_method_not_allowed_response(")
            && mixed.contains(".send_bad_request_response(")
            && !inbound.contains("HttpConnectResponse")
            && !mixed.contains("HttpConnectResponse"),
        "HTTP CONNECT inbound glue should use protocol-owned semantic response methods instead of selecting concrete response frames"
    );
    assert!(
        protocol_inbound.contains("pub async fn send_success_response")
            && protocol_inbound.contains("pub async fn send_bad_request_response")
            && protocol_inbound.contains("pub async fn send_method_not_allowed_response")
            && protocol_inbound.contains("pub async fn send_blocked_response")
            && protocol_inbound.contains("pub async fn send_upstream_failure_response")
            && protocol_inbound.contains("HttpConnectResponse::ConnectionEstablished")
            && protocol_inbound.contains("HttpConnectResponse::BadRequest")
            && protocol_inbound.contains("HttpConnectResponse::MethodNotAllowed")
            && protocol_inbound.contains("HttpConnectResponse::Forbidden")
            && protocol_inbound.contains("HttpConnectResponse::BadGateway"),
        "protocols/http-connect should own concrete response selection for common inbound outcomes"
    );
}

#[test]
fn protocol_inventory_keeps_protocol_instances_private() {
    let content = read("src/inventory.rs");
    let protocols = read("src/inventory/protocols.rs");

    for forbidden in [
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
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
            "src/inventory.rs should keep protocol instances private and delegate protocol classification to ProtocolRegistry; found `{forbidden}`"
        );
    }

    assert!(
        protocols.contains("fn direct_connector(&self)"),
        "src/inventory/protocols.rs should keep the neutral direct connector helper"
    );
}

#[test]
fn inventory_does_not_expose_concrete_protocol_accessors() {
    let protocol_access_patterns = [
        "use http_connect::",
        "use shadowsocks::",
        "use socks5::",
        "use trojan::",
        "use vless::",
        "use vmess::",
        "fn socks5_inbound_protocol(&self)",
        "fn socks5_outbound_protocol(&self)",
        "fn http_connect_inbound_protocol(&self)",
        "fn vless_inbound_protocol(&self)",
        "fn vless_outbound_protocol(&self)",
        "fn shadowsocks_outbound_protocol(&self)",
        "fn trojan_outbound_protocol(&self)",
        "fn vmess_outbound_protocol(&self)",
    ];

    for path in rust_sources_under("src/inventory") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in protocol_access_patterns {
            assert!(
                !content.contains(forbidden),
                "{source} should not import concrete protocol crates or expose concrete protocol accessors; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn socks5_udp_association_runtime_state_stays_out_of_outbound_module() {
    let outbound = manifest_dir().join("src/outbound/socks5.rs");
    let adapter = read("src/adapters/socks5/udp.rs");
    let active = read("src/adapters/socks5/udp/active.rs");
    let establish = read("src/adapters/socks5/udp/establish.rs");
    let model = read("src/adapters/socks5/udp/model.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/socks5/src/udp.rs"))
        .expect("read socks5 udp");
    let packet_path_source = read("src/adapters/socks5/udp/packet_path.rs");
    let send_source = read("src/adapters/socks5/udp/send.rs");
    let send = manifest_dir().join("src/adapters/socks5/udp/send.rs");
    let runtime_source = read("src/adapters/socks5/udp/runtime.rs");
    let runtime = manifest_dir().join("src/adapters/socks5/udp/runtime.rs");
    let packet_path = manifest_dir().join("src/adapters/socks5/udp/packet_path.rs");
    let old_protocol_runtime = manifest_dir().join("src/protocol_runtime/socks5_udp.rs");
    let old_protocol_runtime_dir = manifest_dir().join("src/protocol_runtime/socks5_udp");

    assert!(
        !outbound.exists(),
        "SOCKS5 should not need a protocol-named proxy outbound module; TCP glue lives in adapters/socks5/tcp.rs and protocol handshake lives in protocols/socks5"
    );

    for forbidden in [
        "Socks5UdpRelay",
        "ActiveUpstreamSocks5UdpAssociation",
        "UpstreamAssociationCloseReason",
        "Socks5UdpSend",
        "send_socks5_udp_packet",
        "ensure_socks5_udp_association",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "src/adapters/socks5/udp.rs should stay a thin adapter facade; found `{forbidden}`"
        );
    }

    assert!(
        active.contains("struct ActiveUpstreamSocks5UdpAssociation")
            && active.contains("Socks5EstablishedUdpAssociation<TokioSocket, TokioDatagramSocket>")
            && active.contains("Socks5EstablishedUdpAssociation::from_relay_socket_address")
            && active.contains("socket_addr_to_socket_address(relay_addr)")
            && !active.contains("fn socket_address_from_std")
            && !active.contains("fn ip_address_from_std")
            && !active.contains("SocketAddress::new")
            && !active.contains("IpAddress::V4")
            && !active.contains("IpAddress::V6")
            && !active.contains("socket_addr_to_ip(relay_addr)")
            && !active.contains("Socks5UdpAssociation::from_relay_endpoint")
            && active.contains("impl Socks5UdpAssociationHandle for ActiveUpstreamSocks5UdpAssociation")
            && active.contains("impl Socks5UdpPacketPathAssociation for ActiveUpstreamSocks5UdpAssociation")
            && establish.contains("trait Socks5UdpAssociationEstablisher")
            && establish.contains("struct DefaultSocks5UdpAssociationEstablisher")
            && establish.contains("fn default_establisher()")
            && establish.contains("fn establish_shared_packet_path_association")
            && establish.contains("fn establish_shared_packet_path_carrier")
            && establish.contains("packet_path_carrier_association_target")
            && !establish.contains("carrier.into_association_target()")
            && establish.contains("ActiveUpstreamSocks5UdpAssociation::establish")
            && runtime_source.contains("Box<dyn Socks5UdpAssociationEstablisher>")
            && runtime_source.contains("establish::default_establisher()")
            && !runtime_source.contains("DefaultSocks5UdpAssociationEstablisher")
            && packet_path_source.contains("establish_shared_packet_path_carrier")
            && !packet_path_source.contains("establish_shared_packet_path_association")
            && !packet_path_source.contains("into_association_target()")
            && runtime_source.contains(".decode_response_parts(&buf[..read])")
            && !runtime_source.contains("response.into_parts()")
            && !runtime_source.contains("upstream_response_from_socks5")
            && !runtime_source.contains("Socks5InboundUdpResponse")
            && !runtime_source.contains("response.target().clone()")
            && !runtime_source.contains("response.payload().to_vec()")
            && runtime_source.contains("let target = association.identity()")
            && runtime_source.contains("!a.identity().matches(&target)")
            && !runtime_source.contains("Socks5UdpAssociationSnapshot")
            && !runtime_source.contains("Socks5UdpAssociationTargetSnapshot")
            && !runtime_source.contains(".upstream_endpoint()")
            && !runtime_source.contains("association.outbound_tag()")
            && !runtime_source.contains("association.server()")
            && !runtime_source.contains("association.port()")
            && !packet_path_source.contains("DefaultSocks5UdpAssociationEstablisher")
            && !runtime_source.contains("use super::active::ActiveUpstreamSocks5UdpAssociation")
            && !packet_path_source.contains("use super::active::ActiveUpstreamSocks5UdpAssociation")
            && !active.contains("Socks5UdpAssociation::new")
            && !active.contains("Socks5UdpAssociationTarget::new")
            && !active.contains("Socks5OwnedUdpAssociationConfig")
            && !active.contains("Socks5UdpRelay,")
            && !active.contains("Socks5UdpRelayEndpoint")
            && active.contains("socks5::udp::establish_udp_relay_with_control")
            && !active.contains("_control:")
            && !active.contains("relay:")
            && !active.contains("Socks5UdpRelayTarget")
            && !active.contains("Socks5OutboundAuth")
            && !active.contains(".establish_udp_relay("),
        "SOCKS5 UDP active association wrapper should store the protocol-owned association handle behind narrow proxy traits"
    );
    for source in [
        ("src/adapters/socks5/udp.rs", adapter.as_str()),
        ("src/adapters/socks5/udp/active.rs", active.as_str()),
        ("src/adapters/socks5/udp/model.rs", model.as_str()),
        (
            "src/adapters/socks5/udp/packet_path.rs",
            packet_path_source.as_str(),
        ),
        ("src/adapters/socks5/udp/send.rs", send_source.as_str()),
    ] {
        for forbidden in [
            "Socks5OutboundAuth",
            "Socks5OwnedOutboundAuth",
            "username().zip",
            "username.zip",
            "password()",
            "Option<(&str, &str)>",
        ] {
            assert!(
                !source.1.contains(forbidden),
                "{} should use protocol-owned SOCKS5 UDP association config instead of `{forbidden}`",
                source.0
            );
        }
    }
    assert!(
        model.contains("enum UpstreamAssociationCloseReason")
            && !model
                .lines()
                .any(|line| line.trim() == "pub(super) struct Socks5UdpAssociation {")
            && model.contains("trait Socks5UdpAssociationHandle")
            && model.contains("fn identity(&self) -> socks5::udp::Socks5UdpAssociationIdentity")
            && !model.contains("fn upstream_endpoint(&self)")
            && !model.contains("struct Socks5UdpAssociationSnapshot")
            && !model.contains("fn from_association(association: &dyn Socks5UdpAssociationHandle)")
            && !model.contains("struct Socks5UdpAssociationTargetSnapshot")
            && !model.contains("fn from_target(target: &socks5::udp::Socks5UdpAssociationTarget)")
            && !model.contains("association.upstream_endpoint()")
            && !model.contains("target.outbound_tag().to_owned()")
            && !model.contains("target.server().to_owned()")
            && !model.contains("target.port()")
            && model.contains("type BoxedSocks5UdpAssociation")
            && model.contains("trait Socks5UdpPacketPathAssociation")
            && model.contains("type SharedSocks5UdpPacketPathAssociation")
            && protocol_udp.contains("struct Socks5UdpAssociationTarget")
            && protocol_udp.contains("struct Socks5UdpAssociationIdentity")
            && protocol_udp.contains("struct Socks5UdpAssociationEndpoint")
            && protocol_udp.contains("pub fn identity(&self) -> Socks5UdpAssociationIdentity")
            && protocol_udp.contains("pub fn connect_endpoint(&self) -> Socks5UdpAssociationEndpoint")
            && protocol_udp.contains("pub fn from_relay_socket_address")
            && protocol_udp.contains("pub fn identity(&self) -> Socks5UdpAssociationIdentity")
            && protocol_udp.contains("pub fn matches(&self, other: &Self) -> bool")
            && active.contains("target.connect_endpoint().into_parts()")
            && active.contains("self.association.identity()")
            && !active.contains("target.server()")
            && !active.contains("target.port()")
            && protocol_udp.contains("struct Socks5EstablishedUdpAssociation")
            && protocol_udp.contains("outbound_tag: alloc::string::String")
            && protocol_udp.contains("packet_path_carrier_association_target")
            && protocol_udp.contains("pub fn matches(&self, outbound_tag: &str, server: &str, port: u16) -> bool"),
        "SOCKS5 UDP association handles should live under adapters/socks5/udp/model.rs while protocol association targets stay in protocols/socks5"
    );
    assert!(
        !send_source.contains("send_socks5_udp_packet")
            && !send_source.contains("ensure_socks5_udp_association")
            && !send_source.contains("runtime.upstream")
            && !send_source.contains("runtime.idle_deadline")
            && runtime_source.contains("pub(super) async fn send_packet")
            && runtime_source.contains("async fn ensure_association")
            && runtime_source.contains("fn drop_after_send_error")
            && runtime_source.contains("upstream: Option<BoxedSocks5UdpAssociation>")
            && !runtime_source.contains("Option<ActiveUpstreamSocks5UdpAssociation>")
            && !runtime_source.contains("-> Option<ActiveUpstreamSocks5UdpAssociation>"),
        "SOCKS5 UDP upstream association lifecycle should be owned by runtime.rs behind a neutral association handle"
    );
    assert!(
        !packet_path_source.contains("socks5::parse_udp_packet")
            && !packet_path_source.contains("socks5::decode_udp_associate_response")
            && packet_path_source.contains("SharedSocks5UdpPacketPathAssociation")
            && !packet_path_source.contains("Arc<ActiveUpstreamSocks5UdpAssociation>")
            && packet_path_source.contains(".recv_payload(buf).await"),
        "SOCKS5 packet-path carrier should use a neutral association handle and delegate protocol response decoding to protocols/socks5"
    );
    assert!(
        !adapter.contains("Socks5UdpPacketSend")
            && !adapter.contains("pub(crate) use send::Socks5UdpSend"),
        "SOCKS5 UDP adapter facade should not expose packet-send request models"
    );
    assert!(
        send.exists() && runtime.exists() && packet_path.exists(),
        "SOCKS5 UDP runtime should be split into send.rs, runtime.rs, and packet_path.rs"
    );
    assert!(
        !old_protocol_runtime.exists() && !old_protocol_runtime_dir.exists(),
        "SOCKS5 UDP runtime manager should not live under protocol_runtime"
    );
}

#[test]
fn vless_udp_state_model_lives_outside_runtime_root() {
    let managed = read("src/adapters/vless/udp/managed.rs");
    let model = read("src/adapters/vless/udp/managed/model.rs");
    let establish = read("src/adapters/vless/udp/managed/establish.rs");
    let stream_packet_manager = read("src/runtime/udp_flow/managed/stream_packet_manager.rs");
    let managed_cache = read("src/runtime/udp_flow/managed/cache.rs");
    let old_runtime = manifest_dir().join("src/protocol_runtime/vless_udp.rs");
    let old_runtime_dir = manifest_dir().join("src/protocol_runtime/vless_udp");

    for removed in [
        "src/adapters/vless/udp/manager.rs",
        "src/adapters/vless/udp/manager/model.rs",
        "src/adapters/vless/udp/manager/establish.rs",
        "src/adapters/vless/udp/manager/send.rs",
        "src/adapters/vless/udp/manager/bridge.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "VLESS UDP should use managed.rs plus generic stream packet manager instead of `{removed}`"
        );
    }
    assert!(
        !old_runtime.exists() && !old_runtime_dir.exists(),
        "VLESS UDP manager should not live under protocol_runtime"
    );

    for required in [
        "struct VlessUdpStartFlow",
        "struct VlessUdpRelayTwoStream",
        "struct VlessUdpRelayFinalHopStart",
    ] {
        assert!(
            model.contains(required) && !managed.contains(required),
            "VLESS UDP request model should live in adapters/vless/udp/managed/model.rs, not the manager root; missing `{required}`"
        );
    }
    assert!(
        !managed.contains("struct VlessUdpUpstream {")
            && !managed.contains("VlessUdpUpstream {")
            && managed.contains("pub(crate) use model::{")
            && managed.contains("mod establish;")
            && managed.contains("mod model;")
            && !managed.contains("fn over_stream")
            && !managed.contains("fn direct")
            && !managed.contains("impl ManagedTupleUdpSender")
            && establish.contains("pub(super) async fn over_stream")
            && establish.contains("pub(super) async fn direct_flow")
            && establish.contains("impl ManagedTupleUdpSender for VlessManagedUdpSender")
            && managed.contains("ManagedStreamPacketSender")
            && !managed.contains("VlessUdpOutboundManager")
            && !managed.contains("ManagedStreamConnectionCacheKey")
            && stream_packet_manager.contains(".send_existing_target(")
            && stream_packet_manager.contains(".send_or_insert_target(")
            && stream_packet_manager.contains(".insert_and_bridge_target(")
            && stream_packet_manager.contains("impl super::stream_sender::ManagedStreamFlowSender")
            && !managed.contains("self.upstreams.get(")
            && !managed.contains("self.upstreams.insert(")
            && !managed.contains("self.spawn_bridge(")
            && !managed.contains(".spawn_response_bridge(")
            && managed_cache.contains("struct ManagedStreamConnection")
            && managed_cache.contains("struct ManagedStreamConnectionSend")
            && managed_cache.contains("struct ManagedStreamConnectionCache")
            && managed_cache.contains("pub(crate) async fn send_existing")
            && managed_cache.contains("pub(crate) async fn send_existing_target")
            && managed_cache.contains("pub(crate) async fn send_or_insert")
            && managed_cache.contains("pub(crate) async fn send_or_insert_target")
            && managed_cache.contains("pub(crate) fn insert_and_bridge")
            && managed_cache.contains("pub(crate) fn insert_and_bridge_target")
            && managed_cache.contains("send_stream_connection"),
        "VLESS UDP managed glue should delegate stream cache hit/miss, insertion, and response bridge wiring to the neutral managed stream connection cache"
    );
}

#[test]
fn vless_udp_transport_opening_lives_in_transport_crate() {
    let managed = read("src/adapters/vless/udp/managed.rs");
    let establish = read("src/adapters/vless/udp/managed/establish.rs");
    let flow = read("src/adapters/vless/udp/flow.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/vless_transport.rs"))
        .expect("read crates/transport/src/vless_transport.rs");

    for forbidden in [
        "crate::transport::connect_quic",
        "zero_transport::quic::connect_quic",
        "struct VlessUdpTransport",
        "pub(crate) struct VlessUdpTransport",
    ] {
        assert!(
            !managed.contains(forbidden),
            "VLESS UDP runtime/model should not own transport opening detail; found `{forbidden}`"
        );
    }

    assert!(
        flow.contains("crate::transport::VlessUdpTransportOptions")
            && establish.contains("crate::transport::VlessUdpTransportConnector")
            && managed.contains("crate::transport::build_vless_outbound_transport_over_stream"),
        "VLESS UDP adapter/runtime should request VLESS transport helpers instead of opening QUIC/TCP transports directly"
    );

    for required in [
        "pub struct VlessUdpTransportOptions",
        "pub struct VlessUdpOutboundTransportRequest",
        "pub async fn build_vless_udp_outbound_transport",
        "pub struct VlessUdpTransportConnector",
        "quic::connect_quic",
        "pub struct VlessTransportOptions",
        "pub async fn build_vless_outbound_transport_over_stream",
    ] {
        assert!(
            transport.contains(required),
            "zero-transport should own VLESS UDP transport helper `{required}`"
        );
    }
}

#[test]
fn vless_udp_identity_is_protocol_parsed() {
    let managed = read("src/adapters/vless/udp/managed.rs");
    let model = read("src/adapters/vless/udp/managed/model.rs");
    let adapter = read("src/adapters/vless/udp.rs");
    let flow = read("src/adapters/vless/udp/flow.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/vless_transport.rs"))
        .expect("read zero-transport vless transport source");
    let protocol = fs::read_to_string(repo_root().join("protocols/vless/src/outbound.rs"))
        .expect("read protocols/vless/src/outbound.rs");

    assert!(
        !managed.contains("parse_uuid"),
        "VLESS UDP runtime should receive protocol-parsed UUIDs"
    );
    assert!(
        !model.contains("id: &'a str") && model.contains("vless::udp::VlessUdpFlowConfig"),
        "VLESS UDP request models should carry protocol-owned flow config instead of raw config IDs"
    );
    for forbidden in [
        "pub(crate) id: &'a str",
        "pub(super) id: &'a str",
        "pub(crate) uuid: [u8; 16]",
        "pub(super) uuid: [u8; 16]",
    ] {
        assert!(
            !model.contains(forbidden),
            "VLESS UDP request models should not carry raw config IDs or UUID fields; found `{forbidden}`"
        );
    }
    assert!(
        !adapter.contains("parse_uuid")
            && !adapter.contains("vless::parse_udp_identity")
            && !adapter.contains("VlessUdpFlowConfig::new")
            && !adapter.contains("XhttpMode::parse")
            && adapter.contains("crate::transport::vless_udp_relay_needs_two_streams")
            && transport.contains("pub fn vless_udp_relay_needs_two_streams")
            && transport.contains("XhttpMode::parse(&config.mode)")
            && flow.contains("vless::udp::udp_flow_config_from_config")
            && !flow.contains("vless::udp::VlessUdpFlowConfig::new"),
        "VLESS UDP flow glue should use protocol/transport-owned parsers while the root stays a facade"
    );
    assert!(
        protocol.contains("struct VlessUdpIdentity")
            && protocol.contains("pub fn parse_udp_identity"),
        "protocols/vless should own VLESS UDP identity parsing"
    );
    assert!(
        protocol.contains("struct VlessUdpFlowConfig")
            && protocol.contains("pub fn new(id: &str, flow: Option<&'a str>)")
            && protocol.contains("pub fn udp_flow_config_from_config"),
        "protocols/vless should own VLESS UDP flow config construction"
    );
}

#[test]
fn vless_udp_adapter_delegates_packet_framing_to_protocol_helpers() {
    let adapter = read("src/adapters/vless/udp.rs");

    for forbidden in ["UdpPacketFraming", "VlessUdpPacketTarget"] {
        assert!(
            !adapter.contains(forbidden),
            "VLESS UDP adapter should delegate mux packet framing to protocols/vless helpers; found `{forbidden}`"
        );
    }
    assert!(
        !adapter.contains("vless::build_udp_packet")
            && !adapter.contains("vless::parse_udp_packet"),
        "VLESS UDP adapter should not call low-level packet helpers directly"
    );
    assert!(
        !adapter.contains("vless::encode_udp_flow_packet"),
        "VLESS UDP adapter should leave mux fast-path packet framing to protocols/vless"
    );
}

#[test]
fn vless_udp_runtime_delegates_packet_framing_to_protocol_helpers() {
    let runtime = read("src/adapters/vless/udp/managed.rs");
    let establish = read("src/adapters/vless/udp/managed/establish.rs");
    let model = read("src/runtime/udp_flow/managed/stream_packet_manager.rs");
    let proxy_transport = read("src/transport/mod.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/vless_transport.rs"))
        .expect("read zero-transport vless_transport source");
    let protocol_shared = fs::read_to_string(repo_root().join("protocols/vless/src/shared.rs"))
        .expect("read protocols/vless/src/shared.rs");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vless/src/lib.rs"))
        .expect("read protocols/vless/src/lib.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vless/src/outbound.rs"))
        .expect("read protocols/vless/src/outbound.rs");

    for forbidden in [
        "broadcast::Sender<vless::VlessUdpPacket>",
        "mpsc::Sender<Vec<u8>>",
        "mpsc::channel::<VlessFlowSend>",
        "oneshot::Sender<Result<usize, EngineError>>",
        "struct VlessFlowSend",
        "pub(super) struct VlessFlowSender",
        "UdpPacketFraming",
        "VlessUdpFlowCodec",
        "VlessUdpPacket)",
        "VlessUdpPacketTarget",
        "UdpPacketTunnelProtocol",
        "VlessUdpPacketTunnelTarget",
        "VlessEstablishedUdpFlow",
        "VlessInitialUdpFlowPacket",
        "encode_udp_packet",
        "decode_udp_packet",
        "vless::build_udp_packet",
        "vless::parse_udp_packet",
        "vless::establish_udp_packet_tunnel",
        "vless::encode_udp_flow_packet",
        "vless::decode_udp_flow_packet",
        "vless::establish_udp_flow(",
        "vless::spawn_udp_flow",
        "VlessInitialUdpFlowPacket::from_parts",
        ".encoded_len(&flow_io)",
        ".write_packet_tokio(",
        ".read_packet_tokio(",
        "tokio::select!",
        "tokio::spawn",
    ] {
        assert!(
            !runtime.contains(forbidden) && !establish.contains(forbidden) && !model.contains(forbidden),
            "VLESS UDP runtime should avoid raw packet framing and use protocols/vless flow helpers; found `{forbidden}`"
        );
    }
    assert!(
        !runtime.contains("use zero_core::{Address, Session, UdpFlowPacket}")
            && !establish.contains("use zero_core::{Address, Session, UdpFlowPacket}")
            && !runtime.contains("zero_core::UdpFlowPacket::from_parts")
            && !establish.contains("zero_core::UdpFlowPacket::from_parts")
            && !runtime.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !establish.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !model.contains("UdpFlowPacket::from_parts"),
        "VLESS UDP runtime should not construct core UDP flow packets directly"
    );
    assert!(
        !runtime.contains("vless::open_udp_flow")
            && !runtime.contains("vless::open_mux_udp_flow")
            && !establish.contains("vless::open_udp_flow")
            && !establish.contains("vless::open_mux_udp_flow")
            && establish.contains(".establish_flow_with_initial_packet(")
            && !runtime.contains("vless::establish_udp_flow_with_initial_packet")
            && !establish.contains("vless::establish_udp_flow_with_initial_packet")
            && establish.contains(".mux_initial_flow_packet(")
            && !establish.contains(".encode_initial_flow_packet(")
            && establish.contains(".mux_pool_identity(")
            && !establish.contains(".mux_open_identity(")
            && !establish.contains("MuxIdentity::from_uuid")
            && !runtime.contains("vless::encode_udp_flow_initial_packet")
            && !establish.contains("vless::encode_udp_flow_initial_packet")
            && !runtime.contains("vless::establish_udp_flow_stream")
            && !establish.contains("vless::establish_udp_flow_stream")
            && !runtime.contains("vless::VlessUdpIdentity")
            && !establish.contains("vless::VlessUdpIdentity")
            && !runtime.contains("vless::VlessUdpFlowIo")
            && !establish.contains("vless::VlessUdpFlowIo")
            && !runtime.contains("broadcast::channel::<VlessFlowResponse>")
            && !establish.contains("broadcast::channel::<VlessFlowResponse>")
            && !model.contains("SharedManagedUdpConnection")
            && read("src/runtime/udp_flow/managed/cache.rs").contains("ManagedStreamConnection")
            && !model.contains("vless::VlessUdpFlowConnection")
            && !model.contains("vless::VlessUdpFlowSession")
            && !model.contains("vless::VlessUdpFlowSender")
            && !runtime.contains("VlessUdpFlowConnection::new")
            && !runtime.contains("VlessUdpFlowHandle")
            && !runtime.contains("managed_tuple_udp_connection")
            && establish.contains("managed_tuple_udp_connection")
            && !runtime.contains("impl ManagedUdpConnection for vless::VlessUdpFlowConnection")
            && !runtime.contains("spawn_tuple_response_bridge")
            && !runtime.contains(".recv().await")
            && !runtime.contains("EngineError::Io")
            && protocol_outbound.contains("pub async fn establish_udp_flow_with_initial_packet")
            && protocol_outbound.contains("pub async fn establish_flow_with_initial_packet")
            && protocol_outbound.contains("pub fn encode_initial_flow_packet")
            && protocol_outbound.contains("pub fn mux_initial_flow_packet")
            && protocol_outbound.contains("pub fn mux_open_identity")
            && protocol_outbound.contains("pub fn mux_pool_identity")
            && protocol_outbound.contains("pub fn into_connection")
            && protocol_outbound.contains("pub fn spawn_udp_flow")
            && protocol_outbound.contains("tokio::select!")
            && protocol_outbound.contains("struct VlessUdpFlowSender")
            && !protocol_outbound.contains("pub struct VlessUdpFlowSender")
            && protocol_outbound.contains("pub struct VlessUdpFlowConnection")
            && protocol_outbound.contains("pub struct VlessEstablishedUdpFlowHandle")
            && protocol_outbound.contains("pub struct VlessUdpFlowSession")
            && protocol_outbound.contains("pub type VlessUdpFlowResponseReceiver")
            && !protocol_outbound.contains("pub type VlessUdpFlowResponses")
            && protocol_outbound.contains("pub struct VlessInitialUdpFlowPacket"),
        "VLESS UDP runtime should keep protocol flow I/O inside protocols/vless and leave proxy manager as cache/bridge glue"
    );
    assert!(
        read("src/runtime/udp_flow/managed/cache.rs").contains("SharedManagedUdpConnection")
            && !model.contains("SharedManagedUdpConnection")
            && !model.contains("vless::VlessUdpFlowConnection"),
        "VLESS UDP manager cache should store a neutral stream UDP connection object"
    );
    for forbidden in [
        "VlessUdpFlowStream",
        "VlessUdpResponse",
        "VlessUdpFlowIo",
        "establish_udp_flow_stream",
        "mpsc::channel::<vless::VlessUdpFlowPacket>",
        "mpsc::channel::<UdpFlowPacket>",
        "broadcast::channel::<VlessUdpResponse>",
        "tokio::spawn",
        "encode_vless_udp_flow_packet",
        "send_vless_udp_flow_packet",
        "spawn_vless_udp_packet_flow",
    ] {
        assert!(
            !transport.contains(forbidden) && !proxy_transport.contains(forbidden),
            "zero-transport/proxy transport facade must not own VLESS UDP flow runtime; found `{forbidden}`"
        );
    }
    assert!(
        protocol_outbound.contains("pub struct VlessUdpFlowHandle")
            && protocol_outbound.contains("pub struct VlessEstablishedUdpFlowHandle")
            && protocol_outbound.contains("struct VlessUdpFlowSender")
            && !protocol_outbound.contains("pub struct VlessUdpFlowSender")
            && protocol_outbound.contains("pub struct VlessUdpFlowConnection")
            && protocol_outbound.contains("pub struct VlessUdpFlowSession")
            && protocol_outbound.contains("pub struct VlessInitialUdpFlowPacket")
            && protocol_outbound.contains("pub struct VlessMuxInitialUdpFlowPacket")
            && protocol_outbound.contains("pub struct VlessUdpMuxOpenIdentity")
            && protocol_outbound.contains("pub type VlessUdpFlowResponse")
            && protocol_outbound.contains("pub type VlessUdpFlowResponseReceiver")
            && !protocol_outbound.contains("pub type VlessUdpFlowResponses")
            && !protocol_outbound.contains("pub struct VlessUdpFlowSend")
            && !protocol_outbound.contains("pub async fn open_udp_flow")
            && !protocol_outbound.contains("pub fn open_mux_udp_flow")
            && !protocol_outbound.contains("mpsc::channel::<VlessUdpFlowPacket>")
            && protocol_outbound.contains("fn spawn_udp_flow_task")
            && protocol_outbound.contains("broadcast::channel")
            && protocol_outbound.contains("tokio::spawn")
            && protocol_shared.contains("pub fn encode_udp_flow_initial_packet")
            && !protocol_lib.contains("encode_udp_flow_initial_packet")
            && protocol_shared.contains("pub struct VlessUdpFlowIo")
            && protocol_shared.contains("pub struct VlessUdpFlowPacket")
            && protocol_shared.contains("pub(crate) fn encode_udp_flow_packet")
            && protocol_shared.contains("pub(crate) fn decode_udp_flow_packet"),
        "protocols/vless should own VLESS UDP packet IO helpers and protocol flow pump handles"
    );
}

#[test]
fn vmess_udp_state_model_lives_outside_runtime_root() {
    let managed = read("src/adapters/vmess/udp/managed.rs");
    let model = read("src/adapters/vmess/udp/managed/model.rs");
    let establish = read("src/adapters/vmess/udp/managed/establish.rs");
    let stream_packet_manager = read("src/runtime/udp_flow/managed/stream_packet_manager.rs");
    let managed_cache = read("src/runtime/udp_flow/managed/cache.rs");
    let old_runtime = manifest_dir().join("src/protocol_runtime/vmess_udp.rs");
    let old_runtime_dir = manifest_dir().join("src/protocol_runtime/vmess_udp");
    let bridge = manifest_dir().join("src/adapters/vmess/udp/manager/bridge.rs");

    assert!(
        !old_runtime.exists() && !old_runtime_dir.exists() && !bridge.exists(),
        "VMess UDP manager should live under the VMess adapter without protocol-local bridge modules"
    );

    for forbidden in ["struct VmessUdpUpstream {", "struct VmessUdpTransport"] {
        assert!(
            !managed.contains(forbidden),
            "vmess UDP manager should keep neutral state/cache mechanics outside the protocol connector; found `{forbidden}`"
        );
    }

    for required in ["struct VmessUdpStartFlow", "struct VmessUdpRelayFlowStart"] {
        assert!(
            model.contains(required) && !managed.contains(required),
            "VMess UDP protocol request model should live in adapters/vmess/udp/managed/model.rs, not the manager root; missing `{required}`"
        );
    }
    assert!(
        !managed.contains("struct VmessUdpUpstream {")
            && !managed.contains("struct VmessUdpUpstreamRequest")
            && establish.contains("pub(super) async fn over_stream")
            && establish.contains("pub(super) async fn direct_flow")
            && establish.contains("impl ManagedTupleUdpSender for VmessManagedUdpSender")
            && managed.contains("pub(crate) use model::{")
            && managed.contains("mod establish;")
            && managed.contains("mod model;")
            && managed.contains("ManagedStreamPacketSender")
            && !managed.contains("VmessUdpOutboundManager")
            && !managed.contains("ManagedStreamConnectionCacheKey")
            && managed.contains(".send_or_insert_target(")
            && managed.contains(".insert_and_bridge_target(")
            && !managed.contains("self.upstreams.get(")
            && !managed.contains("self.upstreams.insert(")
            && !managed.contains("self.spawn_bridge(")
            && !managed.contains(".spawn_response_bridge(")
            && stream_packet_manager.contains("struct ManagedStreamPacketSender")
            && stream_packet_manager.contains("impl super::stream_sender::ManagedStreamFlowSender")
            && stream_packet_manager.contains("ManagedStreamConnectionCache")
            && stream_packet_manager.contains(".send_existing_target(")
            && stream_packet_manager.contains(".send_or_insert_target(")
            && stream_packet_manager.contains(".insert_and_bridge_target(")
            && managed_cache.contains("struct ManagedStreamConnection")
            && managed_cache.contains("struct ManagedStreamConnectionSend")
            && managed_cache.contains("struct ManagedStreamConnectionCache")
            && managed_cache.contains("pub(crate) async fn send_existing")
            && managed_cache.contains("pub(crate) async fn send_existing_target")
            && managed_cache.contains("pub(crate) async fn send_or_insert")
            && managed_cache.contains("pub(crate) async fn send_or_insert_target")
            && managed_cache.contains("pub(crate) fn insert_and_bridge")
            && managed_cache.contains("pub(crate) fn insert_and_bridge_target")
            && managed_cache.contains("send_stream_connection"),
        "VMess UDP manager should delegate stream cache hit/miss, insertion, and response bridge wiring to the neutral managed stream connection cache"
    );
}

#[test]
fn vmess_udp_transport_opening_lives_in_transport_crate() {
    let managed = read("src/adapters/vmess/udp/managed.rs");
    let establish = read("src/adapters/vmess/udp/managed/establish.rs");
    let flow = read("src/adapters/vmess/udp/flow.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/vmess_transport.rs"))
        .expect("read crates/transport/src/vmess_transport.rs");

    for forbidden in [
        "zero_transport::tls::connect_tls_upstream",
        "zero_transport::tls::connect_tls_stream",
        "zero_transport::grpc::connect_grpc",
        "zero_transport::ws::connect_ws",
        "struct VmessUdpTransport",
    ] {
        assert!(
            !managed.contains(forbidden),
            "VMess UDP runtime/model should not own transport opening detail; found `{forbidden}`"
        );
    }

    assert!(
        flow.contains("crate::transport::VmessTransportOptions")
            && establish.contains("crate::transport::VmessTransportConnector")
            && managed.contains("crate::transport::build_vmess_outbound_transport_over_stream"),
        "VMess UDP adapter/runtime should request VMess transport helpers instead of opening TLS/WS/gRPC directly"
    );

    for required in [
        "pub struct VmessTransportOptions",
        "pub struct VmessOutboundTransportRequest",
        "pub struct VmessFinalHopTransportRequest",
        "pub async fn build_vmess_outbound_transport",
        "pub async fn build_vmess_outbound_transport_over_stream",
        "pub struct VmessTransportConnector",
        "tls::connect_tls_upstream",
        "tls::connect_tls_stream",
        "grpc::connect_grpc",
        "ws::connect_ws",
    ] {
        assert!(
            transport.contains(required),
            "zero-transport should own VMess transport opening helper `{required}`"
        );
    }
}

#[test]
fn vmess_udp_identity_is_protocol_parsed() {
    let managed = read("src/adapters/vmess/udp/managed.rs");
    let model = read("src/adapters/vmess/udp/managed/model.rs");
    let adapter = read("src/adapters/vmess/udp.rs");
    let flow = read("src/adapters/vmess/udp/flow.rs");
    let protocol = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read protocols/vmess/src/udp.rs");

    for forbidden in ["parse_uuid", "VmessCipher::from_name"] {
        assert!(
            !managed.contains(forbidden) && !model.contains(forbidden),
            "VMess UDP runtime should receive protocol-parsed identity; found `{forbidden}`"
        );
        assert!(
            !adapter.contains(forbidden),
            "VMess UDP adapter should delegate identity parsing detail `{forbidden}` to protocols/vmess"
        );
    }
    assert!(
        !adapter.contains("vmess::parse_udp_identity")
            && !adapter.contains("VmessUdpFlowConfig::new")
            && flow.contains("vmess::udp::udp_flow_config_from_config")
            && !flow.contains("vmess::udp::VmessUdpFlowConfig::new"),
        "VMess UDP flow glue should use the protocol-owned flow config parser while the root stays a facade"
    );
    assert!(
        protocol.contains("struct VmessUdpIdentity")
            && protocol.contains("pub fn parse_udp_identity")
            && protocol.contains("VmessCipher::from_name"),
        "protocols/vmess should own VMess UDP identity and cipher parsing"
    );
    assert!(
        protocol.contains("struct VmessUdpFlowConfig")
            && protocol.contains("pub fn new(id: &str, cipher: &'a str)")
            && protocol.contains("pub fn udp_flow_config_from_config"),
        "protocols/vmess should own VMess UDP flow config construction"
    );

    for forbidden in [
        "pub(crate) id: &'a str",
        "pub(super) id: &'a str",
        "pub(crate) cipher: &'a str",
        "pub(super) cipher: &'a str",
        "pub(crate) uuid: [u8; 16]",
        "pub(super) uuid: [u8; 16]",
        "pub(crate) cipher: vmess::VmessCipher",
        "pub(super) cipher: vmess::VmessCipher",
        "cipher_name: &'a str",
    ] {
        assert!(
            !model.contains(forbidden),
            "VMess UDP request models should carry protocol-owned flow config only; found `{forbidden}`"
        );
    }
    assert!(
        model.contains("vmess::udp::VmessUdpFlowConfig") && !model.contains("vmess::VmessUdpIdentity"),
        "VMess UDP request models should carry protocol-owned flow config for identity and mux keying"
    );
}

#[test]
fn vmess_udp_runtime_delegates_packet_framing_to_protocol_helpers() {
    let runtime = read("src/adapters/vmess/udp/managed.rs");
    let establish = read("src/adapters/vmess/udp/managed/establish.rs");
    let model = read("src/runtime/udp_flow/managed/stream_packet_manager.rs");
    let proxy_transport = read("src/transport/mod.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/vmess_transport.rs"))
        .expect("read zero-transport vmess_transport source");
    let protocol = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read protocols/vmess/src/udp.rs");

    for forbidden in [
        "broadcast::Sender<vmess::VmessUdpPacket>",
        "mpsc::Sender<Vec<u8>>",
        "mpsc::channel::<VmessFlowSend>",
        "oneshot::Sender<Result<usize, EngineError>>",
        "struct VmessFlowSend",
        "pub(super) struct VmessFlowSender",
        "UdpPacketFraming",
        "VmessUdpFlowCodec",
        "VmessUdpPacket)",
        "VmessUdpPacketTarget",
        "VmessAeadStream::establish_udp_outbound",
        "VmessOutbound",
        "encode_udp_packet",
        "decode_udp_packet",
        "vmess::build_udp_packet",
        "vmess::parse_udp_packet",
        "vmess::establish_udp_outbound_stream",
        "vmess::encode_udp_flow_packet",
        "vmess::decode_udp_flow_packet",
        ".write_packet_tokio(",
        ".read_packet_tokio(",
        "tokio::select!",
        "tokio::spawn",
        "vmess::VmessEstablishedUdpFlow",
        "vmess::VmessInitialUdpFlowPacket",
        "vmess::establish_udp_flow(",
        "vmess::spawn_udp_flow",
        "VmessInitialUdpFlowPacket::from_parts",
        "initial_packet.encoded_len(&flow_io)",
    ] {
        assert!(
            !runtime.contains(forbidden) && !establish.contains(forbidden) && !model.contains(forbidden),
            "VMess UDP runtime should avoid raw packet framing and use protocols/vmess flow helpers; found `{forbidden}`"
        );
    }
    assert!(
        !runtime.contains("use zero_core::{Address, Session, UdpFlowPacket}")
            && !establish.contains("use zero_core::{Address, Session, UdpFlowPacket}")
            && !runtime.contains("zero_core::UdpFlowPacket::from_parts")
            && !establish.contains("zero_core::UdpFlowPacket::from_parts")
            && !runtime.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !establish.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !model.contains("UdpFlowPacket::from_parts"),
        "VMess UDP runtime should not construct core UDP flow packets directly"
    );
    assert!(
        !runtime.contains("vmess::open_udp_flow")
            && !runtime.contains("vmess::open_mux_udp_flow")
            && !establish.contains("vmess::open_udp_flow")
            && !establish.contains("vmess::open_mux_udp_flow")
            && establish.contains(".establish_flow_with_initial_packet(")
            && !runtime.contains("vmess::establish_udp_flow_with_initial_packet")
            && !establish.contains("vmess::establish_udp_flow_with_initial_packet")
            && establish.contains(".start_flow_with_initial_packet(")
            && establish.contains(".mux_pool_identity(")
            && !establish.contains(".mux_open_identity(")
            && !establish.contains("VmessMuxIdentity::from_parts")
            && !establish.contains(".uuid()")
            && !establish.contains(".cipher_name()")
            && !establish.contains(".cipher()")
            && !runtime.contains("vmess::start_udp_flow_with_initial_packet")
            && !establish.contains("vmess::start_udp_flow_with_initial_packet")
            && !runtime.contains("vmess::establish_udp_flow_stream")
            && !establish.contains("vmess::establish_udp_flow_stream")
            && !runtime.contains("vmess::encode_udp_flow_initial_packet")
            && !establish.contains("vmess::encode_udp_flow_initial_packet")
            && !runtime.contains("vmess::VmessUdpIdentity")
            && !establish.contains("vmess::VmessUdpIdentity")
            && !runtime.contains("vmess::VmessUdpFlowIo")
            && !establish.contains("vmess::VmessUdpFlowIo")
            && !runtime.contains("broadcast::channel::<VmessFlowResponse>")
            && !establish.contains("broadcast::channel::<VmessFlowResponse>")
            && !model.contains("SharedManagedUdpConnection")
            && read("src/runtime/udp_flow/managed/cache.rs").contains("ManagedStreamConnection")
            && !model.contains("vmess::VmessUdpFlowConnection")
            && !model.contains("vmess::VmessUdpFlowSession")
            && !model.contains("vmess::VmessUdpFlowSender")
            && !runtime.contains("VmessUdpFlowConnection::new")
            && !runtime.contains("VmessUdpFlowHandle")
            && !runtime.contains("managed_tuple_udp_connection")
            && establish.contains("managed_tuple_udp_connection")
            && !runtime.contains("impl ManagedUdpConnection for vmess::VmessUdpFlowConnection")
            && !runtime.contains("spawn_tuple_response_bridge")
            && !runtime.contains(".recv().await")
            && !runtime.contains("EngineError::Io")
            && protocol.contains("pub async fn establish_udp_flow_with_initial_packet")
            && protocol.contains("pub async fn establish_flow_with_initial_packet")
            && protocol.contains("pub fn start_flow_with_initial_packet")
            && protocol.contains("pub fn mux_open_identity")
            && protocol.contains("pub fn mux_pool_identity")
            && protocol.contains("pub struct VmessUdpMuxOpenIdentity")
            && protocol.contains("pub fn into_connection")
            && protocol.contains("pub fn start_udp_flow_with_initial_packet")
            && protocol.contains("pub fn spawn_udp_flow")
            && protocol.contains("tokio::select!")
            && protocol.contains("struct VmessUdpFlowSender")
            && !protocol.contains("pub struct VmessUdpFlowSender")
            && protocol.contains("pub struct VmessUdpFlowConnection")
            && protocol.contains("pub struct VmessUdpFlowSession")
            && protocol.contains("pub type VmessUdpFlowResponseReceiver")
            && !protocol.contains("pub type VmessUdpFlowResponses")
            && protocol.contains("pub struct VmessInitialUdpFlowPacket"),
        "VMess UDP runtime should keep protocol flow I/O inside protocols/vmess and leave proxy manager as cache/bridge glue"
    );
    assert!(
        read("src/runtime/udp_flow/managed/cache.rs").contains("SharedManagedUdpConnection")
            && !model.contains("SharedManagedUdpConnection")
            && !model.contains("vmess::VmessUdpFlowConnection"),
        "VMess UDP manager cache should store a neutral stream UDP connection object"
    );
    for forbidden in [
        "VmessUdpFlowStream",
        "VmessUdpResponse",
        "VmessUdpFlowIo",
        "establish_udp_flow_stream",
        "mpsc::channel::<vmess::VmessUdpFlowPacket>",
        "mpsc::channel::<UdpFlowPacket>",
        "broadcast::channel::<VmessUdpResponse>",
        "tokio::spawn",
        "encode_vmess_udp_flow_packet",
        "send_vmess_udp_flow_packet",
        "spawn_vmess_udp_packet_flow",
    ] {
        assert!(
            !transport.contains(forbidden) && !proxy_transport.contains(forbidden),
            "zero-transport/proxy transport facade must not own VMess UDP flow runtime; found `{forbidden}`"
        );
    }
    assert!(
        protocol.contains("pub struct VmessUdpFlowHandle")
            && protocol.contains("struct VmessUdpFlowSender")
            && !protocol.contains("pub struct VmessUdpFlowSender")
            && protocol.contains("pub struct VmessUdpFlowConnection")
            && protocol.contains("pub struct VmessUdpFlowSession")
            && protocol.contains("pub struct VmessInitialUdpFlowPacket")
            && protocol.contains("pub type VmessUdpFlowResponse")
            && protocol.contains("pub type VmessUdpFlowResponseReceiver")
            && !protocol.contains("pub type VmessUdpFlowResponses")
            && !protocol.contains("pub struct VmessUdpFlowSend")
            && !protocol.contains("pub async fn open_udp_flow")
            && !protocol.contains("pub fn open_mux_udp_flow")
            && !protocol.contains("mpsc::channel::<VmessUdpFlowPacket>")
            && protocol.contains("fn spawn_udp_flow_task")
            && protocol.contains("broadcast::channel")
            && protocol.contains("tokio::spawn")
            && !protocol.contains("pub fn encode_udp_flow_initial_packet")
            && protocol.contains("pub struct VmessUdpFlowIo")
            && protocol.contains("impl VmessUdpFlowIo")
            && protocol.contains("pub fn encode_packet")
            && protocol.contains("pub struct VmessUdpFlowPacket")
            && protocol.contains("pub(crate) fn encode_udp_flow_packet")
            && protocol.contains("pub(crate) fn decode_udp_flow_packet"),
        "protocols/vmess should own VMess UDP packet IO helpers and protocol flow pump handles"
    );
}

#[test]
fn vmess_mux_pool_model_lives_outside_runtime_root() {
    let root = read("src/adapters/vmess/mux_pool.rs");
    let model = read("src/adapters/vmess/mux_pool/model.rs");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read protocols/vmess/src/mux.rs");
    let old_root = manifest_dir().join("src/protocol_runtime/vmess_mux_pool.rs");
    let old_dir = manifest_dir().join("src/protocol_runtime/vmess_mux_pool");

    for forbidden in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxConn",
        "struct VmessMuxOpenRequest",
        "struct VmessMuxConnectionPool",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess adapter mux_pool.rs should keep pool/request models in mux_pool/model.rs; found `{forbidden}`"
        );
    }

    for forbidden in ["struct VmessMuxPoolKey", "enum VmessMuxTransportKey"] {
        assert!(
            !model.contains(forbidden),
            "VMess mux cache identity should live in protocols/vmess, not adapter model; found `{forbidden}`"
        );
    }
    for forbidden in [
        "id: [u8; 16]",
        "cipher_name: String",
        "cipher: vmess::VmessCipher",
    ] {
        assert!(
            !model.contains(forbidden),
            "VMess mux open request should carry protocol-owned identity, not `{forbidden}`"
        );
    }

    for required in [
        "struct VmessMuxOpenRequest",
        "struct VmessMuxConnectionPool",
        "identity: vmess::mux::VmessMuxIdentity",
    ] {
        assert!(
            model.contains(required),
            "VMess MUX proxy I/O request model should live in adapters/vmess/mux_pool/model.rs; missing `{required}`"
        );
    }
    for required in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxIdentity",
        "impl VmessMuxPoolKey",
    ] {
        assert!(
            protocol_mux.contains(required),
            "VMess MUX protocol cache identity should live in protocols/vmess/src/mux.rs; missing `{required}`"
        );
    }
    assert!(
        !old_root.exists() && !old_dir.exists(),
        "VMess MUX pool should not live under protocol_runtime"
    );

    assert!(
        !root.contains("VmessMuxStream::new_with_network"),
        "VMess mux pool runtime should use the protocol mux stream helper instead of constructing VmessMuxStream directly"
    );
    for forbidden in [
        "vmess::mux_cool_session",
        "vmess::VmessOutbound",
        "VmessAeadStream::outbound",
        "establish_tcp_session",
        "read_mux_frame_from_tokio",
        "vmess::mux_stream_with_network",
        "vmess::read_mux_stream_frame",
        "tokio::spawn",
        "write_all(&frame)",
        "mpsc::unbounded_channel::<Vec<u8>>()",
        "struct VmessMuxConn",
        "read_mux_stream_frame(&mut reader)",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess adapter mux pool should not own protocol MUX connection or pump detail `{forbidden}`"
        );
    }
    assert!(
        root.contains("key.establish_mux_outbound_stream(metered)")
            && !root.contains("vmess::mux::establish_mux_outbound_stream")
            && !root.contains("key.uuid()")
            && !root.contains("key.cipher()"),
        "VMess mux pool runtime should ask the protocol key to establish MUX streams without unpacking identity fields"
    );
    assert!(
        root.contains("key.clone().into_pool_conn(stream, request.max_concurrency)")
            && !root.contains("vmess::mux::VmessMuxConn::new"),
        "VMess adapter mux pool should ask the protocol key to wrap established streams as pool connections"
    );
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read protocols/vmess mux source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vmess/src/lib.rs"))
        .expect("read protocols/vmess lib source");
    for required in [
        "pub struct VmessMuxConn",
        "pub fn new<S>",
        "pub fn open_stream",
        "fn spawn_mux_write_relay",
        "fn spawn_mux_read_relay",
        "tokio::spawn",
        "read_mux_server_event(&mut reader)",
        "pub async fn establish_mux_outbound_stream",
        "pub fn into_pool_conn",
    ] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vmess should own VMess MUX connection mechanics through `{required}`"
        );
    }
    assert!(
        protocol_lib.contains("pub mod mux;") && !protocol_lib.contains("pub use mux::"),
        "protocols/vmess should expose MUX details through vmess::mux instead of root re-exports"
    );
    for private_root_item in [
        "VmessInboundMuxAction",
        "VmessInboundMuxSession",
        "VmessInboundMuxWriter",
        "VmessMuxConn",
        "VmessMuxFrameEncoder",
        "VmessMuxIdentity",
        "VmessMuxPoolKey",
        "VmessMuxServerEvent",
        "VmessMuxStream",
        "VmessMuxTransportKey",
        "MuxFrame",
        "MUX_MAX_DATA_LEN",
        "MUX_MAX_META_LEN",
        "MUX_NETWORK_TCP",
        "MUX_NETWORK_UDP",
        "MUX_OPTION_DATA",
        "MUX_OPTION_ERROR",
        "MUX_STATUS_END",
        "MUX_STATUS_KEEP",
        "MUX_STATUS_KEEP_ALIVE",
        "MUX_STATUS_NEW",
        "read_mux_server_event",
        "read_mux_stream_frame",
        "establish_mux_outbound_stream",
        "mux_stream_with_network",
    ] {
        assert!(
            protocol_mux.contains(private_root_item) && !protocol_lib.contains(private_root_item),
            "VMess MUX detail `{private_root_item}` should stay under vmess::mux instead of the crate root"
        );
    }
}

#[test]
fn vless_vmess_udp_packet_models_do_not_expose_raw_fields() {
    let vless_shared = fs::read_to_string(repo_root().join("protocols/vless/src/shared.rs"))
        .expect("read protocols/vless/src/shared.rs");
    let vmess_udp = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read protocols/vmess/src/udp.rs");
    let socks5_shared = fs::read_to_string(repo_root().join("protocols/socks5/src/shared.rs"))
        .expect("read protocols/socks5/src/shared.rs");

    for (source_name, source, struct_name) in [
        (
            "protocols/vless/src/shared.rs",
            vless_shared.as_str(),
            "VlessUdpPacket",
        ),
        (
            "protocols/vless/src/shared.rs",
            vless_shared.as_str(),
            "VlessUdpFlowPacket",
        ),
        (
            "protocols/vmess/src/udp.rs",
            vmess_udp.as_str(),
            "VmessUdpPacket",
        ),
        (
            "protocols/vmess/src/udp.rs",
            vmess_udp.as_str(),
            "VmessUdpFlowPacket",
        ),
        (
            "protocols/vmess/src/udp.rs",
            vmess_udp.as_str(),
            "VmessInboundUdpPayload",
        ),
        (
            "protocols/socks5/src/shared.rs",
            socks5_shared.as_str(),
            "Socks5UdpPacket",
        ),
    ] {
        let struct_body = ["pub struct", "pub(crate) struct"]
            .iter()
            .find_map(|visibility| {
                source
                    .split(&format!("{visibility} {struct_name} {{"))
                    .nth(1)
            })
            .and_then(|tail| tail.split("}\n").next())
            .unwrap_or_else(|| panic!("{source_name} should define {struct_name}"));
        for forbidden in [
            "pub target: Address",
            "pub port: u16",
            "pub payload: Vec<u8>",
        ] {
            assert!(
                !struct_body.contains(forbidden),
                "{source_name} {struct_name} should expose UDP packet contents through methods, not raw field `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_udp_packet_models_do_not_expose_raw_fields() {
    let hysteria2_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read protocols/hysteria2/src/udp.rs");
    let trojan_outbound = fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
        .expect("read protocols/trojan/src/outbound.rs");
    let shadowsocks_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read protocols/shadowsocks/src/outbound.rs");
    let mieru_udp = fs::read_to_string(repo_root().join("protocols/mieru/src/udp.rs"))
        .expect("read protocols/mieru/src/udp.rs");
    let mieru_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read protocols/mieru/src/outbound.rs");

    for (source_name, source, struct_name, forbidden_fields) in [
        (
            "protocols/hysteria2/src/udp.rs",
            hysteria2_udp.as_str(),
            "Hysteria2UdpPacket",
            &[
                "pub session_id: u16",
                "pub packet_id: u16",
                "pub target: Address",
                "pub port: u16",
                "pub payload: Vec<u8>",
            ][..],
        ),
        (
            "protocols/hysteria2/src/udp.rs",
            hysteria2_udp.as_str(),
            "Hysteria2UdpFlowPacket",
            &[
                "pub target: Address",
                "pub port: u16",
                "pub payload: Vec<u8>",
            ][..],
        ),
        (
            "protocols/trojan/src/outbound.rs",
            trojan_outbound.as_str(),
            "TrojanUdpPacket",
            &[
                "pub target: Address",
                "pub port: u16",
                "pub payload: Vec<u8>",
            ][..],
        ),
        (
            "protocols/shadowsocks/src/outbound.rs",
            shadowsocks_outbound.as_str(),
            "ShadowsocksUdpPacket",
            &[
                "pub target: Address",
                "pub port: u16",
                "pub payload: Vec<u8>",
            ][..],
        ),
        (
            "protocols/mieru/src/udp.rs",
            mieru_udp.as_str(),
            "MieruInboundUdpPacket",
            &[
                "pub target: Address",
                "pub port: u16",
                "pub payload: Vec<u8>",
            ][..],
        ),
        (
            "protocols/mieru/src/outbound.rs",
            mieru_outbound.as_str(),
            "MieruUdpFlowPacket",
            &[
                "pub target: Address",
                "pub port: u16",
                "pub payload: Vec<u8>",
            ][..],
        ),
        (
            "protocols/mieru/src/udp.rs",
            mieru_udp.as_str(),
            "MieruUdpAssociatePayload",
            &["pub payload: Vec<u8>"][..],
        ),
    ] {
        let struct_body = source
            .split(&format!("pub struct {struct_name} {{"))
            .nth(1)
            .and_then(|tail| tail.split("}\n").next())
            .unwrap_or_else(|| panic!("{source_name} should define {struct_name}"));
        for forbidden in forbidden_fields {
            assert!(
                !struct_body.contains(forbidden),
                "{source_name} {struct_name} should expose UDP packet contents through methods, not raw field `{forbidden}`"
            );
        }
    }
}

#[test]
fn vmess_mux_pool_transport_opening_lives_in_transport_crate() {
    let root = read("src/adapters/vmess/mux_pool.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/vmess_transport.rs"))
        .expect("read crates/transport/src/vmess_transport.rs");

    for forbidden in [
        "connect_vmess_transport",
        "zero_transport::tls::connect_tls_upstream",
        "zero_transport::grpc::connect_grpc",
        "zero_transport::ws::connect_ws",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess mux pool should not own transport opening detail; found `{forbidden}`"
        );
    }

    assert!(
        root.contains("crate::transport::VmessTransportOptions")
            && root.contains("VmessTransportConnector::new")
            && root.contains(".connect(socket, &key.server, key.port)"),
        "VMess mux pool should request VMess transport helpers instead of opening TLS/WS/gRPC directly"
    );
    assert!(
        transport.contains("pub struct VmessTransportConnector")
            && transport.contains("pub struct VmessTransportOptions")
            && transport.contains("tls::connect_tls_upstream")
            && transport.contains("grpc::connect_grpc")
            && transport.contains("ws::connect_ws"),
        "zero-transport should own VMess mux transport opening helpers"
    );
}

#[test]
fn vmess_mux_pool_receives_adapter_parsed_cipher() {
    let root = read("src/adapters/vmess/mux_pool.rs");
    let model = read("src/adapters/vmess/mux_pool/model.rs");
    let tcp_adapter = read("src/adapters/vmess/tcp.rs");
    let udp_root = read("src/adapters/vmess/udp.rs");
    let udp_flow = read("src/adapters/vmess/udp/flow.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vmess/src/outbound.rs"))
        .expect("read vmess protocol outbound source");

    assert!(
        !root.contains("VmessCipher::from_name"),
        "VMess mux pool should receive parsed cipher values from adapter-owned paths"
    );
    assert!(
        model.contains("identity: vmess::mux::VmessMuxIdentity")
            && root.contains("vmess::mux::VmessMuxPoolKey::from_identity")
            && root.contains("vmess::mux::transport_key_from_config")
            && !root.contains("VmessMuxTransportKey::Grpc")
            && !root.contains("VmessMuxTransportKey::Ws")
            && !root.contains("VmessMuxTransportKey::RawTls")
            && !root.contains("service_names: grpc.service_names.clone()")
            && !root.contains("path: ws.path.clone()")
            && !model.contains("struct VmessMuxPoolKey"),
        "VMess mux pool request should carry parsed identity and ask protocols/vmess to build transport cache identity"
    );
    assert!(
        !tcp_adapter.contains("VmessCipher::from_name")
            && tcp_adapter.contains("VmessTcpConnectConfig::from_config")
            && protocol_outbound.contains("VmessCipher::from_name")
            && udp_flow.contains("vmess::udp::udp_flow_config_from_config")
            && !udp_flow.contains("vmess::udp::VmessUdpFlowConfig::new")
            && !udp_root.contains("vmess::parse_udp_identity")
            && !udp_root.contains("VmessCipher::from_name")
            && !udp_root.contains("VmessUdpFlowConfig::new"),
        "VMess TCP and UDP adapters should delegate cipher parsing to protocols/vmess config builders while adapter roots stay facades"
    );
}

#[test]
fn vless_mux_pool_model_lives_outside_runtime_root() {
    let root = read("src/adapters/vless/mux_pool.rs");
    let model = read("src/adapters/vless/mux_pool/model.rs");
    let protocol_mux_pool = fs::read_to_string(repo_root().join("protocols/vless/src/mux_pool.rs"))
        .expect("read protocols/vless/src/mux_pool.rs");
    let old_root = manifest_dir().join("src/protocol_runtime/vless_mux_pool.rs");
    let old_dir = manifest_dir().join("src/protocol_runtime/vless_mux_pool");

    for forbidden in ["struct MuxConnectionPool", "struct VlessMuxOpenRequest"] {
        assert!(
            !root.contains(forbidden),
            "VLESS adapter mux_pool.rs should keep proxy-layer pool/request models in mux_pool/model.rs; found `{forbidden}`"
        );
    }

    for forbidden in ["id: &'a [u8; 16]", "uuid: [u8; 16]"] {
        assert!(
            !model.contains(forbidden),
            "VLESS mux open request should carry protocol-owned identity, not `{forbidden}`"
        );
    }

    for required in [
        "struct MuxConnectionPool",
        "struct VlessMuxOpenRequest",
        "identity: vless::mux_pool::MuxIdentity",
    ] {
        assert!(
            model.contains(required),
            "VLESS MUX pool model should live in adapters/vless/mux_pool/model.rs; missing `{required}`"
        );
    }
    for required in [
        "pub struct MuxIdentity",
        "impl MuxIdentity",
        "impl PoolKey",
        "pub fn from_identity",
        "pub fn transport_key_from_config(",
    ] {
        assert!(
            protocol_mux_pool.contains(required),
            "VLESS mux protocol identity should live in protocols/vless/src/mux_pool.rs; missing `{required}`"
        );
    }
    assert!(
        !old_root.exists() && !old_dir.exists(),
        "VLESS MUX pool should not live under protocol_runtime"
    );
    for forbidden in [
        "vless::encode_new_stream",
        "vless::encode_data_frame",
        "vless::encode_end_frame",
        "vless::MuxCrypto",
        "MuxCrypto::new",
        "encode_mux_new_stream",
        "encode_mux_data_frame",
        "encode_mux_end_frame",
        "new_mux_crypto",
        "encrypt_mux_payload",
        "decrypt_mux_payload",
        "tokio::spawn",
        "read_exact(&mut buf)",
        "write_all(&frame)",
        "mpsc::unbounded_channel::<Vec<u8>>()",
        "zero_core::Address::Ipv4([0, 0, 0, 0])",
    ] {
        assert!(
            !root.contains(forbidden),
            "VLESS adapter mux pool should not own protocol MUX frame or pump detail `{forbidden}`"
        );
    }
    for required in [
        "open_mux_tcp_stream",
        "open_mux_udp_stream",
        "establish_mux_connection",
        "into_pool_conn",
        "vless::mux_pool::transport_key_from_config",
    ] {
        assert!(
            root.contains(required),
            "VLESS adapter mux pool should delegate protocol MUX stream mechanics through `{required}`"
        );
    }
    for forbidden in [
        "TransportKey::Tls",
        "TransportKey::Reality",
        "TransportKey::Raw",
        "public_key: r.public_key.clone()",
        "server_name.clone().unwrap_or_else",
    ] {
        assert!(
            !root.contains(forbidden),
            "VLESS adapter mux pool should ask protocols/vless to build transport cache identity; found `{forbidden}`"
        );
    }
    assert!(
        !root.contains("VlessOutbound")
            && !root.contains("establish_mux(&mut metered")
            && !root.contains("key.uuid()")
            && !root.contains("MuxPoolConn::new("),
        "VLESS adapter mux pool should not unpack protocol identity or construct MUX connections directly"
    );
    let protocol_mux_pool = fs::read_to_string(repo_root().join("protocols/vless/src/mux_pool.rs"))
        .expect("read protocols/vless mux_pool source");
    for required in [
        "pub fn open_mux_tcp_stream",
        "pub fn open_mux_udp_stream",
        "pub async fn establish_mux_connection",
        "pub fn into_pool_conn",
        "impl MuxPoolConn",
        "tokio::spawn",
        "encrypt_mux_payload",
        "decrypt_mux_payload",
        "encode_mux_data_frame",
        "encode_mux_end_frame",
    ] {
        assert!(
            protocol_mux_pool.contains(required),
            "protocols/vless should own VLESS MUX stream mechanics through `{required}`"
        );
    }
}

#[test]
fn protocol_mux_pools_are_adapter_owned_not_proxy_fields() {
    let runtime = read("src/runtime.rs");
    let vless_adapter = read("src/adapters/vless.rs");
    let vmess_adapter = read("src/adapters/vmess.rs");
    let vless_tcp = read("src/adapters/vless/tcp.rs");
    let vmess_tcp = read("src/adapters/vmess/tcp.rs");
    let vless_udp = read("src/adapters/vless/udp/flow.rs");
    let vmess_udp = read("src/adapters/vmess/udp/flow.rs");

    for forbidden in [
        "mux_pool: MuxConnectionPool",
        "vmess_mux_pool: VmessMuxConnectionPool",
        "vless_mux_pool",
        "vmess_mux_pool",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "Proxy runtime should not own protocol-named MUX pool field `{forbidden}`"
        );
    }
    assert!(
        runtime.contains("self.protocols.on_config_reloaded()"),
        "runtime reload should notify protocol inventory instead of clearing concrete protocol pools"
    );
    assert!(
        vless_adapter.contains("mux_pool: mux_pool::MuxConnectionPool")
            && vless_adapter.contains("fn on_config_reloaded(&self)")
            && vless_adapter.contains("self.mux_pool.evict_all()")
            && vless_tcp.contains("VlessMuxOpenRequest")
            && vless_tcp.contains(".mux_pool")
            && vless_udp.contains("mux_pool: &adapter.mux_pool"),
        "VLESS MUX pool should be owned by VlessAdapter and shared by its TCP/UDP paths"
    );
    assert!(
        vmess_adapter.contains("mux_pool: mux_pool::VmessMuxConnectionPool")
            && vmess_adapter.contains("fn on_config_reloaded(&self)")
            && vmess_adapter.contains("self.mux_pool.evict_all()")
            && vmess_tcp.contains("VmessMuxOpenRequest")
            && vmess_tcp.contains(".mux_pool")
            && vmess_udp.contains("mux_pool: &adapter.mux_pool"),
        "VMess MUX pool should be owned by VmessAdapter and shared by its TCP/UDP paths"
    );
}

#[test]
fn protocol_runtime_udp_and_mux_roots_do_not_reexport_request_models() {
    for (source, forbidden) in [
        ("src/adapters/vless/udp.rs", "VlessUdpStartFlow"),
        ("src/adapters/vless/udp.rs", "VlessUdpRelayTwoStream"),
        ("src/adapters/vless/udp.rs", "VlessUdpRelayFinalHopStart"),
        ("src/adapters/vless/udp.rs", "VlessUdpTransport"),
        ("src/adapters/vmess/udp.rs", "VmessUdpStartFlow"),
        ("src/adapters/vmess/udp.rs", "VmessUdpRelayFlowStart"),
    ] {
        let content = read(source);
        assert!(
            !content.contains(forbidden),
            "{source} should not re-export request model `{forbidden}`"
        );
    }

    assert!(
        read("src/adapters/vless/mux_pool.rs")
            .contains("pub(crate) use model::{MuxConnectionPool, VlessMuxOpenRequest};"),
        "VLESS mux pool root should expose only the adapter-owned pool/request facade"
    );
    assert!(
        read("src/adapters/vmess/mux_pool.rs")
            .contains("pub(crate) use model::{VmessMuxConnectionPool, VmessMuxOpenRequest};"),
        "VMess mux pool root should expose only the adapter-owned pool/request facade"
    );
}

#[test]
fn runtime_registered_owns_upstream_views_outside_protocol_runtime() {
    let old_root = manifest_dir().join("src/protocol_runtime/udp");
    let old_registered_name = manifest_dir().join("src/runtime/udp_flow/protocol_state");
    let registered_root = manifest_dir().join("src/runtime/udp_flow/registered");
    let state = read("src/runtime/udp_flow/registered/mod.rs");

    assert!(
        !old_root.exists()
            && !old_registered_name.exists()
            && registered_root.exists()
            && state.contains("RegisteredUpstreamAssociationView")
            && state.contains("ClosedRegisteredUpstreamAssociation")
            && !state.contains("ProtocolUpstreamUdpPoll"),
        "upstream lifecycle views should be owned by runtime::udp_flow::registered, not protocol_runtime::udp or the legacy protocol_state module"
    );
}

#[test]
fn runtime_registered_consumes_managed_flow_models_without_legacy_facade() {
    let old_root = manifest_dir().join("src/protocol_runtime/udp");
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let managed_state = read("src/runtime/udp_flow/managed/state.rs");
    let managed_flow = read("src/runtime/udp_flow/managed/flow.rs");

    for forbidden in ["mod flows", "pub(crate) use flows::"] {
        assert!(
            !state.contains(forbidden),
            "runtime::udp_flow::registered should not own a legacy flow model facade `{forbidden}`"
        );
    }
    assert!(
        !old_root.exists()
            && !manifest_dir()
                .join("src/runtime/udp_flow/registered/flows.rs")
                .exists()
            && state.contains("ManagedUdpFlowRequest")
            && state.contains("ManagedUdpFlowKind")
            && managed_flow.contains("ManagedDatagramFlow")
            && managed_flow.contains("ManagedStreamPacketFlow")
            && managed_flow.contains("ManagedRelayStreamFlow")
            && managed_state.contains("ManagedDatagramFlow {")
            && managed_state.contains("ManagedStreamPacketFlow {")
            && managed_state.contains("ManagedRelayStreamFlow {"),
        "managed UDP flow request models should live under runtime::udp_flow::managed; registered should only consume the neutral request"
    );
}

#[test]
fn mieru_udp_stream_pump_uses_protocol_flow_io_boundary() {
    let managed = read("src/adapters/mieru/udp/managed.rs");
    let connector = read("src/adapters/mieru/udp/managed/connector.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let stream = manifest_dir().join("src/adapters/mieru/udp/manager/stream.rs");
    let protocol = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/mieru/src/outbound.rs");
    let protocol = std::fs::read_to_string(protocol).expect("read mieru outbound protocol source");

    for forbidden in [
        "mieru::udp_flow_packet",
        ".encode_with(&mut flow_io)",
        "flow_io.push_encrypted_response",
        "flow_io.next_packet",
        "flow_io.write_flow_packet",
        "flow_io.decode_encrypted_response",
        "flow_io.read_flow_packets",
        "tokio::spawn",
        "mpsc::channel::<UdpFlowPacket>",
        "mieru::udp::MieruUdpFlowIo::establish_with_resume",
        "mieru::udp::spawn_udp_flow",
        "mieru::udp::MieruUdpFlowSession::new",
    ] {
        assert!(
            !managed.contains(forbidden)
                && !connector.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "Mieru UDP managed glue should delegate protocol encode/decode and pump detail to protocols/mieru; found `{forbidden}`"
        );
    }
    assert!(
        !stream.exists() && connector.contains("mieru::udp::establish_udp_flow_with_resume"),
        "Mieru UDP managed glue should call the protocol-owned established flow API without a dedicated stream wrapper"
    );
    assert!(
        connector.contains("mieru::udp::MieruUdpFlowConnection")
            && !managed.contains("mieru::udp::MieruUdpFlowConnection")
            && !managed.contains("mieru::udp::MieruUdpFlowSession"),
        "Mieru UDP managed glue should return the protocol-owned flow connection wrapper, not a raw flow session"
    );
    assert!(
        protocol.contains("pub async fn establish_udp_flow_with_resume")
            && protocol.contains("pub fn spawn_udp_flow")
            && protocol.contains("pub struct MieruUdpFlowHandle")
            && protocol.contains("struct MieruUdpFlowSender")
            && !protocol.contains("pub struct MieruUdpFlowSender")
            && protocol.contains("pub struct MieruUdpFlowConnection")
            && protocol.contains("pub struct MieruUdpFlowSession")
            && protocol.contains("pub type MieruUdpFlowResponseReceiver")
            && !protocol.contains("pub type MieruUdpFlowResponses")
            && protocol.contains("broadcast::channel::<MieruUdpFlowResponse>")
            && protocol.contains("mpsc::channel::<zero_core::UdpFlowPacket>")
            && protocol.contains("tokio::spawn")
            && protocol.contains("tokio::select!")
            && protocol.contains("write_flow_packet")
            && protocol.contains("read_flow_packets")
            && protocol.contains("pub fn decode_encrypted_response"),
        "Mieru protocol crate should own encrypted response buffering, UDP packet decode, and stream pump task"
    );
}

#[test]
fn h2_udp_stream_pump_uses_protocol_flow_resume_boundary() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let protocol = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/hysteria2/src/udp.rs");
    let protocol = std::fs::read_to_string(protocol).expect("read hysteria2 udp protocol source");

    for forbidden in [
        "hysteria2::udp_flow_packet",
        ".encode_with(&resume)",
        "resume.encode_flow_packet",
        "resume.decode_flow_packet",
        "flow_io.encode_packet",
        "flow_io.decode_packet",
        "send_datagram",
        "read_datagram",
        "tokio::spawn",
        "mpsc::channel::<UdpFlowPacket>",
        "hysteria2::udp::Hysteria2InitialUdpFlowPacket::from_parts",
        "hysteria2::udp::spawn_udp_flow",
        "hysteria2::udp::Hysteria2UdpFlowSession::new",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should delegate packet construction/encoding and pump detail to protocols/hysteria2; found `{forbidden}`"
        );
    }
    assert!(
        managed.contains("connector::establish_udp_flow_session")
            && managed.contains("ManagedDatagramFlowManager::new")
            && !managed.contains("Hysteria2Connector::from_udp_profile")
            && !managed.contains("connect_raw_with_udp_profile")
            && !managed.contains("resume.connector_profile()"),
        "Hysteria2 UDP managed glue should delegate QUIC/profile setup and protocol flow pumping to the adapter connector"
    );
    assert!(
        !managed
            .contains("impl ManagedUdpConnection for hysteria2::udp::Hysteria2UdpFlowConnection")
            && managed.contains("managed_tuple_udp_connection")
            && managed.contains("SharedManagedUdpConnection")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowSession"),
        "Hysteria2 UDP managed glue should expose a neutral managed connection wrapper, not implement runtime traits on the raw flow session"
    );
    assert!(
        protocol.contains("struct Hysteria2UdpFlowIo")
            && protocol.contains("pub fn encode_packet(&self")
            && protocol.contains("pub fn decode_packet(&self")
            && protocol.contains("pub fn start_udp_flow_with_initial_packet")
            && protocol.contains("pub fn spawn_udp_flow")
            && protocol.contains("pub struct Hysteria2UdpFlowHandle")
            && protocol.contains("struct Hysteria2UdpFlowSender")
            && !protocol.contains("pub struct Hysteria2UdpFlowSender")
            && protocol.contains("pub struct Hysteria2UdpFlowConnection")
            && protocol.contains("pub struct Hysteria2UdpFlowSession")
            && protocol.contains("pub type Hysteria2UdpFlowResponseReceiver")
            && protocol.contains("type Hysteria2UdpFlowResponses")
            && !protocol.contains("pub type Hysteria2UdpFlowResponses")
            && protocol.contains("broadcast::channel::<Hysteria2UdpFlowResponse>")
            && protocol.contains("mpsc::channel::<UdpFlowPacket>")
            && protocol.contains("send_datagram")
            && protocol.contains("read_datagram")
            && protocol.contains("tokio::spawn"),
        "Hysteria2 protocol crate should own flow packet I/O and UDP datagram pump tasks"
    );
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
    assert!(
        !root.contains("read_mux_frame_from_tokio"),
        "VMess inbound MUX runtime should use the protocol mux frame reader helper"
    );
    assert!(
        root.contains("vmess::mux::VmessInboundMuxSession::new()")
            && root.contains("mux_session.read_inbound_action(&mut reader)")
            && root.contains("vmess::mux::VmessInboundMuxAction"),
        "VMess inbound MUX runtime should consume protocol-owned semantic mux actions"
    );
    for forbidden in [
        "vmess::read_mux_stream_frame",
        "vmess::VmessMuxServerEvent",
        "vmess::MUX_STATUS_",
        ".status == vmess::MUX_STATUS_",
        "frame.status",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess inbound MUX runtime should not inspect raw mux frame status; found `{forbidden}`"
        );
    }
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read protocols/vmess/src/mux.rs");
    for required in [
        "VmessInboundMuxAction",
        "VmessInboundMuxSession",
        "VmessInboundMuxWriter",
        "VmessMuxServerEvent",
        "read_mux_server_event",
        "pub async fn read_inbound_action",
        "pub fn write_inbound_stream_data",
        "pub fn write_inbound_stream_payload",
        "pub fn end_inbound_stream",
        "pub fn data",
        "pub fn end",
        "pub(crate) fn frame",
    ] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vmess should own VMess MUX inbound action/writer API `{required}`"
        );
    }
    assert!(
        protocol_mux.contains("try_into_server_event") && protocol_mux.contains("impl From<VmessMuxServerEvent> for VmessInboundMuxAction"),
        "protocols/vmess should classify raw VMess MUX frames into server events and proxy-facing actions"
    );
    assert!(
        root.contains("VmessInboundMuxAction::OpenStream")
            && protocol_mux.contains("ProtocolType::Vmess")
            && !root.contains("network,")
            && !root.contains("Session::new(0,")
            && model.contains("pub(crate) session: Session")
            && !model.contains("pub(crate) target: Address")
            && !model.contains("pub(crate) port: u16"),
        "VMess inbound MUX new-stream Session conversion should be protocol-owned and exposed as an action"
    );
    assert!(
        root.contains(".end_inbound_stream(&writer, mux_session_id)")
            && root.contains(".write_inbound_stream_payload(&writer, mux_session_id")
            && !root.contains(".write_inbound_stream_data(&writer, mux_session_id")
            && root.contains("VmessInboundMuxWriter::new")
            && !root.contains("writer.end(")
            && !root.contains("writer.data(")
            && !root.contains("mux_session.next_action(")
            && !root.contains("vmess::mux::VmessMuxFrameEncoder")
            && !root.contains("frame_encoder.")
            && !model.contains("VmessMuxFrameEncoder")
            && !model.contains("mpsc::UnboundedSender<Vec<u8>>")
            && !root.contains("vmess::mux::read_mux_server_event")
            && !root.contains("vmess::mux::queue_end_stream")
            && !root.contains("vmess::mux::queue_keep_stream")
            && !root.contains("vmess::mux::encode_end_stream")
            && !root.contains("vmess::mux::encode_keep_stream"),
        "VMess inbound MUX runtime should use the protocol-owned inbound MUX session wrapper"
    );
    assert!(
        protocol_mux.contains("if payload.is_empty()")
            && protocol_mux.contains("self.end_inbound_stream(writer, session_id)")
            && protocol_mux
                .contains("self.write_inbound_stream_data(writer, session_id, payload)")
            && root.contains("write_inbound_stream_payload(&writer, mux_session_id, &[])"),
        "VMess inbound MUX downstream payload to DATA/END frame selection should live in protocols/vmess"
    );
    for required in ["queue_keep_stream", "queue_end_stream"] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vmess should own VMess MUX frame queue helper `{required}`"
        );
    }
}

#[test]
fn vmess_inbound_udp_response_encoding_stays_in_protocol_crate() {
    let helper = read("src/inbound/vmess/helpers.rs");
    let mux = read("src/inbound/vmess/mux.rs");
    let protocol_udp = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/vmess/src/udp.rs");
    let protocol_udp = fs::read_to_string(protocol_udp).expect("read vmess protocol udp source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vmess/src/lib.rs"))
        .expect("read vmess protocol lib source");
    let protocol_dispatch_parts = struct_block(&protocol_udp, "VmessInboundUdpDispatchParts");

    assert!(
        !helper.contains("vmess::build_udp_packet"),
        "VMess inbound helper should not build protocol UDP response packets directly"
    );
    assert!(
        !mux.contains("vmess::parse_udp_packet"),
        "VMess inbound MUX/session glue should delegate VMess UDP request parsing to protocols/vmess"
    );
    assert!(
        !mux.contains("socks5::parse_udp_packet")
            && !mux.contains("socks5::decode_udp_associate_response")
            && !mux.contains("udp_response::decode_socks5_upstream_response")
            && mux.contains("upstream_udp.recv_response")
            && !mux.contains("&pkt.target")
            && !mux.contains("pkt.port,")
            && !mux.contains("&pkt.payload")
            && !mux.contains("pkt.payload.len()")
            && !mux.contains("pkt.payload,"),
        "VMess inbound upstream response bridge should consume neutral registered upstream responses"
    );
    for forbidden in [
        "vmess::encode_udp_response",
        "vmess::encode_mux_udp_response",
        "vmess::decode_inbound_udp_payload",
        "vmess::encode_inbound_udp_response",
        "vmess::encode_inbound_mux_udp_response",
        "vmess::decode_inbound_udp_datagram",
    ] {
        assert!(
            !helper.contains(forbidden),
            "VMess inbound helper should use inbound-specific protocol helpers; found `{forbidden}`"
        );
    }
    assert!(
        !helper.contains("VmessInboundUdpPayload")
            && !helper.contains("VmessUdpPayloadMode")
            && !helper.contains("decode_vmess_udp_payload")
            && !helper.contains("encode_vmess_udp_response")
            && !helper.contains("encode_vmess_mux_udp_response")
            && !helper.contains("vmess::VmessInboundUdpCodec")
            && !mux.contains("fn vmess_udp_response_mode")
            && !mux.contains("vmess::VmessUdpPayloadMode")
            && !mux.contains("vmess::VmessUdpPayloadState")
            && !mux.contains("payload_mode")
            && !mux.contains("input.state")
            && !mux.contains("VmessInboundUdpCodec.decode_datagram")
            && !mux.contains(".response_mode(payload_mode)")
            && mux.contains("vmess::VmessInbound.udp_session_for(&session)")
            && mux.contains("udp_session.read_dispatch_parts_tokio")
            && mux.contains("udp_session.decode_mux_dispatch_parts")
            && !mux.contains("decode_dispatch_parts(&payload)")
            && !mux.contains("client.read(&mut client_buf)")
            && !mux.contains("decode_dispatch_parts(&client_buf[..n])")
            && mux.contains("udp_session.write_response_tokio")
            && mux.contains("udp_session.write_mux_response")
            && mux.contains("udp_session.write_response_to_socket_addr_tokio")
            && mux.contains("udp_session.write_mux_response_to_socket_addr")
            && !mux.contains("udp_session(session.target.clone(), session.port)")
            && !mux.contains("request.target")
            && !mux.contains("request.port")
            && !mux.contains("request.payload")
            && !mux.contains("request.client_session_id")
            && mux.contains("request.pipe_parts()")
            && !mux.contains("request.into_parts()")
            && !mux.contains("request.into_dispatch_parts()")
            && mux.contains("pkt.into_parts()")
            && !mux.contains("client_session_id: None")
            && !mux.contains("request.target().clone()")
            && !mux.contains("request.payload()")
            && !mux.contains("pkt.target()")
            && !mux.contains("pkt.payload()")
            && !mux.contains("Address::Ipv4")
            && !mux.contains("Address::Ipv6")
            && !mux.contains("VmessInboundUdpCodec.encode_response_for_state")
            && !mux.contains("VmessInboundUdpCodec.encode_mux_response_for_state")
            && protocol_udp.contains("struct VmessInboundUdpCodec")
            && protocol_udp.contains("struct VmessInboundUdpSession")
            && protocol_udp.contains("struct VmessInboundUdpRequest")
            && protocol_udp.contains("struct VmessInboundUdpDispatchParts")
            && !protocol_dispatch_parts.contains("pub target: Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>")
            && protocol_udp.contains("fn pipe_parts")
            && protocol_udp.contains("fn into_parts")
            && protocol_udp.contains("fn into_dispatch_parts")
            && protocol_udp.contains("pub fn decode_request")
            && protocol_udp.contains("pub fn decode_dispatch_parts")
            && protocol_udp.contains("pub fn decode_mux_dispatch_parts")
            && protocol_udp.contains("pub async fn read_dispatch_parts_tokio")
            && protocol_udp.contains("fn response_mode")
            && protocol_udp.contains("fn encode_response")
            && protocol_udp.contains("fn encode_response_for_state")
            && protocol_udp.contains("fn write_response_tokio")
            && protocol_udp.contains("fn write_response_to_ip_tokio")
            && protocol_udp.contains("fn write_response_to_socket_addr_tokio")
            && protocol_udp.contains("fn encode_mux_response")
            && protocol_udp.contains("fn encode_mux_response_for_state")
            && protocol_udp.contains("fn write_mux_response")
            && protocol_udp.contains("fn write_mux_response_to_ip")
            && protocol_udp.contains("fn write_mux_response_to_socket_addr")
            && protocol_udp.contains("fn decode_datagram"),
        "VMess inbound UDP packet framing and response mode selection should go through protocols/vmess inbound codec"
    );
    for private_helper in [
        "decode_inbound_udp_datagram",
        "encode_inbound_udp_response",
        "encode_inbound_mux_udp_response",
    ] {
        assert!(
            !protocol_udp.contains(&format!("pub fn {private_helper}"))
                && protocol_udp.contains(&format!("fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "VMess inbound UDP helper `{private_helper}` should stay private to protocols/vmess::udp and should not be re-exported"
        );
    }
    for root_private_helper in [
        "build_udp_packet",
        "parse_udp_packet",
        "encode_udp_response",
        "encode_mux_udp_response",
        "encode_udp_flow_packet",
        "decode_udp_flow_packet",
    ] {
        assert!(
            protocol_udp.contains(&format!("pub(crate) fn {root_private_helper}"))
                && !protocol_lib.contains(root_private_helper),
            "VMess low-level UDP helper `{root_private_helper}` should stay crate-private and should not be re-exported from protocols/vmess crate root"
        );
    }
    assert!(
        !protocol_udp.contains("fn encode_udp_flow_initial_packet")
            && !protocol_lib.contains("encode_udp_flow_initial_packet"),
        "obsolete VMess initial UDP flow packet helper should stay deleted instead of being re-exported"
    );
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
    assert!(
        root.contains("VlessInboundMuxWriter::new")
            && root.contains("let writer = mux_writer.clone()")
            && root.contains("writer,")
            && model.contains("writer: vless::mux::VlessInboundMuxWriter")
            && !model.contains("mpsc::UnboundedSender<(u16, Vec<u8>)>"),
        "VLESS inbound MUX task model should carry a protocol-owned writer instead of exposing the raw downlink channel"
    );
}

#[test]
fn vless_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let helper = read("src/inbound/vless/helpers.rs");
    let udp_session = read("src/inbound/vless/udp_session.rs");
    let mux = read("src/inbound/vless/mux.rs");
    let protocol_shared = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/vless/src/shared.rs");
    let protocol_shared =
        fs::read_to_string(protocol_shared).expect("read vless protocol shared source");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read vless protocol udp source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vless/src/lib.rs"))
        .expect("read vless protocol lib source");
    let protocol_dispatch_parts = struct_block(&protocol_shared, "VlessInboundUdpDispatchParts");

    for (source_name, source) in [
        ("inbound/vless/helpers.rs", helper.as_str()),
        ("inbound/vless/udp_session.rs", udp_session.as_str()),
        ("inbound/vless/mux.rs", mux.as_str()),
    ] {
        for forbidden in ["vless::build_udp_packet", "vless::parse_udp_packet"] {
            assert!(
                !source.contains(forbidden),
                "{source_name} should delegate VLESS UDP packet framing to protocols/vless; found `{forbidden}`"
            );
        }
    }
    for (source_name, source) in [
        ("inbound/vless/udp_session.rs", udp_session.as_str()),
        ("inbound/vless/mux.rs", mux.as_str()),
    ] {
        assert!(
            !source.contains("socks5::parse_udp_packet")
                && !source.contains("socks5::decode_udp_associate_response")
                && !source.contains("udp_response::decode_socks5_upstream_response")
                && source.contains("upstream_udp.recv_response")
                && !source.contains("&pkt.target")
                && !source.contains("pkt.port,")
                && !source.contains("&pkt.payload")
                && !source.contains("pkt.payload.len()")
                && !source.contains("pkt.payload,"),
            "{source_name} should consume neutral registered upstream responses"
        );
    }

    for forbidden in [
        "vless::decode_inbound_udp_packet",
        "vless::encode_udp_response",
        "vless::encode_mux_udp_response",
        "vless::decode_inbound_udp_datagram",
        "vless::encode_inbound_udp_response",
        "vless::encode_inbound_mux_udp_response",
    ] {
        assert!(
            !helper.contains(forbidden),
            "VLESS inbound helper should use inbound-specific protocol helpers; found `{forbidden}`"
        );
    }
    assert!(
        !helper.contains("VlessInboundUdpPacket")
            && !helper.contains("decode_vless_udp_packet")
            && !helper.contains("encode_vless_udp_response")
            && !helper.contains("encode_vless_mux_udp_response")
            && !helper.contains("vless::VlessInboundUdpCodec")
            && !udp_session.contains("vless::VlessInboundUdpCodec")
            && !mux.contains("vless::VlessInboundUdpCodec")
            && udp_session.contains("vless::VlessInbound.udp_session()")
            && udp_session.contains("udp_session.read_dispatch_parts_tokio")
            && !udp_session.contains("udp_session.decode_dispatch_parts")
            && !udp_session.contains("client.read(&mut buffer)")
            && !udp_session.contains("decode_dispatch_parts(&buffer[..n])")
            && udp_session.contains("udp_session.write_response_tokio")
            && udp_session.contains("udp_session.write_response_to_socket_addr_tokio")
            && !udp_session.contains("request.target")
            && !udp_session.contains("request.port")
            && !udp_session.contains("request.payload")
            && !udp_session.contains("request.client_session_id")
            && udp_session.contains("request.pipe_parts()")
            && !udp_session.contains("request.into_parts()")
            && !udp_session.contains("request.into_dispatch_parts()")
            && udp_session.contains("pkt.into_parts()")
            && !udp_session.contains("client_session_id: None")
            && !udp_session.contains("request.target().clone()")
            && !udp_session.contains("request.payload()")
            && !udp_session.contains("pkt.target()")
            && !udp_session.contains("pkt.payload()")
            && !udp_session.contains("zero_core::Address::Ipv4")
            && !udp_session.contains("zero_core::Address::Ipv6")
            && !udp_session.contains("VlessInboundUdpCodec.encode_response")
            && mux.contains("vless::VlessInbound.udp_session()")
            && mux.contains("udp_session.decode_mux_dispatch_parts")
            && !mux.contains("decode_dispatch_parts(&payload)")
            && mux.contains("udp_session.send_mux_response")
            && mux.contains("udp_session.send_mux_response_to_socket_addr")
            && mux.contains("writer.end_inbound_stream")
            && mux.contains("mux.send_inbound_stream_payload")
            && !mux.contains("writer.end(")
            && !mux.contains("request.target")
            && !mux.contains("request.port")
            && !mux.contains("request.payload")
            && !mux.contains("request.client_session_id")
            && mux.contains("request.pipe_parts()")
            && !mux.contains("request.into_parts()")
            && !mux.contains("request.into_dispatch_parts()")
            && mux.contains("pkt.into_parts()")
            && !mux.contains("client_session_id: None")
            && !mux.contains("request.target().clone()")
            && !mux.contains("request.payload()")
            && !mux.contains("pkt.target()")
            && !mux.contains("pkt.payload()")
            && !mux.contains("zero_core::Address::Ipv4")
            && !mux.contains("zero_core::Address::Ipv6")
            && !mux.contains("VlessInboundUdpCodec.encode_mux_response")
            && !udp_session.contains("decode_datagram")
            && !mux.contains("VlessInboundUdpCodec.decode_datagram")
            && protocol_shared.contains("struct VlessInboundUdpCodec")
            && protocol_shared.contains("struct VlessInboundUdpSession")
            && protocol_shared.contains("struct VlessInboundUdpRequest")
            && protocol_shared.contains("struct VlessInboundUdpDispatchParts")
            && !protocol_dispatch_parts.contains("pub target: Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>")
            && protocol_shared.contains("fn pipe_parts")
            && protocol_shared.contains("fn into_parts")
            && protocol_shared.contains("fn into_dispatch_parts")
            && protocol_shared.contains("fn decode_request")
            && protocol_shared.contains("fn decode_dispatch_parts")
            && protocol_shared.contains("fn decode_mux_dispatch_parts")
            && protocol_shared.contains("fn read_dispatch_parts_tokio")
            && protocol_shared.contains("fn decode_datagram")
            && protocol_shared.contains("fn encode_response")
            && protocol_shared.contains("fn write_response_tokio")
            && protocol_shared.contains("fn write_response_to_ip_tokio")
            && protocol_shared.contains("fn write_response_to_socket_addr_tokio")
            && protocol_shared.contains("fn encode_mux_response")
            && protocol_shared.contains("fn send_mux_response")
            && protocol_shared.contains("fn send_mux_response_to_ip")
            && protocol_shared.contains("fn send_mux_response_to_socket_addr"),
        "VLESS inbound UDP packet framing should go directly through the protocols/vless inbound codec from inbound glue"
    );
    for private_helper in [
        "decode_inbound_udp_datagram",
        "encode_inbound_udp_response",
        "encode_inbound_mux_udp_response",
    ] {
        assert!(
            !protocol_shared.contains(&format!("pub fn {private_helper}"))
                && protocol_shared.contains(&format!("fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "VLESS inbound UDP helper `{private_helper}` should stay private to protocols/vless::shared and should not be re-exported"
        );
    }
    for root_private_helper in [
        "build_udp_packet",
        "parse_udp_packet",
        "decode_inbound_udp_packet",
        "encode_udp_response",
        "encode_mux_udp_response",
        "encode_udp_flow_packet",
        "decode_udp_flow_packet",
        "encode_udp_flow_initial_packet",
    ] {
        let root_export = format!("{root_private_helper},");
        assert!(
            protocol_shared.contains(root_private_helper) && !protocol_lib.contains(&root_export),
            "VLESS low-level UDP helper `{root_private_helper}` should not be re-exported from protocols/vless crate root"
        );
    }
    for crate_private_helper in [
        "build_udp_packet",
        "parse_udp_packet",
        "decode_inbound_udp_packet",
        "encode_udp_response",
        "encode_mux_udp_response",
        "encode_udp_flow_packet",
        "decode_udp_flow_packet",
    ] {
        assert!(
            protocol_shared.contains(&format!("pub(crate) fn {crate_private_helper}")),
            "VLESS low-level UDP helper `{crate_private_helper}` should stay crate-private"
        );
    }
    assert!(
        protocol_shared.contains("pub struct VlessUdpPacketV2Codec")
            && protocol_shared.contains("pub(crate) fn build_udp_packet_v2")
            && protocol_shared.contains("pub(crate) fn parse_udp_packet_v2")
            && !protocol_lib.contains("build_udp_packet_v2")
            && !protocol_lib.contains("parse_udp_packet_v2")
            && protocol_udp.contains("VlessUdpPacketV2Codec")
            && !protocol_lib.contains("VlessUdpPacketV2Codec"),
        "VLESS v2 UDP packet helpers should be exposed through vless::udp::VlessUdpPacketV2Codec, not the crate root"
    );
}

#[test]
fn upstream_udp_response_decode_lives_behind_registered_handler() {
    let response = read("src/runtime/udp_flow/response.rs");
    let upstream = read("src/runtime/udp_flow/registered/upstream.rs");
    let state = read("src/runtime/udp_flow/state.rs");
    let socks5_runtime = read("src/adapters/socks5/udp/runtime.rs");

    assert_src_pattern_confined(
        "socks5::decode_udp_associate_response",
        &["src/inbound/socks5/udp_associate/upstream_response.rs"],
        &[],
        "raw SOCKS5 UDP response decoding should not leak into generic inbound response bridging",
    );
    assert!(
        response.contains("struct UpstreamUdpResponse")
            && response.contains("fn into_parts(self) -> (Address, u16, Vec<u8>)")
            && !response.contains("fn target(&self)")
            && !response.contains("fn payload(&self)")
            && upstream.contains("Result<UpstreamUdpResponse, EngineError>")
            && state.contains("recv_response")
            && socks5_runtime.contains("socks5::Socks5Inbound")
            && socks5_runtime.contains(".decode_response_parts(&buf[..read])")
            && !socks5_runtime.contains("Socks5InboundUdpCodec")
            && !socks5_runtime.contains("Socks5InboundUdpResponse"),
        "registered upstream handlers should consume protocol-owned response parts and expose neutral UpstreamUdpResponse values"
    );
}

#[test]
fn trojan_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let inbound = read("src/inbound/trojan.rs");
    let protocol_outbound = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/trojan/src/outbound.rs");
    let protocol_outbound =
        fs::read_to_string(protocol_outbound).expect("read trojan protocol outbound source");
    let protocol_inbound = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/trojan/src/inbound.rs");
    let protocol_inbound =
        fs::read_to_string(protocol_inbound).expect("read trojan protocol inbound source");
    let protocol_dispatch_parts = struct_block(&protocol_inbound, "TrojanInboundUdpDispatchParts");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/trojan/src/lib.rs"))
        .expect("read trojan protocol lib source");
    let protocol_shared = fs::read_to_string(repo_root().join("protocols/trojan/src/shared.rs"))
        .expect("read trojan protocol shared source");

    for forbidden in [
        "TrojanUdpPacket {",
        "UdpPacketStreamFraming<TrojanUdpPacket>",
        "TrojanOutbound as UdpPacketStreamFraming",
        "trojan::read_inbound_udp_packet",
        "trojan::write_udp_response",
        "trojan::read_udp_flow_packet",
        "trojan::write_udp_flow_packet",
        "socks5::parse_udp_packet",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "inbound/trojan.rs should delegate Trojan UDP packet framing to protocols/trojan; found `{forbidden}`"
        );
    }
    assert!(
        !inbound.contains("socks5::decode_udp_associate_response")
            && !inbound.contains("udp_response::decode_socks5_upstream_response")
            && inbound.contains("upstream_udp.recv_response")
            && !inbound.contains("&pkt.target")
            && !inbound.contains("pkt.port,")
            && !inbound.contains("&pkt.payload")
            && !inbound.contains("pkt.payload.len()")
            && !inbound.contains("pkt.payload,"),
        "Trojan inbound upstream response bridge should consume neutral registered upstream responses"
    );

    assert!(
        inbound.contains("trojan::TrojanInbound.udp_session()")
            && inbound.contains("udp_session.read_dispatch_parts(&mut client)")
            && !inbound.contains("udp_session.read_dispatch_view(&mut client)")
            && !inbound.contains("view.pipe_parts()")
            && inbound.contains("parts.pipe_parts()")
            && !inbound.contains("parts.into_pipe_parts()")
            && inbound.contains("udp_session.write_response(&mut client")
            && inbound.contains("udp_session")
            && inbound.contains(".write_response_to_socket_addr_tokio(&mut client")
            && !inbound.contains("request.into_dispatch_parts()")
            && !inbound.contains("request.client_session_id")
            && inbound.contains("pkt.into_parts()")
            && !inbound.contains("client_session_id: None")
            && !inbound.contains("request.target().clone()")
            && !inbound.contains("request.payload()")
            && !inbound.contains("pkt.target()")
            && !inbound.contains("pkt.payload()")
            && !inbound.contains("zero_core::Address::Ipv4")
            && !inbound.contains("zero_core::Address::Ipv6")
            && !inbound.contains("TrojanInboundUdpCodec")
            && !inbound.contains(".read_packet(&mut client)")
            && protocol_inbound.contains("struct TrojanInboundUdpCodec")
            && protocol_inbound.contains("struct TrojanInboundUdpSession")
            && protocol_inbound.contains("struct TrojanInboundUdpRequest")
            && protocol_inbound.contains("struct TrojanInboundUdpDispatchParts")
            && !protocol_dispatch_parts.contains("pub target: zero_core::Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>")
            && protocol_inbound.contains("fn into_parts")
            && protocol_inbound.contains("fn into_dispatch_parts")
            && protocol_inbound.contains("fn pipe_parts")
            && protocol_inbound.contains("fn into_pipe_parts")
            && protocol_inbound.contains("fn read_request")
            && protocol_inbound.contains("fn read_dispatch_parts")
            && protocol_inbound.contains("fn read_packet")
            && protocol_inbound.contains("fn write_response")
            && protocol_inbound.contains("fn write_response_to_ip")
            && protocol_inbound.contains("fn write_response_to_socket_addr_tokio")
            && protocol_outbound.contains("read_udp_flow_packet")
            && !protocol_outbound.contains("pub async fn read_udp_flow_packet")
            && protocol_outbound.contains("write_udp_flow_packet"),
        "Trojan inbound UDP packet framing should be owned by protocols/trojan inbound codec"
    );
    for private_helper in [
        "read_inbound_udp_packet",
        "read_udp_flow_packet",
        "write_udp_response",
        "write_udp_flow_packet",
    ] {
        assert!(
            protocol_outbound.contains(&format!("async fn {private_helper}"))
                && !protocol_outbound.contains(&format!("pub async fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "Trojan UDP helper `{private_helper}` should stay private to protocols/trojan::outbound and should not be re-exported"
        );
    }
    assert!(
        !protocol_outbound.contains("fn udp_flow_packet") && !protocol_lib.contains("udp_flow_packet"),
        "Trojan UDP flow packet constructor helper should be removed from the public protocol surface"
    );
    for private_root_item in [
        "read_password",
        "read_request",
        "write_password",
        "write_request",
        "ATYP_DOMAIN",
        "ATYP_IPV4",
        "ATYP_IPV6",
        "CMD_TCP",
        "CMD_UDP",
        "CRLF",
        "PASSWORD_HASH_LEN",
        "hex",
    ] {
        assert!(
            protocol_shared.contains(private_root_item) && !protocol_lib.contains(private_root_item),
            "Trojan wire helper `{private_root_item}` should stay under protocols/trojan::shared instead of the crate root"
        );
    }
}

#[test]
fn mieru_client_stream_model_lives_outside_inbound_root() {
    let root = read("src/inbound/mieru.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/mieru/src/lib.rs"))
        .expect("read mieru protocol lib source");
    let protocol_crypto = fs::read_to_string(repo_root().join("protocols/mieru/src/crypto.rs"))
        .expect("read mieru protocol crypto source");
    let protocol_segment = fs::read_to_string(repo_root().join("protocols/mieru/src/segment.rs"))
        .expect("read mieru protocol segment source");
    let protocol_session = fs::read_to_string(repo_root().join("protocols/mieru/src/session.rs"))
        .expect("read mieru protocol session source");
    let protocol_metadata = fs::read_to_string(repo_root().join("protocols/mieru/src/metadata.rs"))
        .expect("read mieru protocol metadata source");

    for forbidden in [
        "struct MieruClientStream",
        "impl AsyncRead for MieruClientStream",
        "impl AsyncWrite for MieruClientStream",
    ] {
        assert!(
            !root.contains(forbidden),
            "inbound/mieru.rs should keep client stream state in protocols/mieru; found `{forbidden}`"
        );
    }

    for required in [
        "pub struct MieruInboundStream",
        "impl<S> AsyncRead for MieruInboundStream<S>",
        "impl<S> AsyncWrite for MieruInboundStream<S>",
    ] {
        assert!(
            protocol_inbound.contains(required),
            "Mieru client stream state should live in protocols/mieru; missing `{required}`"
        );
    }
    assert!(
        !manifest_dir().join("src/inbound/mieru/model.rs").exists(),
        "Mieru proxy inbound should not keep a protocol data-phase stream model"
    );

    for forbidden in [
        "MieruInboundDataCodec",
        "MieruCipher",
        "derive_key",
        "try_derive_keys",
        "NonceConfig",
        "NoncePattern",
        "USER_HINT_LEN",
        "build_data_segment",
        "build_session_segment",
        "parse_segment",
        "Segment",
        "MAX_FRAGMENT",
        "MieruSession",
        "DataMetadata",
        "SessionMetadata",
        "METADATA_LEN",
    ] {
        assert!(
            !protocol_lib.contains(forbidden),
            "protocols/mieru crate root should not re-export data-phase private detail `{forbidden}`"
        );
    }
    assert!(
        protocol_crypto.contains("pub struct MieruCipher")
            && protocol_crypto.contains("pub fn derive_key")
            && protocol_segment.contains("pub fn build_data_segment")
            && protocol_segment.contains("pub fn parse_segment")
            && protocol_session.contains("pub struct MieruSession")
            && protocol_metadata.contains("pub struct DataMetadata"),
        "Mieru data-phase details should remain available from protocol-owned submodules"
    );
}

#[test]
fn mieru_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let inbound = read("src/inbound/mieru.rs");
    let protocol_udp = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/mieru/src/udp.rs");
    let protocol_udp = fs::read_to_string(protocol_udp).expect("read mieru protocol udp source");

    for forbidden in [
        "mieru::unwrap_udp_associate",
        "mieru::wrap_udp_associate",
        "mieru::decode_inbound_udp_packet",
        "mieru::encode_udp_response",
        "mieru::decode_udp_flow_packet",
        "mieru::encode_udp_flow_packet",
        "socks5::parse_udp_packet",
        "socks5::build_udp_packet",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "inbound/mieru.rs should delegate Mieru UDP packet framing to protocols/mieru; found `{forbidden}`"
        );
    }

    assert!(
        inbound.contains("mieru::MieruInbound.udp_session()")
            && inbound.contains("udp_session.read_dispatch_parts_tokio")
            && !inbound.contains("udp_session.read_dispatch_view_tokio")
            && !inbound.contains("udp_session.decode_dispatch_view")
            && !inbound.contains("client.read(&mut read_buf)")
            && !inbound.contains("decode_dispatch_view(&read_buf[..n])")
            && !inbound.contains("dispatch_view.into_pipe_parts()")
            && inbound.contains("dispatch_parts.pipe_parts()")
            && !inbound.contains("dispatch_parts.into_parts()")
            && !inbound.contains("request.into_dispatch_parts().into_parts()")
            && inbound.contains("UdpPipe::new(self, &mut dispatch)")
            && inbound.contains("protocol: zero_core::ProtocolType::Mieru")
            && !inbound.contains("tokio::net::UdpSocket::bind")
            && !inbound.contains("self.resolver.resolve")
            && !inbound.contains("udp_socket.send_to")
            && !inbound.contains("udp_session.record_request_target")
            && !inbound.contains("request.target_socket_addr()")
            && !inbound.contains("request.target_domain()")
            && !inbound.contains("request.resolved_target_socket_addr(ip)")
            && !inbound.contains("request.into_payload()")
            && !inbound.contains("request.payload()")
            && !inbound.contains("request.target_endpoint()")
            && !inbound.contains("record_target(addr, &request)")
            && !inbound.contains("zero_core::Address::Domain")
            && !inbound.contains("zero_core::Address::Ipv4")
            && !inbound.contains("zero_core::Address::Ipv6")
            && !inbound.contains("fn addr_from_ip")
            && inbound.contains(".write_response_for_target_tokio")
            && !inbound.contains(".write_response_tokio(&mut client")
            && !inbound.contains("mieru::udp::MieruUdpFlowCodec")
            && !inbound.contains("decode_packet")
            && !inbound.contains(".encode_packet(")
            && !inbound.contains("write_all(&frame)")
            && protocol_udp.contains("struct MieruInboundUdpSession")
            && protocol_udp.contains("struct MieruInboundUdpRequest")
            && protocol_udp.contains("struct MieruInboundUdpDispatchParts")
            && !protocol_udp.contains("struct MieruInboundUdpDispatchView")
            && protocol_udp.contains("fn pipe_parts")
            && protocol_udp.contains("fn into_pipe_parts")
            && protocol_udp.contains("fn target_endpoint")
            && protocol_udp.contains("fn into_dispatch_parts")
            && !protocol_udp.contains("fn into_dispatch_view")
            && protocol_udp.contains("fn target_socket_addr")
            && protocol_udp.contains("fn target_domain")
            && protocol_udp.contains("fn resolved_target_socket_addr")
            && protocol_udp.contains("fn into_payload")
            && protocol_udp.contains("fn decode_request")
            && protocol_udp.contains("fn decode_dispatch_parts")
            && !protocol_udp.contains("fn decode_dispatch_view")
            && !protocol_udp.contains("fn read_dispatch_view_tokio")
            && protocol_udp.contains("fn read_dispatch_parts_tokio")
            && protocol_udp.contains("fn record_target")
            && protocol_udp.contains("fn record_request_target")
            && protocol_udp.contains("struct MieruUdpFlowCodec")
            && protocol_udp.contains("fn encode_packet")
            && protocol_udp.contains("fn write_response_tokio")
            && protocol_udp.contains("fn decode_packet"),
        "Mieru inbound UDP packet framing should go through the protocols/mieru inbound UDP session"
    );
}

#[test]
fn socks5_udp_send_details_stay_out_of_udp_dispatch() {
    let managed = read("src/runtime/udp_dispatch/managed.rs");
    let forward = read("src/runtime/udp_dispatch/forward.rs");
    let socks5_adapter = read("src/adapters/socks5/udp.rs");
    let socks5_flow = read("src/adapters/socks5/udp/flow.rs");

    for forbidden in [
        "Socks5UdpAssociation {",
        "send_socks5_udp_packet",
        "UpstreamAssociationCloseReason::Dropped",
        "log_udp_upstream_association_dropped",
        "record_udp_upstream_send_failure",
    ] {
        assert!(
            !managed.contains(forbidden) && !socks5_adapter.contains(forbidden),
            "managed UDP bridge and SOCKS5 adapter facade should delegate packet send details to adapter-owned SOCKS5 UDP runtime; found `{forbidden}`"
        );
    }
    for source in [&forward, &socks5_adapter] {
        assert!(
            !source.contains("Socks5UdpSend"),
            "UDP forward/adapters should use the neutral managed UDP bridge without constructing protocol-runtime request models"
        );
    }
    assert!(
        managed.contains("send_managed_udp")
            && managed.contains("start_tracked_managed_udp")
            && managed.contains("start_tracked_managed_relay")
            && managed.contains("forward_managed_relay_flow")
            && socks5_flow.contains("ManagedRelayStart")
            && socks5_flow.contains(".start_tracked_managed_relay(")
            && !socks5_flow.contains("ManagedUdpFlowKind::RelayStream")
            && !socks5_flow.contains("ManagedUdpFlowResume::new")
            && !managed.contains("Socks5UdpPacketSend")
            && !managed.contains("username: Option<&'a str>")
            && !managed.contains("password: Option<&'a str>")
            && !forward.contains("socks5_relay_auth")
            && !forward.contains("username: auth.username")
            && !forward.contains("password: auth.password"),
        "SOCKS5 UDP should use the neutral managed UDP flow bridge"
    );
}

#[test]
fn socks5_udp_upstream_association_uses_outbound_tag_for_session_lookup() {
    let model = read("src/adapters/socks5/udp/model.rs");
    let send = read("src/adapters/socks5/udp/send.rs");
    let runtime = read("src/adapters/socks5/udp/runtime.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/socks5/src/udp.rs"))
        .expect("read socks5 udp");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/socks5/src/outbound.rs"))
            .expect("read socks5 outbound");
    let upstream = read("src/runtime/udp_flow/registered/upstream.rs");
    let response = read("src/inbound/socks5/udp_associate/upstream_response.rs");

    assert!(
        send.contains("resume.association_send(")
            && send.contains("association.into_target()")
            && !send.contains(".flow(")
            && !send.contains(".association_target()")
            && !send.contains("association.target()")
            && !send.contains("resume.association_target(")
            && protocol_udp.contains("struct Socks5UdpAssociationTarget")
            && protocol_outbound.contains("pub struct Socks5UdpAssociationSend")
            && protocol_outbound.contains("pub fn association_send(")
            && protocol_outbound.contains("pub fn into_target(self) -> Socks5UdpAssociationTarget")
            && protocol_outbound.contains("pub struct Socks5UdpFlowSpec")
            && protocol_outbound.contains("pub fn flow(")
            && protocol_udp.contains("Socks5UdpAssociationSend")
            && protocol_udp.contains("Socks5UdpFlowSpec")
            && protocol_udp.contains("outbound_tag: alloc::string::String")
            && !model
                .lines()
                .any(|line| line.trim() == "pub(super) struct Socks5UdpAssociation {")
            && !model
                .lines()
                .any(|line| line.trim() == "pub(super) tag: String,"),
        "SOCKS5 UDP association identity should be named outbound_tag, not a generic tag"
    );
    assert!(
        upstream.contains("send_upstream(inbound_tag, request)")
            && runtime.contains("let Some(outbound_tag) = request.outbound_tag")
            && runtime.contains("tag: outbound_tag")
            && !upstream.contains("tag: inbound_tag"),
        "SOCKS5 UDP runtime must pass the outbound tag into the upstream association through neutral upstream dispatch"
    );
    assert!(
        send.contains("resume.association_send(")
            && !send.contains("resume.association_target(")
            && !send.contains(".association_target()")
            && !send.contains("association.target()")
            && runtime.contains("let target = association.identity()")
            && runtime.contains("!a.identity().matches(&target)")
            && runtime.contains("target: association,")
            && runtime.contains("target.outbound_tag()")
            && !runtime.contains("active.outbound_tag != target.outbound_tag")
            && !runtime.contains("&target.outbound_tag")
            && !send.contains("association.tag"),
        "SOCKS5 UDP runtime state should store and match the relay outbound tag"
    );
    assert!(
        response.contains("association.outbound_tag")
            && response.contains("dispatch.upstream_response_session_id")
            && !response.contains("inbound_tag, &packet.target"),
        "SOCKS5 upstream responses should look up sessions by outbound tag"
    );
}

#[test]
fn socks5_udp_association_close_details_stay_out_of_udp_associate_loop() {
    let associate = read("src/inbound/socks5/udp_associate.rs");

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
    assert!(
        !manifest_dir()
            .join("src/protocol_runtime/socks5_udp_associate.rs")
            .exists(),
        "SOCKS5 UDP associate inbound glue should not live under protocol_runtime"
    );
    assert!(
        !manifest_dir()
            .join("src/protocol_runtime/socks5_udp_associate")
            .exists(),
        "SOCKS5 UDP associate submodules should not live under protocol_runtime"
    );

    let associate = read("src/inbound/socks5/udp_associate.rs");
    let chain_response = read("src/inbound/socks5/udp_associate/chain_response.rs");
    let cleanup = read("src/inbound/socks5/udp_associate/cleanup.rs");
    let dispatch = read("src/inbound/socks5/udp_associate/dispatch.rs");
    let direct_response = read("src/inbound/socks5/udp_associate/direct_response.rs");
    let idle_timeout = read("src/inbound/socks5/udp_associate/idle_timeout.rs");
    let relay_socket = read("src/inbound/socks5/udp_associate/relay_socket.rs");
    let setup = read("src/inbound/socks5/udp_associate/setup.rs");
    let upstream_response = read("src/inbound/socks5/udp_associate/upstream_response.rs");
    let adapter_active = read("src/adapters/socks5/udp/active.rs");
    let protocol_shared = fs::read_to_string(repo_root().join("protocols/socks5/src/shared.rs"))
        .expect("read protocols/socks5/src/shared.rs");
    let protocol_dispatch_parts = struct_block(&protocol_shared, "Socks5InboundUdpDispatchParts");

    for forbidden in [
        "UdpPipeInput",
        "ProtocolType::Socks5",
        "DnsResolver",
        ".resolver.resolve(",
        "async fn dispatch_packet",
        "async fn forward_direct_udp_response",
        "async fn forward_chain_response",
        "socks5::encode_udp_associate_response(&address_from_socket_addr",
        "direct_response_session_id",
        "record_session_outbound_rx",
        "record_session_inbound_tx",
        "failed to forward direct UDP response",
        "socks5::encode_udp_associate_response(target",
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
            && dispatch.contains(".decode_dispatch_parts_or_resolve_local_dns(")
            && !dispatch.contains(".resolver.resolve("),
        "SOCKS5 UDP packet dispatch should live in inbound/socks5/udp_associate/dispatch.rs"
    );
    assert!(
        direct_response.contains("async fn forward_direct_udp_response")
            && direct_response.contains("async fn forward_relay_socket_response")
            && direct_response.contains("async fn forward_dispatch_socket_response")
            && direct_response.contains("direct_response_session_id")
            && direct_response.contains("socks5::Socks5Inbound.udp_session()")
            && direct_response.contains(".send_response_to_client_socket_addr")
            && direct_response.contains("socket_addr_to_socket_address(client_addr)")
            && direct_response.contains("socket_addr_to_socket_address(sender)")
            && !direct_response.contains("fn socket_address_from_std")
            && !direct_response.contains("fn ip_address_from_std")
            && !direct_response.contains("Socks5UdpRelayEndpoint")
            && !direct_response.contains("Socks5UdpRelayError")
            && direct_response.contains("into_mapped(EngineError::from)")
            && !direct_response.contains("address_from_socket_addr(sender)")
            && !direct_response.contains("socket_addr_to_ip(sender)")
            && !direct_response.contains("udp_session.response_frame")
            && !direct_response.contains("Socks5InboundUdpCodec")
            && !direct_response.contains("socks5::encode_udp_associate_response("),
        "SOCKS5 UDP direct response metering should live in proxy while framing stays behind protocol helpers"
    );
    assert!(
        chain_response.contains("async fn handle_chain_result")
            && chain_response.contains("pub(super) struct ChainResponseRequest")
            && chain_response.contains("struct ForwardChainResponseRequest")
            && chain_response.contains("socks5::Socks5Inbound.udp_session()")
            && chain_response.contains(".send_response_to_client_target")
            && chain_response.contains("socket_addr_to_socket_address(client_addr)")
            && !chain_response.contains("socket_addr_to_ip(client_addr)")
            && !chain_response.contains("udp_session.response_frame")
            && !chain_response.contains("Socks5InboundUdpCodec")
            && !chain_response.contains("socks5::encode_udp_associate_response(")
            && chain_response.contains("failed to send SOCKS5 UDP chain response to client")
            && chain_response.contains("chain response task panicked"),
        "SOCKS5 UDP chain response result handling should live in proxy while framing stays behind protocol helpers"
    );
    assert!(
        setup.contains("send_success_response_with_bound")
            && !setup.contains("Socks5Reply")
            && !setup.contains("send_response_with_bound"),
        "SOCKS5 UDP associate setup should ask protocols/socks5 to choose the success reply frame"
    );
    for (path, source) in [
        ("dispatch.rs", &dispatch),
        ("direct_response.rs", &direct_response),
        ("chain_response.rs", &chain_response),
        ("upstream_response.rs", &upstream_response),
    ] {
        for forbidden in ["socks5::parse_udp_packet", "socks5::build_udp_packet"] {
            assert!(
                !source.contains(forbidden),
                "SOCKS5 UDP associate {path} should use semantic associate packet helpers instead of `{forbidden}`"
            );
        }
    }
    for (path, source) in [
        ("dispatch.rs", &dispatch),
        ("direct_response.rs", &direct_response),
        ("chain_response.rs", &chain_response),
        ("upstream_response.rs", &upstream_response),
    ] {
        for forbidden in [
            "socks5::decode_udp_associate_request",
            "socks5::decode_udp_associate_response",
            "socks5::encode_udp_associate_response_to_client",
        ] {
            assert!(
                !source.contains(forbidden),
                "SOCKS5 UDP associate {path} should call Socks5InboundUdpSession instead of raw helper `{forbidden}`"
            );
        }
    }
    assert!(
        dispatch.contains("socks5::Socks5Inbound.udp_session()")
            && dispatch.contains("udp_session")
            && dispatch.contains(".decode_dispatch_parts_or_resolve_local_dns(")
            && dispatch.contains("request.into_parts()")
            && !dispatch.contains("Socks5InboundUdpDispatchAction")
            && !dispatch.contains("decode_dispatch_action")
            && !dispatch.contains("udp_packet.into_dispatch_parts()")
            && dispatch.contains("protocol_overhead_len")
            && upstream_response.contains("socks5::Socks5Inbound.udp_session()")
            && upstream_response.contains("udp_session.response_session_key_parts")
            && !upstream_response.contains("udp_session.response_key")
            && !upstream_response.contains("response.into_parts()")
            && direct_response.contains("socks5::Socks5Inbound.udp_session()")
            && direct_response.contains(".send_response_to_client_socket_addr")
            && chain_response.contains("socks5::Socks5Inbound.udp_session()")
            && chain_response.contains(".send_response_to_client_target")
            && !dispatch.contains("Socks5InboundUdpCodec")
            && !upstream_response.contains("Socks5InboundUdpCodec")
            && !direct_response.contains("Socks5InboundUdpCodec")
            && !chain_response.contains("Socks5InboundUdpCodec"),
        "SOCKS5 UDP associate dispatch/attribution should use the protocol-owned inbound UDP session"
    );
    assert!(
        !dispatch.contains("udp_packet.into_parts()")
            && !dispatch.contains("udp_session.decode_request")
            && !dispatch.contains("udp_session.local_dns_domain_request")
            && !dispatch.contains("udp_session.request_dispatch_parts")
            && !dispatch.contains("client_session_id: None")
            && !dispatch.contains("request.target")
            && !dispatch.contains("request.port")
            && !dispatch.contains("request.payload")
            && !dispatch.contains("request.client_session_id")
            && !dispatch.contains("udp_packet.target()")
            && !dispatch.contains("udp_packet.port()")
            && !dispatch.contains("udp_packet.dns_domain_request()")
            && !upstream_response.contains("response.target()")
            && !upstream_response.contains("response.port()"),
        "SOCKS5 UDP associate dispatch should consume protocol-owned dispatch parts instead of rebuilding session facts"
    );
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/socks5/src/udp.rs"))
        .expect("read protocols/socks5/src/udp.rs");
    assert!(
        protocol_udp.contains("pub async fn send_response_to_client")
            && protocol_udp.contains("pub async fn send_response_to_client_endpoint")
            && protocol_udp.contains("pub async fn send_response_to_client_target")
            && protocol_udp.contains("pub async fn send_response_to_client_socket_addr")
            && protocol_udp.contains("SocketAddress")
            && protocol_udp.contains("fn address_from_ip")
            && protocol_udp.contains("pub fn decode_dispatch_action")
            && protocol_udp.contains("pub async fn decode_dispatch_parts_or_resolve_local_dns")
            && protocol_udp.contains("pub fn local_dns_domain_request")
            && protocol_udp.contains("pub fn response_session_key_parts")
            && protocol_udp.contains("pub fn into_mapped")
            && protocol_udp.contains("response_frame(")
            && protocol_udp.contains("response_key("),
        "protocols/socks5 should own UDP associate response framing and response attribution helpers"
    );
    assert!(
        !protocol_dispatch_parts.contains("pub target: Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>")
            && protocol_shared.contains("fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>)"),
        "SOCKS5 inbound UDP dispatch parts should expose a one-shot neutral parts API instead of public fields"
    );
    assert!(
        adapter_active.contains("into_mapped(EngineError::from)")
            && !adapter_active.contains("Socks5UdpRelayError::"),
        "SOCKS5 UDP adapter should use protocol-owned relay error mapping instead of unpacking relay error variants"
    );
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/socks5/src/inbound.rs"))
        .expect("read protocols/socks5/src/inbound.rs");
    assert!(
        protocol_inbound.contains("pub async fn send_success_response_with_bound")
            && protocol_inbound.contains("Socks5Reply::Succeeded"),
        "protocols/socks5 should own SOCKS5 UDP associate success reply selection"
    );
    for forbidden in [
        "udp_session.encode_response_to_client",
        "udp_session.decode_response",
        "packet.len() as u64 -",
    ] {
        assert!(
            !dispatch.contains(forbidden)
                && !upstream_response.contains(forbidden)
                && !direct_response.contains(forbidden)
                && !chain_response.contains(forbidden),
            "SOCKS5 UDP associate glue should not rebuild protocol packet accounting/framing detail `{forbidden}`"
        );
    }
    assert!(
        upstream_response.contains("async fn handle_upstream_response")
            && upstream_response.contains("upstream_association_view")
            && upstream_response.contains("upstream_response_session_id")
            && upstream_response.contains("record_udp_upstream_recv_failure")
            && upstream_response.contains("failed to attribute upstream UDP response"),
        "SOCKS5 UDP upstream response attribution and cleanup should live in inbound/socks5/udp_associate/upstream_response.rs"
    );
    assert!(
        idle_timeout.contains("fn handle_idle_timeout")
            && idle_timeout.contains("drop_idle_upstream_association")
            && idle_timeout.contains("log_udp_upstream_association_idle_timeout"),
        "SOCKS5 UDP idle timeout cleanup should live in inbound/socks5/udp_associate/idle_timeout.rs"
    );
    assert!(
        relay_socket.contains("async fn handle_relay_packet")
            && relay_socket.contains("pub(super) struct RelayPacketRequest")
            && relay_socket.contains("client_udp_addr.is_none")
            && relay_socket.contains("failed to process UDP packet")
            && relay_socket.contains("dropping udp packet from unexpected sender"),
        "SOCKS5 UDP relay socket packet classification should live in inbound/socks5/udp_associate/relay_socket.rs"
    );
    assert!(
        setup.contains("async fn setup_association")
            && setup.contains("send_success_response_with_bound")
            && !setup.contains("Socks5Reply")
            && !setup.contains("send_response_with_bound")
            && setup.contains("bind_addr(SocketAddr::new")
            && setup.contains("socks5 udp association ready")
            && setup.contains("drain_traffic"),
        "SOCKS5 UDP associate bind/response setup should live in inbound/socks5/udp_associate/setup.rs"
    );
    assert!(
        cleanup.contains("fn finish_dispatch")
            && cleanup.contains("finish_all")
            && cleanup.contains("log_completed_udp_flow"),
        "SOCKS5 UDP associate cleanup should live in inbound/socks5/udp_associate/cleanup.rs"
    );
}

#[test]
fn udp_dispatch_poll_refs_does_not_expose_socks5_association_type() {
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let flow_state = read("src/runtime/udp_flow/state.rs");

    for forbidden in [
        "Option<&crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation>",
        "self.socks5.upstream()",
        "Socks5UdpAssociationView",
        "ClosedSocks5UdpAssociation",
        "Socks5UdpRuntime",
        "pub(crate) fn socks5_upstream_view",
        "pub(crate) fn socks5_idle_deadline",
        "pub(crate) fn touch_socks5_idle",
        "pub(crate) fn drop_socks5_upstream",
        "pub(crate) fn drop_socks5_idle",
        "pub(crate) fn close_socks5_all",
    ] {
        assert!(
            !lifecycle.contains(forbidden),
            "UdpDispatch lifecycle should expose upstream UDP glue, not SOCKS5 association internals; found `{forbidden}`"
        );
    }
    assert!(
        lifecycle.contains("UpstreamUdpPoll")
            && flow_state.contains("recv_response")
            && flow_state.contains("recv_raw_packet")
            && lifecycle.contains("UpstreamAssociationView")
            && lifecycle.contains("ClosedUpstreamAssociation")
            && lifecycle.contains("upstream_association_view")
            && lifecycle.contains("touch_upstream_idle")
            && lifecycle.contains("drop_upstream_association")
            && lifecycle.contains("drop_idle_upstream_association"),
        "UdpDispatch lifecycle should expose neutral upstream UDP polling and association lifecycle models through UdpFlowState"
    );
}

#[test]
fn inbound_udp_loops_do_not_import_socks5_udp_runtime_helpers() {
    for path in rust_sources_under("src/inbound") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "protocol_runtime::socks5_udp::recv_upstream_packet",
            "recv_upstream_packet(",
            "Socks5UdpRuntime",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should poll upstream UDP through UdpDispatch, not SOCKS5 runtime helper `{forbidden}`"
            );
        }
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
fn udp_flow_helpers_do_not_depend_on_protocol_runtime() {
    let content = read("src/runtime/udp_flow/helpers.rs");

    assert!(
        !content.contains("protocol_runtime::"),
        "src/runtime/udp_flow/helpers.rs should stay protocol-neutral"
    );
}

#[test]
fn udp_packet_path_carrier_snapshot_is_protocol_neutral() {
    let runtime = read("src/runtime/udp_flow/sessions.rs");
    let protocol_runtime = read("src/runtime/udp_flow/packet_path.rs");
    let traits = read("src/runtime/udp_flow/packet_path.rs");

    assert!(
        !runtime.contains("enum UdpPacketPathCarrier"),
        "protocol-named packet-path carrier snapshots should not be declared in generic runtime UDP flow state"
    );
    assert!(
        !protocol_runtime.contains("enum UdpPacketPathCarrier"),
        "packet-path carrier snapshot storage should not remain a protocol-named enum"
    );
    assert!(
        !traits.contains("struct PacketPathCarrierSnapshot")
            && traits.contains("struct PacketPathCarrierDescriptor")
            && traits.contains("carrier_cache_key: String"),
        "packet-path flow snapshots should derive carrier identity directly from the adapter-built carrier descriptor"
    );
    assert!(
        traits.contains("struct PacketPathFlowSnapshot")
            && traits.contains("carrier_cache_key: String")
            && traits.contains("datagram: UdpDatagramKey")
            && !traits.contains("datagram_cache_key: String"),
        "packet-path flow snapshots should store only neutral carrier/datagram cache identities"
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
fn udp_flow_outbound_snapshot_uses_neutral_runtime_variants() {
    let outbound = read("src/runtime/udp_flow/outbound.rs");
    let snapshot = read("src/runtime/udp_flow/managed/flow.rs");
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let managed_state = read("src/runtime/udp_flow/managed/state.rs");

    for required in [
        "Direct {",
        "Relay {",
        "Datagram {",
        "StreamPacket {",
        "PacketPathDatagram {",
        "ManagedUdpFlowRef",
    ] {
        assert!(
            outbound.contains(required),
            "runtime UDP outbound snapshot should expose neutral variant or opaque snapshot `{required}`"
        );
    }

    for forbidden in [
        "Socks5 {",
        "Shadowsocks {",
        "Hysteria2 {",
        "Trojan {",
        "Mieru {",
        "username: Option<String>",
        "password: Option<String>",
        "UdpPacketPathCarrier::",
        "ManagedUdpFlowSnapshot",
        "ManagedUdpFlowResume",
        "crate::protocol_runtime::udp",
    ] {
        assert!(
            !outbound.contains(forbidden),
            "runtime UDP outbound snapshot should not declare protocol detail `{forbidden}`"
        );
    }
    let resume_struct = snapshot
        .split("pub(crate) struct ManagedUdpFlowResume")
        .nth(1)
        .expect("ManagedUdpFlowResume struct should exist")
        .split("impl ManagedUdpFlowResume")
        .next()
        .expect("ManagedUdpFlowResume impl should follow ManagedUdpFlowResume struct");
    let resume_impl = snapshot
        .split("impl ManagedUdpFlowResume")
        .nth(1)
        .expect("ManagedUdpFlowResume impl should exist");
    assert!(
        snapshot.contains("trait ManagedUdpFlowResumeObject")
            && snapshot.contains("inner: Arc<dyn ManagedUdpFlowResumeObject>")
            && snapshot.contains("downcast_ref::<T>()")
            && resume_impl.contains("pub(crate) fn new<T>(")
            && resume_impl.contains("pub(crate) fn as_ref<T>(")
            && resume_impl.contains("pub(crate) fn cloned<T>(")
            && !snapshot.contains("pub(crate) enum ManagedUdpFlowResume")
            && !resume_struct.contains("socks5::")
            && !resume_struct.contains("shadowsocks::")
            && !resume_struct.contains("hysteria2::")
            && !resume_struct.contains("trojan::")
            && !resume_struct.contains("mieru::")
            && !resume_struct.contains("Socks5(socks5::udp::Socks5UdpFlowResume)")
            && !resume_struct.contains("Shadowsocks(shadowsocks::udp::ShadowsocksUdpFlowResume)")
            && !resume_struct.contains("Hysteria2(hysteria2::udp::Hysteria2UdpFlowResume)")
            && !resume_struct.contains("Trojan(trojan::udp::TrojanUdpFlowResume)")
            && !resume_struct.contains("Mieru(mieru::udp::MieruUdpFlowResume)")
            && !resume_struct.contains("username: Option<String>")
            && !resume_struct.contains("password: String")
            && !resume_struct.contains("password: Option<String>")
            && !resume_struct.contains("client_fingerprint: Option<String>")
            && !resume_struct.contains("relay_chain: bool")
            && !resume_struct.contains("cipher_kind: shadowsocks::CipherKind"),
        "ManagedUdpFlowResume should be an opaque type-erased wrapper around protocol-owned resume objects"
    );
    assert!(
        !snapshot.contains("ManagedUdpFlowSnapshot")
            && !managed_state.contains("ManagedUdpFlowSnapshot")
            && managed_state.contains("flows: HashMap<ManagedUdpFlowRef, ManagedUdpFlowResume>"),
        "managed UDP state should store opaque resumes directly instead of reintroducing a single-variant snapshot model"
    );
    assert!(
        !snapshot.contains("PacketPathCarrierSnapshot")
            && !snapshot.contains("UdpPacketPathCarrier::"),
        "protocol UDP flow snapshot should not own packet-path carrier identity"
    );
    assert!(
        outbound.contains("snapshot:")
            && outbound.contains("crate::runtime::udp_flow::packet_path::PacketPathFlowSnapshot"),
        "runtime UDP outbound snapshot should keep packet-path flow identity in a neutral packet-path snapshot"
    );
    assert!(
        outbound.contains("managed: ManagedUdpFlowRef")
            && outbound.contains("pub(crate) fn managed_flow(")
            && outbound.contains("pub(crate) fn relay_managed_flow("),
        "runtime UDP outbound snapshot should store only opaque managed flow references"
    );
    assert!(
        !state.contains("HashMap<ManagedUdpFlowRef, ManagedUdpFlowSnapshot>")
            && !state.contains("fn next_managed_flow_ref")
            && managed_state.contains("next_flow_id: u64")
            && managed_state.contains("fn register_flow")
            && managed_state.contains("fn flow_resume"),
        "ManagedUdpState should own opaque protocol UDP resumes behind runtime managed flow refs"
    );
}

#[test]
fn udp_session_bookkeeping_does_not_match_protocol_outbound_variants() {
    let content = read("src/runtime/udp_flow/sessions.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");

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
    for forbidden in [
        ".direct_sender()",
        ".upstream_response_tag()",
        ".matches_upstream_tag(",
        ".upstream_endpoint()",
        ".success_outcome()",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_flow/sessions.rs should consume outbound index/completion views instead of fine-grained outbound accessors; found `{forbidden}`"
        );
    }
    assert!(
        content.contains(".index_keys()") && content.contains(".completion()"),
        "src/runtime/udp_flow/sessions.rs should use UdpFlowOutbound index/completion views"
    );
    assert!(
        outbound.contains("struct UdpFlowIndexKeys")
            && outbound.contains("struct UdpFlowCompletion")
            && outbound.contains("pub(super) fn index_keys(")
            && outbound.contains("pub(super) fn completion("),
        "src/runtime/udp_flow/outbound.rs should own UDP flow index/completion view derivation"
    );
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
            "src/runtime/udp_dispatch/{file_name} should live outside generic UDP dispatch"
        );
    }
}

#[test]
fn udp_dispatch_keeps_protocol_managers_behind_registered_udp_state() {
    let content = read("src/runtime/udp_dispatch/mod.rs");
    let flow_state = read("src/runtime/udp_flow/state.rs");
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let upstream = read("src/runtime/udp_flow/registered/upstream.rs");
    let register = read("src/register.rs");

    assert!(
        content.contains("flow_state: UdpFlowState")
            && !content.contains("registered: RegisteredUdpState")
            && flow_state.contains("registered: RegisteredUdpState"),
        "UdpDispatch should keep protocol-specific UDP handlers behind UdpFlowState, not direct RegisteredUdpState fields"
    );
    assert!(
        !content.contains("socks5: Socks5UdpRuntime"),
        "UdpDispatch should not hold SOCKS5 UDP association state directly"
    );
    assert!(
        !state.contains("Socks5UdpRuntime") && !state.contains("socks5:"),
        "RegisteredUdpState should not own a SOCKS5-named upstream association field"
    );
    assert!(
        state.contains("upstream: UpstreamAssociationState")
            && upstream.contains("trait UpstreamAssociationHandler")
            && upstream.contains("handlers: UpstreamUdpHandlers")
            && register.contains("socks5_upstream_association_handler"),
        "RegisteredUdpState should drive upstream UDP associations through registered neutral handlers"
    );
    for forbidden in [
        "pub(crate) fn socks5_runtime",
        "pub(crate) fn socks5_upstream_view",
        "pub(crate) fn socks5_idle_deadline",
        "pub(crate) fn touch_socks5_idle",
        "pub(crate) fn drop_socks5_upstream",
        "pub(crate) fn close_socks5_idle",
        "pub(crate) fn close_socks5_all",
    ] {
        assert!(
            !state.contains(forbidden),
            "RegisteredUdpState should expose neutral upstream lifecycle methods, not `{forbidden}`"
        );
    }
    assert!(
        state.contains("pub(crate) async fn recv_upstream_response")
            && state.contains("pub(crate) fn upstream_association_view")
            && state.contains("pub(crate) fn upstream_idle_deadline")
            && state.contains("pub(crate) fn touch_upstream_idle")
            && state.contains("pub(crate) fn drop_upstream_association")
            && state.contains("pub(crate) fn close_idle_upstream")
            && state.contains("pub(crate) fn close_all_upstreams"),
        "RegisteredUdpState should expose neutral upstream lifecycle methods"
    );
    assert!(
        !content.contains("packet_path: PacketPathManager")
            && flow_state.contains("packet_path: PacketPathManager"),
        "UdpDispatch should keep the packet-path manager behind UdpFlowState"
    );
    assert!(
        !state.contains("PacketPathManager") && !state.contains("packet_path:"),
        "RegisteredUdpState should not own generic packet-path runtime infrastructure"
    );

    for forbidden in [
        "socks5_upstream:",
        "socks5_idle_deadline:",
        "ActiveUpstreamSocks5UdpAssociation",
        "vless_manager:",
        "vmess_manager:",
        "ss_manager:",
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
        "flow_state",
        "socks5",
        "registered",
    ] {
        assert!(
            !content.contains(&format!("pub(crate) {field}:")),
            "UdpDispatch field `{field}` should stay private behind methods"
        );
    }
}

#[test]
fn protocol_udp_flow_requests_do_not_extend_udp_dispatch() {
    assert!(
        !manifest_dir()
            .join("src/runtime/udp_flow/registered/flows.rs")
            .exists(),
        "managed UDP flow request models should not live under runtime::udp_flow::registered"
    );
    let content = read("src/runtime/udp_flow/managed/flow.rs");

    for forbidden in [
        "VlessUdpFlow",
        "VlessUdpRelayTwoStream",
        "VlessUdpRelayFinalHop",
        "VmessUdpFlow",
        "VmessUdpRelayFlow",
    ] {
        assert!(
            !content.contains(forbidden),
            "runtime::udp_flow::managed should keep only neutral flow requests, not protocol-specific `{forbidden}`"
        );
    }

    for forbidden in [
        "impl UdpDispatch",
        "use crate::runtime::udp_dispatch::UdpDispatch",
    ] {
        assert!(
            !content.contains(forbidden),
            "runtime::udp_flow::managed should define request types, not extend runtime dispatcher; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_runtime_no_longer_owns_managed_handler_state() {
    let protocol_runtime_udp = manifest_dir().join("src/protocol_runtime/udp");
    let runtime_managed = manifest_dir().join("src/runtime/udp_flow/managed");

    assert!(
        !protocol_runtime_udp.exists(),
        "UDP runtime glue should no longer live under protocol_runtime::udp"
    );
    for path in [
        "mod.rs",
        "state.rs",
        "datagram.rs",
        "stream.rs",
        "model.rs",
        "connection.rs",
        "flow.rs",
    ] {
        assert!(
            runtime_managed.join(path).exists(),
            "runtime::udp_flow::managed should own managed UDP handler module `{path}`"
        );
    }
}

#[test]
fn managed_udp_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/mod.rs");
    let connection = read("src/runtime/udp_flow/managed/connection.rs");
    let flow = read("src/runtime/udp_flow/managed/flow.rs");

    for required in [
        "mod connection;",
        "mod flow;",
        "pub(crate) use connection::{",
        "pub(crate) use flow::{",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed root should wire the submodule `{required}`"
        );
    }
    for forbidden in [
        "trait ManagedUdpConnection",
        "trait ManagedTupleUdpSender",
        "trait ManagedPacketUdpSender",
        "trait ManagedDatagramUdpConnection",
        "fn spawn_response_bridge<T, F>",
        "struct ManagedDatagramFlow",
        "struct ManagedStreamPacketFlow",
        "struct ManagedRelayStreamFlow",
        "struct ManagedUdpFlowRequest",
        "struct ManagedUdpFlowResume",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed root should remain a facade and not own `{forbidden}`"
        );
    }
    assert!(
        connection.contains("trait ManagedUdpConnection")
            && connection.contains("trait ManagedTupleUdpSender")
            && connection.contains("trait ManagedPacketUdpSender")
            && connection.contains("trait ManagedDatagramUdpConnection")
            && connection.contains("fn spawn_response_bridge<T, F>")
            && flow.contains("struct ManagedDatagramFlow")
            && flow.contains("struct ManagedStreamPacketFlow")
            && flow.contains("struct ManagedRelayStreamFlow")
            && flow.contains("struct ManagedUdpFlowRequest")
            && flow.contains("struct ManagedUdpFlowResume"),
        "managed UDP connection wrappers and flow models should live in explicit submodules, not the facade root"
    );
}

#[test]
fn managed_udp_cache_keys_are_internal_details() {
    let cache = read("src/runtime/udp_flow/managed/cache.rs");

    for forbidden in [
        "pub(crate) struct ManagedUdpConnectionCacheKey",
        "pub(crate) struct ManagedStreamConnectionCacheKey",
        "pub(crate) struct ManagedDatagramConnectionCacheKey",
        "pub(crate) async fn send_or_insert_pre_sent(",
        "pub(crate) async fn send_or_insert(",
        "pub(crate) async fn insert_and_send(",
        "pub(crate) async fn send_existing(",
        "pub(crate) async fn get_or_insert_with(",
    ] {
        assert!(
            !cache.contains(forbidden),
            "managed UDP cache should keep typed key/raw cache methods private; found `{forbidden}`"
        );
    }

    for required in [
        "pub(crate) async fn send_or_insert_pre_sent_key",
        "pub(crate) async fn send_or_insert_key",
        "pub(crate) async fn insert_and_send_key",
        "pub(crate) async fn send_existing_target",
        "pub(crate) async fn send_or_insert_target",
        "pub(crate) async fn get_or_insert_key",
        "struct ManagedUdpConnectionCacheKey",
        "struct ManagedStreamConnectionCacheKey",
        "struct ManagedDatagramConnectionCacheKey",
    ] {
        assert!(
            cache.contains(required),
            "managed UDP cache should expose opaque key/target helper `{required}` while owning typed cache identity internally"
        );
    }
}

#[test]
fn protocol_udp_runtime_channels_store_neutral_packets() {
    for path in rust_sources_under("src/runtime/udp_flow/registered") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "mpsc::Sender<vless::VlessUdpFlowPacket>",
            "mpsc::Receiver<vless::VlessUdpFlowPacket>",
            "mpsc::channel::<vless::VlessUdpFlowPacket>",
            "mpsc::Sender<vmess::VmessUdpFlowPacket>",
            "mpsc::Receiver<vmess::VmessUdpFlowPacket>",
            "mpsc::channel::<vmess::VmessUdpFlowPacket>",
            "mpsc::Sender<hysteria2::udp::Hysteria2UdpFlowPacket>",
            "mpsc::Receiver<hysteria2::udp::Hysteria2UdpFlowPacket>",
            "mpsc::channel::<hysteria2::udp::Hysteria2UdpFlowPacket>",
            "mpsc::Sender<mieru::udp::MieruUdpFlowPacket>",
            "mpsc::Receiver<mieru::udp::MieruUdpFlowPacket>",
            "mpsc::channel::<mieru::udp::MieruUdpFlowPacket>",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should store neutral UdpFlowPacket in proxy runtime channels, not protocol packet channel `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_udp_start_logic_is_split_by_protocol_family() {
    let old_root = manifest_dir().join("src/protocol_runtime/udp");
    let root = manifest_dir().join("src/runtime/udp_flow/registered");

    assert!(
        !old_root.exists() && !root.join("start.rs").exists(),
        "protocol UDP start logic should live under runtime::udp_flow::registered, not protocol_runtime::udp or a monolithic start.rs"
    );

    for path in ["mod.rs", "upstream.rs", "forward.rs"] {
        assert!(
            root.join(path).exists(),
            "runtime UDP protocol-state glue should keep neutral module `{path}`"
        );
    }
    for removed in [
        "datagram.rs",
        "stream.rs",
        "vless.rs",
        "vmess.rs",
        "cached.rs",
        "cached_start.rs",
        "socks5.rs",
        "stream_sender.rs",
    ] {
        assert!(
            !root.join(removed).exists(),
            "protocol-specific UDP start logic should live behind registered handlers, not `{removed}`"
        );
    }
}

#[test]
fn protocol_udp_datagram_start_keeps_trojan_and_mieru_in_protocol_modules() {
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let managed = read("src/runtime/udp_flow/managed/state.rs");
    let managed_datagram = read("src/runtime/udp_flow/managed/datagram.rs");
    let register = read("src/register.rs");

    for forbidden in [
        "TrojanUdpFlowRequest",
        "TrojanUdpRelayFlowRequest",
        "MieruUdpFlowRequest",
        "start_mieru_udp_relay_flow",
        "TrojanSendExisting",
        "MieruSendExisting",
    ] {
        assert!(
            !state.contains(forbidden) && !managed.contains(forbidden),
            "runtime UDP managed start glue should keep Trojan and Mieru start facades in protocol modules; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/runtime/udp_flow/registered/datagram.rs")
            .exists()
            && !manifest_dir()
                .join("src/runtime/udp_flow/registered/trojan.rs")
                .exists()
            && !manifest_dir()
                .join("src/runtime/udp_flow/registered/mieru.rs")
                .exists(),
        "Trojan and Mieru UDP start dispatch should be centralized in ManagedUdpState"
    );
    for forbidden in [
        "ManagedUdpFlowResume::Shadowsocks(_)",
        "ManagedUdpFlowResume::Hysteria2(_)",
    ] {
        assert!(
            !state.contains(forbidden),
            "state.rs should delegate datagram resume dispatch to start/datagram.rs; found `{forbidden}`"
        );
    }
    for forbidden in [
        "ManagedUdpFlowResume::Shadowsocks",
        "ManagedUdpFlowResume::Hysteria2",
    ] {
        assert!(
            !managed.contains(forbidden),
            "ManagedUdpState should delegate datagram resume recognition to protocol managers; found `{forbidden}`"
        );
    }
    assert!(
        !state.contains("ManagedUdpFlowKind::Datagram")
            && !state.contains("start_managed_datagram_flow")
            && managed.contains("ManagedUdpFlowKind::Datagram")
            && managed.contains("ManagedDatagramFlow {")
            && managed.contains("self.start_datagram_flow(")
            && managed.contains("ManagedDatagramState")
            && managed.contains("ManagedUdpHandlers")
            && managed_datagram.contains("handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>")
            && !managed_datagram.contains("SsChainManager")
            && !managed_datagram.contains("H2ChainManager")
            && managed_datagram.contains("for handler in &mut self.handlers")
            && managed_datagram.contains("ManagedExistingSend::datagram"),
        "managed datagram UDP flow kind should dispatch through registered datagram handlers"
    );
    assert!(
        register.contains("registered_udp_handlers")
            && register.contains("crate::adapters::shadowsocks_udp_datagram_handler")
            && register.contains("crate::adapters::hysteria2_udp_datagram_handler")
            && !register.contains("crate::protocol_runtime::udp::shadowsocks_datagram_handler")
            && !register.contains("crate::protocol_runtime::udp::hysteria2_datagram_handler"),
        "datagram UDP handler collection should live at the compiled registration boundary"
    );
}

#[test]
fn protocol_udp_upstream_start_dispatch_lives_behind_registered_handlers() {
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let upstream = read("src/runtime/udp_flow/registered/upstream.rs");
    let register = read("src/register.rs");
    let socks5 = read("src/adapters/socks5/udp.rs");
    let socks5_runtime = read("src/adapters/socks5/udp/runtime.rs");

    for forbidden in [
        "ManagedUdpFlowResume::Socks5",
        "Socks5UdpPacketSend",
        "start_socks5_relay_flow",
        "Socks5UdpRuntime",
    ] {
        assert!(
            !state.contains(forbidden),
            "state.rs should delegate upstream UDP relay details to registered handlers; found `{forbidden}`"
        );
    }
    assert!(
        state.contains("ManagedUdpFlowKind::RelayStream")
            && state.contains("self.upstream")
            && state.contains("start_upstream_flow(inbound_tag, request)")
            && upstream.contains("fn supports_upstream_resume")
            && upstream.contains("async fn send_upstream")
            && register.contains("socks5_upstream_association_handler")
            && socks5.contains("pub(crate) fn upstream_association_handler")
            && socks5_runtime.contains("impl UpstreamAssociationHandler for Socks5UdpRuntime")
            && socks5_runtime.contains("self.start_relay_flow(inbound_tag, request).await"),
        "upstream UDP relay start should dispatch through a registered neutral upstream association handler"
    );
}

#[test]
fn protocol_udp_stream_start_dispatch_lives_in_protocol_modules() {
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let managed = read("src/runtime/udp_flow/managed/state.rs");
    let managed_stream = read("src/runtime/udp_flow/managed/stream.rs");
    let register = read("src/register.rs");

    for forbidden in [
        "ManagedUdpFlowResume::Trojan(_)",
        "ManagedUdpFlowResume::Mieru(_)",
        "start_trojan_stream_packet_flow",
        "start_trojan_relay_stream_flow",
        "start_mieru_stream_packet_flow",
        "start_mieru_relay_stream_flow",
    ] {
        assert!(
            !state.contains(forbidden),
            "state.rs should delegate stream/relay resume dispatch to protocol start modules; found `{forbidden}`"
        );
    }
    for forbidden in [
        "ManagedUdpFlowResume::Trojan",
        "ManagedUdpFlowResume::Mieru",
    ] {
        assert!(
            !managed.contains(forbidden),
            "ManagedUdpState should delegate stream resume recognition to protocol managers; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/runtime/udp_flow/registered/stream.rs")
            .exists()
            && !state.contains("ManagedUdpFlowKind::StreamPacket")
            && state.contains("ManagedUdpFlowKind::RelayStream")
            && state.contains("start_upstream_flow(inbound_tag, request)")
            && !state.contains("start_managed_stream_packet_flow")
            && !state.contains("start_managed_relay_stream_flow")
            && managed.contains("ManagedUdpFlowKind::StreamPacket")
            && managed.contains("ManagedUdpFlowKind::RelayStream")
            && managed.contains("ManagedStreamPacketFlow {")
            && managed.contains("ManagedRelayStreamFlow {")
            && managed.contains("self.start_stream_packet_flow(")
            && managed.contains("self.start_relay_stream_flow(")
            && managed.contains("ManagedStreamState")
            && managed.contains("ManagedUdpHandlers")
            && managed_stream.contains("handlers: Vec<Box<dyn ManagedStreamFlowHandler>>")
            && !managed_stream.contains("TrojanChainManager")
            && !managed_stream.contains("MieruChainManager")
            && managed_stream.contains("for handler in &mut self.handlers")
            && managed_stream.contains("ManagedExistingSend::stream_packet")
            && managed_stream.contains("ManagedRelaySend::relay_stream"),
        "stream-packet and relay-stream UDP flow kinds should dispatch through registered stream handlers"
    );
    assert!(
        register.contains("registered_udp_handlers")
            && register.contains("crate::adapters::trojan_udp_stream_handler")
            && register.contains("crate::adapters::mieru_udp_stream_handler")
            && !register.contains("crate::protocol_runtime::udp::trojan_stream_handler")
            && !register.contains("crate::protocol_runtime::udp::mieru_stream_handler"),
        "stream UDP handler collection should live at the compiled registration boundary"
    );
}

#[test]
fn udp_dispatch_does_not_keep_external_managed_flow_handles() {
    let dispatch = read("src/runtime/udp_dispatch/mod.rs");
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let types = read("src/runtime/udp_dispatch/types.rs");

    for source in [&dispatch, &lifecycle] {
        for forbidden in [
            "HashMap<(Address, u16)",
            "SessionHandle",
            "managed_handles",
            "ManagedUdpFlows",
            "managed_flows",
        ] {
            assert!(
                !source.contains(forbidden),
                "UDP dispatch should keep protocol-managed UDP flows in UdpSessionFlows; found external handle storage `{forbidden}`"
            );
        }
    }
    assert!(
        !types.contains("ManagedFlow"),
        "FlowStartResult::ManagedFlow should not preserve a second UDP lifecycle"
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
        "runtime UDP dispatch should not keep broad protocol start wrappers; use narrow per-flow dispatch facades"
    );
    for file_name in [
        "hysteria2_flow.rs",
        "mieru_flow.rs",
        "shadowsocks_flow.rs",
        "socks5_flow.rs",
        "trojan_flow.rs",
        "vless_flow.rs",
        "vmess_flow.rs",
    ] {
        assert!(
            !root.join(file_name).exists(),
            "protocol-named UDP flow facade should not live under runtime/udp_dispatch/{file_name}"
        );
    }

    for path in rust_sources_under("src/runtime/udp_dispatch") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "ShadowsocksUdpFlow",
            "MieruUdpRelayFlow",
            "start_hysteria2_udp_flow",
            "start_shadowsocks_udp_flow",
            "VlessUdpFlow",
            "VmessUdpFlow",
            "Hysteria2UdpFlowRequest",
            "TrojanUdpFlowRequest",
            "TrojanUdpRelayFlowRequest",
            "MieruUdpFlowRequest",
            "ManagedDatagramFlow {",
            "ManagedStreamPacketFlow {",
            "ManagedRelayStreamFlow {",
            "Socks5UdpPacketSend",
            "start_trojan_udp_relay_flow",
            "start_mieru_udp_relay_flow",
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
    let registry = read("src/protocol_registry/registry/mod.rs");
    let tests = manifest_dir().join("src/protocol_registry/registry/tests/mod.rs");

    assert!(
        !registry.contains("mod tests {"),
        "protocol registry tests should live in src/protocol_registry/registry/tests/mod.rs"
    );
    assert!(
        tests.exists(),
        "protocol registry boundary tests should stay in a sibling tests module"
    );
    let tests_content = read("src/protocol_registry/registry/tests/mod.rs");
    assert!(
        !tests_content.contains("use super::*;"),
        "protocol registry tests should import registry dependencies explicitly"
    );
}

#[test]
fn protocol_registry_tests_root_is_facade_only() {
    let tests = read("src/protocol_registry/registry/tests/mod.rs");
    let fixtures = read("src/protocol_registry/registry/tests/fixtures.rs");
    let inbound = read("src/protocol_registry/registry/tests/inbound.rs");
    let outbound = read("src/protocol_registry/registry/tests/outbound.rs");

    for expected in ["mod fixtures;", "mod inbound;", "mod outbound;"] {
        assert!(
            tests.contains(expected),
            "src/protocol_registry/registry/tests/mod.rs should expose test facade item `{expected}`"
        );
    }

    for forbidden in [
        "#[test]",
        "fn compiled_in_inbound_configs",
        "fn compiled_in_outbound_leaves",
        "fn inbound_protocol_name",
        "fn outbound_leaf_name",
        "ResolvedLeafOutbound::",
        "InboundProtocolConfig::",
        "ProtocolRegistry::build()",
    ] {
        assert!(
            !tests.contains(forbidden),
            "src/protocol_registry/registry/tests/mod.rs should remain a facade over fixtures/inbound/outbound test modules; found `{forbidden}`"
        );
    }

    assert!(
        fixtures.contains("fn compiled_in_inbound_configs")
            && fixtures.contains("fn compiled_in_outbound_leaves")
            && fixtures.contains("fn inbound_protocol_name")
            && fixtures.contains("fn outbound_leaf_name"),
        "src/protocol_registry/registry/tests/fixtures.rs should own registry test fixtures"
    );
    assert!(
        inbound.contains("compiled_in_inbound_variants_have_exactly_one_registered_adapter"),
        "src/protocol_registry/registry/tests/inbound.rs should own inbound registry tests"
    );
    assert!(
        outbound.contains("compiled_in_outbound_leaf_variants_have_expected_adapter_claims")
            && outbound.contains("block_outbound_leaf_is_kernel_fact_not_adapter_protocol"),
        "src/protocol_registry/registry/tests/outbound.rs should own outbound registry tests"
    );
}

#[test]
fn protocol_registry_struct_root_is_facade_only() {
    let registry = read("src/protocol_registry/registry/mod.rs");

    for expected in [
        "mod build;",
        "mod inbound;",
        "mod metadata;",
        "mod outbound;",
        "mod runtime;",
        "mod support;",
        "mod validation;",
        "pub(crate) struct ProtocolRegistry",
        "adapters: Vec<std::sync::Arc<dyn crate::protocol_registry::RegisteredProtocolCapability>>",
        "impl fmt::Debug for ProtocolRegistry",
    ] {
        assert!(
            registry.contains(expected),
            "src/protocol_registry/registry/mod.rs should expose registry facade item `{expected}`"
        );
    }

    for forbidden in [
        "pub(crate) fn build",
        "pub(crate) fn register",
        "pub(crate) fn find_inbound",
        "pub(crate) fn find_outbound_leaf",
        "pub(crate) fn bind_inbound",
        "pub(crate) fn inbound_names",
        "pub(crate) fn outbound_names",
        "pub(crate) fn on_config_reloaded",
        "pub(crate) fn supports_inbound",
        "pub(crate) fn supports_outbound",
        "pub(crate) fn validate_inbounds",
        "pub(crate) fn validate_outbounds",
        "adapter.",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_registry/registry/mod.rs should remain a facade over registry submodules; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_build_lives_in_register_surface() {
    let adapters = read("src/adapters/mod.rs");
    let registry = read("src/protocol_registry/registry/mod.rs");
    let build = read("src/protocol_registry/registry/build.rs");
    let register = read("src/register.rs");
    let inventory = read("src/inventory.rs");

    assert!(
        !adapters.contains("build_registry"),
        "src/adapters/mod.rs should not own registry construction"
    );
    assert!(
        !registry.contains("pub(crate) fn build() -> Self"),
        "src/protocol_registry/registry/mod.rs should keep registry construction out of the registry facade"
    );
    assert!(
        !build.contains("pub(crate) fn build() -> Self"),
        "src/protocol_registry/registry/build.rs should only own the low-level register helper"
    );
    assert!(
        register.contains("pub(crate) fn protocol_registry() -> ProtocolRegistry"),
        "src/register.rs should own compiled protocol registry construction"
    );
    assert!(
        inventory.contains("crate::register::protocol_registry()"),
        "src/inventory.rs should build the registry through the register surface"
    );
}

#[test]
fn protocol_registry_imports_live_in_register_surface() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let build = read("src/protocol_registry/registry/build.rs");
    let register = read("src/register.rs");

    for adapter in [
        "DirectAdapter",
        "HttpConnectAdapter",
        "Hysteria2Adapter",
        "MieruAdapter",
        "MixedAdapter",
        "ShadowsocksAdapter",
        "Socks5Adapter",
        "TrojanAdapter",
        "VlessAdapter",
        "VmessAdapter",
    ] {
        assert!(
            !registry.contains(adapter) && !build.contains(adapter),
            "protocol_registry registry modules should keep concrete adapter imports in src/register.rs; found `{adapter}`"
        );
        assert!(
            register.contains(adapter),
            "src/register.rs should own concrete adapter import `{adapter}`"
        );
    }
}

#[test]
fn protocol_registry_register_helper_stays_in_build_module() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let build = read("src/protocol_registry/registry/build.rs");

    assert!(
        !registry.contains("pub(crate) fn register("),
        "src/protocol_registry/registry/mod.rs should keep register helper in src/protocol_registry/registry/build.rs"
    );
    assert!(
        build.contains("pub(crate) fn register<T>(&mut self, adapter: std::sync::Arc<T>)"),
        "src/protocol_registry/registry/build.rs should own the register helper used by src/register.rs"
    );
    assert!(
        build.contains("T: RegisteredProtocolCapability + 'static")
            && build.contains("std::sync::Arc<dyn RegisteredProtocolCapability>"),
        "src/protocol_registry/registry/build.rs should register capability objects directly"
    );
}

#[test]
fn protocol_registry_metadata_lives_in_metadata_module() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let metadata = read("src/protocol_registry/registry/metadata.rs");

    for forbidden in [
        "pub(crate) fn inbound_names",
        "pub(crate) fn outbound_names",
        "pub(crate) fn capabilities",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_registry/registry/mod.rs should keep metadata methods in src/protocol_registry/registry/metadata.rs; found `{forbidden}`"
        );
        assert!(
            metadata.contains(forbidden),
            "src/protocol_registry/registry/metadata.rs should own registry metadata method `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_support_lives_in_support_module() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let metadata = read("src/protocol_registry/registry/metadata.rs");
    let support = read("src/protocol_registry/registry/support.rs");

    for forbidden in [
        "pub(crate) fn supports_inbound",
        "pub(crate) fn supports_outbound",
        "pub(crate) fn inbound_protocol_label",
        "pub(crate) fn inbound_protocol_feature_name",
        "pub(crate) fn outbound_protocol_label",
        "pub(crate) fn outbound_protocol_feature_name",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_registry/registry/mod.rs should keep support methods in src/protocol_registry/registry/support.rs; found `{forbidden}`"
        );
        assert!(
            !metadata.contains(forbidden),
            "src/protocol_registry/registry/metadata.rs should keep support methods in src/protocol_registry/registry/support.rs; found `{forbidden}`"
        );
        assert!(
            support.contains(forbidden),
            "src/protocol_registry/registry/support.rs should own registry support method `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_validation_lives_in_validation_module() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let metadata = read("src/protocol_registry/registry/metadata.rs");
    let validation = read("src/protocol_registry/registry/validation.rs");

    for forbidden in [
        "pub(crate) fn validate_inbounds",
        "pub(crate) fn validate_outbounds",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_registry/registry/mod.rs should keep validation methods in src/protocol_registry/registry/validation.rs; found `{forbidden}`"
        );
        assert!(
            !metadata.contains(forbidden),
            "src/protocol_registry/registry/metadata.rs should keep validation methods in src/protocol_registry/registry/validation.rs; found `{forbidden}`"
        );
        assert!(
            validation.contains(forbidden),
            "src/protocol_registry/registry/validation.rs should own registry validation method `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_outbound_dispatch_lives_in_outbound_module() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let outbound = read("src/protocol_registry/registry/outbound.rs");

    for forbidden in [
        "pub(crate) fn find_outbound_leaf",
        "pub(crate) fn outbound_leaf_runtime",
        "ResolvedLeafOutbound::Block",
        "TcpPathCategory::Block",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_registry/registry/mod.rs should keep outbound dispatch in src/protocol_registry/registry/outbound.rs; found `{forbidden}`"
        );
        assert!(
            outbound.contains(forbidden),
            "src/protocol_registry/registry/outbound.rs should own outbound dispatch item `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_inbound_dispatch_lives_in_inbound_module() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let inbound = read("src/protocol_registry/registry/inbound.rs");

    for forbidden in [
        "pub(crate) fn find_inbound",
        "pub(crate) async fn bind_inbound",
        "InboundListenerCapability::bind_inbound(",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_registry/registry/mod.rs should keep inbound dispatch in src/protocol_registry/registry/inbound.rs; found `{forbidden}`"
        );
        assert!(
            inbound.contains(forbidden),
            "src/protocol_registry/registry/inbound.rs should own inbound dispatch item `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_dispatch_is_not_public_api() {
    let root = read("src/protocol_registry/mod.rs");
    let registry = read("src/protocol_registry/registry/mod.rs");
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in [
        "pub use registry::ProtocolRegistry;",
        "pub struct ProtocolRegistry",
    ] {
        assert!(
            !root.contains(forbidden),
            "protocol adapter dispatch internals should stay crate-private; found `{forbidden}`"
        );
    }

    assert!(
        root.contains("pub(crate) use registry::ProtocolRegistry;"),
        "src/protocol_registry/mod.rs should keep ProtocolRegistry visible only inside zero-proxy"
    );
    assert!(
        !old_adapter.exists()
            && !root.contains("mod adapter;")
            && !root.contains("ProtocolAdapter")
            && !registry.contains("ProtocolAdapter")
            && !capability.contains("ProtocolAdapter"),
        "protocol registry should not keep a ProtocolAdapter marker trait or adapter.rs module"
    );
    assert!(
        capability.contains("pub(crate) trait ProtocolSupportCapability"),
        "src/protocol_registry/capability.rs should own focused adapter capability traits"
    );
    assert!(
        registry.contains("pub(crate) struct ProtocolRegistry"),
        "src/protocol_registry/registry/mod.rs should keep ProtocolRegistry visible only inside zero-proxy"
    );
}

#[test]
fn protocol_registry_root_is_facade_only() {
    let root = read("src/protocol_registry/mod.rs");

    for expected in [
        "mod capability;",
        "mod context;",
        "mod defaults;",
        "mod model;",
        "mod registry;",
        "pub(crate) use capability::",
        "pub(crate) use context::{InboundAdapterContext, OutboundAdapterContext, UdpAdapterContext};",
        "pub(crate) use model::{BoundInbound, OutboundLeafRuntime};",
        "pub(crate) use registry::ProtocolRegistry;",
    ] {
        assert!(
            root.contains(expected),
            "src/protocol_registry/mod.rs should expose facade item `{expected}`"
        );
    }

    for forbidden in [
        "struct ProtocolRegistry",
        "enum BoundInbound",
        "struct OutboundLeafRuntime",
        "impl ProtocolRegistry",
        "impl ProtocolRegistry",
        "async fn",
        "fn find_outbound_leaf",
        "fn find_inbound",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_registry/mod.rs should remain a facade over adapter/defaults/model/registry modules; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_capabilities_are_split_by_responsibility() {
    let root = read("src/protocol_registry/mod.rs");
    let capability = read("src/protocol_registry/capability.rs");
    let context = read("src/protocol_registry/context.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for expected in [
        "pub(crate) trait RegisteredProtocolCapability",
        "pub(crate) trait ProtocolSupportCapability",
        "pub(crate) trait InboundListenerCapability",
        "pub(crate) trait TcpOutboundCapability",
        "pub(crate) trait UdpFlowCapability",
        "pub(crate) trait UdpPacketPathCapability",
    ] {
        assert!(
            capability.contains(expected),
            "src/protocol_registry/capability.rs should expose focused capability trait `{expected}`"
        );
    }

    assert!(
        root.contains("mod capability;"),
        "src/protocol_registry/mod.rs should wire the capability trait module"
    );
    assert!(
        root.contains("mod context;"),
        "src/protocol_registry/mod.rs should wire the adapter context module"
    );
    for expected in [
        "pub(crate) struct InboundAdapterContext",
        "pub(crate) struct OutboundAdapterContext",
        "pub(crate) struct UdpAdapterContext",
    ] {
        assert!(
            context.contains(expected),
            "src/protocol_registry/context.rs should expose narrow adapter context `{expected}`"
        );
    }
    assert!(
        !old_adapter.exists()
            && !root.contains("mod adapter;")
            && !root.contains("ProtocolAdapter")
            && !capability.contains("ProtocolAdapter"),
        "protocol capability split should not retain a compatibility ProtocolAdapter marker"
    );
    assert!(
        capability.contains("impl<T> RegisteredProtocolCapability for T"),
        "src/protocol_registry/capability.rs should provide the registry collector blanket impl"
    );
    assert!(
        !capability.contains("impl<T> TcpOutboundCapability for T"),
        "TCP outbound dispatch should use explicit TcpOutboundCapability impls, not a ProtocolRegistry blanket shim"
    );
    assert!(
        !capability.contains("impl<T> InboundListenerCapability for T"),
        "inbound listener dispatch should use explicit InboundListenerCapability impls, not a ProtocolRegistry blanket shim"
    );
    assert!(
        !capability.contains("impl<T> UdpFlowCapability for T"),
        "UDP flow dispatch should use explicit UdpFlowCapability impls, not a ProtocolRegistry blanket shim"
    );
    assert!(
        !capability.contains("impl<T> UdpPacketPathCapability for T"),
        "UDP packet-path dispatch should use explicit UdpPacketPathCapability impls, not a ProtocolRegistry blanket shim"
    );
}

#[test]
fn protocol_support_capability_is_not_on_monolithic_adapter() {
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in [
        "fn name(&self)",
        "fn feature_name(&self)",
        "fn supports_inbound(&self",
        "fn supports_outbound(&self",
        "fn has_inbound(&self)",
        "fn has_outbound(&self)",
        "impl<T> ProtocolSupportCapability for T",
    ] {
        assert!(
            !old_adapter.exists()
                && (forbidden != "fn name(&self)" || !capability.contains("ProtocolAdapter::name"))
                && (forbidden != "fn feature_name(&self)"
                    || !capability.contains("ProtocolAdapter::feature_name"))
                && (forbidden != "fn supports_inbound(&self"
                    || !capability.contains("ProtocolAdapter::supports_inbound"))
                && (forbidden != "fn supports_outbound(&self"
                    || !capability.contains("ProtocolAdapter::supports_outbound"))
                && (forbidden != "fn has_inbound(&self)"
                    || !capability.contains("ProtocolAdapter::has_inbound"))
                && (forbidden != "fn has_outbound(&self)"
                    || !capability.contains("ProtocolAdapter::has_outbound"))
                && (forbidden != "impl<T> ProtocolSupportCapability for T"
                    || !capability.contains(forbidden)),
            "protocol metadata/support should live in explicit ProtocolSupportCapability impls, not `{forbidden}`"
        );
    }

    for source in [
        "src/adapters/direct.rs",
        "src/adapters/http_connect.rs",
        "src/adapters/hysteria2.rs",
        "src/adapters/mieru.rs",
        "src/adapters/mixed.rs",
        "src/adapters/shadowsocks.rs",
        "src/adapters/socks5.rs",
        "src/adapters/trojan.rs",
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("impl ProtocolSupportCapability for"),
            "{source} should explicitly implement ProtocolSupportCapability"
        );
    }
}

#[test]
fn tcp_outbound_capability_is_not_on_monolithic_adapter() {
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in [
        "fn claims_outbound_leaf(&self",
        "fn outbound_leaf_runtime",
        "async fn connect_tcp",
        "async fn apply_relay_hop",
    ] {
        assert!(
            !old_adapter.exists(),
            "TCP outbound capability should not remain on a monolithic adapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::claims_outbound_leaf",
        "ProtocolAdapter::outbound_leaf_runtime",
        "ProtocolAdapter::connect_tcp",
        "ProtocolAdapter::apply_relay_hop",
    ] {
        assert!(
            !capability.contains(forbidden),
            "TCP outbound capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn inbound_listener_capability_is_not_on_monolithic_adapter() {
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in ["async fn bind_inbound", "fn spawn_inbound"] {
        assert!(
            !old_adapter.exists(),
            "inbound listener capability should not remain on a monolithic adapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::bind_inbound",
        "ProtocolAdapter::spawn_inbound",
    ] {
        assert!(
            !capability.contains(forbidden),
            "inbound listener capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn udp_flow_capability_is_not_on_monolithic_adapter() {
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in [
        "async fn start_udp_flow",
        "fn udp_relay_needs_two_streams",
        "async fn start_udp_relay_two_stream",
        "async fn start_udp_relay_final_hop",
    ] {
        assert!(
            !old_adapter.exists(),
            "UDP flow capability should not remain on a monolithic adapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::start_udp_flow",
        "ProtocolAdapter::udp_relay_needs_two_streams",
        "ProtocolAdapter::start_udp_relay_two_stream",
        "ProtocolAdapter::start_udp_relay_final_hop",
    ] {
        assert!(
            !capability.contains(forbidden),
            "UDP flow capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn udp_packet_path_capability_is_not_on_monolithic_adapter() {
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in [
        "fn udp_packet_path_carrier_descriptor",
        "async fn build_udp_packet_path",
        "fn udp_datagram_source",
    ] {
        assert!(
            !old_adapter.exists(),
            "UDP packet-path capability should not remain on a monolithic adapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::udp_packet_path_carrier_descriptor",
        "ProtocolAdapter::build_udp_packet_path",
        "ProtocolAdapter::udp_datagram_source",
    ] {
        assert!(
            !capability.contains(forbidden),
            "UDP packet-path capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn registered_adapters_implement_inbound_listener_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl InboundListenerCapability for {adapter}")),
            "{source} should explicitly implement InboundListenerCapability for {adapter}"
        );
    }
}

#[test]
fn registered_adapters_implement_udp_flow_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl UdpFlowCapability for {adapter}")),
            "{source} should explicitly implement UdpFlowCapability for {adapter}"
        );
    }
}

#[test]
fn registered_adapters_implement_udp_packet_path_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl UdpPacketPathCapability for {adapter}")),
            "{source} should explicitly implement UdpPacketPathCapability for {adapter}"
        );
    }
}

#[test]
fn registered_adapters_implement_tcp_outbound_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl TcpOutboundCapability for {adapter}")),
            "{source} should explicitly implement TcpOutboundCapability for {adapter}"
        );
    }
}

#[test]
fn protocol_registry_stores_capability_objects() {
    let registry = read("src/protocol_registry/registry/mod.rs");
    let inbound = read("src/protocol_registry/registry/inbound.rs");
    let outbound = read("src/protocol_registry/registry/outbound.rs");

    assert!(
        registry.contains("RegisteredProtocolCapability"),
        "ProtocolRegistry should store registered capability objects"
    );
    for forbidden in [
        "Vec<std::sync::Arc<dyn crate::protocol_registry::ProtocolAdapter>>",
        "Result<Arc<dyn ProtocolAdapter>",
    ] {
        assert!(
            !registry.contains(forbidden)
                && !inbound.contains(forbidden)
                && !outbound.contains(forbidden),
            "ProtocolRegistry dispatch should not expose monolithic adapter object `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_capabilities_use_contexts_not_proxy() {
    let capability = read("src/protocol_registry/capability.rs");
    let old_adapter = manifest_dir().join("src/protocol_registry/adapter.rs");

    for forbidden in ["proxy: &Proxy", "_proxy: &Proxy"] {
        assert!(
            !old_adapter.exists() && !capability.contains(forbidden),
            "adapter dispatch traits should receive narrow adapter contexts, not expose `{forbidden}`"
        );
    }

    assert!(
        !old_adapter.exists() && capability.contains("UdpAdapterContext<'_>"),
        "UDP adapter context should live on UDP capability traits, not ProtocolAdapter"
    );
    assert!(
        !old_adapter.exists() && capability.contains("InboundAdapterContext<'_>"),
        "inbound listener context should live on InboundListenerCapability, not ProtocolAdapter"
    );
    assert!(
        !old_adapter.exists() && capability.contains("OutboundAdapterContext<'_>"),
        "TCP outbound context should live on TcpOutboundCapability, not ProtocolAdapter"
    );
}

#[test]
fn protocol_registry_models_live_outside_trait_root() {
    let root = read("src/protocol_registry/mod.rs");
    let model = read("src/protocol_registry/model/mod.rs");
    let inbound = read("src/protocol_registry/model/inbound.rs");
    let outbound = read("src/protocol_registry/model/outbound.rs");

    for forbidden in ["pub(crate) enum BoundInbound", "impl BoundInbound"] {
        assert!(
            !root.contains(forbidden) && !model.contains(forbidden),
            "src/protocol_registry/mod.rs and src/protocol_registry/model/mod.rs should keep inbound adapter models in src/protocol_registry/model/inbound.rs; found `{forbidden}`"
        );
        assert!(
            inbound.contains(forbidden),
            "src/protocol_registry/model/inbound.rs should own adapter inbound model `{forbidden}`"
        );
    }

    for forbidden in [
        "pub(crate) struct OutboundLeafRuntime",
        "use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory}",
    ] {
        assert!(
            !root.contains(forbidden) && !model.contains(forbidden),
            "src/protocol_registry/mod.rs and src/protocol_registry/model/mod.rs should keep outbound adapter models in src/protocol_registry/model/outbound.rs; found `{forbidden}`"
        );
        assert!(
            outbound.contains(forbidden),
            "src/protocol_registry/model/outbound.rs should own adapter outbound model `{forbidden}`"
        );
    }

    for forbidden in [
        "pub(crate) enum BoundInbound",
        "pub(crate) struct OutboundLeafRuntime",
        "impl BoundInbound",
        "use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory}",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_registry/mod.rs should keep adapter models in src/protocol_registry/model/mod.rs; found `{forbidden}`"
        );
    }
    assert!(
        root.contains("pub(crate) use model::{BoundInbound, OutboundLeafRuntime};"),
        "src/protocol_registry/mod.rs should re-export adapter models crate-privately"
    );
}

#[test]
fn protocol_registry_model_root_is_facade_only() {
    let model = read("src/protocol_registry/model/mod.rs");

    for expected in [
        "mod inbound;",
        "mod outbound;",
        "pub(crate) use inbound::BoundInbound;",
        "pub(crate) use outbound::OutboundLeafRuntime;",
    ] {
        assert!(
            model.contains(expected),
            "src/protocol_registry/model/mod.rs should expose model facade item `{expected}`"
        );
    }

    for forbidden in [
        "enum BoundInbound",
        "struct OutboundLeafRuntime",
        "impl BoundInbound",
        "TcpPathCategory",
        "OutboundEndpoint",
        "TokioListener",
        "QuicInbound",
        "into_tcp",
    ] {
        assert!(
            !model.contains(forbidden),
            "src/protocol_registry/model/mod.rs should remain a facade over inbound/outbound model modules; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_default_errors_live_outside_trait_root() {
    let root = read("src/protocol_registry/mod.rs");
    let defaults = read("src/protocol_registry/defaults/mod.rs");
    let errors = read("src/protocol_registry/defaults/errors.rs");

    for forbidden in [
        "std::io::ErrorKind::Unsupported",
        "TcpOutboundFailure {",
        "FlowFailure {",
        "no_tcp_outbound",
        "no_udp_outbound",
        "no_two_stream_relay",
        "no_udp_relay_final_hop",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_registry/mod.rs should keep default unsupported error construction in src/protocol_registry/defaults/errors.rs; found `{forbidden}`"
        );
        assert!(
            !defaults.contains(forbidden),
            "src/protocol_registry/defaults/mod.rs should keep default unsupported error construction in src/protocol_registry/defaults/errors.rs; found `{forbidden}`"
        );
        assert!(
            errors.contains(forbidden),
            "src/protocol_registry/defaults/errors.rs should own default unsupported error construction `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_default_tcp_bind_lives_outside_trait_root() {
    let root = read("src/protocol_registry/mod.rs");
    let defaults = read("src/protocol_registry/defaults/mod.rs");
    let bind = read("src/protocol_registry/defaults/bind.rs");

    for forbidden in ["TokioListener::bind", "BoundInbound::Tcp"] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_registry/mod.rs should keep default TCP bind construction in src/protocol_registry/defaults/bind.rs; found `{forbidden}`"
        );
        assert!(
            !defaults.contains(forbidden),
            "src/protocol_registry/defaults/mod.rs should keep default TCP bind construction in src/protocol_registry/defaults/bind.rs; found `{forbidden}`"
        );
        assert!(
            bind.contains(forbidden),
            "src/protocol_registry/defaults/bind.rs should own default TCP bind construction `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_defaults_root_is_facade_only() {
    let defaults = read("src/protocol_registry/defaults/mod.rs");

    for expected in [
        "mod bind;",
        "mod errors;",
        "pub(super) use bind::bind_tcp_inbound;",
        "pub(super) use errors::{",
    ] {
        assert!(
            defaults.contains(expected),
            "src/protocol_registry/defaults/mod.rs should expose defaults facade item `{expected}`"
        );
    }

    for forbidden in [
        "async fn",
        "fn unsupported_io",
        "fn udp_flow_unsupported",
        "TcpOutboundFailure {",
        "FlowFailure {",
        "TokioListener::bind",
        "BoundInbound::Tcp",
        "std::io::ErrorKind::Unsupported",
    ] {
        assert!(
            !defaults.contains(forbidden),
            "src/protocol_registry/defaults/mod.rs should remain a facade over bind/errors modules; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_does_not_expose_adapter_trait_objects() {
    let inventory = read("src/inventory.rs");

    for forbidden in [
        "Arc<dyn crate::protocol_registry::ProtocolAdapter>",
        "Arc<dyn ProtocolAdapter>",
        "pub(crate) fn find_outbound_leaf",
        "pub(crate) fn find_inbound",
    ] {
        assert!(
            !inventory.contains(forbidden),
            "src/inventory.rs should expose protocol operations, not adapter trait objects; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_root_is_facade_only() {
    let root = read("src/inventory.rs");

    for expected in [
        "mod inbound;",
        "mod metadata;",
        "mod runtime;",
        "mod tcp;",
        "mod udp;",
        "pub struct ProtocolInventory",
        "registry: ProtocolRegistry",
    ] {
        assert!(
            root.contains(expected),
            "src/inventory.rs should expose facade item `{expected}`"
        );
    }

    for forbidden in [
        "find_inbound",
        "find_outbound_leaf",
        "adapter.",
        "async fn",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "FlowFailure",
        "EngineError",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should remain a facade over inventory submodules; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_metadata_facade_lives_in_metadata_module() {
    let root = read("src/inventory.rs");
    let metadata = read("src/inventory/metadata.rs");

    for forbidden in [
        "supported_inbounds",
        "supported_outbounds",
        "protocol_capabilities",
        "validate_config",
        "supports_inbound_protocol",
        "supports_outbound_protocol",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep metadata facade methods in src/inventory/metadata.rs; found `{forbidden}`"
        );
        assert!(
            metadata.contains(forbidden),
            "src/inventory/metadata.rs should own metadata facade method `{forbidden}`"
        );
    }
}

#[test]
fn inventory_runtime_facts_live_in_runtime_module() {
    let root = read("src/inventory.rs");
    let runtime = read("src/inventory/runtime.rs");

    for forbidden in ["OutboundLeafRuntime", "outbound_leaf_runtime"] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep runtime fact lookup in src/inventory/runtime.rs; found `{forbidden}`"
        );
        assert!(
            runtime.contains(forbidden),
            "src/inventory/runtime.rs should own runtime fact lookup `{forbidden}`"
        );
    }
}

#[test]
fn inventory_udp_adapter_dispatch_lives_in_udp_module() {
    let root = read("src/inventory.rs");
    let udp = read("src/inventory/udp.rs");
    let leaf = read("src/inventory/udp/leaf.rs");
    let relay = read("src/inventory/udp/relay.rs");
    let packet_path = read("src/inventory/udp/packet_path.rs");

    {
        let forbidden = "start_udp_leaf_flow";
        assert!(
            !root.contains(forbidden) && !udp.contains(forbidden),
            "src/inventory.rs and src/inventory/udp.rs should keep UDP leaf dispatch in src/inventory/udp/leaf.rs; found `{forbidden}`"
        );
        assert!(
            leaf.contains(forbidden),
            "src/inventory/udp/leaf.rs should own UDP leaf dispatch method `{forbidden}`"
        );
    }

    for forbidden in [
        "udp_relay_needs_two_streams",
        "start_udp_relay_two_stream",
        "start_udp_relay_final_hop",
    ] {
        assert!(
            !root.contains(forbidden) && !udp.contains(forbidden),
            "src/inventory.rs and src/inventory/udp.rs should keep UDP relay dispatch in src/inventory/udp/relay.rs; found `{forbidden}`"
        );
        assert!(
            relay.contains(forbidden),
            "src/inventory/udp/relay.rs should own UDP relay dispatch method `{forbidden}`"
        );
    }

    for forbidden in [
        "udp_packet_path_pair",
        "resolve_udp_packet_path_candidate",
        "build_udp_packet_path_carrier",
    ] {
        assert!(
            !root.contains(forbidden) && !udp.contains(forbidden),
            "src/inventory.rs and src/inventory/udp.rs should keep UDP packet-path dispatch in src/inventory/udp/packet_path.rs; found `{forbidden}`"
        );
        assert!(
            packet_path.contains(forbidden),
            "src/inventory/udp/packet_path.rs should own UDP packet-path dispatch method `{forbidden}`"
        );
    }
}

#[test]
fn inventory_udp_root_is_facade_only() {
    let udp = read("src/inventory/udp.rs");

    assert!(
        udp.contains("mod leaf;") && udp.contains("mod relay;") && udp.contains("mod packet_path;"),
        "src/inventory/udp.rs should expose the UDP inventory submodules"
    );

    for forbidden in [
        "impl ProtocolInventory",
        "find_outbound_leaf",
        "adapter.",
        "FlowFailure",
        "EngineError",
        "ResolvedLeafOutbound",
    ] {
        assert!(
            !udp.contains(forbidden),
            "src/inventory/udp.rs should remain a facade over leaf/relay/packet_path modules; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_tcp_adapter_dispatch_lives_in_tcp_module() {
    let root = read("src/inventory.rs");
    let tcp = read("src/inventory/tcp.rs");

    for forbidden in ["connect_tcp_leaf", "apply_tcp_relay_hop"] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep TCP adapter dispatch in src/inventory/tcp.rs; found `{forbidden}`"
        );
        assert!(
            tcp.contains(forbidden),
            "src/inventory/tcp.rs should own TCP adapter dispatch method `{forbidden}`"
        );
    }
}

#[test]
fn inventory_inbound_adapter_dispatch_lives_in_inbound_module() {
    let root = read("src/inventory.rs");
    let inbound = read("src/inventory/inbound.rs");

    for forbidden in ["check_inbound_enabled", "bind_inbound", "spawn_inbound"] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep inbound adapter dispatch in src/inventory/inbound.rs; found `{forbidden}`"
        );
        assert!(
            inbound.contains(forbidden),
            "src/inventory/inbound.rs should own inbound adapter dispatch method `{forbidden}`"
        );
    }
}

#[test]
fn transport_facade_exports_are_explicit() {
    let content = read("src/transport/mod.rs");

    for forbidden in [
        "pub(crate) use direct::*;",
        "pub(crate) use metered::*;",
        "pub(crate) use stream::*;",
        "pub(crate) use tcp_flow::*;",
        "pub(crate) use tcp_outbound::*;",
        "pub(crate) use tcp_relay::*;",
    ] {
        assert!(
            !content.contains(forbidden),
            "transport facade should explicitly list exported items; found `{forbidden}`"
        );
    }
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
fn udp_dispatch_root_does_not_reexport_protocol_flow_requests() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for forbidden in [
        "pub(crate) use hysteria2_flow::",
        "pub(crate) use mieru_flow::",
        "pub(crate) use shadowsocks_flow::",
        "pub(crate) use socks5_flow::",
        "pub(crate) use trojan_flow::",
        "pub(crate) use vless_flow::",
        "pub(crate) use vmess_flow::",
        "pub(crate) mod vless_flow",
        "pub(crate) mod vmess_flow",
        "Hysteria2DatagramSend",
        "MieruDatagramSend",
        "MieruRelaySend",
        "ShadowsocksDatagramSend",
        "Socks5RelaySend",
        "TrojanDatagramSend",
        "TrojanRelaySend",
        "VlessDatagramSend",
        "VlessRelayFinalHopSend",
        "VlessRelayTwoStreamSend",
        "VmessDatagramSend",
        "VmessRelaySend",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/mod.rs should not re-export protocol flow request `{forbidden}`"
        );
    }

    assert!(
        content.contains("pub(crate) use types::{FlowFailure, FlowStartResult, UdpCandidate};"),
        "src/runtime/udp_dispatch/mod.rs should keep only generic UDP dispatch result types in the root facade"
    );

    let managed = read("src/runtime/udp_dispatch/managed.rs");
    assert!(
        managed.contains("start_managed_flow")
            && managed.contains("register_managed_flow")
            && managed.contains("managed_flow_resume"),
        "runtime UDP managed bridge should expose only narrow protocol-state helpers"
    );
    for forbidden in [
        "Hysteria2DatagramSend",
        "MieruDatagramSend",
        "MieruRelaySend",
        "ShadowsocksDatagramSend",
        "Socks5RelaySend",
        "TrojanDatagramSend",
        "TrojanRelaySend",
        "VlessUdpFlow",
        "VmessUdpFlow",
    ] {
        assert!(
            !managed.contains(forbidden),
            "runtime UDP managed bridge should not know protocol-named flow request `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_state_manager_fields_are_not_crate_public() {
    let content = read("src/runtime/udp_flow/registered/mod.rs");
    let managed = read("src/runtime/udp_flow/managed/state.rs");
    let stream_sender = read("src/runtime/udp_flow/managed/stream_sender.rs");
    let cached_start = manifest_dir().join("src/runtime/udp_flow/registered/cached_start.rs");
    let datagram = read("src/runtime/udp_flow/managed/datagram.rs");
    let stream = read("src/runtime/udp_flow/managed/stream.rs");
    let register = read("src/register.rs");

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
            "RegisteredUdpState manager field `{field}` should not be crate-public"
        );
        assert!(
            !content.contains(&format!("pub(super) {field}:")),
            "RegisteredUdpState should collapse protocol manager field `{field}` behind ManagedUdpState"
        );
    }
    assert!(
        content.contains("managed: ManagedUdpState")
            && managed.contains("struct ManagedUdpState")
            && managed.contains("handlers: ManagedUdpHandlers")
            && !content.contains("stream_senders: ManagedStreamSenderState")
            && !managed.contains("cached: ManagedCachedState")
            && managed.contains("datagram: ManagedDatagramState")
            && managed.contains("stream: ManagedStreamState")
            && stream.contains("senders: ManagedStreamSenderState")
            && !stream_sender.contains("start_cached_flow")
            && !cached_start.exists()
            && stream_sender.contains("trait ManagedStreamFlowSender")
            && stream_sender.contains("HashMap<ManagedUdpFlowRef, Box<dyn ManagedStreamFlowSender>>")
            && stream_sender.contains("fn sender(")
            && !managed.contains("ManagedCachedFlowSender")
            && !stream_sender.contains("enum CachedUdpFlowStart")
            && datagram.contains("handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>")
            && stream.contains("handlers: Vec<Box<dyn ManagedStreamFlowHandler>>")
            && !managed.contains("pub(crate) vless:")
            && !managed.contains("pub(super) vless:")
            && !managed.contains("vless: VlessUdpOutboundManager")
            && !managed.contains("vmess: VmessUdpOutboundManager")
            && !managed.contains("pub(crate) shadowsocks:")
            && !managed.contains("pub(super) shadowsocks:")
            && !datagram.contains("shadowsocks: SsChainManager")
            && !datagram.contains("hysteria2: H2ChainManager")
            && !stream.contains("trojan: TrojanChainManager")
            && !stream.contains("mieru: MieruChainManager"),
        "RegisteredUdpState should expose neutral managed UDP sub-states instead of protocol manager fields"
    );
    assert!(
        register.contains("registered_udp_handlers")
            && !register
                .contains("RegisteredUdpState::new(crate::register::managed_udp_handlers())"),
        "register should collect protocol UDP handlers without owning protocol state construction"
    );
}

#[test]
fn protocol_udp_root_does_not_reexport_manager_internals() {
    let root = read("src/runtime/udp_flow/registered/mod.rs");
    let managed = read("src/runtime/udp_flow/managed/mod.rs");
    let managed_model = read("src/runtime/udp_flow/managed/model.rs");
    let old_root = manifest_dir().join("src/protocol_runtime/udp");

    for forbidden in [
        "H2SendExisting",
        "MieruSendExisting",
        "MieruRelayExisting",
        "SsSendExisting",
        "TrojanSendExisting",
        "TrojanRelayExisting",
        "pub(crate) use h2_manager::",
        "pub(crate) use mieru_manager::",
        "pub(crate) use ss_manager::",
        "pub(crate) use trojan_manager::",
        "pub(crate) use peer::",
        "mod peer;",
        "SsUdpPeer",
        "H2UdpPeer",
        "TrojanUdpPeer",
        "MieruUdpPeer",
        "UdpPeerEndpoint",
        "shadowsocks_datagram_handler",
        "hysteria2_datagram_handler",
        "trojan_stream_handler",
        "mieru_stream_handler",
        "ManagedExistingSend",
        "ManagedRelaySend",
        "Box<dyn ManagedDatagramFlowHandler>",
        "Box<dyn ManagedStreamFlowHandler>",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/runtime/udp_flow/registered/mod.rs should expose protocol UDP facades, not manager internals; found `{forbidden}`"
        );
    }
    assert!(
        !root.contains("h2_manager")
            && !root.contains("mieru_manager")
            && !root.contains("ss_manager")
            && !root.contains("trojan_manager")
            && !old_root.exists()
            && managed.contains("pub(crate) use model::{")
            && managed_model.contains("ManagedDatagramFlowHandler")
            && managed_model.contains("ManagedStreamFlowHandler"),
        "runtime registered should keep protocol UDP managers out of protocol_runtime::udp and expose managed handler traits from runtime::udp_flow::managed"
    );
}

#[test]
fn protocol_udp_manager_construction_is_adapter_registered() {
    let allowed = [
        "src/adapters/hysteria2/udp.rs",
        "src/adapters/hysteria2/udp/managed.rs",
        "src/adapters/mieru/udp.rs",
        "src/adapters/mieru/udp/managed.rs",
        "src/adapters/shadowsocks/udp/managed.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/trojan/udp.rs",
        "src/adapters/trojan/udp/managed.rs",
    ];

    for path in rust_sources_under("src") {
        let source = relative(&path);
        if source == "src/runtime/udp_flow/registered/mod.rs"
            || allowed.iter().any(|allowed| source.starts_with(allowed))
        {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "protocol_runtime::udp::h2_manager",
            "protocol_runtime::udp::mieru_manager",
            "protocol_runtime::udp::ss_manager",
            "protocol_runtime::udp::trojan_manager",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not construct or import protocol UDP manager module `{forbidden}` directly"
            );
        }
    }
}

#[test]
fn protocol_udp_manager_roots_do_not_reexport_request_models() {
    let trojan = read("src/adapters/trojan/udp/managed.rs");
    for forbidden in ["TrojanSendExisting", "TrojanRelayExisting"] {
        assert!(
            !trojan.contains(forbidden),
            "Trojan managed UDP glue should use generic request models, not `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_manager_request_models_are_manager_private() {
    for removed in [
        "src/adapters/hysteria2/udp/manager.rs",
        "src/adapters/hysteria2/udp/manager/model.rs",
        "src/adapters/hysteria2/udp/manager/send.rs",
        "src/adapters/hysteria2/udp/manager/establish.rs",
        "src/adapters/shadowsocks/udp/manager.rs",
        "src/adapters/shadowsocks/udp/manager/model.rs",
        "src/adapters/shadowsocks/udp/manager/entry.rs",
        "src/adapters/shadowsocks/udp/manager/bridge.rs",
        "src/adapters/mieru/udp/manager.rs",
        "src/adapters/mieru/udp/manager/model.rs",
        "src/adapters/mieru/udp/manager/send.rs",
        "src/adapters/mieru/udp/manager/establish.rs",
        "src/adapters/mieru/udp/manager/connect.rs",
        "src/adapters/trojan/udp/manager.rs",
        "src/adapters/trojan/udp/manager/model.rs",
        "src/adapters/trojan/udp/manager/send.rs",
        "src/adapters/trojan/udp/manager/establish.rs",
        "src/adapters/trojan/udp/manager/connect.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "managed UDP protocols should use generic runtime glue instead of `{removed}`"
        );
    }
}

#[test]
fn stream_udp_managers_do_not_rebuild_protocol_cache_keys() {
    let mieru_managed = read("src/adapters/mieru/udp/managed.rs");
    let mieru_connector = read("src/adapters/mieru/udp/managed/connector.rs");
    let trojan_managed = read("src/adapters/trojan/udp/managed.rs");
    let trojan_connector = read("src/adapters/trojan/udp/managed/connector.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let managed_cache = read("src/runtime/udp_flow/managed/cache.rs");
    assert!(
        stream_manager.contains("ManagedUdpConnectionCache")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowStore")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowStore")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowSessions")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowSessions")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowConnection")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowConnection")
            && mieru_connector.contains("mieru::udp::MieruUdpFlowConnection")
            && trojan_connector.contains("trojan::udp::TrojanUdpFlowConnection")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowStore<mieru::udp::MieruUdpFlowSession>")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowStore<trojan::udp::TrojanUdpFlowSession>")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowStore<mieru::udp::MieruUdpFlowConnection>")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowStore<trojan::udp::TrojanUdpFlowConnection>")
            && !mieru_managed.contains("HashMap<mieru::udp::MieruUdpCacheKey")
            && !trojan_managed.contains("HashMap<trojan::udp::TrojanUdpCacheKey")
            && managed_cache.contains("struct ManagedUdpConnectionCache")
            && managed_cache.contains("struct ManagedUdpConnectionCacheKey")
            && !managed_cache.contains("pub(crate) struct ManagedUdpConnectionCacheKey"),
        "stream UDP managers should cache neutral proxy connection capabilities without holding protocol flow stores"
    );
    assert!(
        !mieru_managed.contains("MieruUdpCacheKey::relay")
            && !trojan_managed.contains("TrojanUdpCacheKey::relay")
            && !mieru_managed.contains("resume.cache_key(endpoint.server, endpoint.port, session_id)")
            && !trojan_managed.contains("resume.cache_key(endpoint.server, endpoint.port, session_id)")
            && mieru_connector.contains("mieru::udp::connector_flow_from_resume")
            && trojan_connector.contains("trojan::udp::connector_flow_from_resume")
            && !mieru_connector.contains("resume.connector_flow(endpoint.server, endpoint.port, session_id)")
            && !trojan_connector.contains("resume.connector_flow(endpoint.server, endpoint.port, session_id)")
            && !mieru_connector.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !trojan_connector.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !mieru_connector.contains("resume.flow_cache_key(")
            && !trojan_connector.contains("resume.flow_cache_key(")
            && !mieru_managed.contains("ManagedUdpConnectionCacheKey")
            && !trojan_managed.contains("ManagedUdpConnectionCacheKey")
            && stream_manager.contains(".send_or_insert_key(")
            && stream_manager.contains(".insert_and_send_key(")
            && !stream_manager.contains("if let Some(entry) = self.upstreams.get(&cache_key)")
            && !stream_manager.contains("self.upstreams.insert(")
            && !stream_manager.contains("entry.spawn_response_bridge(")
            && managed_cache.contains("async fn insert_and_send")
            && !managed_cache.contains("pub(crate) async fn insert_and_send(")
            && managed_cache.contains("pub(crate) async fn insert_and_send_key")
            && managed_cache.contains("pub(crate) async fn send_or_insert_key")
            && managed_cache.contains("self.entries.get(&key)"),
        "stream UDP managers should ask protocol resumes for opaque cache identity instead of choosing protocol key variants"
    );
}

#[test]
fn udp_dispatch_cached_flow_fast_path_delegates_to_registered() {
    let dispatch = read("src/runtime/udp_dispatch/dispatch.rs");
    let forward = read("src/runtime/udp_dispatch/forward.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");

    assert!(
        !dispatch.contains("send_existing_cached_flow")
            && !forward.contains("send_existing_cached_flow")
            && forward.contains("UdpPathCategory::Datagram | UdpPathCategory::StreamPacket")
            && forward.contains("forward_existing_managed_flow")
            && !outbound.contains("Cached {")
            && !outbound.contains("UdpPathCategory::Cached"),
        "UDP dispatch should delegate cached protocol flow reuse to protocol state without exposing a cached path category"
    );

    let normalized = forward.replace("\r\n", "\n");
    for forbidden in [
        ".registered\n            .vless",
        ".registered\n            .vmess",
    ] {
        assert!(
            !normalized.contains(forbidden),
            "src/runtime/udp_dispatch/forward.rs should not reach into protocol manager `{forbidden}` directly"
        );
    }
}

#[test]
fn udp_relay_start_delegates_packet_path_chain_to_dispatch_runtime() {
    let content = read("src/runtime/udp_dispatch/start/relay.rs");
    let root = read("src/runtime/udp_dispatch/mod.rs");
    let packet_path = read("src/runtime/udp_dispatch/packet_path.rs");
    let flow_state = read("src/runtime/udp_flow/state.rs");

    assert!(
        content.contains("send_packet_path_chain"),
        "UDP relay start should delegate packet-path manager work to UdpDispatch"
    );
    assert!(
        packet_path.contains("self.flow_state")
            && root.contains("flow_state: UdpFlowState")
            && !packet_path.contains("self.packet_path")
            && !root.contains("packet_path: PacketPathManager")
            && flow_state.contains("packet_path: PacketPathManager")
            && flow_state.contains("UdpFlowContext"),
        "runtime udp_dispatch/packet_path.rs should delegate packet-path manager work to UdpFlowState"
    );
    assert!(
        !content.contains(".registered") && !content.contains(".packet_path"),
        "src/runtime/udp_dispatch/start/relay.rs should not reach into protocol state or packet_path manager directly"
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
        content.contains("forward_existing_managed_flow"),
        "src/runtime/udp_dispatch/forward.rs should delegate protocol manager forwarding to RegisteredUdpState"
    );

    for forbidden in [
        "SsUdpPeer",
        "H2UdpPeer",
        "TrojanUdpPeer",
        "MieruUdpPeer",
        "UdpPeerEndpoint",
        "Socks5UdpSend",
        "protocol_runtime::socks5_udp",
        ".packet_path",
        "forward_existing_packet_path_flow(&mut self.chain_tasks",
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
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let forward = manifest_dir().join("src/runtime/udp_flow/registered/forward.rs");

    for forbidden in [
        "async fn forward_existing_managed_flow",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
        "UdpFlowOutbound::Direct",
        "UdpFlowOutbound::Socks5",
        "udp_protocol_forward",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/runtime/udp_flow/registered/mod.rs should keep existing-flow forwarding details in state/forward.rs; found `{forbidden}`"
        );
    }
    assert!(
        forward.exists()
            && read("src/runtime/udp_flow/registered/forward.rs")
                .contains("async fn forward_existing_managed_flow"),
        "existing UDP managed-flow forwarding should live in runtime/udp_flow/registered/forward.rs"
    );
}

#[test]
fn protocol_udp_existing_flow_handlers_live_outside_forward_dispatch() {
    let forward = read("src/runtime/udp_flow/registered/forward.rs");
    let normalized_forward = forward.replace("\r\n", "\n");
    let managed = read("src/runtime/udp_flow/managed/state.rs");
    let managed_datagram = read("src/runtime/udp_flow/managed/datagram.rs");
    let managed_model = read("src/runtime/udp_flow/managed/model.rs");
    let managed_stream = read("src/runtime/udp_flow/managed/stream.rs");
    let upstream = read("src/runtime/udp_flow/registered/upstream.rs");
    let socks5_runtime = read("src/adapters/socks5/udp/runtime.rs");

    for forbidden in [
        "SsSendExisting",
        "H2SendExisting",
        "TrojanSendExisting",
        "MieruSendExisting",
        "ExistingFlow {",
        "datagram_cache_key",
        "cipher_kind",
        "client_fingerprint",
        "relay_chain",
        ".upstream()",
        "ManagedUdpFlowResume",
        "snapshot.resume()",
        "Socks5(_)",
    ] {
        assert!(
            !forward.contains(forbidden),
            "state/forward.rs should delegate protocol UDP flow field extraction to state/forward/*.rs; found `{forbidden}`"
        );
    }
    assert!(
        normalized_forward.contains("self\n            .managed\n            .forward_existing_flow")
            && forward.contains("self.upstream.handles_resume(resume)")
            && upstream.contains("fn handles_resume")
            && upstream.contains("handler.supports_upstream_resume(resume)")
            && socks5_runtime.contains("fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume)")
            && socks5_runtime
                .replace(char::is_whitespace, "")
                .contains("resume.as_ref::<socks5::udp::Socks5UdpFlowResume>()")
            && managed.contains("fn forward_existing_flow")
            && managed.contains("is_upstream_resume(&resume)")
            && !forward.contains("managed_flow_snapshot")
            && managed_model.contains("trait ManagedDatagramFlowHandler")
            && managed_model.contains("trait ManagedStreamFlowHandler")
            && managed_model.contains("pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>")
            && managed_model.contains("pub(crate) resume: ManagedUdpFlowResume")
            && managed_datagram.contains("ManagedExistingSend")
            && managed_datagram.contains("send_managed_existing")
            && managed_datagram.contains("for handler in &mut self.handlers")
            && managed_stream.contains("ManagedExistingSend")
            && managed_stream.contains("send_managed_existing")
            && managed_stream.contains("for handler in &mut self.handlers"),
        "existing UDP protocol-flow handling should dispatch neutral send requests through managed handlers"
    );
    for forbidden in [
        "SsSendExisting",
        "H2SendExisting",
        "TrojanSendExisting",
        "MieruSendExisting",
        "TrojanRelayExisting",
        "MieruRelayExisting",
        "ManagedUdpFlowResume::Shadowsocks",
        "ManagedUdpFlowResume::Hysteria2",
        "ManagedUdpFlowResume::Trojan",
        "ManagedUdpFlowResume::Mieru",
    ] {
        assert!(
            !managed.contains(forbidden),
            "ManagedUdpState should not construct protocol manager request model `{forbidden}`"
        );
    }
    for forbidden in [
        "shadowsocks: SsChainManager",
        "hysteria2: H2ChainManager",
        "trojan: TrojanChainManager",
        "mieru: MieruChainManager",
        "self.shadowsocks",
        "self.hysteria2",
        "self.trojan",
        "self.mieru",
    ] {
        assert!(
            !managed_datagram.contains(forbidden) && !managed_stream.contains(forbidden),
            "managed UDP sub-states should dispatch through handler lists, not protocol field `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_cached_flow_fast_path_lives_outside_state_root() {
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let old_cached = manifest_dir().join("src/runtime/udp_flow/registered/cached.rs");
    let protocol_stream_sender =
        manifest_dir().join("src/runtime/udp_flow/registered/stream_sender.rs");
    let stream_sender = manifest_dir().join("src/runtime/udp_flow/managed/stream_sender.rs");
    let managed = read("src/runtime/udp_flow/managed/state.rs");
    let stream_sender_state = read("src/runtime/udp_flow/managed/stream_sender.rs");
    let stream_state = read("src/runtime/udp_flow/managed/stream.rs");
    let protocol_forward = read("src/runtime/udp_flow/registered/forward.rs");
    let vless_flow = manifest_dir().join("src/runtime/udp_flow/registered/vless_flow.rs");
    let vmess_flow = manifest_dir().join("src/runtime/udp_flow/registered/vmess_flow.rs");
    let vless_adapter = read("src/adapters/vless/udp/flow.rs");
    let vmess_adapter = read("src/adapters/vmess/udp/flow.rs");
    let cached_start = manifest_dir().join("src/runtime/udp_flow/registered/cached_start.rs");
    let register = read("src/register.rs");

    for forbidden in [
        "fn send_existing_cached_flow",
        ".vless\n            .send_existing",
        ".vmess\n            .send_existing",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/runtime/udp_flow/registered/mod.rs should keep managed stream forwarding details in managed/stream_sender.rs; found `{forbidden}`"
        );
    }
    assert!(
        stream_sender.exists() && !old_cached.exists() && !protocol_stream_sender.exists(),
        "managed stream UDP sender forwarding should live under managed/stream_sender.rs, not registered"
    );
    assert!(
        !state.contains("stream_senders: ManagedStreamSenderState")
            && !managed.contains("cached: ManagedCachedState")
            && !managed.contains("vless: VlessUdpOutboundManager")
            && !managed.contains("vmess: VmessUdpOutboundManager")
            && stream_state.contains("senders: ManagedStreamSenderState")
            && !stream_sender_state.contains("start_cached_flow")
            && !cached_start.exists()
            && stream_sender_state.contains("struct ManagedStreamSenderState")
            && stream_sender_state.contains("trait ManagedStreamFlowSender")
            && !stream_sender_state.contains("Vec<Box<dyn ManagedStreamFlowSender>>")
            && stream_sender_state
                .contains("HashMap<ManagedUdpFlowRef, Box<dyn ManagedStreamFlowSender>>")
            && stream_sender_state.contains("fn sender(")
            && !protocol_forward.contains("has_stream_flow_sender")
            && !protocol_forward.contains("udp_cached_send")
            && stream_state.contains("udp_stream_send")
            && !stream_sender_state.contains("fn send_existing_cached_flow")
            && !managed.contains("ManagedCachedFlowSender")
            && !stream_sender_state.contains("enum CachedUdpFlowStart")
            && !stream_sender_state.contains("VlessUdpStartFlow")
            && !stream_sender_state.contains("VmessUdpStartFlow")
            && !stream_sender_state.contains("VlessCachedFlowHandler")
            && !stream_sender_state.contains("VmessCachedFlowHandler")
            && !stream_sender_state.contains("vless: Box")
            && !stream_sender_state.contains("vmess: Box")
            && !stream_sender_state.contains(".get_mut(0)")
            && !stream_sender_state.contains(".get_mut(1)")
            && !stream_sender_state.contains("handlers.get_mut")
            && !stream_sender_state.contains("fn senders(")
            && !stream_sender_state.contains("std::any::Any")
            && !stream_sender_state.contains("downcast")
            && !stream_sender_state.contains("as_any")
            && !state.contains("cached_handler_mut")
            && !vless_flow.exists()
            && !vmess_flow.exists()
            && !vless_adapter.contains("VlessUdpOutboundManager")
            && vless_adapter.contains("ManagedStreamPacketSender")
            && vless_adapter.contains("register_managed_stream_packet_flow")
            && !vless_adapter.contains("register_managed_stream_flow_sender")
            && !vless_adapter.contains("cached_handler_mut")
            && !vmess_adapter.contains("VmessUdpOutboundManager")
            && vmess_adapter.contains("ManagedStreamPacketSender")
            && vmess_adapter.contains("register_managed_stream_packet_flow")
            && !vmess_adapter.contains("register_managed_stream_flow_sender")
            && !vmess_adapter.contains("cached_handler_mut")
            && !register.contains("ManagedStreamSenderHandlers")
            && !register.contains("vless_cached_handler")
            && !register.contains("vmess_cached_handler"),
        "managed stream UDP flow starts should use generic stream packet senders while generic state keeps only stream senders without Vec-order protocol identity or runtime downcasts"
    );
}

#[test]
fn protocol_udp_packet_path_facade_lives_in_udp_dispatch_runtime() {
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let packet_path_content = read("src/runtime/udp_dispatch/packet_path.rs");
    let flow_state = read("src/runtime/udp_flow/state.rs");
    let registered_packet_path =
        manifest_dir().join("src/runtime/udp_flow/registered/packet_path.rs");
    let dispatch_packet_path = manifest_dir().join("src/runtime/udp_dispatch/packet_path.rs");

    for forbidden in [
        "fn datagram_chain_flow_outbound",
        "fn send_packet_path_chain",
        "fn forward_existing_packet_path_flow",
        "UdpFlowOutbound::Shadowsocks",
        "packet_path_carrier",
        "PacketPathManager",
        "mod packet_path;",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/runtime/udp_flow/registered/mod.rs should not own packet-path facade details; found `{forbidden}`"
        );
    }
    assert!(
        !registered_packet_path.exists(),
        "UDP packet-path facade should not live in protocol_runtime/udp/state/packet_path.rs"
    );
    assert!(
        dispatch_packet_path.exists(),
        "UDP packet-path facade should live in runtime/udp_dispatch/packet_path.rs"
    );
    assert!(
        packet_path_content.contains("datagram_chain_flow_outbound")
            && packet_path_content.contains("send_packet_path_chain")
            && packet_path_content.contains("self.flow_state")
            && !packet_path_content.contains("PacketPathManager")
            && flow_state.contains("PacketPathManager")
            && flow_state.contains("fn forward_existing_packet_path_flow"),
        "udp_dispatch packet-path facade should expose orchestration helpers while UdpFlowState owns packet-path manager dispatch"
    );
    for forbidden in [
        "ManagedUdpFlowSnapshot::Shadowsocks",
        "ManagedUdpFlowSnapshot",
        "password: datagram.password",
        "cipher_kind: datagram.cipher_kind",
        "datagram_cache_key: datagram.datagram_cache_key",
        ".into_protocol_snapshot()",
        ".with_packet_path_carrier(",
    ] {
        assert!(
            !packet_path_content.contains(forbidden),
            "packet-path state should consume the datagram source protocol snapshot instead of constructing Shadowsocks snapshots directly; found `{forbidden}`"
        );
    }
    assert!(
        packet_path_content.contains("UdpFlowOutbound::PacketPathDatagram")
            && packet_path_content.contains("flow_binding.into_parts()")
            && packet_path_content.contains("snapshot: flow_snapshot"),
        "packet-path dispatch should store a neutral packet-path flow snapshot without converting it to a protocol UDP snapshot"
    );
}

#[test]
fn adapters_do_not_construct_udp_dispatch_peer_helpers() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        if source.contains("/udp/manager") || source.contains("\\udp\\manager") {
            continue;
        }
        let allow_neutral_managed_connector = matches!(
            source.as_str(),
            "src/adapters/hysteria2/udp/managed.rs" | "src/adapters/shadowsocks/udp/managed.rs"
        );
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "SsUdpPeer",
            "H2UdpPeer",
            "TrojanUdpPeer",
            "MieruUdpPeer",
            "UdpFlowContext",
            "UdpPacketRef",
        ] {
            if allow_neutral_managed_connector
                && matches!(forbidden, "UdpFlowContext" | "UdpPacketRef")
            {
                continue;
            }
            assert!(
                !content.contains(forbidden),
                "{source} should not construct udp-dispatch peer helper `{forbidden}`"
            );
        }
    }
}

#[test]
fn packet_path_chain_does_not_own_socks5_runtime_state() {
    let content = read("src/runtime/udp_flow/packet_path_chain.rs");

    for forbidden in [
        "ActiveUpstreamSocks5UdpAssociation",
        "Socks5PacketPath",
        "socks5::parse_udp_packet",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_flow/packet_path_chain.rs should stay generic; found `{forbidden}`"
        );
    }
}

#[test]
fn packet_path_traits_are_grouped_by_responsibility() {
    let packet_path = read("src/runtime/udp_flow/packet_path.rs");
    let protocol_udp_root = read("src/runtime/udp_flow/registered/mod.rs");
    let runtime_root = manifest_dir().join("src/runtime/udp_flow");
    let peer = manifest_dir().join("src/runtime/udp_flow/registered/peer.rs");
    let ss_managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let h2_managed = read("src/adapters/hysteria2/udp/managed.rs");
    let trojan_managed = read("src/adapters/trojan/udp/managed.rs");
    let mieru_managed = read("src/adapters/mieru/udp/managed.rs");

    for required in [
        "trait PacketPathCarrier",
        "struct PacketPathCarrierDescriptor",
        "struct UdpDatagramSource",
        "type ChainTask =",
        "struct UdpFlowContext",
        "struct UdpPacketRef",
    ] {
        assert!(
            packet_path.contains(required),
            "runtime udp_flow packet_path.rs should own neutral packet-path definition `{required}`"
        );
    }
    assert!(
        packet_path.contains("struct UdpDatagramDescriptor")
            && packet_path.contains("tag: String")
            && packet_path.contains("server: String")
            && !packet_path.contains("struct UdpDatagramSourceParts")
            && !packet_path.contains("UdpDatagramSource<'")
            && !packet_path.contains("PacketPathFlowBinding<'"),
        "packet-path datagram source should be an owned neutral runtime object, not a borrowed leaf-shaped parts struct"
    );
    for forbidden in [
        "PacketPathCarrierDescriptor",
        "UdpDatagramSource",
        "PacketPathFlowSnapshot",
        "PacketPathFlowBinding",
        "ChainTask",
        "UdpFlowContext",
        "UdpPacketRef",
    ] {
        assert!(
            !protocol_udp_root.contains(forbidden),
            "protocol_runtime::udp root should not re-export generic packet-path runtime type `{forbidden}`"
        );
    }
    assert!(
        !peer.exists()
            && !runtime_root.join("peer.rs").exists()
            && !ss_managed.contains("struct SsUdpPeer")
            && !h2_managed.contains("struct H2UdpPeer")
            && !trojan_managed.contains("struct TrojanUdpPeer")
            && !mieru_managed.contains("struct MieruUdpPeer"),
        "protocol UDP peer models should not live under runtime packet-path helpers or protocol_runtime::udp root; stream/datagram managers should use neutral OutboundEndpoint directly"
    );
    assert!(
        !packet_path.contains("ProtocolRegistry::"),
        "packet-path trait docs should not describe packet-path products as monolithic ProtocolRegistry outputs"
    );
    assert!(
        packet_path.contains("UdpPacketPathCapability::udp_packet_path_carrier_descriptor")
            && packet_path.contains("UdpPacketPathCapability::udp_datagram_source"),
        "packet-path trait docs should point carrier/datagram products at UdpPacketPathCapability"
    );
}

#[test]
fn stream_protocol_udp_packet_io_stays_in_protocol_crates() {
    let vless_runtime = read("src/adapters/vless/udp/managed/establish.rs");
    let vmess_runtime = read("src/adapters/vmess/udp/managed/establish.rs");
    let vless_shared = fs::read_to_string(repo_root().join("protocols/vless/src/shared.rs"))
        .expect("read VLESS protocol shared source");
    let vless_outbound = fs::read_to_string(repo_root().join("protocols/vless/src/outbound.rs"))
        .expect("read VLESS protocol outbound source");
    let vmess_protocol = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read VMess protocol UDP source");

    for (source, content, flow_helper) in [
        (
            "src/adapters/vless/udp/managed/establish.rs",
            &vless_runtime,
            "establish_flow_with_initial_packet",
        ),
        (
            "src/adapters/vmess/udp/managed.rs",
            &vmess_runtime,
            "establish_flow_with_initial_packet",
        ),
    ] {
        for forbidden in [".encode_packet(", ".decode_packet("] {
            assert!(
                !content.contains(forbidden),
                "{source} should call protocol-owned stream packet IO helpers instead of direct UDP packet framing `{forbidden}`"
            );
        }
        assert!(
            !content.contains(".write_packet_tokio(")
                && !content.contains(".read_packet_tokio(")
                && content.contains(flow_helper),
            "{source} should keep cache/bridge glue and delegate protocol UDP flow pumping"
        );
        assert!(
            !content.contains("::establish_udp_flow_with_initial_packet"),
            "{source} should call flow pumping through the protocol-owned UDP flow config"
        );
    }

    assert!(
        vless_shared.contains("pub async fn write_packet_tokio")
            && vless_shared.contains("pub async fn read_packet_tokio")
            && vless_shared.contains("failed to flush VLESS UDP response")
            && vless_outbound.contains("pub fn spawn_udp_flow")
            && vless_outbound.contains("fn spawn_udp_flow_task")
            && vless_outbound.contains(".write_packet_tokio(")
            && vless_outbound.contains(".read_packet_tokio("),
        "protocols/vless should own async stream packet IO helpers and UDP flow pumping"
    );
    assert!(
        vmess_protocol.contains("pub async fn write_packet_tokio")
            && vmess_protocol.contains("pub async fn read_packet_tokio")
            && vmess_protocol.contains("failed to flush VMess UDP response")
            && vmess_protocol.contains("pub fn spawn_udp_flow")
            && vmess_protocol.contains("fn spawn_udp_flow_task")
            && vmess_protocol.contains(".write_packet_tokio(")
            && vmess_protocol.contains(".read_packet_tokio("),
        "protocols/vmess should own async stream packet IO helpers and UDP flow pumping"
    );
}

#[test]
fn websocket_transport_stream_reader_preserves_frame_progress() {
    let ws = fs::read_to_string(repo_root().join("crates/transport/src/ws.rs"))
        .expect("read websocket transport source");

    assert!(
        ws.contains("self.read_buffer.clear();")
            && ws.contains("self.read_offset = 0;")
            && ws.contains("Message::Binary(data)")
            && ws.contains("Message::Text(data)")
            && ws.contains("_ => continue"),
        "WebSocket transport should clear consumed frame buffers and skip control frames without stalling stream-carried UDP responses"
    );
}

#[test]
fn packet_path_carriers_live_outside_chain_manager() {
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let carriers = manifest_dir().join("src/runtime/udp_flow/packet_path_chain/carriers.rs");

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
    let facade = read("src/runtime/udp_flow/packet_path_chain/carriers.rs");
    let udp_socket = manifest_dir()
        .join("src/runtime/udp_flow/packet_path_chain/carriers/udp_socket_carrier.rs");
    let quic_datagram = manifest_dir()
        .join("src/runtime/udp_flow/packet_path_chain/carriers/quic_datagram_carrier.rs");

    for forbidden in [
        "struct ShadowsocksPacketPath",
        "struct Hysteria2PacketPath",
        "shadowsocks_carrier",
        "hysteria2_carrier",
        "ShadowsocksDatagramCodec",
        "Hysteria2UdpPacketTarget",
        r#"#[cfg(feature = "hysteria2")]"#,
        "connect_raw",
        "build_shadowsocks_packet_path",
        "build_hysteria2_packet_path",
    ] {
        assert!(
            !facade.contains(forbidden),
            "packet_path_chain/carriers.rs should keep concrete carrier internals in protocol files; found `{forbidden}`"
        );
    }
    assert!(
        udp_socket.exists(),
        "UDP socket packet-path carrier should live in carriers/udp_socket_carrier.rs"
    );
    assert!(
        quic_datagram.exists(),
        "QUIC datagram packet-path carrier should live in carriers/quic_datagram_carrier.rs"
    );
}

#[test]
fn packet_path_carrier_transport_runtime_lives_in_zero_transport() {
    let udp_socket = read("src/runtime/udp_flow/packet_path_chain/carriers/udp_socket_carrier.rs");
    let quic_datagram =
        read("src/runtime/udp_flow/packet_path_chain/carriers/quic_datagram_carrier.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/udp_packet_path.rs"))
        .expect("read zero-transport udp packet path source");

    for (source, content) in [
        ("udp_socket_carrier.rs", &udp_socket),
        ("quic_datagram_carrier.rs", &quic_datagram),
    ] {
        assert!(
            content.contains("struct PacketPathCarrierAdapter")
                && content.contains("impl PacketPathCarrier for PacketPathCarrierAdapter"),
            "{source} should only adapt zero-transport packet-path runtime to the proxy carrier trait"
        );
        for forbidden in [
            "struct UdpSocketPacketPath",
            "struct QuicDatagramPacketPath",
            "tokio::net::UdpSocket::bind",
            "send_datagram",
            "read_datagram",
            "failed to decode UDP packet-path datagram",
            "failed to decode QUIC packet-path datagram",
            "exceeds recv buffer",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not own packet-path transport runtime detail `{forbidden}`"
            );
        }
    }
    assert!(
        transport.contains("pub struct UdpSocketPacketPath")
            && transport.contains("pub struct QuicDatagramPacketPath")
            && transport.contains("tokio::net::UdpSocket::bind")
            && transport.contains("send_datagram")
            && transport.contains("read_datagram")
            && transport.contains("failed to decode UDP packet-path datagram")
            && transport.contains("failed to decode QUIC packet-path datagram"),
        "zero-transport should own packet-path socket and QUIC datagram runtime details"
    );
}

#[test]
fn packet_path_chain_root_does_not_reexport_protocol_carrier_builders() {
    let root = read("src/runtime/udp_flow/packet_path_chain.rs");

    for forbidden in [
        "pub(crate) use carriers::build_shadowsocks_packet_path",
        "pub(crate) use carriers::build_hysteria2_packet_path",
    ] {
        assert!(
            !root.contains(forbidden),
            "packet_path_chain.rs should not re-export protocol carrier builder `{forbidden}`"
        );
    }
    assert!(
        root.contains("pub(crate) mod carriers;"),
        "packet_path_chain.rs should expose the explicit carriers module for adapter capability bridges"
    );

    for source in [
        "src/adapters/shadowsocks/udp/packet_path.rs",
        "src/adapters/hysteria2/udp/packet_path.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("packet_path_chain::carriers::")
                && content.contains("_carrier::build("),
            "{source} should call packet-path carrier builders through the explicit carrier submodule"
        );
    }
}

#[test]
fn packet_path_response_bridge_lives_outside_chain_manager() {
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let bridge = manifest_dir().join("src/runtime/udp_flow/packet_path_chain/bridge.rs");

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
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let key = manifest_dir().join("src/runtime/udp_flow/packet_path_chain/key.rs");
    let key_content = read("src/runtime/udp_flow/packet_path_chain/key.rs");
    let model = read("src/runtime/udp_flow/packet_path_chain/model.rs");
    let traits = read("src/runtime/udp_flow/packet_path.rs");

    for forbidden in [
        "struct PathKey",
        "carrier_key: carrier.cache_key().to_owned()",
        "datagram_cache_key: datagram_cache_key.to_owned()",
        "datagram_tag: String",
        "datagram_server: String",
        "datagram_port: u16",
        "datagram_cache_key: String",
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
    assert!(
        !key_content.contains("UdpDatagramSource")
            && !key_content.contains("datagram.datagram_cache_key")
            && !key_content.contains("datagram_tag: String")
            && !key_content.contains("datagram_server: String")
            && !key_content.contains("datagram_port: u16")
            && !key_content.contains("datagram_cache_key: String")
            && key_content.contains("datagram: UdpDatagramKey")
            && !key_content.contains("UdpPacketPathCarrier"),
        "packet-path key model should use opaque carrier/datagram key parts instead of reading source internals"
    );
    assert!(
        model.contains("self.datagram.descriptor().key_part()")
            && traits.contains("struct UdpDatagramKey")
            && traits.contains("fn key_part(&self) -> UdpDatagramKey"),
        "UdpDatagramSource should expose a neutral descriptor that provides the packet-path datagram key part"
    );
}

#[test]
fn packet_path_entry_model_lives_outside_chain_manager() {
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let model = read("src/runtime/udp_flow/packet_path_chain/model.rs");
    let bridge = read("src/runtime/udp_flow/packet_path_chain/bridge.rs");
    let traits = read("src/runtime/udp_flow/packet_path.rs");

    for forbidden in [
        "struct Entry",
        "struct EntryCandidate",
        "fn key(&self) -> PathKey",
        "datagram_server: String",
        "datagram_port: u16",
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
        "datagram_endpoint: UdpDatagramEndpoint",
    ] {
        assert!(
            model.contains(required),
            "packet-path entry model should live in packet_path_chain/model.rs; missing `{required}`"
        );
    }
    assert!(
        !bridge.contains("entry.datagram_server")
            && !bridge.contains("entry.datagram_port")
            && bridge.contains("entry.datagram_endpoint.target()")
            && bridge.contains("entry.datagram_endpoint.upstream()")
            && traits.contains("struct UdpDatagramEndpoint")
            && traits.contains("fn endpoint(&self) -> UdpDatagramEndpoint"),
        "packet-path bridge should use a neutral datagram endpoint instead of unpacking entry datagram fields"
    );
}

#[test]
fn packet_path_entry_build_lives_outside_chain_manager() {
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let entry_content = read("src/runtime/udp_flow/packet_path_chain/entry.rs");
    let entry = manifest_dir().join("src/runtime/udp_flow/packet_path_chain/entry.rs");

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
    assert!(
        !entry_content.contains("ShadowsocksDatagramCodec"),
        "packet-path entry build should use adapter-provided datagram codecs instead of constructing Shadowsocks codec directly"
    );
    assert!(
        entry_content.contains("candidate.datagram.codec.clone()"),
        "packet-path entry build should clone the codec supplied by UdpDatagramSource"
    );
}

#[test]
fn packet_path_diagnostics_live_outside_chain_manager() {
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let diagnostics = manifest_dir().join("src/runtime/udp_flow/packet_path_chain/diagnostics.rs");

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
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let snapshot_content = read("src/runtime/udp_flow/packet_path_chain/snapshot.rs");
    let snapshot = manifest_dir().join("src/runtime/udp_flow/packet_path_chain/snapshot.rs");

    for forbidden in [
        "PathKey::from_lookup",
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
    assert!(
        snapshot_content.contains("lookup_key: PacketPathLookupKey")
            && !snapshot_content.contains("PacketPathFlowSnapshot")
            && !snapshot_content.contains("UdpPacketPathCarrier"),
        "packet-path snapshot lookup should receive a neutral packet-path lookup key"
    );
}

#[test]
fn packet_path_snapshot_send_uses_request_model() {
    let manager = read("src/runtime/udp_flow/packet_path_chain.rs");
    let packet_path = read("src/runtime/udp_dispatch/packet_path.rs");
    let flow_state = read("src/runtime/udp_flow/state.rs");

    assert!(
        manager.contains("struct SendWithSnapshotRequest")
            && manager.contains("request: SendWithSnapshotRequest<'_>")
            && manager.contains("lookup_key: PacketPathLookupKey"),
        "packet-path snapshot send should use a request model"
    );
    assert!(
        packet_path.contains("self.flow_state")
            && !packet_path.contains("SendWithSnapshotRequest {")
            && flow_state.contains("SendWithSnapshotRequest {")
            && flow_state.contains("lookup_key: snapshot.lookup_key()")
            && !flow_state.contains("carrier_cache_key: &snapshot.carrier_cache_key")
            && !flow_state.contains("datagram_cache_key: &snapshot.datagram_cache_key")
            && flow_state.contains("forward_existing_packet_path_flow"),
        "packet-path snapshot forward path should convert snapshots into neutral lookup keys behind UdpFlowState without unpacking cache fields in dispatch"
    );
}

#[test]
fn feature_gated_udp_manager_modules_do_not_embed_disabled_stubs() {
    for source in [
        "src/adapters/mieru/udp/managed.rs",
        "src/adapters/trojan/udp/managed.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains("#[cfg(not(feature ="),
            "{source} should not mix enabled manager logic with disabled-feature stubs"
        );
    }
}

#[test]
fn trojan_udp_socket_wrappers_stay_in_proxy_stream_glue() {
    let managed = read("src/adapters/trojan/udp/managed.rs");
    let stream = manifest_dir().join("src/adapters/trojan/udp/manager/stream.rs");
    let socket = manifest_dir().join("src/adapters/trojan/udp/manager/socket.rs");
    let transport =
        fs::read_to_string(repo_root().join("crates/transport/src/trojan_transport.rs"))
            .expect("read zero-transport trojan_transport source");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    for forbidden in ["struct ReadOnlySocket", "struct WriteOnlySocket"] {
        assert!(
            !managed.contains(forbidden) && !stream.exists(),
            "Trojan proxy glue should not own stream socket adapter `{forbidden}`"
        );
    }
    assert!(
        !socket.exists(),
        "Trojan UDP stream half AsyncSocket adapters should not live in a separate proxy socket module"
    );
    assert!(
        protocol_outbound.contains("struct ReadOnlySocket")
            && protocol_outbound.contains("struct WriteOnlySocket")
            && protocol_outbound.contains("impl<S> AsyncSocket for ReadOnlySocket")
            && protocol_outbound.contains("impl<S> AsyncSocket for WriteOnlySocket")
            && !transport.contains("struct ReadOnlySocket")
            && !transport.contains("struct WriteOnlySocket")
            && !transport.contains("impl AsyncSocket for ReadOnlySocket")
            && !transport.contains("impl AsyncSocket for WriteOnlySocket"),
        "Trojan UDP stream half AsyncSocket adapters should live with protocols/trojan packet pump, not proxy or zero-transport"
    );
}

#[test]
fn trojan_udp_response_bridge_lives_outside_manager() {
    let trojan_managed = read("src/adapters/trojan/udp/managed.rs");
    let connector = read("src/adapters/trojan/udp/managed/connector.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let managed = read("src/runtime/udp_flow/managed/connection.rs");
    let bridge = manifest_dir().join("src/adapters/trojan/udp/manager/bridge.rs");

    for forbidden in ["broadcast::channel", "recv_tx.subscribe", "fn spawn_bridge"] {
        assert!(
            !trojan_managed.contains(forbidden),
            "Trojan managed.rs should not own response bridge details; found `{forbidden}`"
        );
    }
    assert!(
        !bridge.exists()
            && stream_manager.contains(".insert_and_send_key(")
            && !trojan_managed.contains(".spawn_response_bridge(")
            && !trojan_managed.contains("self.upstreams.insert(")
            && !trojan_managed.contains("spawn_trojan_response_bridge")
            && !trojan_managed.contains("spawn_response_bridge(\n")
            && !trojan_managed.contains("impl ManagedUdpConnection for trojan::udp::TrojanUdpFlowConnection")
            && !trojan_managed.contains("managed_packet_udp_connection")
            && connector.contains("managed_packet_udp_connection")
            && !trojan_managed.contains("spawn_response_bridge")
            && managed.contains("pub(crate) fn managed_packet_udp_connection")
            && managed.contains("pub(crate) fn spawn_response_bridge<T, F>")
            && managed.contains("FnMut(T) -> (Address, u16, Vec<u8>)"),
        "Trojan UDP response bridge should hang off the neutral managed packet connection bridge, not adapter send orchestration"
    );
}

#[test]
fn trojan_udp_tls_connect_lives_outside_manager() {
    let connect_path = manifest_dir().join("src/adapters/trojan/udp/manager/connect.rs");
    let managed = read("src/adapters/trojan/udp/managed.rs");
    let connector = read("src/adapters/trojan/udp/managed/connector.rs");
    let outbound = manifest_dir().join("src/outbound/trojan.rs");
    let transport =
        fs::read_to_string(repo_root().join("crates/transport/src/trojan_transport.rs"))
            .expect("read zero-transport trojan_transport source");

    for forbidden in [
        "ClientTlsConfig",
        "connect_tls_upstream",
        "connect_tls_stream",
        "TrojanUdpTlsOptions",
        "resume.tls_profile(",
        "tls_profile.",
        ".connect_host(",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Trojan UDP managed glue should keep TLS config/profile conversion out of adapter glue; found `{forbidden}`"
        );
    }
    assert!(
        !connect_path.exists(),
        "Trojan UDP TLS connect helpers should collapse into managed.rs thin protocol glue"
    );
    for forbidden in [
        "zero_transport::tls::connect_tls_upstream",
        "zero_transport::tls::connect_tls_stream",
        "connect_tls_upstream",
        "connect_tls_stream",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Trojan managed.rs should delegate only raw TLS stream opening through the connector; found `{forbidden}`"
        );
    }
    assert!(
        !managed.contains("crate::outbound::trojan::open_udp_tls_stream")
            && !connector.contains("crate::outbound::trojan::open_udp_tls_stream")
            && !connector.contains("crate::outbound::trojan::open_udp_tls_relay_stream")
            && !outbound.exists()
            && connector.contains("open_trojan_udp_tls_stream")
            && connector.contains("open_trojan_udp_tls_relay_stream")
            && connector.contains("TrojanUdpTlsOptions")
            && connector.contains("tls_profile_spec().tls_profile(")
            && !connector.contains("resume.tls_profile(")
            && connector.contains("tls_profile.server_name()")
            && connector.contains("tls_profile.insecure()")
            && connector.contains("tls_profile.client_fingerprint()")
            && connector.contains("TrojanTlsProfile::from_parts")
            && !connector.contains("fn udp_tls_config(")
            && !connector.contains("ClientTlsConfig")
            && !connector.contains("ClientTlsConfig {")
            && transport.contains("pub struct TrojanUdpTlsOptions")
            && transport.contains("pub struct TrojanTlsProfile")
            && transport.contains("ClientTlsConfig")
            && transport.contains("fn into_tls_config(self) -> ClientTlsConfig")
            && transport.contains("tls_profile: TrojanTlsProfile")
            && transport.contains("crate::tls::connect_tls_upstream")
            && transport.contains("crate::tls::connect_tls_stream")
            && !transport.contains("trojan::")
            && !transport.contains("TrojanUdpTlsProfile")
            && !transport.contains("TrojanTcpTlsProfile"),
        "zero-transport should own neutral TLS stream opening and config materialization; Trojan protocol profiles stay outside transport"
    );
}

#[test]
fn trojan_udp_flow_resume_is_protocol_owned() {
    let adapter = read("src/adapters/trojan/udp.rs");
    let adapter_flow = read("src/adapters/trojan/udp/flow.rs");
    let snapshot = read("src/runtime/udp_flow/managed/flow.rs");
    let forward = read("src/runtime/udp_flow/managed/stream.rs");
    let start = read("src/runtime/udp_flow/managed/stream.rs");
    let managed = read("src/adapters/trojan/udp/managed.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let manager_stream = manifest_dir().join("src/adapters/trojan/udp/manager/stream.rs");
    let transport =
        fs::read_to_string(repo_root().join("crates/transport/src/trojan_transport.rs"))
            .expect("read zero-transport trojan_transport source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/trojan/src/lib.rs"))
        .expect("read trojan protocol lib source");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");
    let connector_flow_impl = impl_block(&protocol_outbound, "TrojanUdpConnectorFlow");

    assert!(
        !adapter.contains("TrojanUdpFlowResume::new")
            && !adapter.contains("TrojanUdpFlowConfig::new")
            && !adapter.contains(".flow_resume(false)")
            && !adapter.contains(".flow_resume(true)")
            && adapter_flow.contains("trojan::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("TrojanUdpFlowConfig::new")
            && !adapter_flow.contains(".flow_resume(request.relay_chain)")
            && protocol_outbound.contains("struct TrojanUdpFlowResume")
            && protocol_outbound.contains("struct TrojanUdpFlowConfig")
            && protocol_outbound.contains("pub fn flow_resume(&self, relay_chain: bool)")
            && protocol_outbound.contains("pub fn udp_flow_resume_from_config(")
            && protocol_outbound.contains("pub struct TrojanUdpConnectorFlow")
            && !connector_flow_impl.contains("pub fn cache_key(&self)")
            && !connector_flow_impl.contains("pub fn requires_relay_upstream(&self)")
            && connector_flow_impl.contains("pub fn into_parts(self) -> (String, bool)")
            && protocol_outbound.contains("pub struct TrojanUdpTlsProfileSpec")
            && protocol_outbound.contains("pub fn connector_flow(")
            && !protocol_outbound.contains("pub struct TrojanUdpFlowSpec")
            && !protocol_outbound.contains("pub struct TrojanUdpFlowRequirement")
            && !protocol_outbound.contains("pub fn flow_requirement(&self)")
            && protocol_outbound.contains("pub fn tls_profile_spec(&self)")
            && protocol_outbound.contains("fn peer_config(&self)")
            && !protocol_outbound.contains("pub fn peer_config(&self)")
            && protocol_outbound.contains("fn flow_key(&self")
            && !protocol_outbound.contains("pub fn flow_key(&self")
            && protocol_outbound.contains("fn cache_key(&self")
            && !protocol_outbound
                .contains("pub fn cache_key(&self, server: &str, port: u16, session_id: u64)")
            && protocol_outbound.contains("pub fn flow_cache_key(&self")
            && protocol_outbound.contains("enum TrojanUdpFlowKey")
            && !protocol_outbound.contains("pub enum TrojanUdpFlowKey")
            && protocol_outbound.contains("enum TrojanUdpCacheKey")
            && !protocol_outbound.contains("pub enum TrojanUdpCacheKey")
            && protocol_outbound.contains("pub struct TrojanUdpFlowStore")
            && protocol_outbound.contains("struct TrojanUdpPeerConfig")
            && !protocol_outbound.contains("pub struct TrojanUdpPeerConfig")
            && protocol_outbound.contains("pub struct TrojanUdpTlsProfile")
            && protocol_outbound.contains("pub fn tls_profile(&self")
            && protocol_outbound.contains("pub async fn establish_udp_tunnel")
            && protocol_outbound.contains("struct TrojanUdpLeafKey")
            && !protocol_outbound.contains("pub struct TrojanUdpLeafKey")
            && protocol_outbound.contains("pub fn client_fingerprint(&self) -> Option<&str>")
            && protocol_outbound.contains("pub fn flow_requires_relay_upstream(&self) -> bool")
            && !protocol_outbound.contains("pub fn relay_chain(&self) -> bool"),
        "Trojan adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    for forbidden in [
        "TrojanUdpFlowKey",
        "TrojanUdpLeafKey",
        "TrojanUdpPeerConfig",
        "TrojanUdpCacheKey",
    ] {
        assert!(
            !protocol_lib.contains(forbidden),
            "protocols/trojan lib root should not re-export UDP cache-key internals `{forbidden}`"
        );
    }
    for forbidden in ["TrojanUdpFlowKey", "TrojanUdpLeafKey", "fn from_flow_key("] {
        assert!(
            !managed.contains(forbidden)
                && !stream_manager.contains(forbidden)
                && !manager_stream.exists(),
            "Trojan UDP managed glue should not match or store protocol-private cache-key internals `{forbidden}`"
        );
    }
    assert!(
        snapshot.contains("resume: ManagedUdpFlowResume")
            && snapshot.contains("inner: Arc<dyn ManagedUdpFlowResumeObject>")
            && !snapshot.contains("Trojan(trojan::udp::TrojanUdpFlowResume)")
            && !snapshot.contains("password: String")
            && !snapshot.contains("client_fingerprint: Option<String>")
            && !snapshot.contains("relay_chain: bool"),
        "Trojan protocol UDP flow snapshot should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("ManagedExistingSend")
            && forward.contains("ManagedExistingSend::forwarded")
            && !forward.contains("existing.resume.password()")
            && !forward.contains("existing.resume.sni()")
            && !forward.contains("existing.resume.insecure()")
            && !forward.contains("existing.resume.client_fingerprint()")
            && !forward.contains("existing.resume.relay_chain()")
            && !forward.contains("password: &'a str")
            && !forward.contains("client_fingerprint: Option<&'a str>")
            && !forward.contains("relay_chain: bool"),
        "existing Trojan UDP flow forwarding should pass the opaque resume descriptor without unpacking auth, TLS, or relay state"
    );
    assert!(
        !start.contains("ManagedUdpFlowResume::Trojan")
            && start.contains("ManagedExistingSend::stream_packet")
            && start.contains("ManagedRelaySend::relay_stream")
            && !start.contains("resume.password()")
            && !start.contains("resume.sni()")
            && !start.contains("resume.insecure()")
            && !start.contains("resume.client_fingerprint()")
            && !start.contains("resume.relay_chain()"),
        "new Trojan UDP flow start should pass the opaque resume descriptor without unpacking auth, TLS, or relay state"
    );
    for forbidden in [
        "request.resume.password()",
        "request.resume.sni()",
        "request.resume.insecure()",
        "request.resume.client_fingerprint()",
        "request.resume.relay_chain()",
        ".peer_config()",
        "peer_config.",
        "peer_config:",
        "TrojanUdpPeerConfig",
        "TrojanKey::Leaf {",
        "password: String",
    ] {
        assert!(
            !managed.contains(forbidden)
                && !stream_manager.contains(forbidden)
                && !manager_stream.exists(),
            "Trojan UDP managed glue should use protocol-owned peer config/key instead of unpacking `{forbidden}`"
        );
    }
    let connector = read("src/adapters/trojan/udp/managed/connector.rs");
    assert!(
        !managed.contains("resume.flow_cache_key(")
            && !connector.contains("resume.flow_cache_key(")
            && connector.contains("trojan::udp::connector_flow_from_resume")
            && !connector.contains("resume.connector_flow(endpoint.server, endpoint.port, session_id)")
            && !connector.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !connector.contains("resume.flow_requirement().requires_relay_upstream()")
            && connector.contains("managed_stream_connector_flow_from_build")
            && !connector.contains("ManagedStreamConnectorFlow::new")
            && !connector.contains("flow.cache_key()")
            && !connector.contains("flow.requires_relay_upstream()")
            && !managed.contains("ManagedUdpConnectionCacheKey")
            && stream_manager.contains(".send_or_insert_key(")
            && stream_manager.contains(".insert_and_send_key(")
            && !stream_manager.contains("if let Some(entry) = self.upstreams.get(&cache_key)")
            && !stream_manager.contains("self.upstreams.insert(")
            && !stream_manager.contains("entry.spawn_response_bridge(")
            && !managed.contains("resume.cache_key(endpoint.server, endpoint.port, session_id)")
            && !managed.contains("peer.endpoint")
            && !managed.contains("TrojanUdpPeer")
            && !managed.contains("resume.flow_requires_relay_upstream()")
            && !connector.contains("resume.flow_requires_relay_upstream()")
            && !managed.contains("resume.tls_profile(")
            && !managed.contains("TrojanUdpTlsOptions")
            && !managed.contains("crate::outbound::trojan::open_udp_tls_stream")
            && connector.contains("open_udp_tls_stream")
            && !manager_stream.exists()
            && !managed.contains("trojan::udp::establish_udp_flow_with_resume")
            && connector.contains("trojan::udp::establish_udp_flow_with_resume")
            && !managed.contains("trojan::udp::TrojanUdpFlowIo")
            && !managed.contains(".establish_with_resume(")
            && protocol_outbound.contains("pub async fn establish_with_resume")
            && protocol_outbound.contains("pub async fn establish_udp_flow_with_resume")
            && !transport.contains("trojan::"),
        "Trojan UDP manager should consume protocol-owned cache key and TLS profile through neutral endpoints without putting protocol calls in zero-transport"
    );
}

#[test]
fn trojan_udp_packet_stream_tasks_live_outside_manager() {
    let managed = read("src/adapters/trojan/udp/managed.rs");
    let connector = read("src/adapters/trojan/udp/managed/connector.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let stream = manifest_dir().join("src/adapters/trojan/udp/manager/stream.rs");
    let transport =
        fs::read_to_string(repo_root().join("crates/transport/src/trojan_transport.rs"))
            .expect("read zero-transport trojan_transport source");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    let forbidden = "MeteredStream";
    assert!(
        !managed.contains(forbidden) && !stream_manager.contains(forbidden),
        "Trojan managed UDP glue should not own packet stream task detail `{forbidden}`"
    );
    for forbidden in [
        "UdpPacketStreamFraming",
        "write_udp_packet",
        "read_udp_packet",
        "establish_udp_packet_tunnel",
        "TrojanUdpFlowIo",
        ".establish_with_resume(",
        "trojan::udp::spawn_udp_flow",
        "TrojanUdpPacket {",
        "trojan::udp::TrojanUdpPacket",
    ] {
        assert!(
            !managed.contains(forbidden)
                && !connector.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "Trojan managed UDP glue should delegate Trojan packet framing to protocols/trojan helpers; found `{forbidden}`"
        );
    }
    for forbidden in ["TrojanUdpPacket {", "trojan::udp::TrojanUdpPacket"] {
        assert!(
            !managed.contains(forbidden)
                && !connector.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "Trojan managed UDP glue should not rebuild Trojan packet framing details; found `{forbidden}`"
        );
    }
    assert!(
        !stream.exists()
            && !managed.contains("trojan::write_udp_response")
            && !managed.contains("trojan::read_inbound_udp_packet"),
        "Trojan UDP managed glue should use flow-specific protocol helpers instead of generic UDP helpers"
    );
    let protocol_shared = fs::read_to_string(repo_root().join("protocols/trojan/src/shared.rs"))
        .expect("read trojan protocol shared source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/trojan/src/lib.rs"))
        .expect("read trojan protocol lib source");
    for private_helper in ["build_udp_packet", "read_udp_packet", "write_udp_packet"] {
        assert!(
            protocol_shared.contains(&format!("pub(crate) {} {private_helper}", if private_helper == "build_udp_packet" { "fn" } else { "async fn" }))
                && !protocol_lib.contains(private_helper),
            "Trojan low-level UDP stream helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
    assert!(
        !managed.contains("trojan::udp::establish_udp_flow_with_resume")
            && connector.contains("trojan::udp::establish_udp_flow_with_resume")
            && !managed.contains("trojan::udp::TrojanUdpFlowConnection")
            && connector.contains("trojan::udp::TrojanUdpFlowConnection")
            && !managed.contains("trojan::udp::TrojanUdpFlowSession")
            && !managed.contains("tokio::io::split")
            && !managed.contains("tokio::spawn")
            && !managed.contains(".write_flow_packet(")
            && !managed.contains(".write_packet(")
            && !managed.contains("&mut send_stream")
            && !managed.contains(".read_flow_packet(")
            && !managed.contains("&mut recv_stream")
            && !managed.contains("trojan::udp::TrojanUdpFlowSession::new")
            && !managed.contains("trojan::udp::TrojanUdpFlowSender")
            && !managed.contains("trojan::udp::TrojanUdpFlowResponses")
            && !managed.contains("broadcast::Sender<UdpFlowPacket>")
            && !managed.contains("mpsc::Sender<UdpFlowPacket>")
            && protocol_outbound.contains("pub fn spawn_udp_flow")
            && protocol_outbound.contains("pub async fn establish_udp_flow_with_resume")
            && protocol_outbound.contains("async fn read_udp_flow_packet")
            && !protocol_outbound.contains("pub async fn read_udp_flow_packet")
            && protocol_outbound.contains("async fn write_udp_flow_packet")
            && !protocol_outbound.contains("pub async fn write_udp_flow_packet")
            && protocol_outbound.contains("struct TrojanUdpFlowSender")
            && !protocol_outbound.contains("pub struct TrojanUdpFlowSender")
            && protocol_outbound.contains("pub struct TrojanUdpFlowConnection")
            && protocol_outbound.contains("pub struct TrojanUdpFlowSession")
            && protocol_outbound.contains("pub type TrojanUdpFlowResponseReceiver")
            && protocol_outbound.contains("type TrojanUdpFlowResponses")
            && !protocol_outbound.contains("pub type TrojanUdpFlowResponses")
            && protocol_outbound.contains("tokio::spawn")
            && protocol_outbound.contains("mpsc::channel::<UdpFlowPacket>")
            && protocol_outbound.contains("broadcast::channel::<UdpFlowPacket>")
            && !managed.contains(".write_stream_packet")
            && !managed.contains(".read_stream_packet")
            && !managed.contains(".read_packet(")
            && !managed.contains("trojan::udp_flow_packet")
            && !transport.contains("trojan::")
            && !managed.contains("packet.write_to")
            && !managed.contains("struct TrojanPacket"),
        "Trojan UDP packet stream tasks should live in protocols/trojan while proxy keeps handshake/cache bridge glue"
    );
}

#[test]
fn mieru_udp_managed_connector_is_thin_protocol_glue() {
    let managed = read("src/adapters/mieru/udp/managed.rs");
    let connector = read("src/adapters/mieru/udp/managed/connector.rs");
    let adapter = read("src/adapters/mieru/udp.rs");
    let adapter_flow = read("src/adapters/mieru/udp/flow.rs");
    let snapshot = read("src/runtime/udp_flow/managed/flow.rs");
    let forward = read("src/runtime/udp_flow/managed/stream.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let transport_manifest = fs::read_to_string(repo_root().join("crates/transport/Cargo.toml"))
        .expect("read zero-transport manifest");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/mieru/src/lib.rs"))
        .expect("read mieru protocol lib source");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/mieru/src/udp.rs"))
        .expect("read mieru protocol udp source");
    let connector_flow_impl = impl_block(&protocol_udp, "MieruUdpConnectorFlow");

    for removed in [
        "src/adapters/mieru/udp/manager.rs",
        "src/adapters/mieru/udp/manager/connect.rs",
        "src/adapters/mieru/udp/manager/establish.rs",
        "src/adapters/mieru/udp/manager/model.rs",
        "src/adapters/mieru/udp/manager/send.rs",
        "src/adapters/mieru/udp/manager/codec.rs",
        "src/adapters/mieru/udp/manager/stream.rs",
        "src/adapters/mieru/udp/manager/socket.rs",
        "src/adapters/mieru/udp/manager/bridge.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "Mieru UDP should use managed.rs plus generic stream manager instead of `{removed}`"
        );
    }

    for forbidden in [
        "UdpPacketFraming",
        "MieruUdpAssociatePacket",
        "MieruInboundUdpPacket",
        "fn encode_associate_packet",
        "fn decode_associate_packet",
        "socks5::build_udp_packet",
        "socks5::parse_udp_packet",
        "MieruUdpFlowKey",
        "MieruUdpLeafKey",
        "MieruUdpPeerConfig",
        "MieruUdpCacheKey",
        "request.resume.username()",
        "request.resume.password()",
        "request.resume.relay_chain()",
        ".peer_config()",
        "MieruKey::Leaf {",
        "username: String",
        "password: String",
        "ManagedUdpConnectionCacheKey",
        "if let Some(entry) = self.upstreams.get(&cache_key)",
        "self.upstreams.insert(",
        "entry.spawn_response_bridge(",
        "resume.cache_key(endpoint.server, endpoint.port, session_id)",
        "peer.endpoint",
        "UdpFlowContext",
        "UdpPacketRef",
    ] {
        assert!(
            !managed.contains(forbidden) && !connector.contains(forbidden),
            "Mieru UDP managed connector should not own protocol-private/cache/runtime orchestration detail `{forbidden}`"
        );
    }

    assert!(
        managed.contains("ManagedStreamFlowManager::new")
            && managed.contains("connector::MieruManagedStreamConnector")
            && !managed.contains("impl ManagedStreamFlowConnector<mieru::udp::MieruUdpFlowResume>")
            && connector.contains("impl ManagedStreamFlowConnector<mieru::udp::MieruUdpFlowResume>")
            && connector.contains("mieru::udp::connector_flow_from_resume")
            && !connector.contains("resume.connector_flow(endpoint.server, endpoint.port, session_id)")
            && !connector.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !connector.contains("resume.flow_requirement().requires_relay_upstream()")
            && connector.contains("managed_stream_connector_flow_from_build")
            && !connector.contains("ManagedStreamConnectorFlow::new")
            && !connector.contains("flow.cache_key()")
            && !connector.contains("flow.requires_relay_upstream()")
            && !connector.contains("resume.flow_cache_key(")
            && !connector.contains("resume.flow_requires_relay_upstream()")
            && connector.contains("mieru::udp::establish_udp_flow_with_resume(stream, &resume)")
            && connector.contains("managed_tuple_udp_connection")
            && connector.contains("impl ManagedTupleUdpSender for MieruManagedUdpSender")
            && stream_manager.contains("ManagedUdpConnectionCache")
            && stream_manager.contains(".send_or_insert_key(")
            && stream_manager.contains(".insert_and_send_key("),
        "Mieru managed.rs should adapt protocol flow establishment while generic stream_manager owns cache and send orchestration"
    );

    assert!(
        protocol_udp.contains("pub(crate) fn udp_flow_codec(")
            && protocol_udp.contains("impl DatagramCodec<Address> for MieruUdpFlowCodec")
            && !adapter.contains("mieru::udp_flow_codec")
            && !adapter.contains("MieruUdpFlowResume::new")
            && !adapter.contains("MieruUdpFlowConfig::new")
            && adapter_flow.contains("mieru::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("MieruUdpFlowConfig::new")
            && !adapter_flow.contains(".flow_resume(request.relay_chain)")
            && protocol_udp.contains("struct MieruUdpFlowResume")
            && protocol_udp.contains("pub fn udp_flow_resume_from_config(")
            && protocol_udp.contains("pub struct MieruUdpConnectorFlow")
            && !connector_flow_impl.contains("pub fn cache_key(&self)")
            && !connector_flow_impl.contains("pub fn requires_relay_upstream(&self)")
            && connector_flow_impl
                .contains("pub fn into_parts(self) -> (alloc::string::String, bool)")
            && protocol_udp.contains("pub fn connector_flow(")
            && !protocol_udp.contains("pub struct MieruUdpFlowSpec")
            && !protocol_udp.contains("pub struct MieruUdpFlowRequirement")
            && !protocol_udp.contains("pub fn flow_requirement(&self)")
            && protocol_udp.contains("pub fn flow_cache_key(&self")
            && protocol_udp.contains("pub fn flow_requires_relay_upstream(&self) -> bool")
            && !protocol_udp.contains("pub fn username(&self)")
            && !protocol_udp.contains("pub fn password(&self)"),
        "Mieru adapter should build and carry an opaque protocol-owned UDP flow resume descriptor"
    );
    for private_helper in [
        "wrap_udp_associate",
        "unwrap_udp_associate",
        "decode_inbound_udp_packet",
        "encode_udp_response",
        "decode_udp_flow_packet",
        "encode_udp_flow_packet",
        "udp_flow_codec",
    ] {
        assert!(
            protocol_udp.contains(&format!("pub(crate) fn {private_helper}("))
                && !protocol_lib.contains(private_helper),
            "Mieru UDP helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");
    assert!(
        !protocol_outbound.contains("pub fn udp_flow_packet")
            && !protocol_outbound.contains("fn udp_flow_packet")
            && !protocol_lib.contains("udp_flow_packet"),
        "Mieru UDP flow packet constructor helper should be removed from the public protocol surface"
    );

    for forbidden in [
        "MieruUdpFlowKey",
        "MieruUdpLeafKey",
        "MieruUdpPeerConfig",
        "MieruUdpCacheKey",
    ] {
        assert!(
            !protocol_lib.contains(forbidden),
            "protocols/mieru lib root should not re-export UDP cache-key internals `{forbidden}`"
        );
    }

    assert!(
        snapshot.contains("resume: ManagedUdpFlowResume")
            && snapshot.contains("inner: Arc<dyn ManagedUdpFlowResumeObject>")
            && !snapshot.contains("Mieru(mieru::udp::MieruUdpFlowResume)")
            && !snapshot.contains("username: String")
            && !snapshot.contains("relay_chain: bool")
            && forward.contains("ManagedExistingSend")
            && forward.contains("ManagedExistingSend::forwarded")
            && !forward.contains("existing.resume.username()")
            && !forward.contains("existing.resume.password()")
            && !forward.contains("existing.resume.relay_chain()")
            && !forward.contains("existing.resume.codec()"),
        "Mieru managed UDP forwarding and snapshots should carry only the unified opaque resume wrapper"
    );

    assert!(
        protocol_outbound.contains("struct MieruUdpFlowIo")
            && protocol_outbound.contains("struct MieruUdpFlowPacket")
            && protocol_outbound.contains("pub struct MieruUdpFlowConnection")
            && protocol_outbound.contains("pub type MieruUdpFlowResponseReceiver")
            && protocol_outbound.contains("pub async fn establish_with_resume")
            && protocol_outbound.contains("encode_udp_flow_packet")
            && protocol_outbound.contains("decode_udp_flow_packet")
            && protocol_outbound.contains("tokio::spawn")
            && !protocol_outbound.contains("pub async fn open_udp_flow")
            && !repo_root()
                .join("crates/transport/src/mieru_transport.rs")
                .exists()
            && !transport_manifest.contains("dep:mieru")
            && !transport_manifest.contains("mieru/crypto"),
        "Mieru UDP associate, encryption, packet codec, and stream pump should stay protocol-owned"
    );
}

#[test]
fn mieru_udp_response_bridge_uses_generic_managed_tuple_connection() {
    let managed = read("src/adapters/mieru/udp/managed.rs");
    let connector = read("src/adapters/mieru/udp/managed/connector.rs");
    let connection = read("src/runtime/udp_flow/managed/connection.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");

    for forbidden in [
        "type RecvItem",
        "broadcast::channel",
        "recv_tx.subscribe",
        "fn spawn_bridge",
        "spawn_tuple_response_bridge",
        ".spawn_response_bridge(",
        "self.upstreams.insert(",
    ] {
        assert!(
            !managed.contains(forbidden) && !connector.contains(forbidden),
            "Mieru managed.rs should not own response bridge or cache details `{forbidden}`"
        );
    }
    assert!(
        !managed.contains("managed_tuple_udp_connection")
            && connector.contains("managed_tuple_udp_connection")
            && connector.contains("fn subscribe_responses")
            && connector.contains("mieru upstream closed")
            && connection.contains("pub(crate) fn managed_tuple_udp_connection")
            && connection.contains("pub(crate) fn spawn_tuple_response_bridge")
            && connection.contains("broadcast::Receiver<(Address, u16, Vec<u8>)>")
            && stream_manager.contains(".insert_and_send_key("),
        "Mieru UDP response bridge should hang off the neutral managed tuple connection bridge"
    );
}

#[test]
fn trojan_udp_managed_connector_is_thin_protocol_glue() {
    let managed = read("src/adapters/trojan/udp/managed.rs");
    let connector = read("src/adapters/trojan/udp/managed/connector.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let connection = read("src/runtime/udp_flow/managed/connection.rs");
    let transport =
        fs::read_to_string(repo_root().join("crates/transport/src/trojan_transport.rs"))
            .expect("read zero-transport trojan_transport source");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    for removed in [
        "src/adapters/trojan/udp/manager.rs",
        "src/adapters/trojan/udp/manager/connect.rs",
        "src/adapters/trojan/udp/manager/establish.rs",
        "src/adapters/trojan/udp/manager/model.rs",
        "src/adapters/trojan/udp/manager/send.rs",
        "src/adapters/trojan/udp/manager/stream.rs",
        "src/adapters/trojan/udp/manager/socket.rs",
        "src/adapters/trojan/udp/manager/bridge.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "Trojan UDP should use managed.rs plus generic stream manager instead of `{removed}`"
        );
    }

    for forbidden in [
        "TrojanSendExisting",
        "TrojanRelaySend",
        "TrojanRelayExisting",
        "UdpFlowContext",
        "UdpPacketRef",
        "ManagedUdpConnectionCacheKey",
        "if let Some(entry) = self.upstreams.get(&cache_key)",
        "self.upstreams.insert(",
        "entry.spawn_response_bridge(",
        "TrojanUdpPacket {",
        "trojan::udp::TrojanUdpPacket",
        "TrojanUdpFlowIo",
        "trojan::udp::spawn_udp_flow",
        "TrojanUdpFlowSession::new",
        "mpsc::Sender<UdpFlowPacket>",
        "broadcast::Sender<UdpFlowPacket>",
        "trojan::udp_flow_packet",
        "resume.cache_key(endpoint.server, endpoint.port, session_id)",
        "resume.tls_profile(",
    ] {
        assert!(
            !managed.contains(forbidden) && !connector.contains(forbidden),
            "Trojan managed.rs should not own protocol-private/cache/runtime orchestration detail `{forbidden}`"
        );
    }
    assert!(
        !managed.contains("TrojanUdpTlsOptions") && connector.contains("TrojanUdpTlsOptions"),
        "Trojan UDP TLS transport options should live in the connector glue, not the managed root"
    );

    assert!(
        managed.contains("ManagedStreamFlowManager::new")
            && managed.contains("connector::TrojanManagedStreamConnector")
            && !managed.contains("impl ManagedStreamFlowConnector<trojan::udp::TrojanUdpFlowResume>")
            && connector.contains("impl ManagedStreamFlowConnector<trojan::udp::TrojanUdpFlowResume>")
            && connector.contains("trojan::udp::connector_flow_from_resume")
            && !connector.contains("resume.connector_flow(endpoint.server, endpoint.port, session_id)")
            && !connector.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !connector.contains("resume.flow_requirement().requires_relay_upstream()")
            && connector.contains("managed_stream_connector_flow_from_build")
            && !connector.contains("ManagedStreamConnectorFlow::new")
            && !connector.contains("flow.cache_key()")
            && !connector.contains("flow.requires_relay_upstream()")
            && !connector.contains("resume.flow_cache_key(")
            && !connector.contains("resume.flow_requires_relay_upstream()")
            && connector.contains("open_udp_tls_stream")
            && connector.contains("open_udp_tls_relay_stream")
            && connector.contains("trojan::udp::establish_udp_flow_with_resume")
            && connector.contains("managed_packet_udp_connection")
            && connector.contains("impl ManagedPacketUdpSender for TrojanManagedUdpSender")
            && stream_manager.contains("ManagedUdpConnectionCache")
            && stream_manager.contains(".send_or_insert_key(")
            && stream_manager.contains(".insert_and_send_key(")
            && connection.contains("pub(crate) fn managed_packet_udp_connection")
            && connection.contains("pub(crate) fn spawn_response_bridge<T, F>"),
        "Trojan managed.rs should adapt TLS stream and protocol flow establishment while generic stream_manager owns cache/send orchestration"
    );

    assert!(
        !protocol_outbound.contains("pub fn udp_flow_packet")
            && !protocol_outbound.contains("fn udp_flow_packet")
            && protocol_outbound.contains("pub async fn read_flow_packet")
            && protocol_outbound.contains("pub async fn write_flow_packet")
            && protocol_outbound.contains("pub fn spawn_udp_flow")
            && protocol_outbound.contains("pub async fn establish_udp_flow_with_resume")
            && protocol_outbound.contains("pub struct TrojanUdpFlowConnection")
            && protocol_outbound.contains("pub type TrojanUdpFlowResponseReceiver")
            && !protocol_outbound.contains("pub async fn open_udp_flow")
            && !transport.contains("mpsc::Sender<UdpFlowPacket>")
            && !transport.contains("trojan::udp_flow_packet")
            && !transport.contains("trojan::udp::TrojanUdpFlowIo"),
        "Trojan UDP packet conversion and flow channels should stay protocol-owned and out of zero-transport"
    );
}

#[test]
fn mieru_udp_packet_stream_tasks_live_outside_manager() {
    let managed = read("src/adapters/mieru/udp/managed.rs");
    let connector = read("src/adapters/mieru/udp/managed/connector.rs");
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let stream = manifest_dir().join("src/adapters/mieru/udp/manager/stream.rs");
    let socket = manifest_dir().join("src/adapters/mieru/udp/manager/socket.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/Cargo.toml"))
        .expect("read zero-transport manifest");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");

    for forbidden in [
        "encrypt_client_data(&payload)",
        "decrypt_server_data_with_consumed(&raw)",
        "decode_udp_flow_packet",
        "encode_udp_flow_packet",
        "parse_udp_packet",
    ] {
        assert!(
            !managed.contains(forbidden)
                && !connector.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "Mieru UDP proxy glue should delegate protocol packet details to protocols/mieru; found `{forbidden}`"
        );
    }
    for forbidden in [
        "write_flow_packet",
        "read_flow_packets",
        "decode_encrypted_response",
        "struct ReadOnlySocket",
        "struct WriteOnlySocket",
    ] {
        assert!(
            !managed.contains(forbidden)
                && !connector.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "Mieru UDP proxy manager should not own protocol flow runtime detail `{forbidden}`"
        );
    }
    assert!(
        !stream.exists() && !socket.exists(),
        "Mieru UDP stream task should live in protocols/mieru without proxy stream/socket wrappers"
    );
    assert!(
        !repo_root()
            .join("crates/transport/src/mieru_transport.rs")
            .exists()
            && !transport.contains("dep:mieru")
            && !transport.contains("mieru/crypto")
            && !managed.contains("MieruFlowSender")
            && !managed.contains("MieruEntry")
            && !managed.contains(".sender")
            && !managed.contains(".recv_tx")
            && stream_manager.contains(".insert_and_send_key(")
            && !managed.contains(".send(packet_ref.target, packet_ref.port, packet_ref.payload)")
            && !managed.contains("UdpFlowPacket")
            && !protocol_outbound.contains("pub async fn open_udp_flow")
            && protocol_outbound.contains("pub struct MieruUdpFlowHandle")
            && protocol_outbound.contains("struct MieruUdpFlowSender")
            && !protocol_outbound.contains("pub struct MieruUdpFlowSender")
            && protocol_outbound.contains("pub struct MieruUdpFlowConnection")
            && protocol_outbound.contains("pub struct MieruUdpFlowSession")
            && protocol_outbound.contains("pub type MieruUdpFlowResponse")
            && protocol_outbound.contains("pub type MieruUdpFlowResponseReceiver")
            && !protocol_outbound.contains("pub type MieruUdpFlowResponses")
            && !protocol_outbound.contains("mpsc::channel::<MieruUdpFlowPacket>")
            && protocol_outbound.contains("broadcast::channel::<MieruUdpFlowResponse>")
            && protocol_outbound.contains("mpsc::channel::<zero_core::UdpFlowPacket>")
            && protocol_outbound.contains("tokio::spawn")
            && protocol_outbound.contains("tokio::select!")
            && !managed.contains("mpsc::channel")
            && !managed.contains("tokio::sync::broadcast::channel")
            && !managed.contains("tokio::spawn")
            && !managed.contains("mieru::udp::establish_udp_flow_with_resume")
            && connector.contains("mieru::udp::establish_udp_flow_with_resume")
            && !managed.contains("mieru::udp::MieruUdpFlowIo::establish_with_resume")
            && !managed.contains("mieru::udp::spawn_udp_flow")
            && !managed.contains("mieru::udp::MieruUdpFlowSession::new")
            && !managed.contains("flow_io.write_flow_packet")
            && !managed.contains("flow_io.decode_encrypted_response")
            && !managed.contains("flow_io.read_flow_packets")
            && !managed.contains("packet.encode_with(&mut flow_io)")
            && !managed.contains("flow_io.push_encrypted_response")
            && !managed.contains("flow_io.next_packet()")
            && protocol_outbound.contains("pub async fn write_packet")
            && protocol_outbound.contains("pub async fn read_packets")
            && protocol_outbound.contains("pub async fn write_flow_packet")
            && protocol_outbound.contains("pub async fn read_flow_packets")
            && protocol_outbound.contains("pub async fn establish_udp_flow_with_resume")
            && protocol_outbound.contains("pub fn decode_encrypted_response"),
        "Mieru UDP stream flow task should stay out of zero-transport and live in protocols/mieru while proxy keeps handshake/cache bridge glue"
    );
}

#[test]
fn h2_udp_datagram_codec_lives_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let connector = read("src/adapters/hysteria2/connector.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/hysteria2_quic.rs"))
        .expect("read zero-transport hysteria2_quic source");
    let adapter = read("src/adapters/hysteria2/udp.rs");
    let snapshot = read("src/runtime/udp_flow/managed/flow.rs");
    let managed_cache = read("src/runtime/udp_flow/managed/cache.rs");
    let forward = read("src/runtime/udp_flow/managed/datagram.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let connector_flow_impl = impl_block(&protocol_udp, "Hysteria2UdpConnectorFlow");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/hysteria2/src/lib.rs"))
        .expect("read hysteria2 protocol lib source");
    let adapter_flow = read("src/adapters/hysteria2/udp/flow.rs");
    let adapter_packet_path = read("src/adapters/hysteria2/udp/packet_path.rs");
    let profile_connector_uses = connector
        .matches("Hysteria2Connector::from_udp_profile")
        .count();

    for forbidden in [
        "UdpDatagramFraming",
        "Hysteria2UdpPacketTarget",
        "Hysteria2UdpPacket",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should not own datagram codec details; found `{forbidden}`"
        );
    }
    for removed in [
        "src/adapters/hysteria2/udp/manager.rs",
        "src/adapters/hysteria2/udp/manager/model.rs",
        "src/adapters/hysteria2/udp/manager/send.rs",
        "src/adapters/hysteria2/udp/manager/establish.rs",
        "src/adapters/hysteria2/udp/manager/codec.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "Hysteria2 UDP should not keep proxy-owned manager file `{removed}`"
        );
    }
    assert!(
        !adapter.contains("hysteria2::udp_flow_codec")
            && !adapter.contains("Hysteria2UdpFlowConfig")
            && !adapter.contains("Hysteria2UdpFlowConfig::new")
            && !adapter.contains("Hysteria2UdpFlowConfig {")
            && adapter_flow.contains("hysteria2::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("Hysteria2UdpFlowConfig::new")
            && adapter_packet_path
                .contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("Hysteria2UdpFlowConfig::new")
            && protocol_udp.contains("pub(crate) fn udp_flow_codec(")
            && protocol_udp.contains("struct Hysteria2UdpFlowConfig")
            && protocol_udp.contains("pub fn new(")
            && protocol_udp.contains("impl DatagramCodec<Address> for Hysteria2DatagramCodec")
            && !protocol_udp.contains("pub fn udp_flow_packet")
            && !protocol_udp.contains("fn udp_flow_packet")
            && protocol_udp.contains("pub fn encode_packet(")
            && protocol_udp.contains("pub fn encode_flow_packet(")
            && protocol_udp.contains("struct Hysteria2UdpFlowIo")
            && protocol_udp.contains("pub fn flow_io(&self)")
            && protocol_udp.contains("pub fn decode_packet(&self"),
        "Hysteria2 adapter and UDP manager should consume protocol-owned UDP flow packet helpers"
    );
    for private_helper in [
        "build_udp_datagram",
        "parse_udp_datagram",
        "encode_udp_flow_packet",
        "decode_udp_flow_packet",
        "udp_flow_codec",
    ] {
        assert!(
            protocol_udp.contains(&format!("pub(crate) fn {private_helper}("))
                && !protocol_lib.contains(private_helper),
            "Hysteria2 UDP helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
    assert!(
        !protocol_udp.contains("fn udp_flow_packet") && !protocol_lib.contains("udp_flow_packet"),
        "Hysteria2 UDP flow packet constructor helper should be removed from the public protocol surface"
    );
    assert!(
        !managed.contains("struct H2Entry")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowSender")
            && !managed.contains("hysteria2::udp_flow_packet")
            && !managed.contains("Hysteria2UdpFlowPacket::from_parts")
            && !managed.contains("use zero_core::UdpFlowPacket")
            && !managed.contains("UdpFlowPacket::from_parts")
            && !managed.contains("zero_core::UdpFlowPacket::from_parts")
            && !managed.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !managed.contains("hysteria2::udp::Hysteria2InitialUdpFlowPacket::from_parts")
            && generic_manager.contains(".send_or_insert_pre_sent_key(")
            && !managed.contains(".send_or_insert(")
            && !managed.contains(".send(packet_ref.target, packet_ref.port, packet_ref.payload)")
            && managed_cache.contains(".send(packet.target, packet.port, packet.payload)")
            && !managed.contains("mpsc::Sender<UdpFlowPacket>")
            && !managed.contains("mpsc::channel::<UdpFlowPacket>")
            && !managed.contains("flow_io.encode_packet")
            && !managed.contains("packet.encode_with(&resume)")
            && !managed.contains("flow_io.decode_packet(&data)")
            && managed.contains("connector::establish_udp_flow_session")
            && !managed.contains("hysteria2::udp::spawn_udp_flow")
            && protocol_udp.contains("struct Hysteria2UdpFlowSender")
            && !protocol_udp.contains("pub struct Hysteria2UdpFlowSender")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowConnection")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowSession")
            && protocol_udp.contains("pub fn subscribe_responses(&self)")
            && protocol_udp.contains("pub struct Hysteria2InitialUdpFlowPacket")
            && protocol_udp.contains("pub fn start_udp_flow_with_initial_packet")
            && protocol_udp.contains("mpsc::channel::<UdpFlowPacket>")
            && protocol_udp.contains("Hysteria2InitialUdpFlowPacket")
            && protocol_udp.contains("flow_io.encode_packet")
            && protocol_udp.contains("flow_io.decode_packet(&data)")
            && !managed.contains("resume.encode_flow_packet")
            && !managed.contains("resume.decode_flow_packet")
            && !managed.contains("establish_hysteria2_udp_flow_stream")
            && !transport.contains("mpsc::Sender<UdpFlowPacket>")
            && !transport.contains("hysteria2::udp_flow_packet")
            && !transport.contains("encode_hysteria2_udp_flow_packet")
            && !transport.contains("resume.decode_flow_packet(&data)")
            && !managed.contains(".encode_packet(")
            && !managed.contains("mpsc::Sender<Vec<u8>>"),
        "Hysteria2 UDP managed glue should store protocol-owned flow sessions while protocols/hysteria2 owns packet encode/decode and flow pump"
    );
    assert!(
        !adapter.contains("Hysteria2UdpFlowResume::new")
            && !adapter.contains(".flow_resume()")
            && !adapter.contains(".packet_path_spec()")
            && adapter_flow.contains("hysteria2::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains(".flow_resume()")
            && adapter_packet_path
                .contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains(".packet_path_spec()")
            && adapter_packet_path.contains("udp_packet_path_carrier_build_from_config")
            && adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("spec.carrier()")
            && !adapter_packet_path.contains("spec.cache_key()")
            && !adapter_packet_path.contains("spec.carrier_cache_key()")
            && !adapter_packet_path.contains("spec.codec()")
            && !adapter_packet_path.contains("build.server()")
            && !adapter_packet_path.contains("build.port()")
            && !adapter_packet_path.contains("build.connector_profile()")
            && !adapter_packet_path.contains("build.codec()")
            && adapter_packet_path.contains("connector::open_udp_packet_path_build")
            && !adapter_packet_path.contains(".packet_path_cache_key()")
            && !adapter_packet_path.contains(".packet_path_codec()")
            && protocol_udp.contains("struct Hysteria2UdpFlowResume")
            && protocol_udp.contains("pub struct Hysteria2UdpConnectorFlow")
            && !connector_flow_impl.contains("pub fn cache_key(&self)")
            && !connector_flow_impl.contains("pub fn connector_profile(&self)")
            && connector_flow_impl.contains("pub fn into_cache_key(self) -> String")
            && connector_flow_impl.contains("pub fn into_connection_parts(self)")
            && !protocol_udp.contains("pub struct Hysteria2UdpFlowSpec")
            && protocol_udp.contains("pub fn connector_profile(&self)")
            && protocol_udp.contains("pub struct Hysteria2UdpPacketPathSpec")
            && protocol_udp.contains("struct Hysteria2UdpFlowConfig")
            && protocol_udp.contains("pub fn new(")
            && protocol_udp.contains("pub fn connector_flow(")
            && protocol_udp.contains("pub fn flow_resume(&self)")
            && protocol_udp.contains("pub fn udp_flow_resume_from_config(")
            && protocol_udp.contains("pub fn packet_path_spec(&self)")
            && protocol_udp.contains("pub fn udp_packet_path_spec_from_config(")
            && !protocol_udp.contains("pub struct Hysteria2UdpPacketPathCarrier {")
            && !protocol_udp.contains("pub fn carrier_cache_key(&self)")
            && !protocol_udp.contains("pub fn carrier(&self)")
            && !protocol_udp.contains("pub fn packet_path_cache_key(&self)")
            && !protocol_udp.contains("pub fn packet_path_codec(&self)")
            && protocol_udp.contains("fn peer_config(&self)")
            && !protocol_udp.contains("pub fn peer_config(&self)")
            && protocol_udp.contains("fn flow_key(&self")
            && !protocol_udp.contains("pub fn flow_key(&self")
            && protocol_udp.contains("fn cache_key(&self, server: &str, port: u16)")
            && !protocol_udp.contains("pub fn cache_key(&self, server: &str, port: u16)")
            && protocol_udp.contains("pub fn flow_cache_key(&self")
            && protocol_udp.contains("enum Hysteria2UdpFlowKey")
            && !protocol_udp.contains("pub enum Hysteria2UdpFlowKey")
            && protocol_udp.contains("struct Hysteria2UdpCacheKey")
            && !protocol_udp.contains("pub struct Hysteria2UdpCacheKey")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowStore")
            && protocol_udp.contains("struct Hysteria2UdpPeerConfig")
            && !protocol_udp.contains("pub struct Hysteria2UdpPeerConfig")
            && protocol_udp.contains("struct Hysteria2UdpConnectorProfile")
            && protocol_udp.contains("pub fn connector_profile(&self)")
            && protocol_udp.contains("pub async fn authenticate_connection")
            && protocol_udp.contains("struct Hysteria2UdpLeafKey")
            && !protocol_udp.contains("pub struct Hysteria2UdpLeafKey")
            && protocol_udp.contains("pub fn codec(&self)")
            && protocol_udp.contains("pub fn client_fingerprint(&self) -> Option<&str>"),
        "Hysteria2 adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    for forbidden in [
        "Hysteria2UdpFlowKey",
        "Hysteria2UdpLeafKey",
        "Hysteria2UdpPeerConfig",
        "Hysteria2UdpCacheKey",
    ] {
        assert!(
            !protocol_lib.contains(forbidden),
            "protocols/hysteria2 lib root should not re-export UDP cache-key internals `{forbidden}`"
        );
    }
    for forbidden in [
        "Hysteria2UdpFlowKey",
        "Hysteria2UdpLeafKey",
        "fn from_flow_key(",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should not match or store protocol-private cache-key internals `{forbidden}`"
        );
    }
    let resume_model = snapshot
        .split("pub(crate) struct ManagedUdpFlowResume")
        .nth(1)
        .expect("ManagedUdpFlowResume struct should exist")
        .split("impl ManagedUdpFlowResume")
        .next()
        .expect("ManagedUdpFlowResume impl should follow ManagedUdpFlowResume struct");
    assert!(
        snapshot.contains("inner: Arc<dyn ManagedUdpFlowResumeObject>")
            && !snapshot.contains("Hysteria2(hysteria2::udp::Hysteria2UdpFlowResume)")
            && !resume_model.contains("password: String")
            && !resume_model.contains("client_fingerprint: Option<String>"),
        "Hysteria2 protocol UDP flow state should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("ManagedExistingSend")
            && forward.contains("ManagedExistingSend::forwarded")
            && !forward.contains("existing.resume.password()")
            && !forward.contains("existing.resume.client_fingerprint()")
            && !forward.contains("existing.resume.codec()")
            && !forward.contains("hysteria2::udp_flow_codec")
            && !forward.contains("password: &'a str")
            && !forward.contains("client_fingerprint: Option<&'a str>"),
        "existing Hysteria2 UDP flow forwarding should pass the opaque resume descriptor without unpacking auth or codec state"
    );
    for forbidden in [
        "request.resume.password()",
        "request.resume.client_fingerprint()",
        ".peer_config()",
        "peer_config.",
        "peer_config:",
        "Hysteria2UdpPeerConfig",
        "password: String",
        "client_fingerprint: Option<String>",
        "peer.password",
        "peer.client_fingerprint",
    ] {
        assert!(
            !managed.contains(forbidden) && !generic_manager.contains(forbidden),
            "Hysteria2 UDP managed glue should use protocol-owned peer config/key instead of unpacking `{forbidden}`"
        );
    }
    assert!(
        managed.contains("managed_datagram_connector_flow_from_build")
            && !managed.contains("ManagedDatagramConnectorFlow::new")
            && !managed.contains("flow.cache_key()")
            && managed.contains("hysteria2::udp::connector_flow_from_resume")
            && !managed.contains("resume.connector_flow(endpoint.server, endpoint.port)")
            && !managed.contains("resume.flow_cache_key(")
            && !managed.contains("resume.flow(endpoint.server, endpoint.port)")
            && generic_manager.contains("self.upstreams")
            && generic_manager.contains("ManagedDatagramConnectorFlow")
            && !generic_manager.contains("fn flow_cache_key")
            && generic_manager.contains(".send_or_insert_pre_sent_key(")
            && !managed.contains(".send_or_insert(")
            && !managed.contains("self.upstreams.get(&cache_key)")
            && managed_cache.contains("self.entries.get(&key)")
            && !managed.contains("resume.cache_key(endpoint.server, endpoint.port)")
            && !managed.contains("peer.endpoint")
            && !managed.contains("H2UdpPeer")
            && managed.contains("connector::establish_udp_flow_session")
            && !managed.contains("Hysteria2Connector::from_udp_profile")
            && !managed.contains("resume.connector_profile()")
            && !managed.contains("connect_raw_with_udp_profile")
            && connector.contains("hysteria2::udp::connector_flow_from_resume")
            && !connector.contains("resume.connector_flow(endpoint.server, endpoint.port)")
            && connector.contains(".into_connection_parts()")
            && connector.contains(".into_profile()")
            && !connector.contains(".connector_profile()")
            && !connector.contains("resume.connector_profile()")
            && connector.contains("async fn open_udp_profile_connection")
            && profile_connector_uses == 1
            && !connector.contains("pub(crate) async fn open_udp_packet_path_connection")
            && connector.contains("connect_raw_with_udp_profile")
            && !connector.contains("profile.password()")
            && !transport.contains("request.resume.connector_profile()"),
        "Hysteria2 UDP managed glue should consume protocol-owned opaque cache keys through neutral endpoints and keep UDP profile/connection setup in the adapter connector"
    );
}

#[test]
fn h2_packet_path_carrier_uses_protocol_built_codec() {
    let adapter = read("src/adapters/hysteria2/udp.rs");
    let adapter_packet_path = read("src/adapters/hysteria2/udp/packet_path.rs");
    let carrier = read("src/runtime/udp_flow/packet_path_chain/carriers/quic_datagram_carrier.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/udp_packet_path.rs"))
        .expect("read zero-transport udp packet path source");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/hysteria2/src/lib.rs"))
        .expect("read hysteria2 protocol lib source");
    let connector = read("src/adapters/hysteria2/connector.rs");
    let profile_connector_uses = connector
        .matches("Hysteria2Connector::from_udp_profile")
        .count();

    assert!(
        !adapter.contains("hysteria2::udp_flow_codec")
            && !adapter.contains("hysteria2::udp_cache_key")
            && !adapter.contains("Hysteria2UdpFlowConfig")
            && adapter_packet_path.contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("Hysteria2UdpFlowConfig"),
        "Hysteria2 packet-path adapter submodule should request protocol-built packet-path cache identity and codec through a protocol config helper"
    );
    assert!(
        protocol_udp.contains("pub(crate) fn udp_flow_codec(")
            && protocol_udp.contains("struct Hysteria2UdpFlowConfig")
            && protocol_udp.contains("impl DatagramCodec<Address> for Hysteria2DatagramCodec"),
        "protocols/hysteria2 should own Hysteria2 UDP flow codec construction"
    );
    for private_helper in [
        "build_udp_datagram",
        "parse_udp_datagram",
        "encode_udp_flow_packet",
        "decode_udp_flow_packet",
        "udp_flow_codec",
    ] {
        assert!(
            !protocol_lib.contains(private_helper),
            "protocols/hysteria2 lib root should not re-export raw UDP helper `{private_helper}`"
        );
    }
    for forbidden in [
        "hysteria2::build_udp_datagram",
        "hysteria2::parse_udp_datagram",
        "hysteria2::encode_udp_flow_packet",
        "hysteria2::decode_udp_flow_packet",
        "Hysteria2UdpPacketTarget",
        "Hysteria2Connector",
        "connect_raw",
        "client_fingerprint",
        "password: &str",
    ] {
        assert!(
            !carrier.contains(forbidden),
            "QUIC datagram packet-path carrier should consume adapter-provided connection/codec objects instead of naming protocol details; found `{forbidden}`"
        );
    }
    assert!(
        carrier.contains("QuicDatagramPacketPath::new")
            && carrier.contains("PacketPathCarrierAdapter")
            && transport.contains("Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>")
            && transport.contains("conn: Arc<quinn::Connection>")
            && !adapter.contains("outbound::hysteria2::open_udp_packet_path_connection")
            && !adapter_packet_path.contains("outbound::hysteria2::open_udp_packet_path_connection")
            && adapter_packet_path.contains("connector::open_udp_packet_path_build")
            && !adapter_packet_path.contains("build.server()")
            && !adapter_packet_path.contains("build.port()")
            && !adapter_packet_path.contains("build.connector_profile()")
            && !adapter_packet_path.contains("build.codec()")
            && connector.contains(".into_shared_codec_parts()")
            && !connector.contains("Arc::new(codec)")
            && protocol_udp.contains("pub fn into_shared_codec_parts")
            && protocol_udp.contains("Arc::new(codec)")
            && connector.contains("async fn open_udp_profile_connection")
            && profile_connector_uses == 1
            && !connector.contains("pub(crate) async fn open_udp_packet_path_connection")
            && !adapter.contains("Hysteria2Connector")
            && !adapter.contains("connect_raw")
            && !adapter_packet_path.contains("Hysteria2Connector")
            && !adapter_packet_path.contains("connect_raw"),
        "Hysteria2 packet-path adapter submodule should request protocol-specific QUIC connection setup from the adapter connector while zero-transport owns connection lifecycle and codec use"
    );
}

#[test]
fn h2_udp_response_bridge_lives_outside_manager() {
    let managed_adapter = read("src/adapters/hysteria2/udp/managed.rs");
    let managed = read("src/runtime/udp_flow/managed/connection.rs");
    let bridge = manifest_dir().join("src/adapters/hysteria2/udp/manager/bridge.rs");

    for forbidden in [
        "type RecvItem",
        "broadcast::channel",
        "recv_tx.subscribe",
        "h2 upstream closed",
    ] {
        assert!(
            !managed_adapter.contains(forbidden) || forbidden == "h2 upstream closed",
            "Hysteria2 UDP managed adapter should not own response bridge details; found `{forbidden}`"
        );
    }
    assert!(
        !bridge.exists()
            && !managed_adapter
                .contains("impl ManagedUdpConnection for hysteria2::udp::Hysteria2UdpFlowConnection")
            && managed_adapter.contains("managed_tuple_udp_connection")
            && !managed_adapter.contains("spawn_tuple_response_bridge")
            && managed.contains("pub(crate) fn managed_tuple_udp_connection")
            && managed.contains("pub(crate) fn spawn_tuple_response_bridge")
            && managed.contains("broadcast::Receiver<(Address, u16, Vec<u8>)>")
            && managed.contains("closed_message"),
        "Hysteria2 UDP response bridge should use generic managed tuple connection glue, not h2_manager/bridge.rs"
    );
}

#[test]
fn h2_udp_packet_stream_tasks_live_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let stream_path = manifest_dir().join("src/adapters/hysteria2/udp/manager/stream.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/hysteria2_quic.rs"))
        .expect("read zero-transport hysteria2_quic source");
    let connector = read("src/adapters/hysteria2/connector.rs");

    for forbidden in [
        "Hysteria2Connector",
        "connect_raw",
        "send_datagram",
        "read_datagram",
        "tokio::spawn",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should keep QUIC packet stream task details in protocols/hysteria2; found `{forbidden}`"
        );
    }
    assert!(
        !stream_path.exists(),
        "Hysteria2 UDP packet stream glue should not need a dedicated h2_manager/stream.rs wrapper"
    );
    for forbidden in [
        "establish_hysteria2_udp_flow_stream",
        "Hysteria2UdpFlowStreamRequest",
        "hysteria2::udp_flow_packet",
        "packet.encode_with(&resume)",
        "resume.encode_flow_packet",
        "resume.decode_flow_packet",
        "resume.decode_packet",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should delegate packet format helpers; found `{forbidden}`"
        );
    }
    assert!(
        managed.contains("connector::establish_udp_flow_session")
            && !managed.contains("Hysteria2Connector::from_udp_profile")
            && !managed.contains("connect_raw_with_udp_profile")
            && !managed.contains("hysteria2::udp::start_udp_flow_with_initial_packet")
            && !managed.contains("hysteria2::udp::spawn_udp_flow")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowSession::new")
            && !managed.contains("send_datagram")
            && !managed.contains("read_datagram")
            && !managed.contains("tokio::spawn")
            && !managed.contains("mpsc::channel::<UdpFlowPacket>")
            && !managed.contains("tokio::sync::broadcast::channel")
            && !managed.contains("flow_io.encode_packet")
            && !managed.contains("flow_io.decode_packet(&data)")
            && !protocol_udp.contains("pub fn open_udp_flow")
            && protocol_udp.contains("pub async fn authenticate_connection")
            && protocol_udp.contains("struct Hysteria2UdpFlowSender")
            && !protocol_udp.contains("pub struct Hysteria2UdpFlowSender")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowConnection")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowHandle")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowSession")
            && protocol_udp.contains("pub fn start_udp_flow_with_initial_packet")
            && protocol_udp.contains("broadcast::channel::<Hysteria2UdpFlowResponse>")
            && protocol_udp.contains("mpsc::channel::<UdpFlowPacket>")
            && protocol_udp.contains("tokio::spawn")
            && protocol_udp.contains("send_datagram")
            && protocol_udp.contains("read_datagram")
            && connector.contains("hysteria2::udp::start_udp_flow_with_initial_packet")
            && connector.contains("Hysteria2Connector::from_udp_profile")
            && connector.contains("connect_raw_with_udp_profile")
            && !transport.contains("pub async fn establish_hysteria2_udp_flow_stream")
            && !transport.contains("Hysteria2UdpFlowStreamRequest")
            && !transport.contains("hysteria2::udp_flow_packet")
            && !transport.contains("resume.encode_flow_packet")
            && !transport.contains("resume.decode_flow_packet"),
        "Hysteria2 UDP flow tasks should stay out of zero-transport and live in protocols/hysteria2 while the adapter connector owns QUIC connect/cache bridge glue"
    );
}

#[test]
fn h2_transport_delegates_protocol_handshake_to_protocol_crate() {
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/hysteria2_quic.rs"))
        .expect("read zero-transport hysteria2_quic source");
    let transport_manifest = fs::read_to_string(repo_root().join("crates/transport/Cargo.toml"))
        .expect("read zero-transport manifest");
    let connector = read("src/adapters/hysteria2/connector.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/outbound.rs"))
            .expect("read hysteria2 protocol outbound source");

    for forbidden in [
        "build_auth_frame",
        "build_tcp_connect_header",
        "parse_auth_response",
        "sign_hmac",
        "parse_tcp_connect_header",
        "Hysteria2Outbound",
        "Hysteria2Connector",
        "hysteria2::",
    ] {
        assert!(
            !transport.contains(forbidden),
            "zero-transport QUIC helper should not depend on Hysteria2 protocol handshake/framing; found `{forbidden}`"
        );
    }
    assert!(
        transport.contains("pub struct Hysteria2Stream")
            && transport.contains("pub struct QuicConnectionOptions")
            && transport.contains("pub struct Hysteria2QuicProfile")
            && transport.contains("pub fn from_parts(client_fingerprint: Option<&str>)")
            && transport.contains("fn client_fingerprint(&self) -> Option<&str>")
            && transport.contains("quic_profile: Hysteria2QuicProfile")
            && !transport.contains("client_fingerprint: Option<&'a str>")
            && transport.contains("pub async fn open_quic_connection")
            && !transport_manifest.contains("hysteria2 = {")
            && connector.contains("struct Hysteria2Connector")
            && connector.contains("open_hysteria2_quic_connection")
            && connector.contains("Hysteria2QuicProfile::from_parts")
            && connector.contains("quic_profile: Hysteria2QuicProfile")
            && !connector.contains("client_fingerprint: Option<String>")
            && connector.contains("hysteria2::Hysteria2OutboundProfile")
            && !connector.contains("password: String")
            && connector.contains("connect_raw_with_udp_profile")
            && !connector.contains("profile.password()")
            && connector.contains(".authenticate_connection(")
            && connector.contains(".establish_tcp_connect(")
            && !connector.contains(".authenticate_with_salt(")
            && !connector.contains(".send_tcp_connect(")
            && !connector.contains(".read_connect_response(")
            && protocol_outbound.contains("struct Hysteria2OutboundProfile")
            && protocol_outbound.contains("pub async fn authenticate_with_salt")
            && protocol_outbound.contains("pub async fn authenticate_connection")
            && protocol_outbound.contains("pub async fn establish_tcp_connect")
            && protocol_outbound.contains("crate::shared::sign_hmac")
            && protocol_outbound.contains("crate::shared::build_auth_frame")
            && protocol_outbound.contains("build_tcp_connect_header"),
        "zero-transport should own only QUIC stream lifecycle; protocols/hysteria2 should own auth and TCP connect framing while proxy connector calls narrow protocol APIs"
    );
}

#[test]
fn h2_udp_state_model_lives_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let model = manifest_dir().join("src/adapters/hysteria2/udp/manager/model.rs");

    for forbidden in [
        "struct H2Entry",
        "struct H2SendExisting",
        "struct H2Key",
        "enum H2Key",
        "H2Key",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should not keep protocol state/request model `{forbidden}`"
        );
    }
    assert!(
        !model.exists()
            && generic_manager.contains("pub(crate) struct ManagedDatagramFlowManager")
            && generic_manager.contains("ManagedExistingSend<'_>"),
        "Hysteria2 UDP should use the generic managed datagram request model instead of h2_manager/model.rs"
    );
}

#[test]
fn h2_udp_model_details_live_outside_manager_root() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let model = manifest_dir().join("src/adapters/hysteria2/udp/manager/model.rs");
    let send = manifest_dir().join("src/adapters/hysteria2/udp/manager/send.rs");
    let establish = manifest_dir().join("src/adapters/hysteria2/udp/manager/establish.rs");
    let stream = manifest_dir().join("src/adapters/hysteria2/udp/manager/stream.rs");

    for forbidden in [
        "struct H2Entry",
        "struct H2SendExisting",
        "struct H2UdpPeer",
        "struct H2Key",
        "enum H2Key",
        "H2Key",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should not keep H2 manager model detail `{forbidden}`"
        );
    }

    assert!(
        !model.exists()
            && !send.exists()
            && !establish.exists()
            && !stream.exists()
            && generic_manager.contains("ManagedUdpConnectionCache")
            && generic_manager.contains("ManagedUdpConnectionCache::new")
            && managed.contains("OutboundEndpoint<'_>")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowStore")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowSessions"),
        "Hysteria2 UDP should use neutral generic cache storage while the protocol resume owns cache-key identity"
    );
}

#[test]
fn h2_udp_send_orchestration_lives_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let send = manifest_dir().join("src/adapters/hysteria2/udp/manager/send.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_cache = read("src/runtime/udp_flow/managed/cache.rs");

    for forbidden in [
        "pub(crate) async fn send_existing",
        "H2Key::from_peer",
        "H2Key::from_resume",
        "h2_udp_packet",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should delegate send orchestration to generic managed runtime; found `{forbidden}`"
        );
    }
    assert!(
        !send.exists(),
        "Hysteria2 UDP send orchestration should not live in h2_manager/send.rs"
    );
    assert!(
        generic_manager.contains(".send_or_insert_pre_sent_key(")
            && !managed.contains(".send_or_insert(")
            && !managed.contains("ManagedUdpConnectionCacheKey")
            && !managed.contains(".spawn_response_bridge(")
            && !managed.contains("self.upstreams.get(&cache_key)")
            && !managed.contains("self.upstreams.insert(cache_key")
            && managed_cache.contains("async fn send_or_insert_pre_sent")
            && !managed_cache.contains("pub(crate) async fn send_or_insert_pre_sent(")
            && managed_cache.contains("pub(crate) async fn send_or_insert_pre_sent_key")
            && managed_cache.contains("connection.spawn_response_bridge(chain_tasks, session_id)")
            && !generic_manager.contains("subscribe_responses()")
            && !generic_manager.contains("spawn_tuple_response_bridge"),
        "Hysteria2 UDP send orchestration should delegate cache hit/miss and response bridge wiring to the neutral UDP connection cache"
    );
}

#[test]
fn h2_udp_establish_logic_lives_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp/managed.rs");
    let establish = manifest_dir().join("src/adapters/hysteria2/udp/manager/establish.rs");

    for forbidden in ["stream::establish", "spawn_response_bridge"] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should keep establish details behind the connector trait; found `{forbidden}`"
        );
    }
    assert!(
        !establish.exists()
            && managed.contains("async fn establish(")
            && managed.contains("connector::establish_udp_flow_session"),
        "Hysteria2 UDP establish glue should live in the thin managed connector, not h2_manager/establish.rs"
    );
}

#[test]
fn shadowsocks_udp_datagram_codec_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let outbound = manifest_dir().join("src/outbound/shadowsocks.rs");
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let adapter_flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let adapter_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let transport =
        fs::read_to_string(repo_root().join("crates/transport/src/shadowsocks_transport.rs"))
            .expect("read shadowsocks transport source");
    let transport_manifest = fs::read_to_string(repo_root().join("crates/transport/Cargo.toml"))
        .expect("read zero-transport manifest");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/shadowsocks/src/lib.rs"))
        .expect("read shadowsocks protocol lib source");

    for forbidden in [
        "UdpDatagramFraming",
        "ShadowsocksUdpPacketTarget",
        "ShadowsocksUdpDecodeContext",
        "ShadowsocksUdpPacket",
        "resume.managed_socket_flow().codec()",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Shadowsocks UDP managed glue should not own datagram codec details; found `{forbidden}`"
        );
    }
    assert!(
        !outbound.exists(),
        "Shadowsocks UDP socket flow glue should not require src/outbound/shadowsocks.rs"
    );
    for removed in [
        "src/adapters/shadowsocks/udp/manager.rs",
        "src/adapters/shadowsocks/udp/manager/model.rs",
        "src/adapters/shadowsocks/udp/manager/entry.rs",
        "src/adapters/shadowsocks/udp/manager/bridge.rs",
        "src/adapters/shadowsocks/udp/manager/codec.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "Shadowsocks UDP should not keep proxy-owned manager file `{removed}`"
        );
    }
    assert!(
        !adapter.contains("shadowsocks::udp_flow_codec")
            && !adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowConfig::new")
            && !adapter.contains(".flow_resume()")
            && !adapter.contains(".packet_path_spec()")
            && adapter_flow.contains("shadowsocks::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("ShadowsocksUdpFlowConfig::new")
            && !adapter_flow.contains(".flow_resume()")
            && adapter_packet_path.contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("ShadowsocksUdpFlowConfig::new")
            && !adapter_packet_path.contains(".packet_path_spec()")
            && adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && adapter_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !adapter_packet_path.contains("descriptor.cache_key()")
            && !adapter_packet_path.contains("descriptor.server()")
            && !adapter_packet_path.contains("descriptor.port()")
            && adapter_packet_path.contains("udp_packet_path_datagram_source_build_from_config")
            && adapter_packet_path.contains("udp_datagram_source_from_build")
            && !adapter_packet_path.contains("spec.datagram_source_parts()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(datagram)")
            && !adapter_packet_path.contains("datagram.cache_key()")
            && !adapter_packet_path.contains("datagram.codec()")
            && adapter_packet_path.contains("self.into_shared_codec_parts()")
            && !adapter_packet_path.contains("Arc::new(codec)")
            && !adapter_packet_path
                .contains("let (tag, server, port, cache_key, codec) = self.into_parts();")
            && !adapter_packet_path.contains("self.codec()")
            && !adapter_packet_path.contains("datagram.tag()")
            && !adapter_packet_path.contains("datagram.server()")
            && !adapter_packet_path.contains("datagram.port()")
            && !adapter_packet_path.contains("spec.carrier()")
            && !adapter_packet_path.contains("spec.datagram_source()")
            && !adapter_packet_path.contains("spec.cache_key()")
            && !adapter_packet_path.contains("spec.carrier_cache_key()")
            && !adapter_packet_path.contains("spec.datagram_cache_key()")
            && !adapter_packet_path.contains("spec.codec()")
            && !adapter_packet_path.contains(".packet_path_cache_key()")
            && !adapter_packet_path.contains(".packet_path_codec()")
            && protocol_outbound.contains("fn udp_flow_codec(")
            && !protocol_outbound.contains("pub fn udp_flow_codec(")
            && protocol_outbound.contains("struct ShadowsocksUdpFlowConfig")
            && protocol_outbound.contains("pub fn flow_resume(&self)")
            && protocol_outbound.contains("pub fn into_shared_codec_parts")
            && protocol_outbound.contains("pub fn udp_flow_resume_from_config(")
            && protocol_outbound.contains("pub fn packet_path_spec(&self)")
            && protocol_outbound.contains("pub fn udp_packet_path_spec_from_config(")
            && protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathSpec")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathCarrier {")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathDatagram {")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathDatagramSourceParts {")
            && !protocol_outbound.contains("pub fn carrier_cache_key(&self)")
            && !protocol_outbound.contains("pub fn datagram_cache_key(&self)")
            && !protocol_outbound.contains("pub fn carrier(&self)")
            && !protocol_outbound.contains("pub fn datagram_source(&self)")
            && !protocol_outbound.contains("pub fn packet_path_cache_key(&self)")
            && !protocol_outbound.contains("pub fn packet_path_codec(&self)")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound
                .contains("impl DatagramCodec<Address> for ShadowsocksDatagramCodec")
            && protocol_outbound.contains("struct ShadowsocksUdpFlowPacket")
            && !protocol_outbound.contains("pub fn udp_flow_packet")
            && !protocol_outbound.contains("fn udp_flow_packet")
            && !managed.contains("shadowsocks::udp_flow_packet")
            && !managed.contains("UdpFlowPacket::from_parts")
            && generic_manager.contains(".send_datagram(")
            && !managed.contains("BridgeWaiters")
            && managed.contains("self.flow.send_datagram(target, port, payload)")
            && transport.contains("send_packet(&self, packet: UdpFlowPacket)")
            && transport.contains("pub async fn send_datagram(")
            && transport.contains("Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>")
            && managed.contains("async fn establish_udp_socket_flow")
            && managed.contains("resume.into_managed_socket_flow_codec()")
            && !managed.contains("resume.managed_socket_flow().codec()")
            && !transport.contains("shadowsocks::")
            && !transport_manifest.contains("dep:shadowsocks")
            && !transport_manifest.contains("shadowsocks = { path = \"../../protocols/shadowsocks\"")
            && !managed.contains("ShadowsocksUdpFlowPacket::from_parts")
            && protocol_outbound.contains("pub fn encode_with(")
            && protocol_outbound.contains("pub fn decode_flow_packet(&self"),
        "Shadowsocks UDP managed glue should send target datagrams through transport while transport consumes a protocol-built codec"
    );
    for private_helper in [
        "encode_udp_datagram",
        "decode_udp_datagram",
        "encode_udp_flow_packet",
        "decode_udp_flow_packet",
        "udp_datagram_codec",
        "udp_flow_codec",
    ] {
        assert!(
            protocol_outbound.contains(&format!("fn {private_helper}("))
                && !protocol_outbound.contains(&format!("pub fn {private_helper}("))
                && !protocol_lib.contains(&format!("{private_helper},")),
            "Shadowsocks UDP helper `{private_helper}` should stay private to protocols/shadowsocks::outbound and should not be re-exported"
        );
    }
    assert!(
        !protocol_outbound.contains("fn udp_flow_packet") && !protocol_lib.contains("udp_flow_packet"),
        "Shadowsocks UDP flow packet constructor helper should be removed from the public protocol surface"
    );
    for forbidden in [".encode_packet(", ".decode_packet("] {
        assert!(
            !managed.contains(forbidden) && !generic_manager.contains(forbidden),
            "Shadowsocks UDP managed glue should not call raw protocol packet codec operations directly; found `{forbidden}`"
        );
    }
    assert!(
        transport.contains("self.codec.encode(target, port, payload)")
            && transport.contains("codec.decode(datagram)")
            && !managed.contains(".encode_with(")
            && !generic_manager.contains(".encode_with(")
            && !managed.contains(".decode_flow_packet(")
            && !generic_manager.contains(".decode_flow_packet("),
        "Shadowsocks UDP flow encode/decode should be delegated through an adapter-provided datagram codec"
    );
}

#[test]
fn shadowsocks_udp_response_bridge_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_datagram = read("src/runtime/udp_flow/managed/datagram.rs");
    let bridge = manifest_dir().join("src/adapters/shadowsocks/udp/manager/bridge.rs");

    for forbidden in [
        "oneshot::channel",
        "VecDeque",
        "struct SsResponseWaiter",
        "fn remove_waiter",
    ] {
        assert!(
            !managed.contains(forbidden) && !generic_manager.contains(forbidden),
            "Shadowsocks UDP managed glue should use neutral managed datagram waiter helpers instead of owning `{forbidden}`"
        );
    }
    for forbidden in [
        "tokio::spawn",
        "flow.subscribe()",
        "while let Ok((target, port, payload))",
    ] {
        assert!(
            !generic_manager.contains(forbidden),
            "generic managed datagram manager should keep response pump details in managed/datagram.rs; found `{forbidden}`"
        );
    }
    for forbidden in [
        "ManagedDatagramResponseWaiters",
        "spawn_datagram_response_bridge",
        "spawn_upstream_response_pump",
        "tokio::spawn",
        "waiters.deliver",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Shadowsocks UDP managed glue should use neutral managed datagram response glue instead of owning `{forbidden}`"
        );
    }
    assert!(
        !bridge.exists()
            && managed.contains("managed_datagram_connection")
            && managed.contains("flow.subscribe()")
            && managed_datagram.contains("pub(crate) struct ManagedDatagramResponseWaiters")
            && managed_datagram.contains("pub(crate) fn spawn_datagram_response_bridge")
            && managed_datagram.contains("fn spawn_upstream_response_pump")
            && managed_datagram.contains("oneshot::channel")
            && managed_datagram.contains("VecDeque"),
        "Shadowsocks UDP response waiter/pump logic should live in neutral managed datagram helpers"
    );
}

#[test]
fn shadowsocks_udp_socket_runtime_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let transport_path = repo_root().join("crates/transport/src/shadowsocks_transport.rs");
    let transport = fs::read_to_string(&transport_path).expect("read shadowsocks transport source");

    for forbidden in [
        "UdpSocket::bind",
        "from_std",
        "fn recv_loop",
        "tokio::spawn(Self::recv_loop",
        "shadowsocks udp recv loop stopped",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Shadowsocks UDP managed glue should keep socket runtime details outside zero-proxy; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/adapters/shadowsocks/udp/manager/socket.rs")
            .exists(),
        "Shadowsocks UDP socket runtime should not live in zero-proxy ss_manager/socket.rs"
    );
    assert!(
        transport_path.exists()
            && transport.contains("pub struct ShadowsocksUdpSocketFlow")
            && transport.contains("tokio::net::UdpSocket::bind")
            && transport.contains("async fn recv_loop"),
        "Shadowsocks UDP socket runtime should live in zero-transport"
    );
}

#[test]
fn shadowsocks_udp_state_model_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let model = manifest_dir().join("src/adapters/shadowsocks/udp/manager/model.rs");

    for forbidden in [
        "struct SsUpstream",
        "struct SsSendExisting",
        "struct SsKey",
        "SsKey",
        "format!(\"{cipher_kind:?}\")",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Shadowsocks UDP managed glue should not keep state/request model `{forbidden}`"
        );
    }
    assert!(
        !model.exists()
            && generic_manager.contains("pub(crate) struct ManagedDatagramSocketFlowManager")
            && generic_manager.contains("ManagedExistingSend<'_>"),
        "Shadowsocks UDP should use the generic managed datagram socket request model instead of ss_manager/model.rs"
    );
}

#[test]
fn shadowsocks_udp_flow_cipher_is_adapter_parsed() {
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let adapter_flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let flows = read("src/runtime/udp_flow/managed/flow.rs");
    let managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let outbound = manifest_dir().join("src/outbound/shadowsocks.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_cache = read("src/runtime/udp_flow/managed/cache.rs");
    let snapshot = read("src/runtime/udp_flow/managed/flow.rs");
    let forward = read("src/runtime/udp_flow/managed/datagram.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");
    let socket_flow_spec_impl = impl_block(&protocol_outbound, "ShadowsocksUdpSocketFlowSpec");

    assert!(
        !adapter.contains("CipherKind::from_str")
            && !adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowConfig::new")
            && adapter_flow.contains("shadowsocks::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("ShadowsocksUdpFlowConfig::new")
            && protocol_outbound.contains("pub fn parse_udp_cipher("),
        "Shadowsocks UDP adapter should ask protocols/shadowsocks to parse ordinary UDP flow cipher config"
    );
    for source in [&managed, &generic_manager] {
        assert!(
            !source.contains("CipherKind::from_str")
                && !source.contains("cipher: shadowsocks::CipherKind")
                && !source.contains("password: &'a str"),
            "Shadowsocks UDP manager state should receive adapter-built codec/cache identity instead of raw cipher/password details"
        );
    }
    let shadowsocks_flow_model = flows
        .split("#[cfg(feature = \"mieru\")]")
        .next()
        .expect("Shadowsocks UDP flow model should appear before Mieru");
    assert!(
        !shadowsocks_flow_model.contains("cipher: shadowsocks::CipherKind")
            && !shadowsocks_flow_model.contains("password: &'a str")
            && !shadowsocks_flow_model.contains("cache_key: String")
            && !shadowsocks_flow_model.contains("DatagramCodec")
            && shadowsocks_flow_model.contains("resume: ManagedUdpFlowResume"),
        "ordinary Shadowsocks UDP flow model should carry only the unified resume descriptor"
    );
    assert!(
        !managed.contains("cache_key: &'a str")
            && !managed.contains("leaf_key:")
            && !managed.contains("SsKey")
            && !managed.contains("fn from_resume(")
            && !managed.contains("socket_flow_cache_key()")
            && generic_manager.contains("ManagedDatagramConnectionCache")
            && generic_manager.contains("ManagedDatagramSocketConnectorFlow")
            && !generic_manager.contains("fn flow_cache_key")
            && !managed.contains("ManagedDatagramConnectionCacheKey")
            && managed.contains("managed_datagram_socket_connector_flow_from_build")
            && managed.contains("shadowsocks::udp::managed_socket_flow_from_resume")
            && !managed.contains("ManagedDatagramSocketConnectorFlow::new")
            && !managed.contains("flow.cache_key()")
            && !managed.contains("resume.managed_socket_flow()")
            && !managed.contains("resume.managed_socket_flow().codec()")
            && managed.contains("async fn establish_udp_socket_flow")
            && managed.contains("shadowsocks_transport::establish_shadowsocks_udp_socket_flow")
            && managed.contains("resume.into_managed_socket_flow_codec()")
            && !managed.contains("outbound::shadowsocks::establish_udp_socket_flow")
            && !outbound.exists()
            && !managed.contains("resume.socket_flow().")
            && !managed.contains("resume.flow_cache_key()")
            && !managed.contains("resume.socket_flow_codec()")
            && generic_manager.contains(".get_or_insert_key(")
            && !managed.contains("upstreams.get(")
            && !managed.contains("upstreams.insert(")
            && managed_cache.contains("struct ManagedDatagramConnectionCache")
            && managed_cache.contains("async fn get_or_insert_with")
            && !managed_cache.contains("pub(crate) async fn get_or_insert_with")
            && managed_cache.contains("pub(crate) async fn get_or_insert_key")
            && !managed.contains("shadowsocks::udp::ShadowsocksUdpFlowEntries")
            && managed.contains("SharedManagedDatagramUdpConnection")
            && !managed.contains("Arc<SsUpstream>")
            && !managed.contains(".waiters")
            && !managed.contains("shadowsocks::udp::ShadowsocksUdpFlowStore<Arc<SsUpstream>>")
            && !managed.contains("HashMap<shadowsocks::udp::ShadowsocksUdpCacheKey"),
        "ordinary Shadowsocks UDP peer model should carry only protocol-owned opaque cache identity and a neutral datagram connection"
    );
    assert!(
        !adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowConfig::new")
            && !adapter.contains(".flow_resume()")
            && adapter_flow.contains("shadowsocks::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("ShadowsocksUdpFlowConfig::new")
            && !adapter_flow.contains(".flow_resume()")
            && !adapter.contains("ShadowsocksUdpFlowResume::new")
            && protocol_outbound.contains("struct ShadowsocksUdpFlowConfig")
            && protocol_outbound.contains("pub fn flow_resume(&self)")
            && protocol_outbound.contains("pub fn udp_flow_resume_from_config(")
            && protocol_outbound.contains("struct ShadowsocksUdpFlowResume")
            && !protocol_outbound.contains("struct ShadowsocksUdpCacheKey")
            && protocol_outbound.contains("pub struct ShadowsocksUdpSocketFlowSpec")
            && !socket_flow_spec_impl.contains("pub fn cache_key(&self)")
            && !socket_flow_spec_impl.contains("pub fn codec(&self)")
            && socket_flow_spec_impl.contains("pub fn into_cache_key")
            && socket_flow_spec_impl
                .contains("pub fn into_codec(self) -> ShadowsocksDatagramCodec")
            && protocol_outbound.contains("pub fn flow_cache_key(&self)")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpCacheKey")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpFlowStore")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpFlowEntries")
            && !protocol_outbound.contains("socket_flow_cache_key")
            && protocol_outbound.contains("pub fn socket_flow_codec(&self)")
            && protocol_outbound.contains("pub fn managed_socket_flow(&self)")
            && protocol_outbound.contains("pub fn managed_socket_flow_from_resume(")
            && !protocol_outbound.contains("pub fn socket_flow(&self)")
            && protocol_outbound.contains("pub fn leaf_cache_key(&self)")
            && protocol_outbound.contains("struct ShadowsocksUdpLeafKey")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound.contains("pub fn codec(&self)")
            && protocol_outbound.contains("pub fn cache_key(&self) -> &str"),
        "Shadowsocks adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    assert!(
        snapshot.contains("resume: ManagedUdpFlowResume")
            && snapshot.contains("inner: Arc<dyn ManagedUdpFlowResumeObject>")
            && !snapshot.contains("Shadowsocks(shadowsocks::udp::ShadowsocksUdpFlowResume)")
            && !snapshot.contains("cipher_kind: shadowsocks::CipherKind")
            && !snapshot.contains("datagram_cache_key: String"),
        "Shadowsocks protocol UDP flow snapshot should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("ManagedExistingSend")
            && forward.contains("ManagedExistingSend::forwarded")
            && !forward.contains("existing.resume.cache_key()")
            && !forward.contains("existing.resume.codec()")
            && !forward.contains("shadowsocks::udp_flow_codec")
            && !forward.contains("password: &'a str")
            && !forward.contains("cipher_kind: shadowsocks::CipherKind")
            && !forward.contains("datagram_cache_key: &'a str"),
        "existing Shadowsocks UDP flow forwarding should pass the opaque resume descriptor without unpacking cache identity or codec state"
    );
    let start = read("src/runtime/udp_flow/managed/datagram.rs");
    assert!(
        !start.contains("ManagedUdpFlowResume::Shadowsocks")
            && start.contains("ManagedExistingSend::datagram")
            && !start.contains("resume.cache_key()")
            && !start.contains("resume.codec()"),
        "new Shadowsocks UDP flow start should pass the unified resume descriptor without unpacking cache identity or codec state"
    );
    for forbidden in [
        "ShadowsocksUdpLeafKey",
        "leaf_cache_key",
        "resume.codec()",
        "request.resume.cache_key()",
        "request.resume.codec()",
        "cache_key: &str",
        "SsKey::new(server",
        "SsKey::from_resume",
    ] {
        assert!(
            !managed.contains(forbidden) && !generic_manager.contains(forbidden),
            "Shadowsocks UDP managed glue should use a protocol-owned cache key/codec handle instead of unpacking `{forbidden}`"
        );
    }
    assert!(
        !managed.contains("cache_key: String"),
        "Shadowsocks UDP managed glue should not declare protocol cache key fields"
    );
}

#[test]
fn shadowsocks_packet_path_cipher_is_adapter_parsed() {
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let adapter_flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let adapter_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let protocol_outbound = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/shadowsocks/src/outbound.rs");
    let protocol_outbound =
        fs::read_to_string(protocol_outbound).expect("read shadowsocks protocol outbound source");
    let carrier = read("src/runtime/udp_flow/packet_path_chain/carriers.rs");
    let udp_socket_carrier =
        read("src/runtime/udp_flow/packet_path_chain/carriers/udp_socket_carrier.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/udp_packet_path.rs"))
        .expect("read zero-transport udp packet path source");
    let entry = read("src/runtime/udp_flow/packet_path_chain/entry.rs");
    let traits = read("src/runtime/udp_flow/packet_path.rs");
    let key = read("src/runtime/udp_flow/packet_path_chain/key.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");
    let shadowsocks_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let carrier_snapshot = read("src/runtime/udp_flow/packet_path.rs");
    let snapshot = read("src/runtime/udp_flow/packet_path_chain/snapshot.rs");
    let forward = read("src/runtime/udp_flow/managed/datagram.rs");

    assert!(
        !adapter.contains("CipherKind::from_str")
            && !adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowConfig::new")
            && adapter_flow.contains("shadowsocks::udp::udp_flow_resume_from_config")
            && !adapter_flow.contains("ShadowsocksUdpFlowConfig::new")
            && adapter_packet_path.contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("ShadowsocksUdpFlowConfig::new"),
        "Shadowsocks adapter should ask protocols/shadowsocks to parse packet-path carrier/datagram cipher config"
    );
    for forbidden in ["ShadowsocksDatagramCodec", "shadowsocks::"] {
        assert!(
            !udp_socket_carrier.contains(forbidden),
            "UDP socket packet-path carrier adapter should consume an adapter-provided codec instead of naming protocol framing; found `{forbidden}`"
        );
    }
    assert!(
        udp_socket_carrier.contains("UdpSocketPacketPath::establish")
            && udp_socket_carrier.contains("PacketPathCarrierAdapter")
            && transport.contains("Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>")
            && transport.contains("self.codec.encode")
            && transport.contains("self.codec.decode"),
        "zero-transport should own UDP socket packet-path codec use while proxy keeps only carrier trait adaptation"
    );
    assert!(
        !carrier_snapshot.contains("ShadowsocksDatagramCodec")
            && !carrier_snapshot.contains("shadowsocks::udp_datagram_codec")
            && !adapter.contains("shadowsocks::udp_datagram_codec")
            && !adapter.contains("resume.codec()")
            && !adapter.contains("resume.packet_path_codec()")
            && !adapter.contains("config.packet_path_spec()")
            && adapter_packet_path.contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains(".packet_path_spec()")
            && !adapter_packet_path.contains("packet_path.cache_key()")
            && !adapter_packet_path.contains("packet_path.codec()")
            && !adapter_packet_path.contains("UdpDatagramSourceParts")
            && adapter_packet_path.contains("udp_datagram_source_from_build(datagram)")
            && adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && adapter_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !adapter_packet_path.contains("descriptor.cache_key()")
            && !adapter_packet_path.contains("descriptor.server()")
            && !adapter_packet_path.contains("descriptor.port()")
            && adapter_packet_path.contains("udp_packet_path_datagram_source_build_from_config")
            && adapter_packet_path.contains("udp_datagram_source_from_build")
            && !adapter_packet_path.contains("spec.datagram_source_parts()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(datagram)")
            && !adapter_packet_path.contains("datagram.cache_key()")
            && !adapter_packet_path.contains("datagram.codec()")
            && shadowsocks_packet_path.contains("self.into_shared_codec_parts()")
            && !shadowsocks_packet_path.contains("Arc::new(codec)")
            && !shadowsocks_packet_path
                .contains("let (tag, server, port, cache_key, codec) = self.into_parts();")
            && !shadowsocks_packet_path.contains("self.codec()")
            && !adapter_packet_path.contains("datagram.tag()")
            && !adapter_packet_path.contains("datagram.server()")
            && !adapter_packet_path.contains("datagram.port()")
            && !adapter_packet_path.contains("spec.carrier()")
            && !adapter_packet_path.contains("spec.datagram_source()")
            && !adapter_packet_path.contains("spec.cache_key()")
            && !adapter_packet_path.contains("spec.carrier_cache_key()")
            && !adapter_packet_path.contains("spec.datagram_cache_key()")
            && !adapter_packet_path.contains("spec.codec()")
            && !adapter_packet_path.contains(".packet_path_cache_key()")
            && !adapter_packet_path.contains(".packet_path_codec()")
            && protocol_outbound.contains("pub fn into_shared_codec_parts")
            && protocol_outbound.contains("Arc::new(codec)"),
        "Shadowsocks adapter should request protocol-built packet-path bundles through explicit protocol packet-path helpers"
    );
    assert!(
        !adapter.contains("shadowsocks::udp_flow_codec")
            && protocol_outbound.contains("fn udp_flow_codec(")
            && protocol_outbound.contains("pub fn from_config("),
        "Shadowsocks adapter should request protocol-built packet-path codecs through opaque resume config"
    );
    assert!(
        !udp_socket_carrier.contains("shadowsocks::encode_udp_flow_packet")
            && !udp_socket_carrier.contains("shadowsocks::decode_udp_flow_packet"),
        "UDP socket packet-path carrier should not call flow-specific protocols/shadowsocks helpers directly"
    );
    for source in [&carrier, &entry] {
        assert!(
            !source.contains("CipherKind::from_str"),
            "packet-path chain should receive adapter-parsed Shadowsocks cipher values"
        );
    }
    assert!(
        !traits.contains("password: &'a str")
            && !traits.contains("cipher_kind: shadowsocks::CipherKind")
            && traits.contains("struct UdpDatagramDescriptor")
            && traits.contains("cache_key: String")
            && traits.contains("descriptor: UdpDatagramDescriptor")
            && traits.contains("tag: String")
            && traits.contains("server: String")
            && !traits.contains("ManagedUdpFlowSnapshot")
            && traits.contains("codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>"),
        "UdpDatagramSource should contain only neutral descriptor identity and adapter-provided packet-path datagram codec"
    );
    assert!(
        !adapter.contains("shadowsocks::udp_cache_key")
            && !adapter.contains("resume.cache_key()")
            && !adapter.contains("resume.packet_path_cache_key()")
            && !adapter.contains("packet_path.cache_key()")
            && !adapter_packet_path.contains("packet_path.cache_key()")
            && adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && adapter_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !adapter_packet_path.contains("descriptor.cache_key()")
            && !adapter_packet_path.contains("descriptor.server()")
            && !adapter_packet_path.contains("descriptor.port()")
            && adapter_packet_path.contains("udp_packet_path_datagram_source_build_from_config")
            && !adapter_packet_path.contains("spec.datagram_source_parts()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(datagram)")
            && !adapter_packet_path.contains("spec.carrier()")
            && !adapter_packet_path.contains("spec.datagram_source()")
            && !adapter_packet_path.contains("spec.cache_key()")
            && !adapter_packet_path.contains("spec.carrier_cache_key()")
            && !adapter_packet_path.contains("spec.datagram_cache_key()")
            && !adapter_packet_path.contains(".packet_path_cache_key()"),
        "Shadowsocks adapter should receive opaque packet-path cache keys from protocols/shadowsocks resume config"
    );
    assert!(
        protocol_outbound.contains("fn udp_cache_key(")
            && !protocol_outbound.contains("pub fn udp_cache_key(")
            && protocol_outbound.contains("pub fn packet_path_spec(&self)")
            && protocol_outbound.contains("pub fn udp_packet_path_spec_from_config(")
            && protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathSpec")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathCarrier {")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathDatagram {")
            && !protocol_outbound.contains("pub struct ShadowsocksUdpPacketPathDatagramSourceParts {")
            && !protocol_outbound.contains("pub fn carrier_cache_key(&self)")
            && !protocol_outbound.contains("pub fn datagram_cache_key(&self)")
            && !protocol_outbound.contains("pub fn carrier(&self)")
            && !protocol_outbound.contains("pub fn datagram_source(&self)")
            && !protocol_outbound.contains("pub fn packet_path_cache_key(&self)")
            && !protocol_outbound.contains("pub fn packet_path_codec(&self)"),
        "protocols/shadowsocks should own Shadowsocks cache identity internally and expose packet-path helpers instead"
    );
    for source in [
        &traits,
        &key,
        &outbound,
        &carrier_snapshot,
        &snapshot,
        &forward,
    ] {
        assert!(
            !source.contains("datagram_cipher") && !source.contains("cipher: &'a str"),
            "packet-path runtime should not carry raw Shadowsocks cipher strings for cache identity"
        );
    }
    assert!(
        !carrier_snapshot.contains("cipher: String")
            && !carrier_snapshot.contains("enum UdpPacketPathCarrier"),
        "packet-path carrier snapshots should use neutral adapter-built cache keys instead of protocol-specific payload fields"
    );
}

#[test]
fn udp_build_traits_consume_protocol_parts() {
    let stream_manager = read("src/runtime/udp_flow/managed/stream_manager.rs");
    let datagram_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let packet_path = read("src/runtime/udp_flow/packet_path.rs");
    let socks5_packet_path = read("src/adapters/socks5/udp/packet_path.rs");
    let shadowsocks_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let shadowsocks_managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let hysteria2_connector = read("src/adapters/hysteria2/connector.rs");
    let trojan_connector = read("src/adapters/trojan/udp/managed/connector.rs");
    let mieru_connector = read("src/adapters/mieru/udp/managed/connector.rs");
    let socks5_shared = fs::read_to_string(repo_root().join("protocols/socks5/src/shared.rs"))
        .expect("read socks5 shared source");
    let shadowsocks_protocol =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");
    let hysteria2_protocol = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let trojan_protocol = fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
        .expect("read trojan protocol outbound source");
    let mieru_protocol = fs::read_to_string(repo_root().join("protocols/mieru/src/udp.rs"))
        .expect("read mieru protocol udp source");
    let socks5_descriptor_impl = impl_block(&socks5_shared, "Socks5UdpPacketPathCarrierDescriptor");
    let socks5_build_impl = impl_block(&socks5_shared, "Socks5UdpPacketPathCarrierBuild");
    let shadowsocks_descriptor_impl = impl_block(
        &shadowsocks_protocol,
        "ShadowsocksUdpPacketPathCarrierDescriptor",
    );
    let shadowsocks_datagram_build_impl = impl_block(
        &shadowsocks_protocol,
        "ShadowsocksUdpPacketPathDatagramSourceBuild",
    );
    let hysteria2_descriptor_impl = impl_block(
        &hysteria2_protocol,
        "Hysteria2UdpPacketPathCarrierDescriptor",
    );
    let hysteria2_build_impl =
        impl_block(&hysteria2_protocol, "Hysteria2UdpPacketPathCarrierBuild");

    assert!(
        stream_manager.contains("fn into_parts(self) -> (String, bool);")
            && stream_manager
                .contains("let (cache_key, requires_relay_upstream) = build.into_parts();")
            && stream_manager
                .contains("let (cache_key, requires_relay_upstream) = connector_flow.into_parts();")
            && !stream_manager.contains("fn cache_key(&self) -> String;")
            && !stream_manager.contains("fn requires_relay_upstream(&self) -> bool;")
            && !stream_manager.contains("connector_flow.cache_key()")
            && !stream_manager.contains("connector_flow.requires_relay_upstream()"),
        "managed stream connector flow builds should consume protocol-provided parts instead of exposing getter traits"
    );
    assert!(
        trojan_connector.contains("fn into_parts(self) -> (String, bool)")
            && trojan_connector.contains("self.into_parts()")
            && mieru_connector.contains("fn into_parts(self) -> (String, bool)")
            && mieru_connector.contains("self.into_parts()")
            && !trojan_connector.contains("self.cache_key()")
            && !trojan_connector.contains("self.requires_relay_upstream()")
            && !mieru_connector.contains("self.cache_key()")
            && !mieru_connector.contains("self.requires_relay_upstream()")
            && trojan_protocol.contains("pub fn into_parts(self) -> (String, bool)")
            && mieru_protocol.contains("pub fn into_parts(self) -> (alloc::string::String, bool)"),
        "Trojan and Mieru stream connector glue should not read protocol cache-key getters"
    );
    assert!(
        datagram_manager.contains("fn into_cache_key(self) -> String;")
            && datagram_manager
                .contains("ManagedDatagramConnectorFlow::new(build.into_cache_key())")
            && datagram_manager
                .contains("ManagedDatagramSocketConnectorFlow::new(build.into_cache_key())")
            && datagram_manager.contains(".into_cache_key()")
            && !datagram_manager.contains("fn cache_key(&self) -> String;")
            && !datagram_manager.contains("fn cache_key(self) -> String")
            && !datagram_manager.contains(".cache_key()")
            && hysteria2_connector.contains("fn into_cache_key(self) -> String")
            && shadowsocks_managed.contains("fn into_cache_key(self) -> String")
            && hysteria2_connector.contains("self.into_cache_key()")
            && shadowsocks_managed.contains("self.into_cache_key()")
            && !hysteria2_connector.contains("self.cache_key()")
            && !shadowsocks_managed.contains("self.cache_key()"),
        "managed datagram connector flow builds should consume cache identity instead of exposing cache-key getters to proxy"
    );
    assert!(
        packet_path.contains("fn into_parts(self) -> (String, String, u16);")
            && packet_path.contains("let (cache_key, server, port) = build.into_parts();")
            && !packet_path.contains("fn server(&self) -> &str;")
            && !packet_path.contains("fn port(&self) -> u16;")
            && socks5_packet_path.contains("self.into_parts()")
            && shadowsocks_packet_path.contains("self.into_parts()")
            && hysteria2_connector.contains("self.into_parts()")
            && !socks5_packet_path.contains("self.server()")
            && !socks5_packet_path.contains("self.port()")
            && !shadowsocks_packet_path.contains("self.server()")
            && !shadowsocks_packet_path.contains("self.port()")
            && !hysteria2_connector.contains("self.server()")
            && !hysteria2_connector.contains("self.port()")
            && socks5_shared.contains("pub fn into_parts(self) -> (String, String, u16)")
            && shadowsocks_protocol.contains(
                "pub fn into_parts(self) -> (alloc::string::String, alloc::string::String, u16)"
            )
            && hysteria2_protocol.contains("pub fn into_parts(self) -> (String, String, u16)"),
        "packet-path carrier descriptors should cross into proxy as consumed neutral parts"
    );
    for (name, source) in [
        ("socks5 descriptor", &socks5_descriptor_impl),
        ("shadowsocks descriptor", &shadowsocks_descriptor_impl),
        ("hysteria2 descriptor", &hysteria2_descriptor_impl),
    ] {
        for forbidden in [
            "pub fn cache_key(&self)",
            "pub fn server(&self)",
            "pub fn port(&self)",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} should expose consumed packet-path identity parts instead of getter `{forbidden}`"
            );
        }
    }
    for (name, source) in [
        ("socks5 carrier build", &socks5_build_impl),
        ("hysteria2 carrier build", &hysteria2_build_impl),
    ] {
        for forbidden in [
            "pub fn cache_key(&self)",
            "pub fn server(&self)",
            "pub fn port(&self)",
            "pub fn connector_profile(&self)",
            "pub fn codec(&self)",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} should hand packet-path carrier state to proxy through consuming helpers, not getter `{forbidden}`"
            );
        }
    }
    assert!(
        !shadowsocks_protocol.contains("impl ShadowsocksUdpPacketPathCarrierBuild")
            && shadowsocks_protocol.contains("pub struct ShadowsocksUdpPacketPathCarrierBuild"),
        "Shadowsocks packet-path carrier build should be opaque data with no public getter impl"
    );
    assert!(
        packet_path.contains("trait UdpDatagramSourceBuild")
            && packet_path.contains("Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>")
            && packet_path
                .contains("let (tag, server, port, cache_key, codec) = build.into_parts();")
            && !packet_path.contains("fn tag(&self) -> &str;")
            && !packet_path.contains("fn cache_key(&self) -> String;")
            && shadowsocks_packet_path.contains("self.into_shared_codec_parts()")
            && !shadowsocks_packet_path
                .contains("let (tag, server, port, cache_key, codec) = self.into_parts();")
            && !shadowsocks_packet_path.contains("self.into_codec()")
            && !shadowsocks_packet_path.contains("Arc::new(codec)")
            && shadowsocks_protocol.contains("pub fn into_shared_codec_parts")
            && shadowsocks_protocol.contains("Arc::new(codec)")
            && shadowsocks_protocol.contains("pub fn into_parts(")
            && shadowsocks_protocol.contains("self.tag, self.server, self.port, self.cache_key")
            && hysteria2_connector.contains(".into_shared_codec_parts()")
            && !hysteria2_connector.contains("Arc::new(codec)")
            && hysteria2_protocol.contains("pub fn into_shared_codec_parts")
            && hysteria2_protocol.contains("Arc::new(codec)"),
        "packet-path datagram sources should consume protocol-built source parts and codec in one step"
    );
    for forbidden in [
        "pub fn tag(&self)",
        "pub fn server(&self)",
        "pub fn port(&self)",
        "pub fn cache_key(&self)",
        "pub fn codec(&self)",
    ] {
        assert!(
            !shadowsocks_datagram_build_impl.contains(forbidden),
            "Shadowsocks datagram source build should expose consumed parts instead of getter `{forbidden}`"
        );
    }
}

#[test]
fn adapters_do_not_own_udp_packet_path_cache_key_formats() {
    for source in [
        "src/adapters/socks5/udp.rs",
        "src/adapters/socks5/udp/packet_path.rs",
        "src/adapters/shadowsocks/udp/packet_path.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        for forbidden in ["socks5|", "shadowsocks|", "hysteria2|", "|auth:", "|fp:"] {
            assert!(
                !content.contains(forbidden),
                "{source} should ask the owning protocol/runtime helper for packet-path cache identity instead of formatting `{forbidden}`"
            );
        }
    }

    let udp_root = read("src/runtime/udp_flow/registered/mod.rs");
    let packet_path_snapshot = read("src/runtime/udp_flow/packet_path.rs");
    let socks5_shared = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/socks5/src/shared.rs");
    let socks5_shared =
        fs::read_to_string(socks5_shared).expect("read socks5 protocol shared source");
    let socks5_lib = fs::read_to_string(repo_root().join("protocols/socks5/src/lib.rs"))
        .expect("read socks5 protocol lib source");
    let hysteria2_udp = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/hysteria2/src/udp.rs");
    let hysteria2_udp =
        fs::read_to_string(hysteria2_udp).expect("read hysteria2 protocol udp source");
    assert!(
        !udp_root.contains("mod cache_key")
            && !packet_path_snapshot.contains("socks5_udp_cache_key"),
        "protocol_runtime::udp should not own packet-path cache identity helpers"
    );
    assert!(
        socks5_shared.contains("fn udp_cache_key(")
            && !socks5_shared.contains("pub fn udp_cache_key(")
            && socks5_shared.contains("socks5|"),
        "protocols/socks5 should own SOCKS5 cache identity construction internally"
    );
    let proxy_test_support = fs::read_to_string(manifest_dir().join("tests/support/mod.rs"))
        .expect("read proxy test support source");
    assert!(
        socks5_shared.contains("pub(crate) struct Socks5UdpPacket")
            && socks5_shared.contains("Socks5InboundUdpRequest")
            && socks5_shared.contains("Socks5InboundUdpResponse")
            && !socks5_lib.contains("Socks5UdpPacket,")
            && !proxy_test_support.contains("socks5::udp::Socks5UdpPacket"),
        "SOCKS5 raw UDP packet model should remain protocol-private; public callers use inbound UDP request/response views"
    );
    for private_helper in [
        "parse_udp_packet",
        "build_udp_packet",
        "decode_udp_associate_request",
        "decode_udp_associate_response",
        "encode_udp_associate_response",
        "encode_udp_associate_response_to_client",
    ] {
        assert!(
            socks5_shared.contains(&format!("pub(crate) fn {private_helper}("))
                && !socks5_lib.contains(private_helper),
            "SOCKS5 UDP helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
    let socks5_adapter = read("src/adapters/socks5/udp.rs");
    let socks5_packet_path = read("src/adapters/socks5/udp/packet_path.rs");
    assert!(
        !socks5_adapter.contains("socks5::udp_cache_key")
            && !socks5_adapter.contains("Socks5UdpFlowConfig::new")
            && socks5_packet_path.contains("socks5::udp::udp_packet_path_carrier_descriptor_from_config")
            && !socks5_packet_path.contains("Socks5UdpFlowConfig::new")
            && !socks5_packet_path.contains(".packet_path_spec()")
            && socks5_packet_path.contains("udp_packet_path_carrier_build_from_config")
            && socks5_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && socks5_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !socks5_packet_path.contains("descriptor.cache_key()")
            && !socks5_packet_path.contains("descriptor.server()")
            && !socks5_packet_path.contains("descriptor.port()")
            && !socks5_packet_path.contains("spec.carrier()")
            && !socks5_packet_path.contains("spec.cache_key()")
            && !socks5_packet_path.contains("spec.carrier_cache_key()")
            && !socks5_packet_path.contains("spec.association_target()")
            && !socks5_packet_path.contains("spec.carrier_build().association_target()")
            && !socks5_packet_path.contains("carrier.association_target()")
            && !socks5_packet_path.contains("into_association_target()")
            && socks5_shared.contains("pub fn packet_path_carrier_association_target")
            && socks5_shared.contains("carrier.into_association_target()")
            && !socks5_packet_path.contains(".packet_path_cache_key()")
            && !socks5_adapter.contains("Socks5UdpFlowConfig {")
            && !socks5_packet_path.contains("Socks5UdpFlowConfig {")
            && socks5_shared.contains("struct Socks5UdpFlowConfig")
            && socks5_shared.contains("pub fn new(")
            && socks5_shared.contains("pub struct Socks5UdpPacketPathSpec")
            && socks5_shared.contains("pub fn packet_path_spec(&self)")
            && socks5_shared.contains("pub fn udp_packet_path_spec_from_config(")
            && !socks5_shared.contains("pub fn carrier_cache_key(&self)")
            && !socks5_shared.contains("pub struct Socks5UdpPacketPathCarrier {")
            && !socks5_shared.contains("pub fn packet_path_cache_key(&self)")
            && !socks5_shared.contains("pub fn packet_path_association_config(&self)"),
        "SOCKS5 adapter should request packet-path cache identity through a protocol-owned config helper"
    );
    assert!(
        hysteria2_udp.contains("fn udp_cache_key(")
            && !hysteria2_udp.contains("pub fn udp_cache_key(")
            && hysteria2_udp.contains("hysteria2|")
            && hysteria2_udp.contains("pub fn packet_path_spec(&self)")
            && hysteria2_udp.contains("pub fn udp_packet_path_spec_from_config(")
            && hysteria2_udp.contains("pub struct Hysteria2UdpPacketPathSpec")
            && !hysteria2_udp.contains("pub fn carrier_cache_key(&self)")
            && !hysteria2_udp.contains("pub struct Hysteria2UdpPacketPathCarrier {")
            && !hysteria2_udp.contains("pub fn packet_path_cache_key(&self)")
            && !hysteria2_udp.contains("pub fn packet_path_codec(&self)"),
        "protocols/hysteria2 should own Hysteria2 cache identity construction internally"
    );
}

#[test]
fn adapters_do_not_construct_udp_packet_path_snapshots_directly() {
    for source in [
        "src/adapters/socks5/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        for forbidden in [
            "PacketPathCarrierDescriptor {",
            "UdpDatagramSource {",
            "UdpPacketPathCarrier::Socks5",
            "UdpPacketPathCarrier::Shadowsocks",
            "UdpPacketPathCarrier::Hysteria2",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should use runtime::udp_flow packet-path constructors instead of `{forbidden}`"
            );
        }
    }

    let snapshot = read("src/runtime/udp_flow/packet_path.rs");
    let root = read("src/runtime/udp_flow/registered/mod.rs");
    for required in ["packet_path_carrier_descriptor", "udp_datagram_source"] {
        assert!(
            snapshot.contains(required),
            "runtime::udp_flow packet-path snapshot module should own neutral constructor `{required}`"
        );
    }
    for forbidden in [
        "socks5_packet_path_carrier_descriptor",
        "shadowsocks_packet_path_carrier_descriptor",
        "shadowsocks_udp_datagram_source",
        "shadowsocks_packet_path_flow_snapshot",
        "hysteria2_packet_path_carrier_descriptor",
        "socks5::",
        "shadowsocks::",
        "hysteria2::",
        "socks5_packet_path_carrier_snapshot",
        "shadowsocks_packet_path_carrier_snapshot",
        "hysteria2_packet_path_carrier_snapshot",
        "UdpPacketPathCarrier::Socks5",
        "UdpPacketPathCarrier::Shadowsocks",
        "UdpPacketPathCarrier::Hysteria2",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "packet-path snapshot module should not retain protocol-named carrier snapshot storage `{forbidden}`"
        );
    }
    assert!(
        !snapshot.contains("ManagedUdpFlowSnapshot::shadowsocks(")
            && !snapshot.contains("ManagedUdpFlowSnapshot"),
        "packet-path snapshot helpers should not construct or name protocol flow snapshots"
    );
    assert!(
        !snapshot.contains("protocol_snapshot:"),
        "packet-path datagram source should not carry the protocol flow snapshot"
    );
    assert!(
        !snapshot.contains("ManagedUdpFlowSnapshot::Shadowsocks {"),
        "packet-path flow snapshot helper should not construct Shadowsocks snapshot fields directly"
    );
    for forbidden in [
        "pub(crate) use packet_path::{",
        "socks5_packet_path_carrier_descriptor",
        "shadowsocks_packet_path_carrier_descriptor",
        "shadowsocks_udp_datagram_source",
        "shadowsocks_packet_path_flow_snapshot",
        "hysteria2_packet_path_carrier_descriptor",
        "pub(crate) use packet_path_chain::build_shadowsocks_packet_path",
        "pub(crate) use packet_path_chain::build_hysteria2_packet_path",
    ] {
        assert!(
            !root.contains(forbidden),
            "protocol_runtime::udp root should not re-export protocol-named packet-path helper `{forbidden}`"
        );
    }
    for source in [
        "src/adapters/socks5/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("crate::runtime::udp_flow::packet_path::"),
            "{source} should call packet-path snapshot helpers through the explicit snapshot module"
        );
    }
    for source in [
        "src/adapters/shadowsocks/udp/packet_path.rs",
        "src/adapters/hysteria2/udp/packet_path.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("crate::runtime::udp_flow::packet_path_chain::"),
            "{source} should call packet-path carrier builders through the explicit chain module"
        );
    }
}

#[test]
fn shadowsocks_udp_entry_cache_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp/managed.rs");
    let generic_manager = read("src/runtime/udp_flow/managed/datagram_manager.rs");
    let entry = manifest_dir().join("src/adapters/shadowsocks/udp/manager/entry.rs");

    for forbidden in [
        "fn ensure_entry",
        "SsKey::new",
        "socket::bind_for_target",
        "BridgeWaiters::new",
        "ManagedDatagramResponseWaiters::new",
        "socket::spawn_recv_loop",
    ] {
        assert!(
            !managed.contains(forbidden),
            "Shadowsocks UDP managed glue should not own old manager entry/cache detail `{forbidden}`"
        );
    }
    assert!(
        !managed.contains("ManagedDatagramConnectionCacheKey")
            && !generic_manager.contains("ManagedDatagramConnectionCacheKey"),
        "Shadowsocks UDP managed glue should pass opaque cache identity strings to runtime cache helpers"
    );
    assert!(
        !entry.exists(),
        "Shadowsocks UDP entry/cache construction should use generic managed datagram runtime glue"
    );
    assert!(
        generic_manager.contains(".send_datagram(")
            && !managed.contains(".waiters")
            && !managed.contains("BridgeWaiters")
            && !managed.contains("impl ManagedDatagramUdpConnection")
            && !managed.contains("SsUpstream")
            && !managed.contains("self.waiters.register")
            && managed.contains("impl ManagedDatagramSender")
            && managed.contains("self.flow.send_datagram"),
        "Shadowsocks UDP managed glue should send through a neutral datagram connection while only adapting protocol flow sending"
    );
}

#[test]
fn adapters_do_not_reach_into_udp_dispatch_manager_fields() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            ".registered",
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
fn udp_adapters_use_neutral_managed_bridge_for_registered() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("protocol_parts()"),
            "{source} should ask UdpDispatch bridges to start protocol state instead of borrowing protocol_parts()"
        );
        assert!(
            !content.contains("ManagedUdpFlowSnapshot"),
            "{source} should ask UdpDispatch bridges to describe protocol UDP flow snapshots"
        );
        if !matches!(source.as_str(), "src/adapters/direct/udp.rs") {
            assert!(
                !content.contains("FlowStartResult::Flow"),
                "{source} should let UdpDispatch bridges build tracked protocol UDP flow results"
            );
        }
    }

    for (source, required) in [
        ("src/adapters/socks5/udp/flow.rs", "ManagedRelayStart"),
        (
            "src/adapters/shadowsocks/udp/flow.rs",
            "ManagedDatagramStart",
        ),
        ("src/adapters/hysteria2/udp/flow.rs", "ManagedDatagramStart"),
        (
            "src/adapters/trojan/udp/flow.rs",
            "ManagedStreamPacketStart",
        ),
        ("src/adapters/mieru/udp/flow.rs", "ManagedStreamPacketStart"),
    ] {
        let adapter = read(source);
        assert!(
            adapter.contains(required)
                && !adapter.contains("ManagedUdpSend")
                && !adapter.contains("ManagedUdpOutboundKind")
                && !adapter.contains("start_tracked_managed_udp"),
            "{source} should use the narrow neutral managed UDP start bridge `{required}` instead of owning runtime flow construction"
        );
        for forbidden in [
            "ManagedUdpFlowResume::Socks5",
            "ManagedUdpFlowResume::Shadowsocks",
            "ManagedUdpFlowResume::Hysteria2",
            "ManagedUdpFlowResume::Trojan",
            "ManagedUdpFlowResume::Mieru",
            ".start_socks5_relay_flow",
            ".start_shadowsocks_datagram_flow",
            ".start_hysteria2_datagram_flow",
            ".start_trojan_datagram_flow",
            ".start_trojan_relay_flow",
            ".start_mieru_datagram_flow",
            ".start_mieru_relay_flow",
        ] {
            assert!(
                !adapter.contains(forbidden),
                "{source} should not call removed protocol-named UdpDispatch facade `{forbidden}`"
            );
        }
    }

    for source in [
        "src/adapters/vless/udp/flow.rs",
        "src/adapters/vmess/udp/flow.rs",
    ] {
        let adapter = read(source);
        assert!(
            adapter.contains("register_managed_stream_packet_flow")
                && !adapter.contains("UdpFlowOutbound::StreamPacket")
                && !adapter.contains("register_managed_stream_flow_sender"),
            "{source} should let UdpDispatch build stream-packet UDP flow results through neutral managed flow refs"
        );
    }

    for removed in [
        "src/runtime/udp_flow/registered/hysteria2_flow.rs",
        "src/runtime/udp_flow/registered/mieru_flow.rs",
        "src/runtime/udp_flow/registered/shadowsocks_flow.rs",
        "src/runtime/udp_flow/registered/socks5_flow.rs",
        "src/runtime/udp_flow/registered/trojan_flow.rs",
        "src/runtime/udp_flow/registered/vless_flow.rs",
        "src/runtime/udp_flow/registered/vmess_flow.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "{removed} should not exist as a protocol-named UdpDispatch facade"
        );
    }

    let managed = read("src/runtime/udp_dispatch/managed.rs");
    assert!(
        managed.contains("managed_udp_chain_tasks")
            && managed.contains("register_managed_stream_flow_sender")
            && managed.contains("register_managed_stream_packet_flow")
            && !managed.contains("protocol_udp_state_and_chain_tasks"),
        "runtime UDP dispatch should expose only narrow managed stream-flow registration glue, not protocol flow bridges"
    );
    for forbidden in [
        "ManagedProtocolUdpSend",
        "ManagedProtocolUdpState",
        "send_managed_protocol_udp",
        "start_tracked_managed_protocol_udp",
        "start_managed_protocol_flow",
        "register_managed_protocol_flow",
        "managed_protocol_flow_resume",
        "forward_existing_protocol_flow",
        "protocol_udp_chain_tasks",
    ] {
        assert!(
            !managed.contains(forbidden),
            "runtime UDP managed bridge should use neutral managed UDP names, not `{forbidden}`"
        );
    }

    for (source, manager) in [
        (
            "src/adapters/vless/udp/flow.rs",
            "ManagedStreamPacketSender",
        ),
        (
            "src/adapters/vmess/udp/flow.rs",
            "ManagedStreamPacketSender",
        ),
    ] {
        let adapter = read(source);
        assert!(
            adapter.contains(manager)
                && adapter.contains("managed_udp_chain_tasks")
                && adapter.contains("register_managed_stream_packet_flow")
                && !adapter.contains("VlessUdpOutboundManager")
                && !adapter.contains("VmessUdpOutboundManager")
                && !adapter.contains("register_managed_stream_flow_sender"),
            "{source} should own managed stream UDP flow starts through the generic stream packet sender while UdpDispatch builds tracked flow results"
        );
    }
}

#[test]
fn managed_udp_flow_resumes_stay_opaque_without_snapshot_model() {
    assert!(
        !manifest_dir()
            .join("src/runtime/udp_flow/registered/flow_snapshot.rs")
            .exists(),
        "managed UDP flow resume state should live under runtime::udp_flow, not protocol_runtime::udp"
    );

    let snapshot = read("src/runtime/udp_flow/managed/flow.rs");

    for forbidden in [
        "ManagedUdpFlowSnapshot",
        "pub(crate) fn managed(",
        "pub(crate) fn resume(",
        "pub(crate) fn shadowsocks(",
        "pub(crate) fn hysteria2(",
        "pub(crate) fn trojan(",
        "pub(crate) fn mieru(",
        "pub(crate) fn socks5(",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "runtime::udp_flow::managed should not keep snapshot constructors or protocol-specific resume accessors `{forbidden}`"
        );
    }
    assert!(
        snapshot.contains("inner: Arc<dyn ManagedUdpFlowResumeObject>")
            && snapshot.contains("pub(crate) fn new<T>(")
            && snapshot.contains("pub(crate) fn as_ref<T>(")
            && snapshot.contains("pub(crate) fn cloned<T>(")
            && !snapshot.contains("socks5::")
            && !snapshot.contains("shadowsocks::")
            && !snapshot.contains("hysteria2::")
            && !snapshot.contains("trojan::")
            && !snapshot.contains("mieru::")
            && !snapshot.contains("Socks5(socks5::udp::Socks5UdpFlowResume)"),
        "managed UDP state should use the unified opaque ManagedUdpFlowResume wrapper directly"
    );
}

#[test]
fn udp_dispatch_does_not_unpack_protocol_flow_resume() {
    let managed = read("src/runtime/udp_dispatch/managed.rs");
    for source in [
        "src/runtime/udp_dispatch/managed.rs",
        "src/adapters/socks5/udp/flow.rs",
        "src/adapters/hysteria2/udp/flow.rs",
        "src/adapters/mieru/udp/flow.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/trojan/udp/flow.rs",
    ] {
        let content = read(source);
        for forbidden in [
            ".shadowsocks()",
            ".hysteria2()",
            ".trojan()",
            ".mieru()",
            "resume.cache_key()",
            "resume.username()",
            "resume.password()",
            "resume.codec()",
            "codec: std::sync::Arc<dyn DatagramCodec",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should pass ManagedUdpFlowResume through without unpacking `{forbidden}`"
            );
        }
    }
    assert!(
        managed.contains("resume: ManagedUdpFlowResume")
            && managed.contains("resume: request.resume"),
        "managed UDP bridge should carry ManagedUdpFlowResume without unpacking protocol internals"
    );
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
            "crate::runtime::udp_dispatch::TrojanUdpRelayFlowRequest",
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

#[test]
fn managed_udp_resume_variants_are_confined_to_managed_flow_model() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        if source == "src/runtime/udp_flow/managed/flow.rs" {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "ManagedUdpFlowResume::Socks5",
            "ManagedUdpFlowResume::Shadowsocks",
            "ManagedUdpFlowResume::Hysteria2",
            "ManagedUdpFlowResume::Trojan",
            "ManagedUdpFlowResume::Mieru",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should use ManagedUdpFlowResume constructors/accessors instead of matching variant `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_crate_roots_do_not_reexport_udp_manager_internals() {
    for (protocol, forbidden) in [
        (
            "vless",
            &[
                "VlessUdpFlowConfig",
                "VlessUdpFlowConnection",
                "VlessUdpFlowHandle",
                "VlessUdpFlowIo",
                "VlessUdpFlowPacket",
                "VlessUdpPacketV2Codec",
                "VlessUdpPacketTarget",
                "VlessUdpPacketTunnelTarget",
            ][..],
        ),
        (
            "vmess",
            &[
                "VmessUdpFlowConfig",
                "VmessUdpFlowConnection",
                "VmessUdpFlowHandle",
                "VmessUdpFlowIo",
                "VmessUdpFlowPacket",
                "VmessUdpPacketTarget",
                "VmessUdpPacketTunnelTarget",
            ][..],
        ),
        (
            "hysteria2",
            &[
                "Hysteria2DatagramCodec",
                "Hysteria2InboundUdpCodec",
                "Hysteria2InboundUdpDispatchParts",
                "Hysteria2InboundUdpRequest",
                "Hysteria2InboundUdpSession",
                "Hysteria2InitialUdpFlowPacket",
                "Hysteria2UdpConnectorFlow",
                "Hysteria2UdpConnectorProfile",
                "Hysteria2UdpFlowConfig,",
                "Hysteria2UdpFlowConnection",
                "Hysteria2UdpFlowHandle",
                "Hysteria2UdpFlowIo,",
                "Hysteria2UdpFlowPacket,",
                "Hysteria2UdpFlowResponse",
                "Hysteria2UdpFlowResponseReceiver",
                "Hysteria2UdpFlowResume",
                "Hysteria2UdpFlowSession",
                "Hysteria2UdpFlowSessions",
                "Hysteria2UdpFlowStore,",
                "Hysteria2UdpPacket,",
                "Hysteria2UdpPacketPathCarrierBuild",
                "Hysteria2UdpPacketPathCarrierBuildParts",
                "Hysteria2UdpPacketPathCarrierDescriptor",
                "Hysteria2UdpPacketPathSpec",
                "Hysteria2UdpPacketTarget,",
                "connector_flow_from_resume",
                "spawn_udp_flow",
                "start_udp_flow_with_initial_packet",
                "udp_flow_resume_from_config",
                "udp_packet_path_carrier_build_from_config",
                "udp_packet_path_carrier_descriptor_from_config",
                "udp_packet_path_spec_from_config",
            ][..],
        ),
        (
            "shadowsocks",
            &[
                "ShadowsocksDatagramCodec",
                "ShadowsocksInboundUdpCodec",
                "ShadowsocksInboundUdpDispatchParts",
                "ShadowsocksInboundUdpPacket",
                "ShadowsocksInboundUdpResponse",
                "ShadowsocksInboundUdpResponseTarget",
                "ShadowsocksInboundUdpSession",
                "ShadowsocksUdpDecodeContext",
                "ShadowsocksUdpFlowConfig",
                "ShadowsocksUdpFlowEntries",
                "ShadowsocksUdpFlowPacket",
                "ShadowsocksUdpFlowResume",
                "ShadowsocksUdpFlowStore",
                "ShadowsocksUdpLeafKey",
                "ShadowsocksUdpPacket",
                "ShadowsocksUdpPacketPathCarrierBuild",
                "ShadowsocksUdpPacketPathCarrierDescriptor",
                "ShadowsocksUdpPacketPathDatagramSourceBuild",
                "ShadowsocksUdpPacketPathSpec",
                "ShadowsocksUdpPacketTarget",
                "ShadowsocksUdpSocketFlowSpec",
                "managed_socket_flow_from_resume",
                "parse_udp_cipher",
                "udp_flow_resume_from_config",
                "udp_packet_path_carrier_descriptor_from_config",
                "udp_packet_path_datagram_source_build_from_config",
                "udp_packet_path_spec_from_config",
            ][..],
        ),
        (
            "socks5",
            &[
                "Socks5EstablishedUdpAssociation",
                "Socks5InboundUdpCodec",
                "Socks5InboundUdpDispatchParts",
                "Socks5InboundUdpRequest",
                "Socks5InboundUdpResponse",
                "Socks5InboundUdpResponseFrame",
                "Socks5InboundUdpResponseKey",
                "Socks5InboundUdpSession",
                "Socks5UdpAssociateRequest",
                "Socks5OwnedUdpAssociationConfig",
                "Socks5UdpAssociation",
                "Socks5UdpAssociationConfig",
                "Socks5UdpAssociationEndpoint",
                "Socks5UdpAssociationIdentity",
                "Socks5UdpAssociationSend",
                "Socks5UdpAssociationTarget",
                "Socks5UdpFlowConfig",
                "Socks5UdpFlowResume",
                "Socks5UdpFlowSpec",
                "Socks5UdpPacketPathCarrierBuild",
                "Socks5UdpPacketPathCarrierDescriptor",
                "Socks5UdpPacketPathSpec",
                "Socks5UdpRelay",
                "Socks5UdpRelayEndpoint",
                "Socks5UdpRelayError",
                "Socks5UdpRelayTargetAddress",
                "establish_udp_relay_with_control",
                "packet_path_carrier_association_target",
                "udp_flow_resume_from_config",
                "udp_packet_path_carrier_build_from_config",
                "udp_packet_path_carrier_descriptor_from_config",
                "udp_packet_path_spec_from_config",
            ][..],
        ),
        (
            "trojan",
            &[
                "TrojanInboundUdpCodec",
                "TrojanInboundUdpDispatchParts",
                "TrojanInboundUdpRequest",
                "TrojanInboundUdpSession",
                "TrojanUdpConnectorFlow",
                "TrojanUdpFlowConfig",
                "TrojanUdpFlowConnection",
                "TrojanUdpFlowHandle",
                "TrojanUdpFlowIo",
                "TrojanUdpFlowResponseReceiver",
                "TrojanUdpFlowResume",
                "TrojanUdpFlowSession",
                "TrojanUdpFlowSessions",
                "TrojanUdpFlowStore",
                "TrojanUdpPacket",
                "TrojanUdpPacketTunnelTarget",
                "TrojanUdpTlsProfile",
                "TrojanUdpTlsProfileSpec",
                "build_udp_request",
                "connector_flow_from_resume",
                "establish_udp_flow_with_resume",
                "establish_udp_packet_tunnel",
                "spawn_udp_flow",
                "udp_flow_resume_from_config",
            ][..],
        ),
        (
            "mieru",
            &[
                "MieruInboundUdpPacket",
                "MieruInboundUdpRequest",
                "MieruInboundUdpSession",
                "MieruUdpAssociatePacket",
                "MieruUdpAssociatePayload",
                "MieruUdpConnectorFlow",
                "MieruUdpFlowCodec",
                "MieruUdpFlowConfig",
                "MieruUdpFlowConnection",
                "MieruUdpFlowHandle",
                "MieruUdpFlowIo",
                "MieruUdpFlowPacket",
                "MieruUdpFlowResponse",
                "MieruUdpFlowResponseReceiver",
                "MieruUdpFlowResume",
                "MieruUdpFlowSession",
                "MieruUdpFlowSessions",
                "MieruUdpFlowStore",
                "connector_flow_from_resume",
                "establish_udp_flow_with_resume",
                "spawn_udp_flow",
                "udp_flow_resume_from_config",
            ][..],
        ),
    ] {
        let protocol_lib =
            fs::read_to_string(repo_root().join(format!("protocols/{protocol}/src/lib.rs")))
                .unwrap_or_else(|error| panic!("read protocols/{protocol}/src/lib.rs: {error}"));
        for forbidden_item in forbidden {
            assert!(
                !protocol_lib.contains(forbidden_item),
                "protocols/{protocol} crate root should not re-export UDP manager/internal model `{forbidden_item}`"
            );
        }
    }
}
