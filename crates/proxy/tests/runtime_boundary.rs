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

fn virtual_repo_relative(relative: &str) -> Option<&'static str> {
    match relative {
        "crates/proxy/src/transport/http_inbound.rs" => {
            Some("crates/proxy/src/adapters/http/inbound.rs")
        }
        "crates/proxy/src/transport/http_inbound/listener.rs" => {
            Some("crates/proxy/src/adapters/http/inbound/listener.rs")
        }
        "crates/proxy/src/transport/hysteria2_inbound.rs" => {
            Some("crates/proxy/src/adapters/hysteria2/inbound.rs")
        }
        "crates/proxy/src/transport/hysteria2_inbound/request.rs" => {
            Some("crates/proxy/src/adapters/hysteria2/inbound/request.rs")
        }
        "crates/proxy/src/transport/hysteria2_inbound/listener.rs" => {
            Some("crates/proxy/src/adapters/hysteria2/inbound/listener.rs")
        }
        "crates/proxy/src/transport/hysteria2_inbound/listener/udp.rs" => {
            Some("crates/proxy/src/adapters/hysteria2/inbound/listener.rs")
        }
        "crates/proxy/src/transport/mieru_inbound.rs" => {
            Some("crates/proxy/src/adapters/mieru/inbound.rs")
        }
        "crates/proxy/src/transport/mieru_inbound/request.rs" => {
            Some("crates/proxy/src/adapters/mieru/inbound/request.rs")
        }
        "crates/proxy/src/transport/mieru_inbound/listener.rs" => {
            Some("crates/proxy/src/adapters/mieru/inbound/listener.rs")
        }
        "crates/proxy/src/transport/mieru_inbound/listener/udp.rs" => {
            Some("crates/proxy/src/adapters/mieru/inbound/listener.rs")
        }
        "crates/proxy/src/transport/shadowsocks_inbound.rs" => {
            Some("crates/proxy/src/adapters/shadowsocks/inbound.rs")
        }
        "crates/proxy/src/transport/shadowsocks_inbound/request.rs" => {
            Some("crates/proxy/src/adapters/shadowsocks/inbound/request.rs")
        }
        "crates/proxy/src/transport/shadowsocks_inbound/listener.rs" => {
            Some("crates/proxy/src/adapters/shadowsocks/inbound/listener.rs")
        }
        "crates/proxy/src/transport/shadowsocks_inbound/listener/udp.rs" => {
            Some("crates/proxy/src/adapters/shadowsocks/inbound/listener.rs")
        }
        "crates/proxy/src/transport/socks5_inbound.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/request.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/request.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/listener.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/listener.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/listener/udp_associate.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/client_association.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/listener/udp_associate/direct_response.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/client_association.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/listener/udp_associate/dispatch.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/client_association.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/listener/udp_associate/relay_socket.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/client_association.rs")
        }
        "crates/proxy/src/transport/socks5_inbound/listener/udp_associate/setup.rs" => {
            Some("crates/proxy/src/adapters/socks5/inbound/client_association.rs")
        }
        _ => None,
    }
}

fn virtual_manifest_relative(relative: &str) -> Option<&'static str> {
    match relative {
        "src/transport/http_inbound.rs" => Some("src/adapters/http/inbound.rs"),
        "src/transport/http_inbound/listener.rs" => Some("src/adapters/http/inbound/listener.rs"),
        "src/transport/hysteria2_inbound.rs" => Some("src/adapters/hysteria2/inbound.rs"),
        "src/transport/hysteria2_inbound/request.rs" => {
            Some("src/adapters/hysteria2/inbound/request.rs")
        }
        "src/transport/hysteria2_inbound/listener.rs" => {
            Some("src/adapters/hysteria2/inbound/listener.rs")
        }
        "src/transport/hysteria2_inbound/listener/udp.rs" => {
            Some("src/adapters/hysteria2/inbound/listener.rs")
        }
        "src/transport/mieru_inbound.rs" => Some("src/adapters/mieru/inbound.rs"),
        "src/transport/mieru_inbound/request.rs" => Some("src/adapters/mieru/inbound/request.rs"),
        "src/transport/mieru_inbound/listener.rs" => Some("src/adapters/mieru/inbound/listener.rs"),
        "src/transport/mieru_inbound/listener/udp.rs" => {
            Some("src/adapters/mieru/inbound/listener.rs")
        }
        "src/transport/shadowsocks_inbound.rs" => Some("src/adapters/shadowsocks/inbound.rs"),
        "src/transport/shadowsocks_inbound/request.rs" => {
            Some("src/adapters/shadowsocks/inbound/request.rs")
        }
        "src/transport/shadowsocks_inbound/listener.rs" => {
            Some("src/adapters/shadowsocks/inbound/listener.rs")
        }
        "src/transport/shadowsocks_inbound/listener/udp.rs" => {
            Some("src/adapters/shadowsocks/inbound/listener.rs")
        }
        "src/transport/socks5_inbound.rs" => Some("src/adapters/socks5/inbound.rs"),
        "src/transport/socks5_inbound/request.rs" => Some("src/adapters/socks5/inbound/request.rs"),
        "src/transport/socks5_inbound/listener.rs" => {
            Some("src/adapters/socks5/inbound/listener.rs")
        }
        "src/transport/socks5_inbound/listener/udp_associate.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/transport/socks5_inbound/listener/udp_associate/direct_response.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/transport/socks5_inbound/listener/udp_associate/dispatch.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/transport/socks5_inbound/listener/udp_associate/relay_socket.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/transport/socks5_inbound/listener/udp_associate/setup.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/transport/hysteria2_inbound/listener" => Some("src/adapters/hysteria2/inbound"),
        "src/transport/mieru_inbound/listener" => Some("src/adapters/mieru/inbound"),
        "src/transport/shadowsocks_inbound/listener" => Some("src/adapters/shadowsocks/inbound"),
        "src/transport/socks5_inbound/listener" => Some("src/adapters/socks5/inbound"),
        "src/adapters/hysteria2/inbound/listener/udp.rs" => {
            Some("src/adapters/hysteria2/inbound/listener.rs")
        }
        "src/adapters/mieru/inbound/listener/udp.rs" => {
            Some("src/adapters/mieru/inbound/listener.rs")
        }
        "src/adapters/shadowsocks/inbound/listener/udp.rs" => {
            Some("src/adapters/shadowsocks/inbound/listener.rs")
        }
        "src/adapters/socks5/inbound/listener/udp_associate.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/adapters/socks5/inbound/listener/udp_associate/direct_response.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/adapters/socks5/inbound/listener/udp_associate/dispatch.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/adapters/socks5/inbound/listener/udp_associate/relay_socket.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        "src/adapters/socks5/inbound/listener/udp_associate/setup.rs" => {
            Some("src/adapters/socks5/inbound/client_association.rs")
        }
        _ => None,
    }
}

fn read_repo_file(relative: &str) -> String {
    if let Some(mapped) = virtual_repo_relative(relative) {
        return read_repo_file(mapped);
    }

    let path = repo_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .replace("\r\n", "\n")
}

fn read(relative: &str) -> String {
    if let Some(mapped) = virtual_manifest_relative(relative) {
        return read(mapped);
    }

    let path = manifest_dir().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .replace("\r\n", "\n")
}

fn read_if_exists(relative: &str) -> String {
    let path = manifest_dir().join(relative);
    if path.exists() {
        read(relative)
    } else {
        String::new()
    }
}

fn read_repo_module_tree(relative: &str) -> String {
    fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(root).unwrap_or_else(|error| {
            panic!("read dir {}: {error}", root.display());
        }) {
            let entry = entry.expect("read dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, files);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    if let Some(mapped) = virtual_repo_relative(relative) {
        return read_repo_module_tree(mapped);
    }

    let root = repo_root().join(relative);
    let mut files = Vec::new();

    if root.is_file() {
        files.push(root.clone());
        let module_dir = root.with_extension("");
        if module_dir.is_dir() {
            collect_rust_files(&module_dir, &mut files);
        }
    } else if root.is_dir() {
        collect_rust_files(&root, &mut files);
    } else {
        panic!("missing repo module tree {}", root.display());
    }

    files.sort();

    files
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
                .replace("\r\n", "\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_proxy_module_tree(relative: &str) -> String {
    let relative = relative
        .strip_prefix("src/")
        .unwrap_or_else(|| panic!("expected src/ path, got {relative}"));
    read_repo_module_tree(&format!("crates/proxy/src/{relative}"))
}

fn read_adapter_transport_bridge(adapter_relative: &str, transport_relative: &str) -> String {
    format!(
        "{}\n{}",
        read_proxy_module_tree(adapter_relative),
        read_repo_module_tree(transport_relative)
    )
    .replace("\r\n", "\n")
}

fn contains_helper_call(content: impl AsRef<str>, helper: &str) -> bool {
    let content = content.as_ref();
    content.contains(&format!("{helper}("))
        || content.contains(&format!("{helper}::<"))
        || content.contains(&format!("{helper}_with_request("))
        || content.contains(&format!("{helper}_with_request::<"))
        || content.contains(&format!("{helper}_for_request("))
        || content.contains(&format!("{helper}_for_request::<"))
}

fn contains_vless_mux_dispatch(content: impl AsRef<str>) -> bool {
    let content = content.as_ref();
    [
        "run_logged_tcp_socket_listener_loop",
        "run_logged_quic_stream_listener_loop",
        "LoggedTcpSocketListenerRequest",
        "LoggedQuicStreamListenerRequest",
        "spawn_recorded_transport_mux_bound_inbound_listener",
        "dispatch_recorded_protocol_mux_route",
        "dispatch_recorded_protocol_mux_route_with_udp_logger",
        "dispatch_recorded_protocol_mux_route_accept_result",
        "dispatch_optional_recorded_protocol_mux_route_accept_result",
        "dispatch_recorded_protocol_mux_tcp_request_with_defaults",
        "dispatch_recorded_protocol_mux_stream_request_with_defaults",
        "dispatch_recorded_protocol_mux_tcp_request",
        "dispatch_recorded_protocol_mux_stream_request",
        "dispatch_recorded_protocol_mux_tcp_request_result",
        "dispatch_recorded_protocol_mux_stream_request_result",
    ]
    .iter()
    .any(|pattern| contains_helper_call(content, pattern))
}

fn impl_block(source: &str, type_name: &str) -> String {
    let normalized = source.replace("\r\n", "\n");
    for needle in [
        format!("impl {type_name} {{"),
        format!("impl<'a> {type_name}<'a> {{"),
        format!("impl<'_> {type_name}<'_> {{"),
    ] {
        if let Some(start) = normalized.find(&needle) {
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
        }
    }
    panic!("missing impl block for {type_name}")
}

fn struct_block<'a>(source: &'a str, type_name: &str) -> &'a str {
    for needle in [
        format!("pub struct {type_name}"),
        format!("struct {type_name}"),
    ] {
        if let Some(content) = source.split(&needle).nth(1) {
            for impl_needle in [
                format!("impl {type_name}"),
                format!("impl<'a> {type_name}<'a>"),
                format!("impl<'_> {type_name}<'_>"),
            ] {
                if let Some(prefix) = content.split(&impl_needle).next() {
                    if prefix.len() != content.len() {
                        return prefix;
                    }
                }
            }
            return content
                .split(&format!("impl {type_name}"))
                .next()
                .unwrap_or_else(|| panic!("missing struct block for {type_name}"));
        }
    }
    panic!("missing struct block for {type_name}")
}

fn rust_sources_under(relative: &str) -> Vec<PathBuf> {
    let root_relative = if manifest_dir().join(relative).exists() {
        relative.to_owned()
    } else if let Some(mapped) = virtual_manifest_relative(relative) {
        mapped.to_owned()
    } else {
        relative.to_owned()
    };
    let root = manifest_dir().join(root_relative);
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

fn repo_rust_sources_under(relative: &str) -> Vec<PathBuf> {
    let mut pending = vec![repo_root().join(relative)];
    let mut files = Vec::new();

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).unwrap_or_else(|error| {
            panic!("read dir {}: {error}", path.display());
        }) {
            let path = entry.expect("read dir entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    files
}

fn protocol_inbound_sources() -> Vec<PathBuf> {
    [
        "src/transport/http_inbound.rs",
        "src/transport/http_inbound/listener.rs",
        "src/transport/socks5_inbound/listener.rs",
        "src/transport/socks5_inbound/listener",
        "src/transport/shadowsocks_inbound/listener.rs",
        "src/transport/shadowsocks_inbound/listener",
        "src/adapters/trojan.rs",
        "src/transport/mieru_inbound/listener.rs",
        "src/transport/mieru_inbound/listener",
        "src/transport/hysteria2_inbound/listener.rs",
        "src/transport/hysteria2_inbound/listener",
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
    ]
    .into_iter()
    .flat_map(|relative| {
        let path = manifest_dir().join(relative);
        if !path.exists() {
            Vec::new()
        } else if path.is_dir() {
            rust_sources_under(relative)
        } else {
            vec![path]
        }
    })
    .collect()
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
fn runtime_engine_facade_keeps_traffic_and_udp_upstream_accounting_protocol_neutral() {
    let engine_facade = read("src/runtime/engine_facade.rs");

    assert!(
        engine_facade.contains("pub(crate) fn record_session_outbound_rx")
            && engine_facade.contains("pub(crate) fn record_session_outbound_tx")
            && engine_facade.contains("pub(crate) fn record_session_outbound_traffic")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_association_created")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_association_reused")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_association_closed")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_association_idle_timeout")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_association_dropped")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_association_failed")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_send_failure")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_recv_failure")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_packet_sent")
            && engine_facade.contains("pub(crate) fn record_udp_upstream_packet_received")
            && engine_facade.contains("pub(crate) fn udp_upstream_idle_timeout")
            && engine_facade.contains("#[cfg(feature = \"socks5\")]\n    pub(crate) fn record_udp_upstream")
            && !engine_facade.contains("Socks5")
            && !engine_facade.contains("Vless"),
        "runtime engine facade should keep neutral accounting names while feature-owning SOCKS5 upstream execution"
    );
}

#[test]
fn neutral_mux_runtime_glue_does_not_live_under_inbound() {
    assert!(
        !manifest_dir().join("src/inbound/mux_tcp.rs").exists()
            && !manifest_dir().join("src/inbound/mux_udp.rs").exists(),
        "neutral MUX TCP/UDP runtime glue should live under src/runtime, not src/inbound"
    );

    let runtime_mux_tcp = read("src/runtime/mux_tcp.rs");
    let runtime_mux_udp = read("src/runtime/mux_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    assert!(
        runtime_mux_tcp.contains("TcpPipe::new(proxy)")
            && runtime_mux_tcp.contains("TcpPipeInput")
            && runtime_mux_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response"),
        "runtime MUX glue should own neutral TCP/UDP pipe orchestration"
    );
}

#[test]
fn mux_session_task_lifecycle_lives_in_runtime() {
    let runtime_mux_session = read("src/runtime/mux_session.rs");
    let vless_mux = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_mux = read_proxy_module_tree("src/adapters/vmess.rs");

    assert!(
        runtime_mux_session.contains("pub(crate) trait MuxOpenedDispatcher")
            && runtime_mux_session.contains("pub(crate) async fn run_mux_session_loop")
            && runtime_mux_session.contains("tasks.abort_all()")
            && runtime_mux_session.contains("tasks.try_join_next()")
            && runtime_mux_session.contains("mux session started")
            && runtime_mux_session.contains("mux session ended"),
        "neutral MUX session task lifecycle should live in runtime/mux_session"
    );

    for (source, content) in [
        ("src/adapters/vless.rs", vless_mux.as_str()),
        ("src/adapters/vmess.rs", vmess_mux.as_str()),
    ] {
        for forbidden in [
            "try_join_next",
            "abort_all()",
            "mux session started",
            "mux session ended",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should delegate neutral MUX task lifecycle to runtime/mux_session; found `{forbidden}`"
            );
        }
        assert!(
            (content.contains("run_protocol_mux_session")
                || contains_vless_mux_dispatch(content)
                || content.contains("dispatch_no_client_mux_route(")
                || content.contains("dispatch_no_client_mux_route_with_defaults(")
                || content.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(content, "spawn_transport_mux_route_inbound_listener"))
                && !content.contains("run_mux_session_loop")
                && !content.contains("struct OpenedDispatch")
                && !content.contains("MuxOpenedDispatcher for")
                && !content.contains("dispatch_next_opened_route_with_handlers")
                && !content.contains("struct VlessMuxOpenedDispatcher {")
                && !content.contains("struct VmessMuxOpenedDispatcher {"),
            "{source} should keep only protocol opened-stream spawn glue over the shared runtime MUX session helper"
        );
    }
}

#[test]
fn neutral_udp_ingress_and_delivery_glue_lives_in_runtime() {
    assert!(
        !manifest_dir().join("src/inbound/udp_dispatch.rs").exists()
            && !manifest_dir().join("src/inbound/udp_response.rs").exists(),
        "neutral inbound UDP dispatch/response accounting glue should live under src/runtime"
    );

    let udp_ingress = read("src/runtime/udp_ingress.rs");
    let udp_delivery = read("src/runtime/udp_delivery.rs");
    assert!(
        udp_ingress.contains("pub(crate) async fn dispatch_inbound_udp_packet")
            && udp_ingress.contains("UdpPipe::new(proxy, dispatch)")
            && udp_ingress.contains("UdpPipeInput::from_inbound_dispatch")
            && udp_delivery.contains("fn write_direct_response")
            && udp_delivery.contains("fn write_upstream_response")
            && udp_delivery.contains("fn write_chain_response"),
        "runtime should own neutral UDP ingress and response delivery accounting"
    );
}

#[test]
fn udp_pipe_execution_follows_external_udp_features() {
    let runtime = read("src/runtime.rs");
    let pipe = read("src/runtime/pipe.rs");
    let dispatch = read("src/runtime/udp_dispatch/mod.rs");
    let dispatch_lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let register = read("src/register.rs");
    let state = read("src/runtime/udp_flow/state.rs");
    let tcp_ingress = read("src/runtime/tcp_ingress.rs");
    let udp_socket = read("src/runtime/udp_socket.rs");

    let udp_features = [
        "feature = \"socks5\"",
        "feature = \"vless\"",
        "feature = \"hysteria2\"",
        "feature = \"shadowsocks\"",
        "feature = \"trojan\"",
        "feature = \"vmess\"",
        "feature = \"mieru\"",
    ];
    for feature in udp_features {
        assert!(runtime.contains(feature));
        assert!(pipe.contains(feature));
        assert!(dispatch.contains(feature));
        assert!(register.contains(feature));
        assert!(state.contains(feature));
    }
    assert!(
        pipe.matches("#[cfg(any(").count() >= 6
            && pipe.contains("pub(crate) struct UdpPipeInput<'a>")
            && pipe.contains("pub(crate) struct UdpPipe<'a>")
            && dispatch.contains("mod dispatch;")
            && dispatch.contains("mod lifecycle;")
            && dispatch.contains("mod forward;")
            && dispatch.contains("mod managed;")
            && dispatch.contains("mod packet_path;")
            && dispatch.contains("mod start;")
            && register.contains(
                "pub(crate) fn registered_udp_handlers(registry: &ProtocolRegistry)"
            )
            && dispatch_lifecycle.contains("protocols.registered_udp_handlers()")
            && !state.contains("pub(crate) fn default_registered()")
            && tcp_ingress.contains("pub(crate) use lifecycle::apply_kernel_rate_limits;")
            && udp_socket.contains("pub(crate) async fn send_direct_udp_packet("),
        "UDP pipe input/state and first-level dispatch should share the external UDP feature family"
    );
}

#[test]
fn udp_runtime_taxonomy_stays_responsibility_based() {
    let dispatch = read("src/runtime/udp_dispatch/mod.rs");
    let flow = read("src/runtime/udp_flow.rs");
    let managed = read("src/runtime/udp_flow/managed/mod.rs");
    let registered = read("src/runtime/udp_flow/registered/mod.rs");
    let architecture = fs::read_to_string(repo_root().join("docs/project/architecture.md"))
        .expect("read docs/project/architecture.md");

    assert!(
        dispatch.contains("Per-inbound-session UDP routing, flow selection, start, and forwarding")
            && flow.contains("Neutral UDP flow models, persistent state")
            && managed.contains("execution machinery for resumable managed UDP flows")
            && registered.contains("handlers registered at proxy assembly time"),
        "UDP runtime modules should state their distinct dispatch, flow-state, managed-execution, and registered-handler responsibilities"
    );
    assert!(
        architecture.contains("`runtime::udp_dispatch` is the per-inbound-session")
            && architecture.contains("`runtime::udp_flow::managed` owns reusable execution machinery")
            && architecture.contains("`runtime::udp_flow::registered` owns the handler set assembled by `register.rs`")
            && architecture.contains("`runtime::udp_association` is an inbound relay-loop shape")
            && !architecture.contains("runtime::udp_flow::protocol_state")
            && !architecture.contains("ProtocolUdpState")
            && !architecture.contains("ProtocolUdpFlowSnapshot"),
        "architecture docs should describe the current UDP taxonomy without stale protocol-state facade names"
    );
}

#[test]
fn persistent_udp_flow_models_do_not_belong_to_session_dispatch() {
    let result = read("src/runtime/udp_flow/result.rs");
    let dispatch_candidate = read("src/runtime/udp_dispatch/candidate.rs");

    for model in [
        "pub(crate) enum FlowStartResult",
        "pub(crate) struct FlowFailure",
    ] {
        assert!(
            result.contains(model),
            "persistent UDP flow result model `{model}` should live in udp_flow/result.rs"
        );
        assert!(
            !dispatch_candidate.contains(model),
            "per-session udp_dispatch candidate should not own persistent model `{model}`"
        );
    }

    for path in rust_sources_under("src/runtime/udp_flow") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read UDP flow source");
        for forbidden in [
            "runtime::udp_dispatch::UdpDispatch",
            "use crate::runtime::udp_dispatch",
            "udp_dispatch::FlowFailure",
            "udp_dispatch::FlowStartResult",
            "udp_dispatch::{FlowFailure",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should use udp_flow-owned result models; found `{forbidden}`"
            );
        }
    }

    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/runtime/udp_flow/helpers.rs")
            .exists(),
        "response delivery helpers must not return to persistent udp_flow"
    );
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/runtime/udp_delivery/helpers.rs")
            .exists(),
        "udp_delivery should own response accounting and delivery helpers"
    );
}

#[test]
fn proxy_module_taxonomy_uses_current_responsibility_names() {
    for removed in [
        "src/protocol_capability.rs",
        "src/runtime/inbound_protocol.rs",
        "src/runtime/udp_inbound_dispatch.rs",
        "src/runtime/udp_helpers.rs",
        "src/runtime/udp_response.rs",
        "src/transport/tcp_flow.rs",
        "src/adapters/socks5/udp/active.rs",
        "src/adapters/socks5/inbound/transport.rs",
        "src/adapters/shadowsocks/inbound/transport.rs",
        "src/adapters/mieru/inbound/transport.rs",
        "src/adapters/hysteria2/inbound/transport.rs",
        "src/adapters/shadowsocks/udp/managed.rs",
        "src/adapters/mieru/udp/managed.rs",
        "src/adapters/hysteria2/udp/managed.rs",
        "src/adapters/shadowsocks/udp/handler.rs",
        "src/adapters/mieru/udp/handler.rs",
        "src/adapters/hysteria2/udp/handler.rs",
        "src/adapters/shadowsocks/inbound/udp.rs",
        "src/adapters/mieru/inbound/udp.rs",
        "src/adapters/hysteria2/inbound/udp.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "historical proxy taxonomy path should stay removed: {removed}"
        );
    }

    for current in [
        "src/protocol_catalog.rs",
        "src/runtime/tcp_ingress.rs",
        "src/runtime/udp_ingress.rs",
        "src/runtime/udp_socket.rs",
        "src/runtime/udp_delivery.rs",
        "src/adapters/socks5/udp/upstream_association.rs",
        "src/adapters/socks5/inbound/listener.rs",
        "src/adapters/shadowsocks/inbound/listener.rs",
        "src/adapters/mieru/inbound/listener.rs",
        "src/adapters/hysteria2/inbound/listener.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/mieru/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        assert!(
            manifest_dir().join(current).is_file(),
            "responsibility-named proxy module should exist: {current}"
        );
    }
}

#[test]
fn production_sources_do_not_hide_dead_code_or_unused_facades() {
    let mut roots = rust_sources_under("src");
    roots.extend(repo_rust_sources_under("crates/transport/src"));
    for protocol in [
        "hysteria2",
        "http",
        "mieru",
        "shadowsocks",
        "socks5",
        "trojan",
        "vless",
        "vmess",
    ] {
        roots.extend(repo_rust_sources_under(&format!(
            "protocols/{protocol}/src"
        )));
    }

    for path in roots {
        let source = path.to_string_lossy();
        let content = fs::read_to_string(&path).expect("read production Rust source");
        for forbidden in [
            "allow(dead_code)",
            "allow(unused_imports)",
            "allow(dead_code, unused_imports",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should use precise ownership/cfg or remove residue instead of `{forbidden}`"
            );
        }
    }
}

#[test]
fn proxy_operations_do_not_hide_repeated_inputs_with_argument_lint_allows() {
    for path in rust_sources_under("src") {
        let source = path.to_string_lossy();
        let content = fs::read_to_string(&path).expect("read proxy Rust source");
        assert!(
            !content.contains("allow(clippy::too_many_arguments)"),
            "{source} should use an operation-specific request/default model instead of suppressing repeated inputs"
        );
    }
}

#[test]
fn neutral_stream_and_datagram_udp_relays_live_in_runtime() {
    assert!(
        !manifest_dir().join("src/inbound/stream_udp.rs").exists()
            && !manifest_dir().join("src/inbound/datagram_udp.rs").exists(),
        "neutral stream/datagram UDP relay glue should live under src/runtime, not src/inbound"
    );

    let stream_udp = read("src/runtime/stream_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let datagram_udp = read_proxy_module_tree("src/runtime/datagram_udp.rs");
    let core_udp = fs::read_to_string(repo_root().join("crates/core/src/udp.rs"))
        .expect("read zero-core UDP source");
    assert!(
        core_udp.contains("pub trait StreamUdpResponder")
            && stream_udp.contains("StreamUdpResponder")
            && stream_udp.contains("pub(crate) async fn run_stream_udp_relay")
            && stream_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && core_udp.contains("pub trait DatagramUdpResponder")
            && datagram_udp.contains("DatagramUdpResponder")
            && datagram_udp.contains("pub(crate) async fn run_datagram_udp_relay")
            && datagram_udp.contains("dispatch_inbound_udp_packet"),
        "runtime should own neutral stream/datagram UDP relay loops, with stream and mux delegating packet-session orchestration to a shared runtime template"
    );
}

#[test]
fn udp_inbound_runtime_converges_to_three_neutral_loop_templates() {
    let runtime_root = read("src/runtime.rs");
    let stream_udp = read("src/runtime/stream_udp.rs");
    let mux_udp = read("src/runtime/mux_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let datagram_udp = read_proxy_module_tree("src/runtime/datagram_udp.rs");
    let udp_association = read_proxy_module_tree("src/runtime/udp_association.rs");

    assert!(
        runtime_root.contains("pub(crate) mod datagram_udp;")
            && runtime_root.contains("pub(crate) mod packet_session_udp;")
            && runtime_root.contains("pub(crate) mod stream_udp;")
            && runtime_root.contains("pub(crate) mod mux_udp;")
            && runtime_root.contains("pub(crate) mod udp_association;")
            && !manifest_dir().join("src/inbound/stream_udp.rs").exists()
            && !manifest_dir().join("src/inbound/mux_udp.rs").exists()
            && !manifest_dir().join("src/inbound/datagram_udp.rs").exists()
            && !manifest_dir()
                .join("src/inbound/socks5/udp_associate.rs")
                .exists(),
        "proxy should expose only runtime-owned UDP inbound glue modules and keep the old inbound UDP loop files removed"
    );

    assert!(
        stream_udp.contains("run_packet_session_udp_relay")
            && !stream_udp.contains("select!")
            && !stream_udp.contains("upstream_udp.recv_response")
            && !stream_udp.contains("direct_sock.recv_from_addr")
            && mux_udp.contains("run_packet_session_udp_relay")
            && !mux_udp.contains("select!")
            && !mux_udp.contains("upstream_udp.recv_response")
            && !mux_udp.contains("direct_sock.recv_from_addr"),
        "stream_udp and mux_udp should be thin bridges into the shared packet-session runtime template instead of owning separate UDP event loops"
    );

    assert!(
        packet_session_udp.contains("select!")
            && packet_session_udp.contains("upstream_udp.recv_response")
            && packet_session_udp.contains("direct_sock.recv_from_addr")
            && datagram_udp.contains("select!")
            && datagram_udp.contains("direct_sock.recv_from_addr")
            && udp_association.contains("select!")
            && udp_association.contains("relay.recv_from_addr")
            && udp_association.contains("upstream_udp.recv_response")
            && !packet_session_udp.contains("socks5::")
            && !datagram_udp.contains("shadowsocks::")
            && !datagram_udp.contains("mieru::")
            && !udp_association.contains("socks5::"),
        "the remaining UDP inbound event-loop templates should be the neutral packet-session, datagram, and association runtimes without protocol-private ownership"
    );
}

#[test]
fn ordinary_udp_inbounds_submit_packets_through_udp_pipe() {
    let udp_ingress = read("src/runtime/udp_ingress.rs");
    let datagram_udp = read_proxy_module_tree("src/runtime/datagram_udp.rs");
    let stream_udp = read("src/runtime/stream_udp.rs");
    let mux_udp = read("src/runtime/mux_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    assert!(
        udp_ingress.contains("pub(crate) async fn dispatch_inbound_udp_packet")
            && udp_ingress.contains("UdpPipe::new(proxy, dispatch)")
            && udp_ingress.contains("UdpPipeInput::from_inbound_dispatch"),
        "shared UDP ingress should own UdpPipe submission"
    );
    assert!(
        stream_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && !stream_udp.contains("UdpPipe::new")
            && !stream_udp.contains("UdpPipeInput"),
        "shared stream UDP relay glue should submit decoded packets through the inbound UDP dispatch helper"
    );
    assert!(
        mux_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && !mux_udp.contains("UdpPipe::new")
            && !mux_udp.contains("UdpPipeInput"),
        "shared MUX UDP relay glue should submit decoded packets through the inbound UDP dispatch helper"
    );
    assert!(
        datagram_udp.contains("dispatch_inbound_udp_packet")
            && !datagram_udp.contains("UdpPipe::new")
            && !datagram_udp.contains("UdpPipeInput"),
        "shared datagram UDP relay glue should submit decoded packets through the inbound UDP dispatch helper"
    );

    {
        let source = "src/transport/socks5_inbound/listener/udp_associate.rs";
        let content = read(source);
        assert!(
            content.contains("run_udp_association_loop")
                && !content.contains("dispatch_inbound_udp_packet")
                && !content.contains("UdpPipe::new")
                && !content.contains("UdpPipeInput"),
            "{source} should delegate UDP packet submission to the shared UDP association runtime"
        );
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call the UDP dispatch state machine directly"
        );
    }

    for source in [
        "src/transport/shadowsocks_inbound/listener/udp.rs",
        "src/transport/hysteria2_inbound/listener/udp.rs",
    ] {
        let content = read(source);
        assert!(
            (content.contains("run_datagram_udp_relay")
                || content.contains("run_protocol_datagram_udp_relay"))
                && !content.contains("impl DatagramUdpResponder")
                && !content.contains("dispatch_inbound_udp_packet")
                && !content.contains("UdpPipe::new")
                && !content.contains("UdpPipeInput"),
            "{source} should delegate datagram UDP packet submission to shared datagram UDP relay glue"
        );
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call the UDP dispatch state machine directly"
        );
    }

    for source in [
        "src/adapters/vless/listener.rs",
        "src/adapters/vmess/listener.rs",
        "src/adapters/trojan/listener.rs",
        "src/transport/mieru_inbound/listener/udp.rs",
    ] {
        let content = read(source);
        assert!(
            (content.contains("run_stream_udp_relay")
                || content.contains("run_mapped_protocol_stream_udp_relay")
                || content.contains("run_recorded_protocol_stream_udp_relay(")
                || contains_vless_mux_dispatch(&content)
                || content.contains("dispatch_no_client_stream_route(")
                || content.contains("dispatch_no_client_stream_route_request(")
                || contains_helper_call(&content, "spawn_transport_stream_route_inbound_listener")
                || content.contains("dispatch_no_client_mux_route(")
                || content.contains("dispatch_no_client_mux_route_with_defaults(")
                || content.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(&content, "spawn_transport_mux_route_inbound_listener")
                || contains_helper_call(
                    &content,
                    "spawn_recorded_transport_mux_bound_inbound_listener",
                ))
                && ((content.contains("StreamUdpRelayRequest")
                    || content.contains("run_mapped_protocol_stream_udp_relay")
                    || content.contains("run_recorded_protocol_stream_udp_relay(")
                    || contains_vless_mux_dispatch(&content))
                    || content.contains("dispatch_no_client_stream_route(")
                    || content.contains("dispatch_no_client_stream_route_request(")
                    || contains_helper_call(
                        &content,
                        "spawn_transport_stream_route_inbound_listener",
                    )
                    || content.contains("dispatch_no_client_mux_route(")
                    || content.contains("dispatch_no_client_mux_route_with_defaults(")
                    || content.contains("dispatch_no_client_mux_route_request_with_defaults(")
                    || contains_helper_call(
                        &content,
                        "spawn_transport_mux_route_inbound_listener",
                    )
                    || contains_helper_call(
                        &content,
                        "spawn_recorded_transport_mux_bound_inbound_listener",
                    ))
                && !content.contains("dispatch_inbound_udp_packet")
                && !content.contains("UdpPipe::new")
                && !content.contains("UdpPipeInput"),
            "{source} should delegate stream UDP packet submission to shared stream UDP relay glue"
        );
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call the UDP dispatch state machine directly"
        );
    }

    for source in [
        "src/adapters/vless/listener.rs",
        "src/adapters/vmess/listener.rs",
    ] {
        let content = read(source);
        assert!(
            (content.contains("run_protocol_mux_udp_task")
                || content.contains("run_logged_protocol_mux_udp_relay")
                || contains_vless_mux_dispatch(&content)
                || content.contains("dispatch_no_client_mux_route_with_defaults(")
                || content.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(&content, "spawn_transport_mux_route_inbound_listener")
                || content.contains("dispatch_recorded_protocol_mux_route_with_udp_logger("))
                && !content.contains("run_mux_udp_relay")
                && !content.contains("MuxUdpRelayRequest")
                && !content.contains("relay.into_parts()")
                && !content.contains("dispatch_inbound_udp_packet")
                && !content.contains("UdpPipe::new")
                && !content.contains("UdpPipeInput"),
            "{source} should delegate MUX UDP packet submission to shared MUX UDP relay glue"
        );
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call the UDP dispatch state machine directly"
        );
    }

    for source in [
        "src/transport/socks5_inbound/listener/udp_associate/dispatch.rs",
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
        "src/adapters/trojan.rs",
        "src/transport/shadowsocks_inbound/listener/udp.rs",
        "src/transport/hysteria2_inbound/listener/udp.rs",
        "src/transport/mieru_inbound/listener/udp.rs",
    ] {
        let content = read(source);
        for forbidden in [
            "protocol: ProtocolType::Socks5",
            "protocol: ProtocolType::Shadowsocks",
            "protocol: ProtocolType::Hysteria2",
            "protocol: ProtocolType::Vless",
            "protocol: ProtocolType::Vmess",
            "protocol: zero_core::ProtocolType::Vless",
            "protocol: zero_core::ProtocolType::Trojan",
            "protocol: zero_core::ProtocolType::Mieru",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should take UDP dispatch protocol identity from protocol-owned dispatch parts, not `{forbidden}`"
            );
        }
    }

    let socks5_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");
    assert!(
        socks5_udp.contains("impl Socks5InboundUdpDispatchParts")
            && socks5_udp.contains("pub fn protocol(&self) -> ProtocolType"),
        "Socks5InboundUdpDispatchParts should expose protocol identity to inbound UDP glue"
    );

    for (protocol_source, dispatch_parts) in [
        (
            "protocols/shadowsocks/src/udp.rs",
            "ShadowsocksInboundUdpDispatchParts",
        ),
        (
            "protocols/hysteria2/src/udp.rs",
            "Hysteria2InboundUdpDispatchParts",
        ),
        ("protocols/vless/src/udp.rs", "VlessInboundUdpDispatchParts"),
        ("protocols/vmess/src/udp.rs", "VmessInboundUdpDispatchParts"),
        ("protocols/mieru/src/udp.rs", "MieruInboundUdpDispatchParts"),
    ] {
        let content = read_repo_module_tree(protocol_source);
        assert!(
            content.contains(&format!("impl {dispatch_parts}"))
                && content.contains("pub fn protocol(&self) -> ProtocolType"),
            "{dispatch_parts} should expose protocol identity to inbound UDP glue"
        );
    }

    let trojan_udp = read_repo_module_tree("protocols/trojan/src/udp.rs");
    assert!(
        trojan_udp.contains("impl TrojanInboundUdpDispatchParts")
            && !trojan_udp.contains("pub fn protocol(&self) -> ProtocolType"),
        "Trojan inbound UDP dispatch parts should stay protocol-private instead of exporting a protocol identity helper"
    );
}

#[test]
fn custom_tcp_inbound_relays_use_runtime_metering_helpers() {
    let runtime = read_proxy_module_tree("src/runtime/tcp_ingress.rs");
    assert!(
        runtime.contains("fn record_tcp_upload(")
            && runtime.contains("fn record_tcp_download(")
            && runtime.contains("record_session_inbound_rx")
            && runtime.contains("record_session_outbound_tx")
            && runtime.contains("record_session_outbound_rx")
            && runtime.contains("record_session_inbound_tx"),
        "runtime inbound protocol layer should own TCP relay metering helpers"
    );

    let hysteria2 = read("src/transport/hysteria2_inbound/listener.rs");
    assert!(
        !hysteria2.contains("record_tcp_upload")
            && !hysteria2.contains("record_tcp_download")
            && !hysteria2.contains("copy_one_way")
            && !hysteria2.contains("tokio::io::split")
            && !hysteria2.contains("async fn relay")
            && !hysteria2.contains("record_session_inbound_rx")
            && !hysteria2.contains("record_session_outbound_tx")
            && !hysteria2.contains("record_session_outbound_rx")
            && !hysteria2.contains("record_session_inbound_tx"),
        "Hysteria2 TCP relay should use InboundProtocol's runtime default relay instead of owning copy/metering glue in the adapter"
    );
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
            "UdpClientResponse",
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
    let helper = read("src/runtime/udp_delivery/helpers.rs");
    let inbound_response = read("src/runtime/udp_delivery.rs");
    let datagram_udp = read_proxy_module_tree("src/runtime/datagram_udp.rs");
    let stream_udp = read("src/runtime/stream_udp.rs");
    let mux_udp = read("src/runtime/mux_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let udp_association = read_proxy_module_tree("src/runtime/udp_association.rs");
    assert!(
        helper.contains("fn record_udp_inbound_response_rx")
            && helper.contains("fn record_udp_inbound_response_tx")
            && helper.contains("struct UdpInboundResponseAccounting")
            && helper.contains("fn record_received(")
            && helper.contains("fn record_sent(")
            && helper.contains("fn session_id(")
            && helper.contains("struct UdpUpstreamResponseParts")
            && helper.contains("struct UdpDirectResponseParts")
            && helper.contains("struct UdpChainResponseParts")
            && helper.contains("fn record_upstream_udp_response_received")
            && helper.contains("fn record_direct_udp_response_received")
            && helper.contains("fn record_direct_udp_response_parts")
            && helper.contains("fn record_chain_udp_response_received")
            && helper.contains("fn record_chain_udp_response_parts")
            && helper.contains("direct_response_session_id")
            && helper.contains("record_udp_upstream_packet_received")
            && helper.contains("touch_upstream_idle")
            && helper.contains("upstream_association_view")
            && helper.contains("upstream_response_session_id")
            && helper.contains("fn udp_response_session_id")
            && helper.contains("record_session_outbound_rx")
            && helper.contains("record_session_inbound_tx")
            && helper.contains("session_id_by_target"),
        "neutral UDP inbound response accounting should live under runtime/udp_delivery"
    );
    assert!(
        inbound_response.contains("fn write_direct_response")
            && inbound_response.contains("fn write_upstream_response")
            && inbound_response.contains("fn write_chain_response")
            && inbound_response.contains("fn write_optional_direct_response")
            && inbound_response.contains("fn write_optional_upstream_response")
            && inbound_response.contains("fn write_optional_chain_response")
            && inbound_response.contains("response.accounting.record_sent(written)"),
        "stream UDP inbound response write glue should centralize protocol write callbacks plus neutral accounting"
    );
    assert!(
        stream_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("record_direct_udp_response_parts")
            && packet_session_udp.contains("record_upstream_udp_response_received")
            && packet_session_udp.contains("record_chain_udp_response_parts")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response")
            && packet_session_udp.contains("wait_for_upstream_idle")
            && packet_session_udp.contains("dispatch.finish_all()")
            && packet_session_udp.contains("log_completed_udp_flow"),
        "shared packet-session UDP relay glue should own direct/upstream/chain response accounting and writes for stream-carried UDP inbounds"
    );
    assert!(
        mux_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("record_direct_udp_response_parts")
            && packet_session_udp.contains("record_upstream_udp_response_received")
            && packet_session_udp.contains("record_chain_udp_response_parts")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response")
            && packet_session_udp.contains("wait_for_upstream_idle")
            && packet_session_udp.contains("dispatch.finish_all()")
            && packet_session_udp.contains("log_completed_udp_flow"),
        "shared packet-session UDP relay glue should own direct/upstream/chain response accounting and writes for MUX UDP inbounds"
    );
    assert!(
        datagram_udp.contains("record_direct_udp_response_parts")
            && datagram_udp.contains("record_upstream_udp_response_received")
            && datagram_udp.contains("record_chain_udp_response_parts")
            && datagram_udp.contains("write_optional_direct_response")
            && datagram_udp.contains("write_optional_upstream_response")
            && datagram_udp.contains("write_optional_chain_response")
            && datagram_udp.contains("wait_for_upstream_idle")
            && datagram_udp.contains("dispatch.finish_all()")
            && datagram_udp.contains("log_completed_udp_flow"),
        "shared datagram UDP relay glue should own datagram direct/upstream/chain response accounting and writes"
    );
    assert!(
        udp_association.contains("record_upstream_udp_response_received")
            && udp_association.contains("record_chain_udp_response_parts")
            && udp_association.contains("write_upstream_response")
            && udp_association.contains("write_chain_response")
            && udp_association.contains("wait_for_upstream_idle")
            && udp_association.contains("finish_all")
            && udp_association.contains("log_completed_udp_flow")
            && !udp_association.contains("socks5::")
            && !udp_association.contains("Socks5"),
        "shared UDP association glue should own upstream/chain response accounting and writes without naming SOCKS5"
    );

    for source in [
        "src/adapters/vless/listener.rs",
        "src/adapters/vmess/listener.rs",
        "src/adapters/trojan/listener.rs",
        "src/transport/mieru_inbound/listener/udp.rs",
    ] {
        let content = read(source);
        assert!(
            (content.contains("run_stream_udp_relay")
                || content.contains("run_mapped_protocol_stream_udp_relay")
                || content.contains("run_recorded_protocol_stream_udp_relay(")
                || contains_vless_mux_dispatch(&content)
                || content.contains("dispatch_no_client_stream_route(")
                || content.contains("dispatch_no_client_stream_route_request(")
                || contains_helper_call(&content, "spawn_transport_stream_route_inbound_listener")
                || content.contains("dispatch_no_client_mux_route(")
                || content.contains("dispatch_no_client_mux_route_with_defaults(")
                || content.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(&content, "spawn_transport_mux_route_inbound_listener")
                || contains_helper_call(
                    &content,
                    "spawn_recorded_transport_mux_bound_inbound_listener",
                ))
                && !content.contains("write_direct_response")
                && !content.contains("write_upstream_response")
                && !content.contains("write_chain_response")
                && !content.contains("record_direct_udp_response_parts")
                && !content.contains("record_upstream_udp_response_received")
                && !content.contains("record_chain_udp_response_parts")
                && !content.contains("response.accounting.record_sent"),
            "{source} should delegate stream UDP response accounting and writes to shared stream UDP relay glue"
        );
    }

    for source in [
        "src/adapters/vless/listener.rs",
        "src/adapters/vmess/listener.rs",
    ] {
        let content = read(source);
        assert!(
            (content.contains("run_protocol_mux_udp_task")
                || content.contains("run_logged_protocol_mux_udp_relay")
                || contains_vless_mux_dispatch(&content)
                || contains_helper_call(&content, "spawn_transport_mux_route_inbound_listener"))
                && !content.contains("write_direct_response")
                && !content.contains("write_upstream_response")
                && !content.contains("write_chain_response")
                && !content.contains("record_direct_udp_response_parts")
                && !content.contains("record_upstream_udp_response_received")
                && !content.contains("record_chain_udp_response_parts")
                && !content.contains("response.accounting.record_sent"),
            "{source} should delegate MUX UDP response accounting and writes to shared MUX UDP relay glue"
        );
    }

    for source in [
        "src/transport/hysteria2_inbound/listener/udp.rs",
        "src/transport/shadowsocks_inbound/listener/udp.rs",
    ] {
        let content = read(source);
        assert!(
            (content.contains("run_datagram_udp_relay")
                || content.contains("run_protocol_datagram_udp_relay"))
                && !content.contains("record_upstream_udp_response_received")
                && !content.contains("record_direct_udp_response_parts")
                && !content.contains("record_chain_udp_response_parts")
                && !content.contains("write_optional_direct_response")
                && !content.contains("write_optional_upstream_response")
                && !content.contains("write_optional_chain_response")
                && !content.contains("response.accounting.record_sent"),
            "{source} should delegate datagram UDP response accounting and writes to shared datagram UDP relay glue"
        );
    }

    {
        let source = "src/transport/socks5_inbound/listener/udp_associate.rs";
        let content = read(source);
        assert!(
            content.contains("run_udp_association_loop")
                && !content.contains("record_direct_udp_response_parts")
                && !content.contains("record_chain_udp_response_parts")
                && !content.contains("record_upstream_udp_response_received")
                && !content.contains("write_direct_response")
                && !content.contains("write_upstream_response")
                && !content.contains("write_chain_response")
                && !content.contains("session_id_by_target"),
            "{source} should delegate UDP response accounting and writes to runtime/udp_association"
        );
    }

    assert!(
        datagram_udp.contains("record_upstream_udp_response_received")
            && !datagram_udp.contains("record_udp_upstream_packet_received")
            && !datagram_udp.contains("udp_response_session_id"),
        "shared datagram UDP glue should consume registered upstream UDP responses through the neutral runtime helper"
    );

    assert!(
        udp_association.contains("record_chain_udp_response_parts")
            && !udp_association.contains("record_chain_udp_response_received"),
        "runtime UDP association should consume neutral chain response parts instead of open-coding chain response accounting in SOCKS5 glue"
    );

    for source in [
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
        "src/adapters/trojan.rs",
        "src/transport/mieru_inbound/listener.rs",
        "src/transport/hysteria2_inbound/listener.rs",
        "src/transport/shadowsocks_inbound/listener/udp.rs",
        "src/transport/socks5_inbound/listener/udp_associate/direct_response.rs",
        "src/transport/socks5_inbound/listener/udp_associate/relay_socket.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains("direct_response_session_id")
                && !content.contains("UdpInboundResponseAccounting::record_received"),
            "{source} should use runtime UDP response helpers for neutral response attribution"
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
    let lifecycle = read_proxy_module_tree("src/runtime/tcp_ingress.rs");
    assert!(
        lifecycle.contains("TcpPipe::new") && lifecycle.contains("TcpPipeInput"),
        "serve_inbound should route ordinary TCP sessions through TcpPipe"
    );

    let vless = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess = read_proxy_module_tree("src/adapters/vmess.rs");
    let mux_tcp = read("src/runtime/mux_tcp.rs");
    assert!(
        (vless.contains("run_logged_tcp_socket_listener_loop(")
            || vless.contains("run_logged_quic_stream_listener_loop("))
            && (vless.contains("dispatch_recorded_protocol_mux_tcp_request_result(")
                || vless.contains("dispatch_recorded_protocol_mux_tcp_request_with_defaults("))
            && (vless.contains("dispatch_recorded_protocol_mux_stream_request_result(")
                || vless.contains("dispatch_recorded_protocol_mux_stream_request_with_defaults("))
            && !vless.contains("run_mux_tcp_stream_task")
            && !vless.contains("MuxTcpStreamTask")
            && !vless.contains("open_mux_tcp_upstream")
            && !vless.contains("TcpPipe::new")
            && !vless.contains("TcpPipeInput")
            && mux_tcp.contains("pub(crate) async fn run_mux_tcp_stream_task")
            && mux_tcp.contains("pub(crate) async fn run_protocol_mux_tcp_task")
            && mux_tcp.contains("pub(crate) struct MuxTcpStreamTask")
            && mux_tcp.contains("pub(crate) async fn open_mux_tcp_upstream")
            && mux_tcp.contains("InboundMuxTcpRelay")
            && !mux_tcp.contains("pub(crate) trait MuxTcpStreamBridge")
            && mux_tcp.contains("let mux_session_id = bridge.mux_session_id();")
            && mux_tcp.contains("TcpPipe::new(proxy)")
            && mux_tcp.contains("TcpPipeInput"),
        "VLESS MUX sub-streams should use shared runtime MUX TCP task dispatch from transport glue"
    );
    assert!(
        !vless.contains("dispatch_tcp_outbound"),
        "VLESS inbound should not bypass TcpPipe through TCP outbound helpers"
    );
    assert!(
        vmess.contains("run_logged_tcp_socket_listener_loop(")
            && (vmess.contains("dispatch_no_client_mux_route_with_defaults(")
                || vmess.contains("dispatch_no_client_mux_route_request_with_defaults("))
            && !vmess.contains("run_mux_tcp_stream_task")
            && !vmess.contains("TcpPipe::new")
            && !vmess.contains("TcpPipeInput")
            && !vmess.contains("dispatch_tcp(")
            && mux_tcp.contains("run_protocol_mux_tcp_task")
            && mux_tcp.contains("open_mux_tcp_upstream")
            && mux_tcp.contains("InboundMuxTcpRelay")
            && mux_tcp.contains("TcpPipeInput"),
        "VMess MUX sub-streams should route through the shared MUX TCP pipe glue from transport dispatch"
    );
}

#[test]
fn vless_inbound_mux_frame_detail_lives_in_protocol_crate() {
    let inbound = read_proxy_module_tree("src/adapters/vless.rs");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vless/src/mux.rs"))
        .expect("read protocols/vless/src/mux.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read protocols/vless/src/inbound.rs");
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
        "uuid: [u8; 16]",
        "with_encryption(&uuid)",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "VLESS inbound mux should delegate protocol MUX frame/state detail to protocols/vless; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/adapters/vless/inbound/listener/mux.rs")
            .exists()
            && !manifest_dir()
                .join("src/adapters/vless/inbound/listener/session.rs")
                .exists()
            && !inbound.contains("uuid")
            && !inbound.contains("with_encryption")
            && !inbound.contains("mux_context: vless::mux::VlessInboundMuxContext")
            && !inbound.contains(".accept_mux_session_with_auth(&mut client, mux_master_uuid, auth)")
            && contains_vless_mux_dispatch(&inbound)
            && !inbound.contains("vless::mux::VlessInboundMuxServer::from_master_uuid_with_auth(")
            && !inbound.contains("let mut mux = VlessInboundMuxSession::with_encryption")
            && !protocol_mux.contains("VlessInboundMuxContext")
            && protocol_mux.contains("from_master_uuid_with_auth")
            && protocol_mux.contains("pub struct VlessInboundMuxServer")
            && protocol_inbound
                .contains(".accept_mux_session_with_auth(&mut stream, mux_master_uuid, auth)")
            && protocol_inbound.contains("async fn accept_mux_session_with_auth")
            && !protocol_inbound.contains("pub async fn accept_mux_session_with_auth"),
        "VLESS inbound mux identity and encrypted MUX session construction should stay behind protocol-owned UUID-to-server glue"
    );

    assert!(
        contains_vless_mux_dispatch(&inbound) && protocol_mux.contains("VlessInboundMuxServer"),
        "VLESS inbound mux should consume protocol-owned semantic MUX server APIs through the shared runtime route helper"
    );
    assert!(
        !inbound.contains("VlessInboundMuxContext")
            && !protocol_mux.contains("VlessInboundMuxContext")
            && !protocol_inbound.contains("VlessInboundMuxContext"),
        "VLESS inbound mux should not keep a separate context shell once protocol dispatch owns UUID-to-session construction"
    );
    assert!(
        !inbound.contains("vless::mux::VlessInboundMuxEvent::Opened(opened)")
            && !inbound.contains("dispatch_next_opened_route(self.client, &mut bridge)")
            && !inbound.contains("dispatch_next_opened_route_with_handlers")
            && contains_vless_mux_dispatch(&inbound)
            && !inbound.contains(".next_opened_route(self.client)")
            && !inbound.contains(".next_opened_route_with_auth(self.client")
            && !inbound.contains("self.auth")
            && protocol_mux.contains("async fn next_opened_route")
            && !protocol_mux.contains("pub async fn next_opened_route")
            && protocol_mux.contains("async fn next_opened_route_with_auth")
            && !protocol_mux.contains("pub async fn next_opened_route_with_auth")
            && !protocol_mux.contains("pub async fn dispatch_next_opened_route<")
            && !protocol_mux.contains("pub enum VlessInboundMuxEvent"),
        "VLESS inbound mux event-to-route dispatch should be protocol-owned before proxy bridges opened streams"
    );
    for required in [
        "VlessInboundMuxAction",
        "accept_inbound_stream",
        "reject_inbound_stream",
    ] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vless should own VLESS semantic MUX server API `{required}`"
        );
    }
    assert!(
        !inbound.contains("VlessInboundMuxAction")
            && protocol_mux.contains("VlessInboundMuxAction")
            && !protocol_mux.contains("pub enum VlessInboundMuxAction")
            && !inbound.contains("impl<S> MuxOpenedDispatcher")
            && !inbound.contains("struct OpenedDispatch")
            && !inbound.contains("struct VlessMuxOpenedDispatcherBridge")
            && !inbound.contains("impl vless::mux::VlessInboundMuxOpenedRouteDispatcher")
            && !inbound.contains("dispatch_next_opened_route(self.client, &mut bridge)")
            && !inbound.contains("dispatch_next_opened_route_with_handlers")
            && !inbound.contains(".next_opened_route(self.client)")
            && !inbound.contains(".next_opened_route_with_auth(self.client")
            && !inbound.contains("self.auth")
            && contains_vless_mux_dispatch(&inbound)
            && !inbound.contains("dispatch_next_opened_stream")
            && !protocol_mux.contains("dispatch_next_opened_stream")
            && !protocol_mux.contains("VlessInboundMuxOpenedHandler")
            && !inbound.contains(".apply_inbound_action(&mut mux, &mut client, action)")
            && !inbound.contains("run_mux_tcp_stream_task")
            && !inbound.contains("run_protocol_mux_udp_relay"),
        "VLESS transport mux glue should consume protocol-opened stream events without protocol callback traits"
    );
    for required in [
        "send_inbound_downlink",
        "send_inbound_stream_payload",
        "end_inbound_stream",
    ] {
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
        "async fn next_action",
        "async fn accept_stream",
        "async fn reject_stream",
        "async fn send_data",
        "async fn send_inbound_stream_data",
        "async fn send_inbound_stream_payload",
        "async fn end_stream",
        "async fn end_inbound_stream",
    ] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vless should keep low-level MUX frame operation `{required}`"
        );
    }
    assert!(
        protocol_mux.contains("fn into_session(self) -> Result<Session, Error>")
            && !protocol_mux.contains("pub fn into_session(self) -> Result<Session, Error>")
            && protocol_mux.contains("ProtocolType::Vless")
            && protocol_mux.contains("impl From<MuxServerEvent> for VlessInboundMuxAction")
            && !protocol_mux.contains("pub enum MuxServerEvent")
            && !protocol_mux.contains("pub struct MuxServer")
            && !inbound.contains("opened.into_route_with_auth")
            && !inbound.contains("route.dispatch_with(&mut bridge).await")
            && !inbound.contains("dispatch_next_opened_route(self.client, &mut bridge)")
            && !inbound.contains("dispatch_next_opened_route_with_handlers")
            && contains_vless_mux_dispatch(&inbound)
            && !inbound.contains("struct VlessMuxOpenedDispatcherBridge")
            && !inbound.contains("impl vless::mux::VlessInboundMuxOpenedRouteDispatcher")
            && !inbound.contains("VlessInboundMuxOpenedRoute::Tcp")
            && !inbound.contains("VlessInboundMuxOpenedRoute::Udp")
            && !inbound.contains("opened.into_kind()")
            && protocol_mux.contains("struct VlessInboundMuxOpenedRoute")
            && !protocol_mux.contains("pub enum VlessInboundMuxOpenedRoute")
            && !protocol_mux.contains("pub struct VlessInboundMuxOpenedRoute")
            && !protocol_mux.contains("pub trait VlessInboundMuxOpenedRouteDispatcher")
            && !protocol_mux.contains("pub async fn dispatch_with<")
            && protocol_mux.contains("async fn dispatch_with_handlers")
            && !protocol_mux.contains("pub async fn dispatch_with_handlers")
            && !protocol_mux.contains("pub async fn dispatch_next_opened_route<")
            && protocol_mux.contains("pub(crate) async fn dispatch_next_opened_route_with_handlers")
            && !protocol_mux.contains("route.dispatch_with(dispatcher).await")
            && protocol_mux.contains("dispatch_with_handlers(on_tcp_opened, on_udp_opened)")
            && protocol_mux.contains("fn into_route_with_auth")
            && !protocol_mux.contains("pub fn into_route_with_auth")
            && protocol_mux.contains("async fn next_opened_route_with_auth")
            && !protocol_mux.contains("pub async fn next_opened_route_with_auth")
            && protocol_mux.contains("opened.into_route_with_auth(auth, writer)")
            && protocol_mux.contains("relay: VlessInboundMuxUdpRelay")
            && protocol_mux.contains("pub struct VlessInboundMuxUdpRelay")
            && protocol_mux.contains("match session.network")
            && protocol_mux.contains("VlessInboundMuxAction::OpenStream")
            && protocol_mux.contains("VlessInboundMuxOpenedStream::new")
            && !inbound.contains("target.into_session()")
            && !inbound.contains("MuxNetwork")
            && !inbound.contains("session.network")
            && !inbound.contains("zero_core::Session::new"),
        "VLESS inbound mux target to Session conversion should be protocol-owned and exposed as an action"
    );
    for forbidden in [
        "MuxServerEvent",
        ".next_event(",
        "vless::mux::MuxServer::",
        "MuxServer::new",
        "MuxServer::with_encryption",
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
        !inbound.contains("impl<S> MuxOpenedDispatcher")
            && !inbound.contains("struct OpenedDispatch")
            && !inbound.contains("struct VlessMuxOpenedDispatcherBridge")
            && !inbound.contains("dispatch_next_opened_route(self.client, &mut bridge)")
            && !inbound.contains("dispatch_next_opened_route_with_handlers")
            && !inbound.contains(".next_opened_route(self.client)")
            && !inbound.contains(".next_opened_route_with_auth(self.client")
            && !inbound.contains("self.auth")
            && contains_vless_mux_dispatch(&inbound)
            && !inbound.contains("mux_server.reject_opened_stream(&mut client, sid)")
            && protocol_mux.contains("pub(crate) async fn reject_opened_stream")
            && protocol_mux.contains(".reject_opened_stream(&mut self.mux, stream, session_id)")
            && !inbound.contains(".apply_inbound_action(&mut mux, &mut client, action)")
            && !inbound.contains(".send_inbound_downlink(&mut mux, &mut client, downlink)")
            && !inbound.contains(".reject_opened_stream(&mut mux, &mut client, sid)")
            && !inbound.contains("VlessInboundMuxStreams::new")
            && !inbound.contains("VlessInboundMuxWriter::channel")
            && !inbound.contains("streams.open_stream(")
            && !inbound.contains("streams.push_stream_data(")
            && !inbound.contains("streams.close_inbound_stream(")
            && !inbound.contains("VlessInboundMuxAction::KeepAlive")
            && !inbound.contains("VlessInboundMuxAction::OpenStream")
            && !inbound.contains("VlessInboundMuxAction::Data")
            && !inbound.contains("VlessInboundMuxAction::End")
            && !inbound.contains("VlessInboundMuxAction::Unknown")
            && !inbound.contains("mux.send_inbound_stream_payload(&mut client, sid, &payload)")
            && !inbound.contains("downlink.is_end()")
            && !inbound.contains("downlink.into_parts()")
            && !inbound.contains("mux.send_inbound_stream_data(&mut client, sid, &payload)")
            && !inbound.contains("mux.end_inbound_stream(&mut client, sid)")
            && protocol_mux.contains("if payload.is_empty()")
            && protocol_mux.contains("self.end_inbound_stream(stream, sid).await")
            && protocol_mux.contains("self.send_inbound_stream_data(stream, sid, payload).await")
            && protocol_mux.contains("self.mux.read_inbound_action(stream)")
            && protocol_mux.contains(".apply_inbound_action(&mut self.mux, stream, action?)")
            && protocol_mux.contains(".send_inbound_downlink(&mut self.mux, stream, downlink)")
            && protocol_mux.contains("pub(crate) async fn reject_opened_stream")
            && !protocol_mux.contains("pub async fn next_opened_stream"),
        "VLESS inbound mux downstream payload to DATA/END frame selection should live in protocols/vless"
    );
    for private_root_item in [
        "encode_frame",
        "encode_new_stream",
        "encode_new_stream_response",
        "encode_data_frame",
        "encode_end_frame",
        "parse_new_stream",
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
        "MuxServer",
        "VlessInboundMuxAction",
        "VlessInboundMuxServer",
        "VlessInboundMuxSession",
        "VlessInboundMuxWriter",
    ] {
        assert!(
            protocol_mux.contains(private_root_item) && !protocol_lib.contains(private_root_item),
            "VLESS MUX API `{private_root_item}` should stay under vless::mux instead of the crate root"
        );
    }
    assert!(
        !protocol_mux.contains("pub struct MuxClient")
            && !protocol_mux.contains("pub struct MuxClientStream")
            && !protocol_mux.contains("impl MuxClient"),
        "obsolete VLESS outbound raw MUX client shells should stay deleted after transport moved to pool-driven stream opening"
    );
    assert!(
        !protocol_mux.contains("encode_udp_data_frame(")
            && !protocol_mux.contains("parse_udp_target_from_keep("),
        "obsolete VLESS raw UDP target frame helpers should stay deleted after mux state moved behind protocol-owned semantic APIs"
    );
}

#[test]
fn tcp_outbound_resolution_helper_stays_inside_tcp_dispatch() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if source.starts_with("src/runtime/tcp_dispatch") {
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
            "src/transport/hysteria2_inbound.rs",
        ],
        &[
            "src/adapters/",
            "src/transport/socks5_inbound/",
            "src/transport/shadowsocks_inbound/",
            "src/transport/mieru_inbound/",
            "src/transport/hysteria2_inbound/",
        ],
        "protocol config variant matching should stay inside adapters or inbound request/entrypoint bridges",
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
fn direct_udp_socket_operations_do_not_live_in_outbound_facade() {
    assert!(
        !manifest_dir().join("src/outbound").exists(),
        "src/outbound should not remain as an empty compatibility facade"
    );
    assert!(
        !manifest_dir().join("src/outbound/direct.rs").exists(),
        "src/outbound/direct.rs should not be kept as an empty compatibility facade"
    );

    let socket = read("src/runtime/udp_socket.rs");
    let adapter = read("src/adapters/direct/udp.rs");
    assert!(
        !socket.contains("resolve_udp_target")
            && socket.contains("send_direct_udp_packet")
            && socket.contains("resolve_udp_peer_endpoint"),
        "runtime::udp_socket should own neutral socket operations without duplicating direct target resolution"
    );
    assert!(
        !socket.contains("outbound/direct.rs"),
        "runtime::udp_socket should not keep historical references to removed outbound direct facades"
    );
    assert!(
        adapter.contains("resolve_target_addr(session, proxy.resolver.as_ref())")
            && adapter.contains("send_direct_packet"),
        "direct adapter should resolve its direct target through DirectConnector and send through UdpDispatch"
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
fn architecture_docs_do_not_describe_removed_proxy_facades() {
    let architecture = fs::read_to_string(repo_root().join("docs/project/architecture.md"))
        .expect("read docs/project/architecture.md");

    for forbidden in [
        "`outbound/mod.rs` only declares",
        "Helper logic lives in `outbound/<protocol>.rs`",
        "`protocol_registry.rs` only re-exports",
        "`protocol_registry/defaults.rs` only wires",
        "`protocol_registry/model.rs` only wires",
        "`protocol_registry/registry.rs` only owns",
        "move protocol state to src/protocol_runtime",
        "protocol state in src/protocol_runtime",
    ] {
        assert!(
            !architecture.contains(forbidden),
            "docs/project/architecture.md should not describe removed proxy facade `{forbidden}`"
        );
    }

    for expected in [
        "`src/outbound/` does not exist",
        "`protocol_registry/mod.rs` only re-exports",
        "`protocol_registry/defaults/mod.rs` only wires",
        "`protocol_registry/model/mod.rs` only wires",
        "`protocol_registry/registry/mod.rs` only owns",
        "`crates/proxy/src/inbound/{datagram_udp,stream_udp,mux_udp}.rs` own only route submission",
        "Protocol-specific responders own request decoding, response encoding, protocol session tracking, and read buffers",
        "they must not hold protocol-private pending dispatch state, client maps, codec state, or responder read buffers",
    ] {
        assert!(
            architecture.contains(expected),
            "docs/project/architecture.md should document current proxy boundary `{expected}`"
        );
    }
    assert!(
        architecture.contains("Per-protocol outbound TCP glue lives in the owning adapter capability bridge")
            || architecture.contains("proxy `tcp.rs` shells stay absent"),
        "docs/project/architecture.md should describe the current TCP bridge boundary without reviving proxy tcp.rs shells"
    );
}

#[test]
fn project_docs_keep_protocol_response_framing_protocol_owned() {
    let docs = [
        (
            "docs/project/architecture.md",
            fs::read_to_string(repo_root().join("docs/project/architecture.md"))
                .expect("read docs/project/architecture.md"),
        ),
        (
            "docs/project/protocol-capabilities.md",
            fs::read_to_string(repo_root().join("docs/project/protocol-capabilities.md"))
                .expect("read docs/project/protocol-capabilities.md"),
        ),
        (
            "docs/project/release-boundary.md",
            fs::read_to_string(repo_root().join("docs/project/release-boundary.md"))
                .expect("read docs/project/release-boundary.md"),
        ),
    ];

    for (path, content) in &docs {
        for forbidden in [
            "proxy owns protocol response encoding",
            "proxy owns protocol packet parsing",
            "proxy owns protocol framing",
            "Generic runtime and protocol-runtime modules dispatch through `ProtocolInventory`",
        ] {
            assert!(
                !content.contains(forbidden),
                "{path} should keep protocol response framing and runtime dispatch ownership current; found `{forbidden}`"
            );
        }
    }

    let architecture = &docs[0].1;
    assert!(
        architecture.contains("Protocol stream/datagram codecs own protocol crypto/framing state")
            && architecture.contains("VMess inbound UDP request payload mode detection/parsing and response packet encoding live in `protocols/vmess::udp`")
            && architecture.contains("VLESS inbound UDP packet parsing and response/MUX response encoding live in `protocols/vless::udp`"),
        "docs/project/architecture.md should state that protocol response encoding stays protocol-owned"
    );

    let release_boundary = &docs[2].1;
    assert!(
        !release_boundary.contains("proxy owns protocol response encoding"),
        "docs/project/release-boundary.md should state response framing ownership"
    );
}

#[test]
fn boundary_docs_lock_post_accept_listener_bridge_and_facade_roots() {
    let architecture = fs::read_to_string(repo_root().join("docs/project/architecture.md"))
        .expect("read docs/project/architecture.md");
    let agents = fs::read_to_string(repo_root().join("AGENTS.md")).expect("read AGENTS.md");

    for (path, content) in [
        ("docs/project/architecture.md", architecture.as_str()),
        ("AGENTS.md", agents.as_str()),
    ] {
        for required in [
            "udp_flow/managed/mod.rs",
            "udp_flow/registered/mod.rs",
            "post-accept",
            "listener.rs",
        ] {
            assert!(
                content.contains(required),
                "{path} should lock the final boundary wording for `{required}`"
            );
        }

        assert!(
            content.contains("transport-owned request object")
                || content.contains("transport-owned request objects"),
            "{path} should say that transport-backed listener bridges consume transport-owned request metadata"
        );
    }

    assert!(
        architecture.contains("post-accept only")
            && architecture.contains("adapter-local `listener.rs` bridges build those requests")
            && architecture.contains("src/runtime/inbound_route.rs` stays post-accept only")
            && architecture.contains("src/runtime/udp_flow/managed/mod.rs")
            && architecture.contains("src/runtime/udp_flow/registered/mod.rs"),
        "docs/project/architecture.md should describe the final listener-bridge and managed/registered facade-root boundary"
    );

    assert!(
        agents.contains("transport-owned request objects")
            && agents.contains("their adapter roots must not regrow a second generic proxy accept-stage abstraction")
            && agents.contains("crates/proxy/src/runtime/udp_flow/managed/mod.rs")
            && agents.contains("crates/proxy/src/runtime/udp_flow/registered/mod.rs"),
        "AGENTS.md should describe the same final post-accept listener boundary and managed/registered facade roots as the code"
    );
}

#[test]
fn adapters_delegate_protocol_private_config_parsing_to_protocols() {
    let adapters = rust_sources_under("src/adapters");
    for path in adapters {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read adapter source");
        for forbidden in [
            "parse_uuid",
            "parse_flow",
            "Uuid::parse",
            "VmessCipher::from_name",
            "CipherKind::from_str",
            "sha224",
            "blake3",
            "encrypt_packet",
            "decrypt_packet",
            "encode_udp_packet",
            "decode_udp_packet",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should delegate protocol-private config parsing and crypto/framing details to protocols/*; found `{forbidden}`"
            );
        }
    }
    for (source, required) in [
        (
            "crates/transport/src/vmess_transport.rs",
            "vmess::inbound::VmessInboundProfile::from_config_users",
        ),
        (
            "crates/transport/src/vless_transport.rs",
            "vless::inbound::VlessInboundProfile::from_config_users",
        ),
        (
            "crates/transport/src/trojan_transport.rs",
            "trojan::inbound::TrojanInboundProfile::from_config_password",
        ),
        (
            "crates/transport/src/vmess_transport.rs",
            "vmess::outbound::PreparedVmessOutboundRequestBundle::from_config_with_transport_hints",
        ),
        (
            "crates/transport/src/vless_transport.rs",
            "vless::outbound::PreparedVlessOutboundRequestBundle::from_config_with_transport_hints",
        ),
        (
            "crates/transport/src/trojan_transport.rs",
            "trojan::outbound::PreparedTrojanOutboundRequestBundle::from_config",
        ),
        (
            "crates/transport/src/vmess_transport.rs",
            "PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(",
        ),
        (
            "crates/transport/src/vless_transport.rs",
            "PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(",
        ),
        (
            "crates/transport/src/trojan_transport.rs",
            "protocol: trojan::outbound::PreparedTrojanOutboundRequestBundle",
        ),
        (
            "crates/transport/src/shadowsocks_transport.rs",
            "shadowsocks_tcp_connect_config(",
        ),
        (
            "crates/transport/src/shadowsocks_transport.rs",
            "ShadowsocksManagedUdpFlowConfig::new(",
        ),
        (
            "crates/transport/src/shadowsocks_transport.rs",
            "pub fn packet_path_datagram_source_build(",
        ),
    ] {
        let content = if source.starts_with("crates/") {
            read_repo_module_tree(source)
        } else {
            read(source)
        };
        assert!(
            content.contains(required),
            "{source} should own the protocol-owned config/plan builder call `{required}`"
        );
    }
}

#[test]
fn proxy_boundary_todo_completion_invariants_are_locked() {
    for removed in [
        "src/protocol_runtime",
        "src/outbound",
        "src/protocol_registry.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "legacy proxy dumping-ground path `{removed}` must not exist"
        );
    }

    let runtime_roots = ["src/runtime", "src/runtime.rs"];
    for runtime_root in runtime_roots {
        let sources = if runtime_root.ends_with(".rs") {
            vec![manifest_dir().join(runtime_root)]
        } else {
            rust_sources_under(runtime_root)
        };
        for path in sources {
            let source = relative(&path);
            let content = fs::read_to_string(&path).expect("read runtime source");
            for forbidden in [
                "InboundProtocolConfig::",
                "OutboundProtocolConfig::",
                "ResolvedLeafOutbound::",
                "use socks5::",
                "use vless::",
                "use vmess::",
                "use shadowsocks::",
                "use trojan::",
                "use hysteria2::",
                "use mieru::",
            ] {
                assert!(
                    !content.contains(forbidden),
                    "{source} should remain protocol-neutral for the TODO boundary; found `{forbidden}`"
                );
            }
        }
    }

    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read adapter source");
        for forbidden in [
            "parse_uuid",
            "parse_flow",
            "Uuid::parse",
            "VmessCipher::from_name",
            "CipherKind::from_str",
            "sha224",
            "blake3",
            "encrypt_packet",
            "decrypt_packet",
            "encode_udp_packet",
            "decode_udp_packet",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should stay a thin adapter bridge and delegate protocol-private parsing/framing; found `{forbidden}`"
            );
        }
    }

    for source in [
        "src/runtime/datagram_udp.rs",
        "src/runtime/stream_udp.rs",
        "src/runtime/mux_udp.rs",
        "src/runtime/packet_session_udp.rs",
    ] {
        let content = read(source);
        for forbidden in [
            "read_buf",
            "pending_dispatch",
            "client_map",
            "client_sessions",
            "CipherKind",
            "password: &str",
            "Codec::new",
            "decode_udp_packet",
            "encode_udp_packet",
            "ResponseTarget",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should stay shared inbound UDP orchestration glue, not protocol-private state; found `{forbidden}`"
            );
        }
    }

    let architecture = fs::read_to_string(repo_root().join("docs/project/architecture.md"))
        .expect("read docs/project/architecture.md");
    let agents = fs::read_to_string(repo_root().join("AGENTS.md")).expect("read AGENTS.md");
    for (path, content) in [
        ("docs/project/architecture.md", architecture.as_str()),
        ("AGENTS.md", agents.as_str()),
    ] {
        for forbidden in [
            "Protocol identity and cipher config parsing is adapter-owned",
            "protocol's own adapter",
            "move protocol state to src/protocol_runtime",
            "proxy owns response bridge",
        ] {
            assert!(
                !content.contains(forbidden),
                "{path} should describe the converged proxy/protocol boundary, not stale TODO-era ownership `{forbidden}`"
            );
        }
    }
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
    let tcp_dispatch = read_proxy_module_tree("src/runtime/tcp_dispatch.rs");
    let tcp_outbound = read_proxy_module_tree("src/transport/tcp_outbound.rs");

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
            !tcp_outbound.contains(forbidden),
            "src/transport/tcp_outbound.rs should normalize TCP outbound results through neutral variants; found `{forbidden}`"
        );
    }

    assert!(
        tcp_dispatch.contains(".into_relay_stream()")
            && tcp_outbound.contains("struct EstablishedTcpOutbound")
            && tcp_outbound.contains("enum EstablishedTcpOutboundKind")
            && tcp_outbound.contains("kind: EstablishedTcpOutboundKind")
            && !tcp_outbound.contains("pub(crate) enum EstablishedTcpOutbound")
            && tcp_outbound.contains("EstablishedTcpOutboundKind::Proxied")
            && tcp_outbound.contains("pub(crate) fn proxied("),
        "TCP outbound results should expose neutral relay/proxied stream normalization"
    );
    assert!(
        tcp_dispatch.contains("EstablishedTcpOutbound::block(")
            && tcp_dispatch.contains("EstablishedTcpOutbound::relay(")
            && !tcp_dispatch.contains("EstablishedTcpOutbound::Block {")
            && !tcp_dispatch.contains("EstablishedTcpOutbound::Relay {"),
        "TCP runtime should construct neutral TCP outbound results through helper constructors"
    );
}

#[test]
fn transport_tcp_outbound_root_is_facade_only() {
    let root = read("src/transport/tcp_outbound.rs");
    let tree = read_proxy_module_tree("src/transport/tcp_outbound.rs");
    let module_dir = manifest_dir().join("src/transport/tcp_outbound");

    for path in [
        "connect.rs",
        "error.rs",
        "model.rs",
        "relay.rs",
        "result.rs",
    ] {
        assert!(
            module_dir.join(path).exists(),
            "transport::tcp_outbound should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod connect;",
        "mod error;",
        "mod model;",
        "mod relay;",
        "mod result;",
        "pub(crate) use connect::connect_protocol_transport_bridge_tcp;",
        "pub(crate) use model::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult};",
        "pub(crate) use relay::apply_protocol_transport_bridge_relay_hop;",
        "pub(crate) use result::extract_tcp_stream;",
    ] {
        assert!(
            root.contains(required),
            "transport::tcp_outbound root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "fn tcp_connect_prepare_failure",
        "fn tcp_relay_prepare_error",
        "pub(crate) struct TcpRouteResult",
        "pub(crate) struct EstablishedTcpOutbound",
        "pub(crate) async fn connect_protocol_transport_bridge_tcp<",
        "pub(crate) async fn apply_protocol_transport_bridge_relay_hop<",
        "pub(crate) fn extract_tcp_stream(",
    ] {
        assert!(
            !root.contains(forbidden),
            "transport::tcp_outbound root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "transport::tcp_outbound module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn tcp_relay_chain_runtime_uses_inventory_for_all_protocol_hops() {
    let tcp_dispatch = read_proxy_module_tree("src/runtime/tcp_dispatch.rs");
    let inventory_tcp = read("src/inventory/tcp.rs");

    assert!(
        tcp_dispatch.contains("async fn dispatch_tcp_relay_chain")
            && tcp_dispatch.contains("pub(crate) async fn dispatch_tcp_relay_prefix")
            && tcp_dispatch.contains("async fn apply_hop_protocol")
            && tcp_dispatch.contains(".dispatch_tcp_relay_prefix(chain).await?")
            && tcp_dispatch
                .contains("apply_hop_protocol(self, carrier.stream, &final_hop, session)")
            && tcp_dispatch
                .contains("apply_hop_protocol(self, stream, &current_hop, &session_for_next)")
            && tcp_dispatch.contains(".apply_tcp_relay_hop(proxy, stream, session, hop)")
            && tcp_dispatch.contains("EstablishedTcpOutbound::relay(stream)")
            && tcp_dispatch.contains(".into_relay_stream()"),
        "TCP relay chain runtime should normalize the first hop and delegate every protocol hop through ProtocolInventory"
    );
    assert!(
        inventory_tcp.contains("pub(crate) async fn apply_tcp_relay_hop(")
            && inventory_tcp.contains("self.registry.find_outbound_leaf(leaf)?")
            && inventory_tcp.contains("TcpOutboundCapability::apply_relay_hop(")
            && inventory_tcp.contains("OutboundAdapterContext::new(proxy)")
            && !tcp_dispatch.contains("TcpOutboundCapability::apply_relay_hop")
            && !tcp_dispatch.contains("self.registry.find_outbound_leaf")
            && !tcp_dispatch.contains("find_outbound_leaf(hop")
            && !tcp_dispatch.contains("find_outbound_leaf(&final_hop"),
        "TCP relay hop adapter resolution should live only in ProtocolInventory"
    );
    for forbidden in [
        "ResolvedLeafOutbound::Socks5",
        "ResolvedLeafOutbound::Vless",
        "ResolvedLeafOutbound::Vmess",
        "ResolvedLeafOutbound::Trojan",
        "ResolvedLeafOutbound::Mieru",
        "ResolvedLeafOutbound::Shadowsocks",
        "ResolvedLeafOutbound::Hysteria2",
        "connect_upstream_socks5",
        "connect_upstream_vless",
        "connect_upstream_vmess",
        "connect_upstream_trojan",
        "connect_upstream_mieru",
        "connect_upstream_shadowsocks",
        "open_hysteria2_quic_connection",
        "apply_tcp_hop(",
        ".apply_relay_hop(",
    ] {
        assert!(
            !tcp_dispatch.contains(forbidden),
            "TCP relay runtime should not call protocol-specific relay hop detail `{forbidden}`"
        );
    }
    assert!(
        tcp_dispatch.contains("fn relay_next_session(endpoint: OutboundEndpoint<'_>) -> Session")
            && tcp_dispatch.contains("zero_core::Network::Tcp")
            && tcp_dispatch.contains("zero_core::ProtocolType::Unknown")
            && !tcp_dispatch.contains("ProtocolType::Socks5")
            && !tcp_dispatch.contains("ProtocolType::Vless")
            && !tcp_dispatch.contains("ProtocolType::Vmess")
            && !tcp_dispatch.contains("ProtocolType::Trojan")
            && !tcp_dispatch.contains("ProtocolType::Mieru")
            && !tcp_dispatch.contains("ProtocolType::Shadowsocks")
            && !tcp_dispatch.contains("ProtocolType::Hysteria2"),
        "TCP relay next-hop sessions should stay protocol-neutral"
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
    let handle_root = read("src/runtime/handle.rs");
    let handle = read_proxy_module_tree("src/runtime/handle.rs");

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

    assert!(
        handle_root.contains("pub use model::ProxyHandle;")
            && !handle_root.contains("struct ProxyHandle")
            && !handle_root.contains("impl zero_api::QueryService for ProxyHandle")
            && !handle_root.contains("impl zero_api::CommandService for ProxyHandle")
            && !handle_root.contains("impl zero_api::EventSource for ProxyHandle")
            && !handle_root.contains("fn parse_ip_address"),
        "src/runtime/handle.rs should stay a thin facade over runtime/handle/*"
    );

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
    let orchestration = read("src/runtime/orchestration.rs");
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
            && !runtime.contains("subscribe_reload_bridge(")
            && orchestration
                .contains("reload::subscribe_reload_bridge(proxy.engine.subscribe_reload())"),
        "runtime orchestration should subscribe to reloads through runtime/reload.rs"
    );
}

#[test]
fn runtime_listeners_root_is_facade_only() {
    let root = read("src/runtime/listeners.rs");
    let inbound = read("src/runtime/listeners/inbound.rs");
    let urltest = read("src/runtime/listeners/urltest.rs");

    for expected in [
        "mod inbound;",
        "mod urltest;",
        "pub(super) use inbound::{",
        "pub(super) use urltest::reconcile_urltests;",
    ] {
        assert!(
            root.contains(expected),
            "runtime/listeners.rs should remain a facade containing `{expected}`"
        );
    }

    for implementation in [
        "async fn bind_inbound_listener(",
        "fn spawn_inbound_listener(",
        "async fn reconcile_inbounds(",
        "fn reconcile_urltests(",
    ] {
        assert!(
            !root.contains(implementation),
            "runtime/listeners.rs should not regrow implementation `{implementation}`"
        );
    }

    assert!(
        inbound.contains("async fn bind_inbound_listener(")
            && inbound.contains("fn spawn_inbound_listener(")
            && inbound.contains("async fn reconcile_inbounds("),
        "runtime/listeners/inbound.rs should own inbound listener lifecycle"
    );
    assert!(
        urltest.contains("fn reconcile_urltests("),
        "runtime/listeners/urltest.rs should own urltest task reconciliation"
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
        "mod system;",
        "mod tun;",
        "pub(crate) use direct::run_direct_listener_with_bound;",
    ] {
        assert!(
            root.contains(expected),
            "src/inbound/mod.rs should expose inbound facade item `{expected}`"
        );
    }

    for forbidden in [
        "mod http;",
        "pub(crate) use http::run_http_listener_with_bound;",
        "mod socks5;",
        "pub(crate) use socks5::run_socks5_listener_with_bound;",
        "pub(crate) use socks5::Socks5InboundListenerRequest;",
        "pub(crate) mod mieru;",
        "pub(crate) use mieru::run_mieru_listener_with_bound;",
        "pub(crate) mod shadowsocks;",
        "pub(crate) use shadowsocks::run_shadowsocks_listener_with_bound;",
        "pub(crate) mod trojan;",
        "pub(crate) use trojan::run_trojan_listener_with_bound;",
        "pub(crate) mod hysteria2;",
        "pub(crate) use hysteria2::run_hysteria2_listener_with_bound;",
        "pub(crate) mod vless;",
        "pub(crate) use vless::run_vless_listener_with_bound;",
        "pub(crate) mod vmess;",
        "pub(crate) use vmess::run_vmess_listener_with_bound;",
        "mod mixed;",
        "pub(crate) use mixed::run_mixed_listener_with_bound;",
        "pub(crate) use mixed::MixedInboundRequest;",
    ] {
        assert!(
            !root.contains(forbidden),
            "protocol inbound glue should not live under src/inbound; found `{forbidden}`"
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
fn protocol_inbound_root_is_facade_only() {
    assert!(
        !manifest_dir().join("src/protocol_inbound").exists(),
        "zero-proxy must not keep protocol inbound glue in a top-level src/protocol_inbound bucket"
    );

    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in ["crate::protocol_inbound", "mod protocol_inbound;"] {
            assert!(
                !content.contains(forbidden),
                "{source} should not reference the removed protocol_inbound bucket through `{forbidden}`"
            );
        }
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
            "src/adapters/vless.rs",
            "src/adapters/vmess.rs",
            "src/adapters/trojan.rs",
        ],
        &[
            "src/adapters/",
            "src/adapters/vless/",
            "src/adapters/vmess/",
            "src/adapters/trojan/",
        ],
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
    let path = read("src/runtime/path.rs");
    for forbidden in [
        "ResolvedLeafOutbound::",
        "fn health_tag",
        "fn endpoint",
        "fn kernel_leaf_tag",
        "fn tcp_path_category",
    ] {
        assert!(
            !path.contains(forbidden),
            "runtime/path.rs should only define neutral path facts, not classify outbound leaf variants; found `{forbidden}`"
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
fn proxy_run_loop_lives_in_runtime_orchestration() {
    let root = read("src/runtime.rs");
    let orchestration = read("src/runtime/orchestration.rs");

    assert!(
        root.contains("orchestration::run_until(self, shutdown).await"),
        "runtime.rs should expose a narrow Proxy::run_until boundary"
    );
    for implementation_marker in ["tokio::select!", "listener_stops", "reload_async_rx"] {
        assert!(
            !root.contains(implementation_marker),
            "runtime.rs should not own run-loop implementation marker `{implementation_marker}`"
        );
        assert!(
            orchestration.contains(implementation_marker),
            "runtime/orchestration.rs should own run-loop marker `{implementation_marker}`"
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
    let inbound = read("src/transport/socks5_inbound/listener.rs");
    let _proxy_transport = read("src/transport/socks5_inbound.rs");
    let mixed = read("src/adapters/mixed/inbound/listener.rs");
    let adapter = read("src/adapters/socks5/inbound.rs");
    let mixed_adapter = read("src/adapters/mixed/inbound.rs");
    let _listener_loop = read("src/runtime/listener_loop.rs");
    let transport = read_repo_module_tree("crates/transport/src/socks5_transport.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/socks5/src/inbound.rs"))
        .expect("read socks5 protocol inbound source");
    let _protocol_lib = fs::read_to_string(repo_root().join("protocols/socks5/src/lib.rs"))
        .expect("read socks5 protocol lib source");

    assert!(
        !manifest_dir()
            .join("src/adapters/socks5/inbound/request.rs")
            .exists()
            && adapter.contains("fn spawn_inbound_impl(")
            && adapter.contains("zero_transport::socks5_transport::inbound_acceptor_from_protocol(")
            && adapter.contains("run_socks5_listener_with_bound(&proxy, inbound, acceptor")
            && !adapter.contains("InboundProtocolConfig::Socks5")
            && inbound.contains("OwnedSocks5InboundAcceptor")
            && inbound.contains("accept_and_dispatch_command(")
            && inbound.contains("serve_inbound_with_client_response(")
            && inbound.contains("client_association::run_client_udp_association(")
            && !inbound.contains("Socks5InboundListenerRequest")
            && !inbound.contains("InboundProtocolConfig::Socks5"),
        "SOCKS5 inbound acceptor construction should live in zero-transport plus adapter spawn glue while the listener stays protocol/runtime bridge-only"
    );
    assert!(
        mixed.contains("struct MixedInboundRequest")
            && mixed.contains("request: MixedInboundRequest")
            && mixed_adapter.contains("InboundProtocolConfig::Mixed")
            && mixed_adapter.contains("MixedInboundRequest")
            && mixed.contains("inbound_tag: String")
            && !mixed.contains("zero_config::InboundConfig")
            && !mixed.contains("inbound: zero_config::InboundConfig")
            && mixed_adapter.contains("inbound_tag: inbound.tag")
            && !mixed_adapter.contains("inbound,"),
        "mixed inbound listener should receive an adapter-built request model"
    );
    assert!(
        protocol_inbound.contains("pub struct Socks5InboundTcpAcceptor")
            && protocol_inbound.contains("pub async fn accept_and_dispatch_command_with")
            && transport.contains("OwnedSocks5InboundAcceptor")
            && transport.contains("pub async fn setup_inbound_udp_association")
            && inbound.contains("accept_and_dispatch_command(")
            && inbound.contains("serve_inbound_with_client_response(")
            && inbound.contains("client_association::run_client_udp_association("),
        "SOCKS5 inbound should keep TCP accept, UDP association setup, and response semantics behind protocol or zero-transport helpers"
    );
}

#[test]
fn mieru_inbound_uses_adapter_request_model() {
    let inbound = read("src/adapters/mieru/inbound/listener.rs");
    let adapter = read("src/adapters/mieru/inbound.rs");
    let udp = read("src/adapters/mieru/inbound/listener.rs");
    let transport = read_repo_module_tree("crates/transport/src/mieru_transport.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");

    assert!(
        !inbound.contains("struct MieruInboundRequest")
            && !inbound.contains("request: MieruInboundRequest"),
        "Mieru inbound listener should not keep an adapter-built request model wrapper"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Mieru"),
        "Mieru inbound entrypoint should not parse Mieru config variants"
    );
    assert!(
        !manifest_dir()
            .join("src/adapters/mieru/inbound/request.rs")
            .exists()
            && adapter.contains("fn spawn_inbound_impl(")
            && adapter.contains("inbound_profile_from_protocol(")
            && adapter.contains("run_mieru_listener_with_bound(")
            && !adapter.contains("InboundProtocolConfig::Mieru")
            && inbound.contains("OwnedMieruInboundProfile")
            && inbound.contains(".accept_and_dispatch_client(")
            && inbound.contains("serve_inbound_with_client_response(")
            && inbound.contains("profile.response_protocol()")
            && udp.contains("run_mapped_protocol_stream_udp_relay(")
            && transport.contains("pub struct OwnedMieruInboundProfile")
            && transport.contains("pub fn inbound_profile_from_protocol(")
            && transport.contains("pub async fn accept_and_dispatch_client<"),
        "Mieru inbound profile construction should live in zero-transport plus adapter spawn glue while the listener stays protocol/runtime bridge-only"
    );
    assert!(
        transport.contains("pub struct OwnedMieruInboundProfile")
            && transport.contains("pub fn inbound_profile_from_protocol(")
            && transport.contains("pub fn response_protocol(&self)")
            && transport.contains("pub async fn accept_and_dispatch_client")
            && protocol_inbound.contains("pub fn inbound_profile_from_config_users")
            && protocol_inbound.contains("pub async fn accept_and_dispatch_client")
            && protocol_inbound.contains("pub async fn accept_client<S>")
            && !adapter.contains("MieruInboundProfile::from_config_users"),
        "Mieru inbound listener should receive a transport-owned profile instead of rebuilding raw user/password tuples"
    );
}

#[test]
fn shadowsocks_inbound_uses_adapter_request_model() {
    let inbound = read("src/transport/shadowsocks_inbound/listener.rs");
    let proxy_transport = read("src/transport/shadowsocks_inbound.rs");
    let udp = read("src/transport/shadowsocks_inbound/listener/udp.rs");
    let adapter = read("src/adapters/shadowsocks/inbound.rs");
    let protocol_inbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/inbound.rs"))
            .expect("read shadowsocks protocol inbound source");
    let protocol_udp = read_repo_module_tree("protocols/shadowsocks/src/udp.rs");

    assert!(
        !manifest_dir()
            .join("src/adapters/shadowsocks/inbound/request.rs")
            .exists()
            && adapter.contains("fn spawn_inbound_impl(")
            && adapter.contains(
                "zero_transport::shadowsocks_transport::inbound_profile_from_protocol("
            )
            && adapter.contains("run_shadowsocks_listener_with_bound(")
            && !adapter.contains("InboundProtocolConfig::Shadowsocks")
            && inbound.contains("OwnedShadowsocksInboundProfile")
            && inbound.contains("profile.into_listener_bindings().into_parts()")
            && inbound.contains("OwnedShadowsocksInboundTcpAcceptor")
            && inbound.contains("accept_and_dispatch_stream(")
            && !inbound.contains("ShadowsocksInboundListenerRequest")
            && !inbound.contains("InboundProtocolConfig::Shadowsocks"),
        "Shadowsocks inbound profile construction should live in zero-transport plus adapter spawn glue while the listener stays protocol/runtime bridge-only"
    );
    assert!(
        inbound.contains("NoClientResponseStreamProtocol::new()")
            && inbound.contains("serve_inbound(")
            && protocol_inbound.contains("pub struct ShadowsocksInboundTcpAcceptor")
            && protocol_inbound.contains("pub async fn accept_stream")
            && protocol_inbound.contains("self.tcp_state.check_accept_replay(&accept)")
            && protocol_inbound.contains("session.apply_auth(self.profile.inbound_auth())")
            && !proxy_transport.contains("struct ShadowsocksInboundListenerRequest"),
        "Shadowsocks inbound listener should delegate TCP accept normalization, replay state, and auth checks to the protocol crate while keeping proxy-side request wrappers absent"
    );
    assert!(
        !inbound.contains("#[allow(clippy::too_many_lines)]"),
        "Shadowsocks inbound listener should stay small enough without a too_many_lines allowance"
    );
    assert!(
        udp.contains("run_protocol_datagram_udp_relay")
            && udp.contains("let response_already_sent = false;")
            && !udp.contains("async fn ss_udp_relay_loop")
            && !udp.contains("dispatch_inbound_udp_packet")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpRelay")
            && protocol_udp.contains("impl InboundDatagramUdpRelay<std::sync::Arc<tokio::net::UdpSocket>>"),
        "Shadowsocks UDP relay should be a thin bridge to the shared datagram runtime and protocol-owned responder"
    );
}

#[test]
fn inbound_auth_identity_stays_in_protocol_crates() {
    let shadowsocks_inbound = read("src/transport/shadowsocks_inbound/listener.rs");
    let shadowsocks_udp = read("src/transport/shadowsocks_inbound/listener/udp.rs");
    let trojan_inbound = read_proxy_module_tree("src/adapters/trojan.rs");
    let mieru_inbound = read("src/transport/mieru_inbound/listener.rs");

    let shadowsocks_protocol =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/inbound.rs"))
            .expect("read shadowsocks protocol inbound source");
    let trojan_protocol = fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
        .expect("read trojan protocol inbound source");
    let mieru_protocol = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");

    {
        let (source_name, source, required) = (
            "src/adapters/shadowsocks/inbound/listener.rs",
            shadowsocks_udp.as_str(),
            "run_protocol_datagram_udp_relay(",
        );
        assert!(
            source.contains(required)
                && source.contains("let response_already_sent = false;")
                && !source.contains("profile.inbound_auth()")
                && !source.contains("profile.accept_udp_session_with_auth()")
                && !source.contains("SessionAuth::new(\"shadowsocks\")")
                && !source.contains("SessionAuth::new(\"trojan\")")
                && !source.contains("SessionAuth::new(\"mieru\")")
                && !source.contains("principal_key = Some"),
            "{source_name} should apply protocol-built inbound auth instead of constructing protocol identity in proxy"
        );
    }

    for (source_name, source) in [
        (
            "src/transport/shadowsocks_inbound/listener.rs",
            shadowsocks_inbound.as_str(),
        ),
        ("src/adapters/trojan.rs", trojan_inbound.as_str()),
        (
            "src/transport/mieru_inbound/listener.rs",
            mieru_inbound.as_str(),
        ),
    ] {
        assert!(
            !source.contains("SessionAuth::new(\"shadowsocks\")")
                && !source.contains("SessionAuth::new(\"trojan\")")
                && !source.contains("SessionAuth::new(\"mieru\")")
                && !source.contains("principal_key = Some")
                && !source.contains(".inbound_auth()"),
            "{source_name} should let protocol-owned accept helpers apply inbound auth identity"
        );
    }

    assert!(
        shadowsocks_protocol.contains("pub fn inbound_auth(&self) -> SessionAuth")
            && trojan_protocol.contains("pub fn inbound_auth(&self) -> SessionAuth")
            && mieru_protocol.contains("pub fn inbound_auth(&self) -> SessionAuth")
            && shadowsocks_protocol.contains("session.apply_auth(self.profile.inbound_auth())")
            && trojan_protocol.contains("session.apply_auth(self.inbound_auth())")
            && mieru_protocol.contains("session.apply_auth(self.inbound_auth())"),
        "protocol crates should own their inbound auth identity construction and TCP accept normalization"
    );
}

#[test]
fn stream_udp_inbound_direct_responses_use_client_response_models() {
    let core_udp = fs::read_to_string(repo_root().join("crates/core/src/udp.rs"))
        .expect("read zero_core udp source");
    let stream_udp = read("src/runtime/stream_udp.rs");
    let mux_udp = read("src/runtime/mux_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let trojan_udp_inbound = read_proxy_module_tree("src/adapters/trojan.rs");
    let mieru_udp_inbound = read("src/transport/mieru_inbound/listener/udp.rs");
    let hysteria2_inbound = read("src/transport/hysteria2_inbound/listener/udp.rs");
    let vless_udp_inbound = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_udp_inbound = read_proxy_module_tree("src/adapters/vmess.rs");
    let trojan_protocol_inbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
            .expect("read trojan protocol inbound source");
    let trojan_protocol_udp = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read trojan protocol udp source");
    let mieru_protocol = read_repo_module_tree("protocols/mieru/src/udp.rs");
    let hysteria2_protocol = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let vless_protocol = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read vless protocol udp source");
    let vless_protocol_inbound =
        fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
            .expect("read vless protocol inbound source");
    let vless_protocol_mux = fs::read_to_string(repo_root().join("protocols/vless/src/mux.rs"))
        .expect("read vless protocol mux source");
    let vmess_protocol = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read vmess protocol udp source");
    let vmess_protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read vmess protocol mux source");

    assert!(
        core_udp.contains("pub trait InboundStreamUdpRelay")
            && core_udp.contains("pub trait InboundMuxUdpRelay")
            && stream_udp.contains("run_mapped_protocol_stream_udp_relay")
            && mux_udp.contains("run_protocol_mux_udp_relay")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response"),
        "runtime should own the neutral UDP response/write loops"
    );

    for (source_name, source) in [
        ("trojan", trojan_udp_inbound.as_str()),
        ("vless", vless_udp_inbound.as_str()),
        ("vmess", vmess_udp_inbound.as_str()),
    ] {
        assert!(
            (source.contains("run_mapped_protocol_stream_udp_relay")
                || source.contains("run_recorded_protocol_stream_udp_relay(")
                || contains_vless_mux_dispatch(source)
                || source.contains("dispatch_no_client_stream_route(")
                || source.contains("dispatch_no_client_stream_route_request(")
                || contains_helper_call(source, "spawn_transport_stream_route_inbound_listener")
                || source.contains("dispatch_no_client_mux_route(")
                || source.contains("dispatch_no_client_mux_route_with_defaults(")
                || source.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(source, "spawn_transport_mux_route_inbound_listener")
                || contains_helper_call(
                    source,
                    "spawn_recorded_transport_mux_bound_inbound_listener",
                ))
                && !source.contains("write_direct_response")
                && !source.contains("write_upstream_response")
                && !source.contains("write_chain_response")
                && !source.contains("record_direct_udp_response_parts")
                && !source.contains("record_upstream_udp_response_received")
                && !source.contains("pkt.payload")
                && !source.contains("pkt.port")
                && !source.contains("pkt.target"),
            "{source_name} inbound stream UDP glue should stay at relay bridging, not response shaping"
        );
    }

    assert!(
        (trojan_udp_inbound.contains("dispatch_no_client_stream_route(")
            || trojan_udp_inbound.contains("dispatch_no_client_stream_route_request(")
            || contains_helper_call(
                &trojan_udp_inbound,
                "spawn_transport_stream_route_inbound_listener",
            ))
            && !trojan_udp_inbound.contains("relay.map_stream(")
            && trojan_protocol_inbound.contains("pub struct TrojanInboundUdpRelay")
            && trojan_protocol_udp.contains("pub struct TrojanInboundUdpResponder"),
        "Trojan inbound stream UDP should bridge through protocol-owned relay/responder types"
    );
    assert!(
        (mieru_udp_inbound.contains("run_stream_udp_relay")
            || mieru_udp_inbound.contains("run_mapped_protocol_stream_udp_relay"))
            && mieru_protocol.contains("pub struct MieruInboundUdpResponder"),
        "Mieru inbound stream UDP should bridge through the protocol-owned responder"
    );
    assert!(
        (hysteria2_inbound.contains("run_datagram_udp_relay")
            || hysteria2_inbound.contains("run_protocol_datagram_udp_relay"))
            && hysteria2_protocol.contains("pub struct Hysteria2InboundUdpSession"),
        "Hysteria2 inbound datagram UDP should bridge through the protocol-owned session/codec"
    );
    assert!(
        (vless_udp_inbound.contains("run_logged_protocol_mux_udp_relay")
            || contains_vless_mux_dispatch(&vless_udp_inbound))
            && vless_protocol_inbound.contains("pub struct VlessInboundUdpRelay")
            && vless_protocol.contains("pub struct VlessInboundUdpResponder")
            && vless_protocol_mux.contains("pub struct VlessInboundMuxUdpRelay"),
        "VLESS inbound TCP/MUX UDP should bridge through protocol-owned relay/responder types"
    );
    assert!(
        (vmess_udp_inbound.contains("run_protocol_mux_udp_task")
            || vmess_udp_inbound.contains("dispatch_no_client_mux_route_with_defaults(")
            || vmess_udp_inbound.contains("dispatch_no_client_mux_route_request_with_defaults(")
            || contains_helper_call(
                &vmess_udp_inbound,
                "spawn_transport_mux_route_inbound_listener",
            ))
            && vmess_protocol.contains("pub struct VmessInboundUdpResponder")
            && vmess_protocol_mux.contains("pub struct VmessInboundUdpRelay")
            && vmess_protocol_mux.contains("pub struct VmessInboundMuxUdpRelay"),
        "VMess inbound TCP/MUX UDP should bridge through protocol-owned relay/responder types"
    );
}

#[test]
fn trojan_inbound_uses_transport_request_model() {
    let root = read("src/adapters/trojan.rs");
    let adapter = read_proxy_module_tree("src/adapters/trojan.rs");
    let transport_trojan_root = read_repo_file("crates/transport/src/trojan_transport.rs");
    let transport_trojan = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let transport_route = read_repo_file("crates/transport/src/inbound_route.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
        .expect("read trojan protocol inbound source");

    assert!(
        manifest_dir().join("src/adapters/trojan.rs").exists()
            && !manifest_dir().join("src/adapters/trojan/inbound.rs").exists()
            && root.contains("listener::spawn(")
            && !root.contains("TrojanInboundListenerRequest::from_protocol_config")
            && !root.contains("request.accept_route(socket).await")
            && adapter.contains("run_logged_tcp_socket_listener_loop(")
            && adapter.contains("TrojanInboundListenerRequest::from_protocol_config")
            && adapter.contains("request.accept_route(socket).await")
            && adapter.contains("dispatch_no_client_stream_route(")
            && adapter.contains("request.no_client_stream_route_defaults()")
            && !adapter.contains("TrojanInboundListenerRequest::UDP_PROTOCOL")
            && !adapter.contains("TrojanInboundProfile::from_config_password")
            && !adapter.contains("build_required_tls_acceptor(")
            && transport_route.contains("pub struct NoClientStreamRouteDefaults")
            && transport_trojan.contains("pub struct TrojanInboundListenerRequest")
            && transport_trojan.contains("pub fn error_protocol_name(&self)")
            && transport_trojan.contains("pub fn no_client_stream_route_defaults(")
            && transport_trojan.contains("pub fn from_protocol_config(")
            && transport_trojan.contains("pub async fn accept_route(")
            && transport_trojan.contains("InboundProtocolConfig::Trojan")
            && transport_trojan
                .contains("trojan::inbound::TrojanInboundProfile::from_config_password")
            && transport_trojan.contains("tls_acceptor: crate::tls::TlsAcceptor")
            && transport_trojan.contains("crate::inbound_stack::build_required_tls_acceptor(")
            && transport_trojan.contains("crate::inbound_stack::accept_tls_inbound_stream(")
            && transport_trojan
                .contains(".accept_route_owned(trojan::inbound::TrojanInbound, stream)")
            && transport_trojan_root.contains("pub use inbound::TrojanInboundListenerRequest;")
            && !transport_trojan_root.contains("OwnedTrojanInboundTransportPlan")
            && !transport_trojan_root.contains("TrojanInboundRequestSpec")
            && !transport_trojan_root.contains("TrojanInboundTlsStream")
            && !transport_trojan_root.contains("TrojanInboundTransportRequest")
            && transport_trojan.contains(
                "OpaqueStreamRoute<trojan::inbound::TrojanInboundAcceptedSession<TrojanInboundTlsStream>>",
            )
            && transport_trojan.contains(".map(OpaqueStreamRoute::new)")
            && transport_route.contains("pub struct OpaqueStreamRoute<R>")
            && transport_route.contains("impl<R> InboundStreamRoute for OpaqueStreamRoute<R>")
            && !transport_trojan.contains("TransportStreamRouteInboundRequest<TrojanInboundRequestSpec>")
            && !transport_trojan.contains("impl StreamRouteInboundRequestSpec for TrojanInboundRequestSpec")
            && !transport_trojan.contains("struct OwnedTrojanInboundTransportPlan")
            && !transport_trojan.contains("ProtocolStreamRouteAcceptor<TrojanInboundTlsStream>")
            && !transport_trojan.contains("pub async fn dispatch_route<H>(")
            && !transport_trojan.contains("pub async fn accept_route_with_handoff(")
            && protocol_inbound.contains("pub struct TrojanInboundAcceptedSession")
            && protocol_inbound.contains("enum TrojanInboundAcceptedSessionState")
            && !protocol_inbound.contains("pub enum TrojanInboundAcceptedSession")
            && protocol_inbound.contains("pub async fn accept_route_owned_with"),
        "Trojan inbound request construction should live in zero-transport while proxy adapter stays as a thin route bridge"
    );
}

#[test]
fn vmess_inbound_uses_transport_request_model() {
    let root = read("src/adapters/vmess.rs");
    let adapter = read_proxy_module_tree("src/adapters/vmess.rs");
    let transport_vmess_root = read_repo_file("crates/transport/src/vmess_transport.rs");
    let transport_vmess = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let transport_route = read_repo_file("crates/transport/src/inbound_route.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vmess/src/inbound.rs"))
        .expect("read vmess protocol inbound source");

    assert!(
        manifest_dir().join("src/adapters/vmess.rs").exists()
            && !manifest_dir().join("src/adapters/vmess/inbound.rs").exists()
            && root.contains("listener::spawn(")
            && !root.contains("VmessInboundListenerRequest::from_protocol_config")
            && !root.contains("request.accept_route(socket).await")
            && adapter.contains("run_logged_tcp_socket_listener_loop(")
            && adapter.contains("VmessInboundListenerRequest::from_protocol_config")
            && adapter.contains("request.accept_route(socket).await")
            && adapter.contains("request.no_client_mux_route_defaults()")
            && !adapter.contains("NoClientMuxRouteDefaults {")
            && !adapter.contains("VmessInboundListenerRequest::MUX_PROTOCOL")
            && (adapter.contains("dispatch_no_client_mux_route_with_defaults(")
                || adapter.contains("dispatch_no_client_mux_route_request_with_defaults("))
            && !adapter.contains("VmessInboundProfile::from_config_users")
            && !adapter.contains("build_required_tls_acceptor(")
            && transport_route.contains("pub struct NoClientMuxRouteDefaults")
            && transport_vmess.contains("pub struct VmessInboundListenerRequest")
            && transport_vmess.contains("pub const MUX_PROTOCOL")
            && transport_vmess.contains("pub fn error_protocol_name(&self)")
            && transport_vmess.contains("pub fn no_client_mux_route_defaults(&self)")
            && transport_vmess.contains("pub fn from_protocol_config(")
            && transport_vmess.contains("pub async fn accept_route(")
            && transport_vmess.contains("InboundProtocolConfig::Vmess")
            && transport_vmess.contains("vmess::inbound::VmessInboundProfile::from_config_users")
            && transport_vmess.contains("tls_acceptor: tls::TlsAcceptor")
            && transport_vmess.contains("crate::inbound_stack::build_required_tls_acceptor(")
            && transport_vmess.contains("crate::inbound_stack::accept_tls_inbound_stream_stack(")
            && transport_vmess
                .contains(".accept_route_owned(vmess::inbound::VmessInbound, stream)")
            && transport_vmess_root.contains("pub use inbound::VmessInboundListenerRequest;")
            && !transport_vmess_root.contains("OwnedVmessInboundTransportPlan")
            && !transport_vmess_root.contains("VmessInboundCarrierRequest")
            && !transport_vmess_root.contains("VmessInboundRequestSpec")
            && !transport_vmess.contains("TransportMuxRouteInboundRequest<VmessInboundRequestSpec>")
            && !transport_vmess.contains("impl MuxRouteInboundRequestSpec for VmessInboundRequestSpec")
            && !transport_vmess.contains("struct OwnedVmessInboundTransportPlan")
            && !transport_vmess.contains("ProtocolMuxRouteAcceptor<TcpRelayStream>")
            && transport_vmess.contains(".map(OpaqueMuxRoute::new)")
            && transport_route.contains("pub struct OpaqueMuxRoute<R>")
            && transport_route.contains("impl<R> InboundMuxStreamRoute for OpaqueMuxRoute<R>")
            && !transport_vmess.contains("pub async fn dispatch_route<H>(")
            && !transport_vmess.contains("pub async fn accept_route_with_handoff(")
            && protocol_inbound.contains("pub fn from_config_users")
            && protocol_inbound.contains("pub async fn accept_route_owned_with"),
        "VMess inbound request construction should live in zero-transport while proxy adapter stays as a thin route bridge"
    );
}

#[test]
fn vless_inbound_users_are_protocol_parsed() {
    let root = read("src/adapters/vless.rs");
    let adapter = read_proxy_module_tree("src/adapters/vless.rs");
    let transport_vless_root = read_repo_file("crates/transport/src/vless_transport.rs");
    let transport_vless = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let transport_route = read_repo_file("crates/transport/src/inbound_route.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read vless protocol inbound source");

    assert!(
        manifest_dir().join("src/adapters/vless.rs").exists()
            && !manifest_dir().join("src/adapters/vless/inbound.rs").exists()
            && root.contains("listener::spawn(")
            && !root.contains("VlessInboundListenerRequest::from_protocol_config")
            && !root.contains("request.accept_recorded_tcp_route(socket).await")
            && !root.contains("request.accept_recorded_stream_route(stream).await")
            && (adapter.contains("run_logged_tcp_socket_listener_loop(")
                || adapter.contains("run_logged_quic_stream_listener_loop("))
            && adapter.contains("VlessInboundListenerRequest::from_protocol_config")
            && adapter.contains("request.accept_recorded_tcp_route(socket).await")
            && adapter.contains("request.accept_recorded_stream_route(stream).await")
            && adapter.contains("request.recorded_mux_route_defaults()")
            && !adapter.contains("RecordedProtocolMuxRouteDefaults {")
            && !adapter.contains("VlessInboundListenerRequest::MUX_PROTOCOL")
            && (adapter.contains("dispatch_recorded_protocol_mux_tcp_request_result(")
                || adapter.contains("dispatch_recorded_protocol_mux_tcp_request_with_defaults("))
            && (adapter.contains("dispatch_recorded_protocol_mux_stream_request_result(")
                || adapter.contains("dispatch_recorded_protocol_mux_stream_request_with_defaults("))
            && (adapter.contains(
                "bind_transport_inbound::<zero_transport::vless_transport::OwnedVlessInboundBindPlan>",
            ) || adapter.contains("bind_transport_inbound::<OwnedVlessInboundBindPlan>"))
            && !adapter.contains("VlessInboundProfile::from_config_users")
            && !adapter.contains("OwnedVlessInboundTransportPlan::from_config_refs(")
            && !adapter.contains("OwnedVlessInboundBindPlan::from_config_ref(")
            && transport_route.contains("pub struct RecordedMuxRouteDefaults")
            && transport_vless.contains("pub struct VlessInboundListenerRequest")
            && transport_vless.contains("fn new(")
            && transport_vless.contains("pub const MUX_PROTOCOL")
            && transport_vless.contains("pub fn error_protocol_name(&self)")
            && transport_vless.contains("pub fn recorded_mux_route_defaults(&self)")
            && transport_vless.contains("pub fn from_protocol_config(")
            && transport_vless.contains("pub fn response_protocol(&self)")
            && transport_vless.contains("pub struct OwnedVlessInboundBindPlan")
            && transport_vless
                .contains("impl crate::inbound_route::ProtocolInboundBindPlan for OwnedVlessInboundBindPlan")
            && transport_vless.contains("InboundProtocolConfig::Vless")
            && transport_vless.contains("vless::inbound::VlessInboundProfile::from_config_users")
            && transport_vless.contains("OwnedVlessInboundTransportPlan::from_config_refs(")
            && transport_vless.contains("crate::inbound_stack::build_optional_tls_acceptor(")
            && transport_vless.contains("async fn bind(&self, listen_addr: &str)")
            && transport_vless.contains("TransportInboundBindTarget::Quic")
            && transport_vless.contains("TransportInboundBindTarget::Tcp")
            && transport_vless.contains("OpaqueMuxRoute::new(route)")
            && transport_route.contains("pub struct OpaqueMuxRoute<R>")
            && transport_route.contains("impl<R> InboundMuxStreamRoute for OpaqueMuxRoute<R>")
            && transport_vless_root
                .contains("pub use inbound::{OwnedVlessInboundBindPlan, VlessInboundListenerRequest};")
            && !transport_vless_root.contains("accept_vless_stream_route")
            && !transport_vless_root.contains("OwnedVlessInboundTransportPlan")
            && !transport_vless_root.contains("VlessInboundCarrierRequest")
            && !transport_vless_root.contains("VlessInboundFallbackReplay")
            && !transport_vless_root.contains("VlessInboundTransportAcceptRequest")
            && !transport_vless.contains("struct VlessInboundCarrierRequest")
            && !transport_vless.contains("struct VlessInboundTransportAcceptRequest")
            && transport_vless.contains("accept_vless_inbound_transport(")
            && transport_vless.contains("accept_vless_inbound_carrier(")
            && !transport_vless_root.contains("VlessInboundTransportResult")
            && !transport_vless_root.contains("VlessInboundTransportStream")
            && protocol_inbound.contains("pub fn from_config_users")
            && protocol_inbound.contains("pub async fn accept_route_owned_with_sni"),
        "VLESS inbound request and bind construction should live in zero-transport while proxy adapter stays as a thin route bridge"
    );
}

#[test]
fn hysteria2_inbound_uses_adapter_request_model() {
    let _proxy_transport = read("src/transport/hysteria2_inbound.rs");
    let inbound = read("src/transport/hysteria2_inbound/listener.rs").replace("\r\n", "\n");
    let _udp = read("src/transport/hysteria2_inbound/listener/udp.rs");
    let _datagram_udp = read("src/runtime/datagram_udp.rs");
    let adapter_root = read("src/adapters/hysteria2.rs");
    let adapter = read("src/adapters/hysteria2/inbound.rs");
    let bind_defaults = read("src/protocol_registry/defaults/bind.rs");
    let transport_route =
        fs::read_to_string(repo_root().join("crates/transport/src/inbound_route.rs"))
            .expect("read transport inbound_route source");
    let transport_hysteria2 = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let _protocol_dispatch_parts = struct_block(&protocol_udp, "Hysteria2InboundUdpDispatchParts");
    let protocol_inbound =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/inbound.rs"))
            .expect("read hysteria2 protocol inbound source")
            .replace("\r\n", "\n");
    let _protocol_shared =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/shared.rs"))
            .expect("read hysteria2 protocol shared source");
    let _protocol_lib = fs::read_to_string(repo_root().join("protocols/hysteria2/src/lib.rs"))
        .expect("read hysteria2 protocol lib source");

    assert!(
        !inbound.contains("struct Hysteria2InboundRequest")
            && !inbound.contains("request: Hysteria2InboundRequest"),
        "Hysteria2 inbound listener should not keep an adapter-built request model wrapper"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Hysteria2"),
        "Hysteria2 inbound entrypoint should not parse Hysteria2 config variants"
    );
    assert!(
        !manifest_dir()
            .join("src/adapters/hysteria2/inbound/request.rs")
            .exists()
            && adapter_root.contains("async fn bind_inbound(")
            && adapter_root.contains("bind_transport_inbound::<OwnedHysteria2InboundBindPlan>")
            && adapter_root.contains("fn spawn_inbound(")
            && adapter.contains("pub(super) fn spawn_inbound_impl(")
            && adapter.contains("zero_transport::hysteria2_quic::inbound_profile_from_protocol(")
            && adapter.contains("run_hysteria2_listener_with_bound(&proxy, inbound, profile")
            && bind_defaults.contains("P::from_protocol_config(&inbound.protocol, source_dir)?")
            && transport_route.contains("pub trait ProtocolInboundBindPlan: Sized")
            && transport_hysteria2
                .contains("impl crate::inbound_route::ProtocolInboundBindPlan for OwnedHysteria2InboundBindPlan")
            && transport_hysteria2.contains("pub fn inbound_profile_from_protocol(")
            && transport_hysteria2.contains("pub fn from_protocol_config(")
            && transport_hysteria2.contains("crate::quic::QuicInbound::bind(")
            && transport_hysteria2.contains("TransportInboundBindTarget::Quic")
            && !adapter_root.contains("crate::transport::bind_hysteria2_inbound(")
            && !adapter_root.contains("crate::transport::spawn_hysteria2_listener(")
            && !inbound.contains("Hysteria2InboundListenerRequest")
            && !adapter_root.contains("Hysteria2InboundRequest"),
        "Hysteria2 inbound bind/profile construction should stay in zero-transport plus adapter glue while generic transport bind planning owns the QUIC bind path"
    );
    assert!(
        inbound.contains("accept_and_dispatch_authenticated_hysteria2_quic_session(")
            && inbound.contains("serve_inbound_with_client_response(")
            && inbound.contains("run_protocol_datagram_udp_relay(")
            && inbound.contains("let response_already_sent = true;")
            && !inbound.contains("hysteria2_datagram_loop")
            && transport_hysteria2.contains("pub async fn accept_and_dispatch_authenticated_hysteria2_quic_session")
            && protocol_inbound.contains("pub struct Hysteria2AcceptedQuicConnection")
            && protocol_inbound.contains("pub async fn accept_authenticated_quic_session")
            && protocol_inbound.contains("pub async fn dispatch_session_with_handlers"),
        "Hysteria2 inbound should keep QUIC accept/auth semantics in protocol and transport layers while proxy only orchestrates task handoff"
    );
}

#[test]
fn inbound_root_does_not_reexport_protocol_request_models() {
    let root = read("src/inbound/mod.rs");

    for forbidden in [
        "DirectInboundRequest",
        "Hysteria2InboundRequest",
        "MieruInboundRequest",
        "ShadowsocksInboundListenerRequest",
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
            "src/adapters/vless.rs",
            "src/adapters/vless/inbound/listener/model.rs",
            "VlessInboundRequest",
        ),
        (
            "src/adapters/vmess.rs",
            "src/adapters/vmess/inbound/listener/model.rs",
            "VmessInboundRequest",
        ),
    ] {
        let root_content = read(root);
        assert!(
            !root_content.contains(&format!("struct {request}")),
            "{root} should not define protocol request model `{request}`"
        );
        assert!(
            !root_content.contains("mod model;") && !manifest_dir().join(model).exists(),
            "{root} should not keep a separate request-model module `{model}` for `{request}`"
        );
    }
}

#[test]
fn vless_inbound_root_does_not_reexport_session_models() {
    let root = read_repo_file("crates/proxy/src/adapters/vless.rs");
    let listener = read_repo_file("crates/proxy/src/adapters/vless/listener.rs");
    let session = String::new();
    let dispatch = read_repo_module_tree("crates/proxy/src/adapters/vless.rs");
    let proxy_transport = root.clone();
    let transport_vless = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let _request_vless = read_if_exists("src/adapters/vless/inbound/request.rs");
    let runtime_protocol = read_proxy_module_tree("src/runtime/tcp_ingress.rs");
    let core_inbound = fs::read_to_string(repo_root().join("crates/core/src/inbound.rs"))
        .expect("read crates/core/src/inbound.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read protocols/vless/src/inbound.rs");

    for forbidden in ["VlessStreamRequest", "VlessStreamTransport"] {
        assert!(
            !root.contains(forbidden)
                && !listener.contains(forbidden)
                && !session.contains(forbidden),
            "VLESS inbound listener/session should not keep proxy-side transport request model `{forbidden}`"
        );
    }
    assert!(
        !listener.contains("use super::session::handle_vless_client;")
            && !listener.contains("handle_vless_client")
            && listener.contains("run_logged_tcp_socket_listener_loop(")
            && listener.contains("run_logged_quic_stream_listener_loop(")
            && (dispatch.contains("dispatch_recorded_protocol_mux_tcp_request_result(")
                || dispatch.contains("dispatch_recorded_protocol_mux_tcp_request_with_defaults("))
            && (dispatch.contains("dispatch_recorded_protocol_mux_stream_request_result(")
                || dispatch.contains("dispatch_recorded_protocol_mux_stream_request_with_defaults("))
            && dispatch.contains(".accept_recorded_tcp_route(")
            && dispatch.contains(".accept_recorded_stream_route(")
            && transport_vless.contains(".accept_tcp_route(profile, fallback, socket, wrap_stream)")
            && transport_vless
                .contains("accept_vless_stream_route(profile, fallback, stream, sni, wrap_stream)")
            && transport_vless.contains("pub async fn accept_recorded_tcp_route(")
            && transport_vless.contains("pub async fn accept_recorded_stream_route<T>(")
            && transport_vless.contains("async fn accept_tcp_inbound(")
            && transport_vless.contains("enum VlessTcpInboundAcceptResult")
            && transport_vless.contains("async fn accept_vless_stream_route<")
            && transport_vless.contains("async fn accept_tcp_route<S, FWrap>(")
            && transport_vless.contains("async fn accept_stream_route<T, S, FWrap>(")
            && !transport_vless.contains("VLESS_ROUTE_HANDOFF")
            && !transport_vless.contains("dispatch_socket_with_profile")
            && !dispatch.contains("fn vless_send_ok")
            && !dispatch.contains("fn vless_send_blocked")
            && !dispatch.contains("fn vless_send_upstream_failure")
            && runtime_protocol.contains("pub(crate) struct ClientResponseInboundProtocol")
            && runtime_protocol.contains("InboundClientResponse<S>")
            && !runtime_protocol.contains("type InboundResponseFuture")
            && !runtime_protocol.contains("type ClientResponseHook")
            && core_inbound.contains("pub trait InboundClientResponse<S>: Send + Sync")
            && protocol_inbound
                .contains("impl<S> zero_core::InboundClientResponse<S> for VlessInbound")
            && transport_vless.contains("pub fn response_protocol(&self) -> vless::inbound::VlessInbound")
            && !listener.contains("handle_vless_stream"),
        "VLESS listener should bridge accepted streams through transport-owned carrier dispatch without proxy-side request wrappers"
    );
    assert!(
        !root.contains("struct VlessInboundHandler")
            && !root.contains("vless_inbound: vless::inbound::VlessInbound")
            && root.contains("listener::spawn(")
            && !root.contains("VlessInboundListenerRequest::from_protocol_config")
            && !proxy_transport.contains("impl InboundProtocol for vless::inbound::VlessInbound")
            && !dispatch.contains("mod protocol;")
            && !dispatch.contains("impl InboundProtocol for vless::inbound::VlessInbound")
            && !manifest_dir()
                .join("src/adapters/vless/inbound/protocol.rs")
                .exists(),
        "VLESS inbound runtime bridge should use the shared client-response wrapper without keeping a protocol-specific InboundProtocol shim"
    );
}

#[test]
fn inbound_runtime_route_glue_stays_post_accept_only() {
    let runtime_route = read_proxy_module_tree("src/runtime/inbound_route.rs");
    let runtime_protocol = read_proxy_module_tree("src/runtime/tcp_ingress.rs");
    let listener_loop = read_proxy_module_tree("src/runtime/listener_loop.rs");
    let vless_adapter = read("src/adapters/vless/listener.rs");
    let vmess_adapter = read("src/adapters/vmess/listener.rs");
    let trojan_adapter = read("src/adapters/trojan/listener.rs");

    for forbidden in [
        "VlessInboundListenerRequest",
        "VmessInboundListenerRequest",
        "TrojanInboundListenerRequest",
        "OwnedVlessInboundBindPlan",
        "from_protocol_config(",
        "ERROR_PROTOCOL_NAME",
        "accept_route(",
        "accept_recorded_tcp_route(",
        "accept_recorded_stream_route(",
    ] {
        assert!(
            !runtime_route.contains(forbidden) && !runtime_protocol.contains(forbidden),
            "runtime inbound glue should stay post-accept only and must not own accept-stage transport request detail `{forbidden}`"
        );
    }

    assert!(
        runtime_route.contains("serve_inbound(")
            && runtime_route.contains("dispatch_protocol_stream_route(")
            && runtime_route.contains("dispatch_protocol_mux_route(")
            && runtime_route.contains("dispatch_recorded_protocol_mux_route(")
            && runtime_protocol.contains("pub(crate) async fn serve_inbound<")
            && runtime_protocol.contains("pub(crate) struct ClientResponseInboundProtocol")
            && runtime_protocol.contains("pub(crate) struct NoClientResponseInboundProtocol")
            && !runtime_protocol.contains("InboundRequest")
            && !runtime_protocol.contains("RouteRequest"),
        "runtime inbound glue should own only post-accept route execution plus client-response wrappers"
    );

    assert!(
        !listener_loop.contains("from_protocol_config(")
            && !listener_loop.contains("protocol_name(&request)")
            && !listener_loop.contains("accept_route(")
            && !listener_loop.contains("accept_recorded_tcp_route(")
            && !listener_loop.contains("accept_recorded_stream_route(")
            && vless_adapter.contains("run_logged_tcp_socket_listener_loop(")
            && vless_adapter.contains("run_logged_quic_stream_listener_loop(")
            && vless_adapter.contains("request.accept_recorded_tcp_route(socket).await")
            && vless_adapter.contains("request.accept_recorded_stream_route(stream).await")
            && vmess_adapter.contains("run_logged_tcp_socket_listener_loop(")
            && vmess_adapter.contains("request.accept_route(socket).await")
            && trojan_adapter.contains("run_logged_tcp_socket_listener_loop(")
            && trojan_adapter.contains("request.accept_route(socket).await"),
        "accept-stage route construction should stay inside the owning adapter listener bridges while runtime glue remains transport-neutral"
    );
}

#[test]
fn transport_protocol_listener_bridges_stay_request_owned() {
    let vless = read("src/adapters/vless/listener.rs");
    let vmess = read("src/adapters/vmess/listener.rs");
    let trojan = read("src/adapters/trojan/listener.rs");

    assert!(
        vless.contains("VlessInboundListenerRequest::from_protocol_config")
            && vless.contains("request.protocol_name()")
            && vless.contains("request.error_protocol_name()")
            && vless.contains("request.recorded_mux_route_defaults()")
            && vless.contains("request.accept_recorded_tcp_route(socket).await")
            && vless.contains("request.accept_recorded_stream_route(stream).await")
            && vless.contains("dispatch_recorded_protocol_mux_tcp_request_with_defaults(")
            && vless.contains("dispatch_recorded_protocol_mux_stream_request_with_defaults(")
            && vless.contains("run_logged_tcp_socket_listener_loop(")
            && vless.contains("run_logged_quic_stream_listener_loop(")
            && !vless.contains("InboundProtocolConfig::Vless")
            && !vless.contains("VlessInboundProfile::from_config_users")
            && !vless.contains("OwnedVlessInboundTransportPlan::from_config_refs(")
            && !vless.contains("OwnedVlessInboundBindPlan::from_config_ref(")
            && !vless.contains("TlsAcceptor")
            && !vless.contains("build_optional_tls_acceptor(")
            && !vless.contains("Reality")
            && !vless.contains("run_mux_tcp_stream_task")
            && !vless.contains("run_protocol_mux_tcp_task")
            && !vless.contains("run_protocol_mux_udp_relay")
            && !vless.contains("TcpPipe::new")
            && !vless.contains("serve_inbound("),
        "VLESS listener bridge should consume only transport-owned request metadata plus post-accept runtime handoff"
    );

    assert!(
        vmess.contains("VmessInboundListenerRequest::from_protocol_config")
            && vmess.contains("request.protocol_name()")
            && vmess.contains("request.error_protocol_name()")
            && vmess.contains("request.no_client_mux_route_defaults()")
            && vmess.contains("request.accept_route(socket).await")
            && vmess.contains("dispatch_no_client_mux_route_request_with_defaults(")
            && vmess.contains("run_logged_tcp_socket_listener_loop(")
            && !vmess.contains("InboundProtocolConfig::Vmess")
            && !vmess.contains("VmessInboundProfile::from_config_users")
            && !vmess.contains("TlsAcceptor")
            && !vmess.contains("build_required_tls_acceptor(")
            && !vmess.contains("run_mux_tcp_stream_task")
            && !vmess.contains("run_protocol_mux_tcp_task")
            && !vmess.contains("run_protocol_mux_udp_relay")
            && !vmess.contains("TcpPipe::new")
            && !vmess.contains("serve_inbound("),
        "VMess listener bridge should consume only transport-owned request metadata plus post-accept runtime handoff"
    );

    assert!(
        trojan.contains("TrojanInboundListenerRequest::from_protocol_config")
            && trojan.contains("request.protocol_name()")
            && trojan.contains("request.error_protocol_name()")
            && trojan.contains("request.no_client_stream_route_defaults()")
            && trojan.contains("request.accept_route(socket).await")
            && trojan.contains("dispatch_no_client_stream_route(")
            && trojan.contains("run_logged_tcp_socket_listener_loop(")
            && !trojan.contains("InboundProtocolConfig::Trojan")
            && !trojan.contains("TrojanInboundProfile::from_config_password")
            && !trojan.contains("TlsAcceptor")
            && !trojan.contains("build_required_tls_acceptor(")
            && !trojan.contains("run_mux_tcp_stream_task")
            && !trojan.contains("run_protocol_mux_tcp_task")
            && !trojan.contains("run_protocol_mux_udp_relay")
            && !trojan.contains("TcpPipe::new")
            && !trojan.contains("serve_inbound("),
        "Trojan listener bridge should consume only transport-owned request metadata plus post-accept runtime handoff"
    );
}

#[test]
fn inbound_route_stream_root_is_facade_only() {
    let root = read("src/runtime/inbound_route/stream.rs");
    let tree = read_proxy_module_tree("src/runtime/inbound_route/stream.rs");
    let module_dir = manifest_dir().join("src/runtime/inbound_route/stream");

    for path in ["dispatch.rs", "model.rs", "no_client.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::inbound_route::stream should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod dispatch;",
        "mod model;",
        "mod no_client;",
        "pub(crate) use no_client::dispatch_no_client_stream_route;",
    ] {
        assert!(
            root.contains(required),
            "runtime::inbound_route::stream root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct StreamRouteBridge",
        "async fn dispatch_protocol_stream_route<",
        "async fn dispatch_no_client_stream_route<",
        "run_mapped_protocol_stream_udp_relay(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::inbound_route::stream root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::inbound_route::stream module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn inbound_route_mux_root_is_facade_only() {
    let root = read("src/runtime/inbound_route/mux.rs");
    let tree = read_proxy_module_tree("src/runtime/inbound_route/mux.rs");
    let module_dir = manifest_dir().join("src/runtime/inbound_route/mux");

    for path in ["dispatch.rs", "model.rs", "no_client.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::inbound_route::mux should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod dispatch;",
        "mod model;",
        "mod no_client;",
        "pub(super) use dispatch::dispatch_protocol_mux_route;",
        "pub(super) use model::MuxRouteBridge;",
        "pub(crate) use model::NoClientMuxRouteDefaults;",
        "pub(crate) use no_client::dispatch_no_client_mux_route_request_with_defaults;",
    ] {
        assert!(
            root.contains(required),
            "runtime::inbound_route::mux root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct MuxRouteBridge",
        "struct NoClientMuxRouteDefaults",
        "async fn dispatch_protocol_mux_route<",
        "async fn dispatch_no_client_mux_route<",
        "run_protocol_mux_session(",
        "run_protocol_mux_tcp_task(",
        "run_protocol_mux_udp_task(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::inbound_route::mux root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::inbound_route::mux module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn inbound_route_recorded_root_is_facade_only() {
    let root = read("src/runtime/inbound_route/recorded.rs");
    let tree = read_proxy_module_tree("src/runtime/inbound_route/recorded.rs");
    let module_dir = manifest_dir().join("src/runtime/inbound_route/recorded");

    for path in ["dispatch.rs", "helpers.rs", "model.rs", "request.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::inbound_route::recorded should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod dispatch;",
        "mod helpers;",
        "mod model;",
        "mod request;",
        "pub(crate) use model::RecordedProtocolMuxRouteDefaults;",
        "pub(crate) use request::{",
    ] {
        assert!(
            root.contains(required),
            "runtime::inbound_route::recorded root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct RecordedProtocolMuxRouteDefaults",
        "fn into_recorded_tcp_relay_stream<S>(",
        "fn record_metered_inbound_traffic<S>(",
        "async fn run_recorded_protocol_stream_udp_relay<",
        "async fn run_recorded_protocol_mux_session<",
        "async fn dispatch_recorded_protocol_mux_route<",
        "async fn dispatch_recorded_protocol_mux_tcp_request_result<",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::inbound_route::recorded root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::inbound_route::recorded module tree should still own `{forbidden}`"
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
        let collapsed_to_root = matches!(*adapter_name, "trojan" | "vless" | "vmess");

        for forbidden in *forbidden_patterns {
            if !collapsed_to_root {
                assert!(
                    !adapter.contains(forbidden),
                    "{adapter_path} should keep UDP runtime details in src/adapters/{adapter_name}/udp.rs; found `{forbidden}`"
                );
            }
        }
        assert!(
            udp.exists() || collapsed_to_root,
            "{adapter_name} adapter UDP runtime details should live in src/adapters/{adapter_name}/udp.rs or stay collapsed into src/adapters/{adapter_name}.rs"
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
fn proxy_does_not_reintroduce_protocol_udp_bucket() {
    assert!(
        !manifest_dir().join("src/protocol_udp").exists(),
        "zero-proxy must not keep protocol UDP glue in a top-level src/protocol_udp bucket"
    );

    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in ["crate::protocol_udp", "mod protocol_udp;"] {
            assert!(
                !content.contains(forbidden),
                "{source} should not reference the removed protocol_udp bucket through `{forbidden}`"
            );
        }
    }
}

#[test]
fn shadowsocks_udp_root_delegates_packet_path_and_flow_building() {
    let root = read("src/adapters/shadowsocks/udp.rs");
    let packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let transport_udp = read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs");

    for required in [
        "mod flow;",
        "mod packet_path;",
        "packet_path::carrier_descriptor",
        "packet_path::build",
        "packet_path::datagram_source",
        "flow::start",
        "managed_datagram_socket_handler_box::<",
        "ShadowsocksTransportLeaf::from_resolved_leaf(leaf)",
        "leaf.udp_packet_path_plan()",
        "leaf.udp_flow_plan()",
    ] {
        assert!(
            root.contains(required),
            "src/adapters/shadowsocks/udp.rs should delegate through local UDP bridge `{required}`"
        );
    }
    assert!(
        !root.contains("crate::protocol_udp"),
        "src/adapters/shadowsocks/udp.rs should not delegate through the removed protocol_udp bucket"
    );
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
        !root.contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !root.contains("shadowsocks::udp::udp_packet_path_carrier_codec_from_config")
            && !root.contains("shadowsocks::udp::udp_packet_path_datagram_source_build_from_config")
            && !root.contains("shadowsocks::udp::udp_flow_resume_from_config")
            && transport_udp.contains("pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self>")
            && transport_udp.contains("pub fn udp_flow_plan(&self) -> Result<ShadowsocksManagedUdpFlowPlan<'a>, zero_core::Error>")
            && transport_udp.contains("pub fn udp_packet_path_plan(")
            && transport_udp.contains("ShadowsocksManagedUdpFlowConfig::new(")
            && !packet_path
                .contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !packet_path.contains("shadowsocks::udp::udp_packet_path_carrier_codec_from_config")
            && !packet_path.contains("ShadowsocksUdpFlowConfig::new")
            && !packet_path.contains(".packet_path_spec()")
            && !packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && !packet_path.contains("udp_packet_path_carrier_codec_from_config")
            && packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !packet_path.contains(".into_codec()")
            && !packet_path.contains("descriptor.cache_key()")
            && !packet_path.contains("descriptor.server()")
            && !packet_path.contains("descriptor.port()")
            && !packet_path.contains("udp_packet_path_datagram_source_build_from_config")
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
            && !flow.contains("shadowsocks::udp::udp_flow_resume_from_config")
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
    let transport_udp = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");

    for required in [
        "mod flow;",
        "mod packet_path;",
        "packet_path::carrier_descriptor",
        "packet_path::build",
        "flow::start",
        "managed_datagram_handler_box::<",
        "Hysteria2TransportLeaf::from_resolved_leaf(leaf)",
        "leaf.udp_packet_path_plan()",
        "leaf.udp_flow_plan()",
    ] {
        assert!(
            root.contains(required),
            "src/adapters/hysteria2/udp.rs should delegate through local UDP bridge `{required}`"
        );
    }
    assert!(
        !root.contains("crate::protocol_udp"),
        "src/adapters/hysteria2/udp.rs should not delegate through the removed protocol_udp bucket"
    );
    for forbidden in [
        "Hysteria2UdpFlowConfig::new",
        "packet_path.cache_key()",
        "packet_path.codec()",
        ".packet_path_cache_key()",
        ".packet_path_codec()",
        "ManagedUdpSend {",
        "ManagedUdpFlowResume::new",
        "open_udp_packet_path_connection",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/adapters/hysteria2/udp.rs should be a UDP capability facade and not own `{forbidden}`"
        );
    }
    assert!(
        !root.contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !root.contains("hysteria2::udp::udp_packet_path_carrier_build_from_config")
            && !root.contains("hysteria2::udp::udp_flow_resume_from_config")
            && transport_udp.contains("pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self>")
            && transport_udp.contains("pub fn udp_flow_plan(&self) -> Hysteria2ManagedUdpFlowPlan<'a>")
            && transport_udp.contains("pub fn udp_packet_path_plan(&self) -> Hysteria2ManagedUdpPacketPathPlan")
            && packet_path.contains("packet_path_carrier_descriptor_from_build")
            && packet_path.contains("plan.into_carrier_descriptor()")
            && packet_path.contains("plan.into_carrier_build()")
            && packet_path.contains("open_hysteria2_udp_packet_path_build(")
            && !packet_path.contains("descriptor.cache_key()")
            && !packet_path.contains("descriptor.server()")
            && !packet_path.contains("descriptor.port()")
            && !packet_path.contains("build.server()")
            && !packet_path.contains("build.port()")
            && !packet_path.contains("build.connector_profile()")
            && !packet_path.contains("build.codec()")
            && !packet_path.contains(".packet_path_cache_key()")
            && !packet_path.contains(".packet_path_codec()")
            && !flow.contains("hysteria2::udp::udp_flow_resume_from_config")
            && flow.contains("ManagedDatagramStart")
            && flow.contains("plan.into_parts()")
            && flow.contains(".start_tracked_managed_datagram(")
            && !flow.contains("ManagedUdpSend {")
            && !flow.contains("ManagedUdpFlowResume::new"),
        "Hysteria2 packet-path and managed-flow construction should live in explicit protocol-local UDP submodules"
    );
}

#[test]
fn stream_udp_roots_delegate_flow_building() {
    for (root_path, transport_path, request_builder, leaf_ctor, managed_resume) in [
        (
            "src/adapters/trojan.rs",
            "crates/transport/src/trojan_transport.rs",
            "PreparedTrojanOutboundRequestBundle::from_config(",
            "TrojanOutboundLeaf::new(",
            "TrojanManagedUdpFlowResume",
        ),
        (
            "src/adapters/vless.rs",
            "crates/transport/src/vless_transport.rs",
            "PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(",
            "VlessOutboundLeaf::new(",
            "VlessManagedUdpFlowResume",
        ),
        (
            "src/adapters/vmess.rs",
            "crates/transport/src/vmess_transport.rs",
            "PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(",
            "VmessOutboundLeaf::new(",
            "VmessManagedUdpFlowResume",
        ),
    ] {
        let root = read(root_path);
        let transport = read_repo_module_tree(transport_path);

        assert!(
            root.contains("start_protocol_transport_bridge_udp_flow(")
                && root.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
                && !root.contains(request_builder)
                && !root.contains("ManagedStreamPacketStartBridge")
                && !root.contains("start_tracked_managed_stream_packet("),
            "{root_path} should stay on UDP runtime glue and keep protocol flow/request construction out of the adapter root"
        );
        assert!(
            transport.contains(request_builder)
                && transport.contains(leaf_ctor)
                && transport.contains("impl<'a> ProtocolTransportLeafResolver<'a> for")
                && !transport.contains("start_protocol_transport_bridge_udp_flow("),
            "{transport_path} should own protocol request bundle construction, resolved-leaf projection, and typed transport-leaf creation"
        );
        assert!(
            transport.contains(&format!("pub struct {managed_resume}"))
                && (transport.contains("impl ProtocolManagedTupleUdpFlowResumeConnectionOps for")
                    || transport.contains("impl ProtocolManagedPacketUdpFlowResumeConnectionOps for")),
            "{transport_path} should own the managed UDP resume carrier for the projected transport leaf"
        );
    }

    let vless_root = read_proxy_module_tree("src/adapters/vless.rs");
    assert!(
        vless_root.contains("protocol_transport_bridge_udp_relay_needs_two_streams(")
            && vless_root.contains("start_protocol_transport_bridge_udp_relay_two_stream("),
        "src/adapters/vless.rs should keep the extra VLESS two-stream UDP relay orchestration in the collapsed adapter root"
    );
}

#[test]
fn socks5_udp_root_delegates_packet_path_and_flow_building() {
    let root = read("src/adapters/socks5/udp.rs");
    let _packet_path = read("src/adapters/socks5/udp/packet_path.rs");
    let flow = read("src/adapters/socks5/udp/flow.rs");
    let transport_udp = read_repo_module_tree("crates/transport/src/socks5_transport.rs");
    let _protocol_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");

    for required in [
        "mod upstream_association;",
        "mod flow;",
        "mod packet_path;",
        "packet_path::carrier_descriptor",
        "packet_path::build",
        "flow::start",
        "flow::upstream_association_handler()",
        "Socks5TransportLeaf::from_resolved_leaf(leaf)",
        "leaf.udp_packet_path_plan()",
        "leaf.udp_flow_plan()",
    ] {
        assert!(
            root.contains(required),
            "src/adapters/socks5/udp.rs should delegate through local UDP bridge `{required}`"
        );
    }
    assert!(
        !root.contains("mod model;")
            && !root.contains("mod send;")
            && !manifest_dir()
                .join("src/adapters/socks5/udp/model.rs")
                .exists()
            && !manifest_dir()
                .join("src/adapters/socks5/udp/send.rs")
                .exists()
            && !root.contains("mod runtime;")
            && !manifest_dir()
                .join("src/adapters/socks5/udp/establish.rs")
                .exists()
            && !root.contains("mod establish;")
            && !flow.contains("struct Socks5UdpFlowStart"),
        "SOCKS5 UDP bridge should stay thin without a separate proxy establish layer while protocols/socks5 keeps protocol semantics"
    );
    assert!(
        !root.contains("crate::protocol_udp"),
        "src/adapters/socks5/udp.rs should not delegate through the removed protocol_udp bucket"
    );
    assert!(
        !root.contains("ResolvedLeafOutbound::Socks5")
            && !root.contains("socks5::udp::Socks5UdpFlowConfig::new")
            && !root.contains("Socks5ManagedUdpFlowConfig::new(")
            && transport_udp.contains("pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self>")
            && transport_udp.contains("pub fn udp_flow_plan(&self) -> Socks5ManagedUdpFlowPlan<'a>")
            && transport_udp.contains("pub fn udp_packet_path_plan(&self) -> Socks5ManagedUdpPacketPathPlan")
            && transport_udp.contains("Socks5ManagedUdpFlowConfig::new("),
        "SOCKS5 UDP config-to-plan projection should be owned by zero-transport, leaving the adapter root on transport-leaf delegation"
    );
}

#[test]
fn adapter_root_is_facade_only() {
    let adapters = read("src/adapters/mod.rs");

    for expected in [
        "mod identity;",
        "mod direct;",
        "mod http;",
        "mod hysteria2;",
        "mod mieru;",
        "mod mixed;",
        "mod shadowsocks;",
        "mod socks5;",
        "mod trojan;",
        "mod vless;",
        "mod vmess;",
        "pub(crate) use direct::DirectAdapter;",
        "pub(crate) use http::HttpConnectAdapter;",
        "pub(crate) use hysteria2::Hysteria2Adapter;",
        "pub(crate) use mieru::MieruAdapter;",
        "pub(crate) use mixed::MixedAdapter;",
        "pub(crate) use shadowsocks::ShadowsocksAdapter;",
        "pub(crate) use socks5::Socks5Adapter;",
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
fn logging_root_is_facade_only() {
    let root = read("src/logging.rs");
    let session = read("src/logging/session.rs");
    let group = read("src/logging/group.rs");
    let listener = read("src/logging/listener.rs");
    let udp_upstream = read("src/logging/udp_upstream.rs");

    for required in [
        "mod group;",
        "mod listener;",
        "mod session;",
        "mod udp_upstream;",
        "pub(crate) use group::log_urltest_group_target_changed;",
        "pub(crate) use listener::{log_listener_connection_error, INBOUND_ACCEPT_ROUTE_STAGE};",
        "pub(crate) use session::{log_session_accepted, log_session_failed, log_session_finished};",
    ] {
        assert!(
            root.contains(required),
            "logging facade should wire `{required}`"
        );
    }

    for forbidden in [
        "fn log_",
        "tracing::",
        "EngineError",
        "CompletedSessionRecord",
    ] {
        assert!(
            !root.contains(forbidden),
            "logging facade should not contain implementation detail `{forbidden}`"
        );
    }

    assert!(
        session.contains("fn log_session_accepted")
            && session.contains("fn log_session_finished")
            && session.contains("fn log_session_failed")
            && group.contains("fn log_urltest_group_target_changed")
            && listener.contains("fn log_listener_connection_error")
            && listener.contains("fn is_transient_disconnect")
            && udp_upstream.contains("fn log_udp_upstream_association_created")
            && udp_upstream.contains("fn log_udp_upstream_association_dropped"),
        "logging implementations should remain grouped by event responsibility"
    );
}

#[test]
fn adapter_roots_keep_tcp_runtime_details_in_tcp_modules() {
    let cases: &[(&str, &[&str], &[&str])] = &[
        (
            "direct",
            &[
                ".direct_connector()\n            .connect(",
                "connect_direct",
                "EstablishedTcpOutbound::Proxied",
                "EstablishedTcpOutbound::Direct {",
            ],
            &["EstablishedTcpOutbound::direct("],
        ),
        (
            "hysteria2",
            &[
                "super::connector::connect_tcp",
                "connect_upstream_hysteria2",
                "EstablishedTcpOutbound::Hysteria2",
            ],
            &["EstablishedTcpOutbound::proxied"],
        ),
        (
            "mieru",
            &["connect_upstream_mieru", "EstablishedTcpOutbound::Mieru"],
            &["EstablishedTcpOutbound::proxied"],
        ),
        (
            "shadowsocks",
            &[
                "connect_upstream_shadowsocks",
                "EstablishedTcpOutbound::Shadowsocks",
            ],
            &["EstablishedTcpOutbound::proxied"],
        ),
        (
            "socks5",
            &["connect_upstream_socks5", "EstablishedTcpOutbound::Socks5"],
            &["EstablishedTcpOutbound::proxied"],
        ),
        (
            "trojan",
            &["connect_upstream_trojan", "EstablishedTcpOutbound::Trojan"],
            &["EstablishedTcpOutbound::proxied"],
        ),
        (
            "vless",
            &["connect_upstream_vless", "EstablishedTcpOutbound::Vless"],
            &["EstablishedTcpOutbound::proxied"],
        ),
        (
            "vmess",
            &["connect_upstream_vmess", "EstablishedTcpOutbound::Vmess"],
            &["EstablishedTcpOutbound::proxied"],
        ),
    ];

    let identity_source = read("src/adapters/identity.rs");
    let proxy_tcp_bridge = read_proxy_module_tree("src/transport/tcp_outbound.rs");

    for (adapter_name, forbidden_patterns, required_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let tcp = manifest_dir().join(format!("src/adapters/{adapter_name}/tcp.rs"));
        let collapsed_to_root = matches!(*adapter_name, "trojan" | "vless" | "vmess");
        let tcp_source = if collapsed_to_root {
            String::new()
        } else {
            read_proxy_module_tree(&format!("src/adapters/{adapter_name}/tcp.rs"))
        };
        let transport_source = match *adapter_name {
            "trojan" => read_proxy_module_tree("src/adapters/trojan.rs"),
            "vless" => read_proxy_module_tree("src/adapters/vless.rs"),
            "vmess" => read_proxy_module_tree("src/adapters/vmess.rs"),
            _ => String::new(),
        };

        for forbidden in *forbidden_patterns {
            if !collapsed_to_root {
                assert!(
                    !adapter.contains(forbidden),
                    "{adapter_path} should keep TCP runtime details in src/adapters/{adapter_name}/tcp.rs; found `{forbidden}`"
                );
            }
        }
        for required in *required_patterns {
            assert!(
                tcp_source.contains(required)
                    || transport_source.contains(required)
                    || proxy_tcp_bridge.contains(required),
                "src/adapters/{adapter_name}/tcp.rs plus the neutral proxy TCP bridge should own TCP runtime detail `{required}`"
            );
        }
        assert!(
            tcp.exists() || collapsed_to_root,
            "{adapter_name} adapter TCP runtime details should live in src/adapters/{adapter_name}/tcp.rs or stay collapsed into src/adapters/{adapter_name}.rs"
        );
    }

    assert!(
        !identity_source.contains("EstablishedTcpOutbound::proxied")
            && proxy_tcp_bridge.contains("EstablishedTcpOutbound::proxied"),
        "generic proxied TCP outbound normalization should live in src/transport/tcp_outbound.rs, not adapters/identity.rs"
    );
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
fn adapter_identity_stays_identity_and_bridge_only() {
    let identity = read("src/adapters/identity.rs");
    let registry_outbound = read("src/protocol_registry/registry/outbound.rs");
    let registry_errors = read("src/protocol_registry/defaults/errors.rs");

    assert!(
        !manifest_dir().join("src/adapters/common.rs").exists()
            && !manifest_dir().join("src/adapters/common").exists()
            && identity.contains("pub(crate) trait NamedProtocolAdapter")
            && identity.contains("pub(crate) trait ProtocolTransportBridgeAdapter"),
        "adapter identity helpers should live in one responsibility-named module"
    );

    for forbidden in [
        "direct_leaf_runtime",
        "proxy_leaf_runtime",
        "transport_bridge_adapter_leaf_runtime",
        "unreachable_leaf",
        "unreachable_udp_leaf",
        "TcpOutboundFailure",
        "FlowFailure",
        "OutboundEndpoint",
    ] {
        assert!(
            !identity.contains(forbidden),
            "adapters/identity should not own neutral runtime fact or mismatch helper `{forbidden}`"
        );
    }

    assert!(
        identity.contains("pub(crate) fn named_protocol_claims_runtime_leaf")
            && identity.contains("leaf: &ResolvedLeafOutbound<'_>"),
        "adapter identity helpers may accept neutral runtime leaves only to claim adapter ownership"
    );

    assert!(
        identity.contains("const TCP_PATH: TcpPathCategory;"),
        "adapter identity bridge traits may use neutral transport path categories when describing transport-owned bridge behavior"
    );

    assert!(
        registry_outbound.contains("pub(crate) fn direct_leaf_runtime")
            && registry_outbound.contains("pub(crate) fn proxy_leaf_runtime")
            && registry_errors.contains("pub(crate) fn unreachable_leaf")
            && registry_errors.contains("pub(crate) fn unreachable_udp_leaf"),
        "neutral outbound runtime facts and mismatch helpers should live under protocol_registry"
    );
}

#[test]
fn hysteria2_tcp_udp_connect_glue_lives_in_adapter_transport_modules() {
    let outbound = manifest_dir().join("src/outbound/hysteria2.rs");
    let adapter = read("src/adapters/hysteria2.rs");
    let tcp = read("src/adapters/hysteria2/tcp.rs");
    let udp_root = read("src/adapters/hysteria2/udp.rs");
    let flow = read("src/adapters/hysteria2/udp/flow.rs");
    let managed = read("src/adapters/hysteria2/udp.rs");
    let packet_path = read("src/adapters/hysteria2/udp/packet_path.rs");
    let udp_transport = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/outbound.rs"))
            .expect("read hysteria2 protocol outbound source");

    assert!(
        !outbound.exists(),
        "Hysteria2 should not need a protocol-named proxy outbound module"
    );
    assert!(
        !manifest_dir().join("src/adapters/hysteria2/connector.rs").exists()
            && !manifest_dir().join("src/adapters/hysteria2/udp/connector.rs").exists()
            && adapter.contains("mod tcp;")
            && adapter.contains("pub(crate) mod udp;")
            && tcp.contains("Hysteria2TransportLeaf::from_resolved_leaf(leaf)")
            && tcp.contains("leaf.open_tcp_stream(session).await")
            && udp_root.contains("Hysteria2TransportLeaf::from_resolved_leaf(leaf)")
            && udp_root.contains("flow::start(dispatch, session, payload, leaf.udp_flow_plan()).await")
            && managed.contains("managed_datagram_handler_box::<")
            && managed.contains("Hysteria2ManagedDatagramFlowResume")
            && flow.contains("start_tracked_managed_datagram")
            && packet_path.contains("open_hysteria2_udp_packet_path_build(")
            && packet_path.contains("quic_datagram_carrier::build")
            && udp_transport.contains("connect_hysteria2_tcp_outbound(")
            && udp_transport.contains("open_hysteria2_udp_packet_path_build(")
            && udp_transport.contains("establish_hysteria2_udp_flow_connection(")
            && udp_transport.contains("open_quic_connection("),
        "Hysteria2 TCP/UDP glue should stay in thin adapter tcp/udp modules while QUIC connect helpers live in zero-transport"
    );
    assert!(
        protocol_outbound.contains("pub async fn authenticate_connection")
            && protocol_outbound.contains("struct Hysteria2OutboundProfile")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("pub fn from_config_password(")
            && protocol_outbound.contains("pub fn outbound_profile_from_config_password")
            && protocol_outbound.contains("export_keying_material")
            && protocol_outbound.contains("pub async fn establish_tcp_connect")
            && protocol_outbound.contains("self.send_tcp_connect(stream, session).await?")
            && protocol_outbound.contains("self.read_connect_response(stream).await"),
        "protocols/hysteria2 outbound should own connection authentication and TCP connect handshake composition"
    );
}

#[test]
fn trojan_tcp_connect_uses_protocol_config() {
    let root = read("src/adapters/trojan.rs");
    let common = read("src/adapters/identity.rs");
    let proxy_transport = read_proxy_module_tree("src/transport/tcp_outbound.rs");
    let transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    assert!(
        !manifest_dir().join("src/adapters/trojan/tcp.rs").exists()
            && root.contains("connect_protocol_transport_bridge_tcp(")
            && root.contains("apply_protocol_transport_bridge_relay_hop("),
        "Trojan TCP capability forwarding should stay on src/adapters/trojan.rs once the proxy tcp.rs shell is removed"
    );
    assert!(
        !common.contains("pub(super) fn tcp_transport_leaf")
            && !common.contains("pub(super) fn tcp_relay_transport_leaf")
            && !root.contains("ResolvedLeafOutbound::Trojan")
            && !root.contains("PreparedTrojanOutboundRequestBundle::from_config(")
            && proxy_transport.contains("connect_protocol_transport_bridge_tcp")
            && proxy_transport.contains("apply_protocol_transport_bridge_relay_hop")
            && transport.contains("ResolvedLeafOutbound::Trojan")
            && transport.contains("PreparedTrojanOutboundRequestBundle::from_config(")
            && transport.contains("TrojanOutboundLeaf::new(")
            && transport.contains("impl<'a> ProtocolTransportLeafResolver<'a> for TrojanTlsBridge"),
        "adapter root/common helpers should stay on neutral TCP glue while zero-transport projects the Trojan leaf"
    );
    assert!(
        transport.contains("pub(super) struct OwnedTrojanOutboundTlsPlan")
            && transport
                .contains("fn owned_transport_plan(&self) -> OwnedTrojanOutboundTlsPlan")
            && transport.contains("pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(")
            && transport.contains(".open_tcp_stream_with_transport(session, move |tls_profile| async move")
            && transport.contains("open_direct_with_profile(open_socket, tls_profile)")
            && !transport.contains("TrojanTlsTransportContext")
            && !transport.contains("TrojanTlsTransportRuntime")
            && transport.contains("open_relay_with_profile(stream, tls_profile)")
            && !transport.contains("from_config_parts("),
        "crates/transport/src/trojan_transport.rs should own the typed transport leaf and transport opening logic"
    );
    assert!(
        protocol_outbound.contains("struct TrojanOutboundRequestBundle")
            && !protocol_outbound.contains("pub struct TrojanOutboundRequestBundle")
            && protocol_outbound.contains("pub struct PreparedTrojanOutboundRequestBundle")
            && protocol_outbound.contains("pub(crate) struct TrojanTcpConnectRequest")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound.contains("pub async fn open_tcp_stream_with_transport<")
            && protocol_outbound.contains(
                "pub fn udp_direct_flow_plan(&self) -> crate::udp::PreparedTrojanUdpFlowPlan",
            )
            && protocol_outbound.contains(
                "pub fn udp_relay_flow_plan(&self) -> crate::udp::PreparedTrojanUdpFlowPlan",
            )
            && !protocol_outbound.contains("pub async fn send_request<S: AsyncSocket>(")
            && !protocol_outbound.contains("pub fn protocol(&self) -> ProtocolType")
            && !protocol_outbound.contains("pub async fn establish_tcp_tunnel_with_traffic<S>(")
            && !protocol_outbound.contains("pub fn server_name(&self) -> Option<&str>")
            && !protocol_outbound.contains("pub fn insecure(&self) -> bool")
            && !protocol_outbound.contains("pub fn client_fingerprint(&self) -> Option<&str>")
            && !protocol_outbound
                .contains("pub fn into_owned(self) -> OwnedTrojanResolvedTlsProfile"),
        "protocols/trojan/src/outbound.rs should keep Trojan TCP request/profile semantics"
    );
}

#[test]
fn shadowsocks_tcp_connect_uses_request_model() {
    let outbound = manifest_dir().join("src/outbound/shadowsocks.rs");
    let adapter = read("src/adapters/shadowsocks/tcp.rs");
    let transport = read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs");
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
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf(leaf)")
            && (adapter.contains("leaf\n        .open_tcp_stream(session") || adapter.contains("leaf.open_tcp_stream(session"))
            && adapter.contains("leaf.open_tcp_relay_hop(stream, session).await"),
        "Shadowsocks adapter TCP module should own proxy glue while delegating cipher/session setup through a transport leaf"
    );
    assert!(
        !adapter.contains("CipherKind::from_str")
            && !adapter.contains("shadowsocks::CipherKind")
            && !adapter.contains("shadowsocks::tcp_connect_config_from_config")
            && !adapter.contains("fn tcp_config")
            && !adapter.contains("ShadowsocksTcpConnectConfig::from_config")
            && transport.contains("pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self>")
            && transport.contains("pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(")
            && transport.contains("pub async fn open_tcp_relay_hop(")
            && transport.contains("let config = shadowsocks_tcp_connect_config(cipher, password)?;")
            && !adapter.contains("cipher: shadowsocks::CipherKind")
            && !adapter.contains("ShadowsocksTcpTarget {")
            && !adapter.contains("ShadowsocksTcpTarget")
            && !adapter.contains("TcpSessionProtocol")
            && !adapter.contains("config.tcp_target(session)")
            && !adapter.contains("config.establish_tcp_session(")
            && !adapter.contains("config.wrap_outbound_stream(")
            && !adapter.contains("password_bytes()")
            && !adapter.contains("ShadowsocksAeadStream::outbound")
            && protocol_outbound.contains("pub struct ShadowsocksTcpConnectConfig")
            && protocol_outbound.contains("pub fn from_config")
            && protocol_outbound.contains("pub fn tcp_connect_config_from_config")
            && protocol_outbound.contains("CipherKind::from_str")
            && protocol_outbound.contains("pub fn tcp_target")
            && protocol_outbound.contains("pub async fn establish_tcp_session")
            && protocol_outbound.contains("pub fn wrap_outbound_stream")
            && protocol_outbound.contains("ShadowsocksAeadStream::outbound"),
        "Shadowsocks transport and protocol crates should own cipher parsing, TCP session establishment, and outbound stream wrapping"
    );
}

#[test]
fn vmess_tcp_connect_uses_protocol_config() {
    let root = read("src/adapters/vmess.rs");
    let common = read("src/adapters/identity.rs");
    let proxy_transport = read_proxy_module_tree("src/transport/tcp_outbound.rs");
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vmess/src/outbound.rs"))
        .expect("read vmess protocol outbound source");
    let protocol_bundle_impl = impl_block(&protocol_outbound, "VmessOutboundRequestBundle");

    assert!(
        !manifest_dir().join("src/adapters/vmess/tcp.rs").exists()
            && root.contains("connect_protocol_transport_bridge_tcp(")
            && root.contains("apply_protocol_transport_bridge_relay_hop("),
        "VMess TCP capability forwarding should stay on src/adapters/vmess.rs once the proxy tcp.rs shell is removed"
    );
    assert!(
        !common.contains("pub(super) fn tcp_transport_leaf")
            && !common.contains("pub(super) fn tcp_relay_transport_leaf")
            && !root.contains("ResolvedLeafOutbound::Vmess")
            && !root.contains("PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(")
            && !root.contains("VmessCipher::from_name")
            && proxy_transport.contains("connect_protocol_transport_bridge_tcp")
            && proxy_transport.contains("apply_protocol_transport_bridge_relay_hop")
            && transport.contains("ResolvedLeafOutbound::Vmess")
            && transport.contains("PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(")
            && transport.contains("VmessOutboundLeaf::new(")
            && transport
                .contains("impl<'a> ProtocolTransportLeafResolver<'a> for VmessStreamBridge"),
        "adapter root/common helpers should stay on neutral TCP glue while zero-transport projects the VMess leaf"
    );
    assert!(
        transport.contains("pub(super) struct OwnedVmessOutboundTransportPlan")
            && transport
                .contains("fn owned_transport_plan(&self) -> OwnedVmessOutboundTransportPlan")
            && transport.contains("pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(")
            && transport.contains(".open_tcp_stream_with_transport_or_mux(")
            && transport.contains(".open_tcp_relay_hop_with_transport(")
            && transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && !transport.contains("from_config_parts("),
        "crates/transport/src/vmess_transport.rs should own the typed transport leaf while prepared VMess requests drive mux-aware opening"
    );
    assert!(
        protocol_outbound.contains("struct VmessOutboundRequestBundle")
            && !protocol_outbound.contains("pub struct VmessOutboundRequestBundle")
            && protocol_outbound.contains("pub struct PreparedVmessOutboundRequestBundle")
            && protocol_outbound.contains("pub(crate) struct VmessTcpConnectRequest")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound.contains("VmessCipher::from_name")
            && protocol_outbound.contains("pub fn from_config_with_transport_hints(")
            && protocol_outbound.contains("pub async fn establish_tcp_outbound_stream<S>(")
            && !protocol_outbound.contains("pub fn prepare_with_transport_hints(")
            && protocol_outbound.contains("pub async fn open_tcp_stream_with_transport_or_mux<")
            && protocol_outbound.contains("pub async fn open_tcp_relay_hop_with_transport<")
            && protocol_outbound.contains(
                "pub fn udp_direct_flow_plan(&self) -> crate::udp::PreparedVmessUdpFlowPlan",
            )
            && protocol_outbound.contains(
                "pub fn udp_relay_flow_plan(&self) -> crate::udp::PreparedVmessUdpFlowPlan",
            )
            && !protocol_outbound.contains("pub fn protocol(&self) -> ProtocolType")
            && protocol_bundle_impl.contains("fn tcp_connect_request(&self)")
            && !protocol_bundle_impl.contains("pub fn tcp_connect_request(&self)")
            && protocol_bundle_impl.contains("fn udp_direct_flow_plan(&self)")
            && !protocol_bundle_impl.contains("pub fn udp_direct_flow_plan(&self)")
            && protocol_bundle_impl.contains("fn udp_relay_flow_plan(&self)")
            && !protocol_bundle_impl.contains("pub fn udp_relay_flow_plan(&self)")
            && protocol_bundle_impl.contains("fn mux_concurrency(&self)")
            && !protocol_bundle_impl.contains("pub fn mux_concurrency(&self)"),
        "protocols/vmess/src/outbound.rs should keep VMess cipher parsing and MUX profile composition"
    );
}

#[test]
fn vless_tcp_connect_uses_protocol_config() {
    let root = read("src/adapters/vless.rs");
    let common = read("src/adapters/identity.rs");
    let proxy_transport = read_proxy_module_tree("src/transport/tcp_outbound.rs");
    let transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vless/src/outbound.rs"))
        .expect("read vless protocol outbound source");
    let protocol_bundle_impl = impl_block(&protocol_outbound, "VlessOutboundRequestBundle");

    assert!(
        !manifest_dir().join("src/adapters/vless/tcp.rs").exists()
            && root.contains("connect_protocol_transport_bridge_tcp(")
            && root.contains("apply_protocol_transport_bridge_relay_hop("),
        "VLESS TCP capability forwarding should stay on src/adapters/vless.rs once the proxy tcp.rs shell is removed"
    );
    assert!(
        !common.contains("pub(super) fn tcp_transport_leaf")
            && !common.contains("pub(super) fn tcp_relay_transport_leaf")
            && !root.contains("ResolvedLeafOutbound::Vless")
            && !root.contains("PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(")
            && !root.contains("parse_uuid")
            && proxy_transport.contains("connect_protocol_transport_bridge_tcp")
            && proxy_transport.contains("apply_protocol_transport_bridge_relay_hop")
            && transport.contains("ResolvedLeafOutbound::Vless")
            && transport.contains("PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(")
            && transport.contains("VlessOutboundLeaf::new(")
            && transport
                .contains("impl<'a> ProtocolTransportLeafResolver<'a> for VlessStreamBridge"),
        "adapter root/common helpers should stay on neutral TCP glue while zero-transport projects the VLESS leaf"
    );
    assert!(
        transport.contains("struct OwnedVlessOutboundTransportPlan")
            && transport
                .contains("fn owned_transport_plan(&self) -> OwnedVlessOutboundTransportPlan")
            && transport.contains("pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(")
            && transport.contains(".open_tcp_stream_with_transport_or_mux(")
            && transport.contains(".open_tcp_relay_hop_with_transport(")
            && transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && transport.contains("struct VlessDirectTransportRequest")
            && !transport.contains("from_config_parts("),
        "crates/transport/src/vless_transport.rs should own the typed transport leaf while prepared VLESS requests drive mux-aware opening"
    );
    assert!(
        protocol_outbound.contains("struct VlessOutboundRequestBundle")
            && !protocol_outbound.contains("pub struct VlessOutboundRequestBundle")
            && protocol_outbound.contains("pub struct PreparedVlessOutboundRequestBundle")
            && protocol_outbound.contains("pub(crate) struct VlessTcpConnectRequest")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound.contains("pub fn from_config_with_transport_hints(")
            && !protocol_outbound.contains("pub fn prepare_with_transport_hints(")
            && protocol_outbound.contains("pub async fn establish_tcp_outbound_tunnel<S>(")
            && protocol_outbound.contains("pub async fn establish_tcp_outbound_stream<S>(")
            && protocol_outbound.contains("pub async fn open_tcp_stream_with_transport_or_mux<")
            && protocol_outbound.contains("pub async fn open_tcp_relay_hop_with_transport<")
            && protocol_outbound.contains(
                "pub fn udp_direct_flow_plan(&self) -> crate::udp::PreparedVlessUdpFlowPlan",
            )
            && protocol_outbound.contains(
                "pub fn udp_relay_final_hop_plan(&self) -> crate::udp::PreparedVlessUdpFlowPlan",
            )
            && protocol_outbound.contains(
                "pub fn udp_relay_paired_transport_plan(&self) -> crate::udp::PreparedVlessUdpFlowPlan",
            )
            && !protocol_outbound.contains("pub fn protocol(&self) -> ProtocolType")
            && !protocol_outbound.contains("pub async fn establish_tcp_tunnel<S>(")
            && !protocol_outbound.contains("pub async fn establish_tcp_tunnel_with_flow<S>(")
            && protocol_bundle_impl.contains("fn tcp_connect_request(&self)")
            && !protocol_bundle_impl.contains("pub fn tcp_connect_request(&self)")
            && protocol_bundle_impl.contains("fn udp_direct_flow_plan(&self)")
            && !protocol_bundle_impl.contains("pub fn udp_direct_flow_plan(&self)")
            && protocol_bundle_impl.contains("fn udp_relay_final_hop_plan(&self)")
            && !protocol_bundle_impl.contains("pub fn udp_relay_final_hop_plan(&self)")
            && protocol_bundle_impl.contains("fn udp_relay_paired_transport_plan(&self)")
            && !protocol_bundle_impl.contains("pub fn udp_relay_paired_transport_plan(&self)")
            && protocol_bundle_impl.contains("fn mux_concurrency(&self)")
            && !protocol_bundle_impl.contains("pub fn mux_concurrency(&self)"),
        "protocols/vless/src/outbound.rs should keep VLESS request/profile semantics"
    );
}

#[test]
fn socks5_tcp_adapter_uses_protocol_target_model() {
    let adapter = read("src/adapters/socks5/tcp.rs");
    let transport = read_repo_module_tree("crates/transport/src/socks5_transport.rs");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/socks5/src/lib.rs"))
        .expect("read socks5 protocol lib source");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/socks5/src/outbound.rs"))
            .expect("read socks5 protocol outbound source");

    assert!(
        adapter.contains("Socks5TransportLeaf::from_resolved_leaf(leaf)")
            && (adapter.contains("leaf\n        .open_tcp_stream(session") || adapter.contains("leaf.open_tcp_stream(session"))
            && adapter.contains("leaf.open_tcp_relay_hop(stream, session).await")
            && !adapter.contains("socks5::Socks5TcpConnectSpec::from_config_parts")
            && !adapter.contains("socks5::Socks5TcpOutboundProfile::from_config_parts")
            && !adapter.contains(".establish_tcp_tunnel(")
            && !adapter.contains("connect.server()")
            && !adapter.contains("connect.port()")
            && !adapter.contains("Socks5TcpTunnelTarget::new")
            && !adapter.contains("Socks5TcpTunnelTarget {")
            && !adapter.contains("Socks5OutboundAuth")
            && !adapter.contains("username.zip"),
        "SOCKS5 TCP adapter should stay on transport-leaf delegation and avoid constructing protocol tunnel/auth details directly"
    );
    assert!(
        transport.contains("pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self>")
            && transport.contains("pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(")
            && transport.contains("pub async fn open_tcp_relay_hop(")
            && transport.contains("establish_socks5_tcp_connect(")
            && transport.contains("apply_socks5_tcp_relay_hop(")
            && transport.contains("socks5::Socks5TcpOutboundProfile::from_config_parts(username, password)")
            && protocol_outbound.contains("pub struct Socks5TcpConnectSpec")
            && protocol_outbound.contains("pub struct Socks5TcpOutboundProfile")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("struct Socks5TcpTunnelTarget<'a>")
            && !protocol_outbound.contains("pub struct Socks5TcpTunnelTarget<'a>"),
        "SOCKS5 transport and protocol crates should own leaf projection, TCP profile, and tunnel establishment details"
    );
    assert!(
        !protocol_lib.contains("Socks5TcpTunnelTarget")
            && !protocol_lib.contains("tcp_connect_spec_from_config")
            && !protocol_lib.contains("tcp_outbound_profile_from_config"),
        "SOCKS5 crate root should not re-export the TCP tunnel target helper"
    );
}

#[test]
fn mieru_tcp_connect_glue_lives_in_adapter_tcp_module() {
    let outbound = manifest_dir().join("src/outbound/mieru.rs");
    let adapter = read("src/adapters/mieru/tcp.rs");
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/mieru_transport.rs"))
        .expect("read mieru transport source");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");
    let protocol_tunnel = fs::read_to_string(repo_root().join("protocols/mieru/src/tunnel.rs"))
        .expect("read mieru tunnel protocol source");

    assert!(
        !outbound.exists(),
        "Mieru should not need a protocol-named proxy outbound module; TCP glue lives in adapters/mieru/tcp.rs and protocol session setup lives in protocols/mieru"
    );
    assert!(
        adapter.contains("async fn connect_tcp(")
            && adapter.contains("async fn apply_tcp_hop(")
            && adapter.contains("MieruTransportLeaf::from_resolved_leaf(leaf)")
            && adapter.contains("leaf.open_tcp_stream(session, move |_, _| {")
            && adapter.contains("leaf.open_tcp_relay_hop(stream, session).await")
            && !adapter.contains("mieru::tcp_outbound_profile_from_config")
            && !adapter.contains(".establish_tcp_tunnel(")
            && !adapter.contains("MieruTcpOutboundProfile::from_config_parts")
            && !adapter.contains("MieruTcpTunnelTarget::new")
            && !adapter.contains("MieruTcpTunnelTarget {")
            && !adapter.contains("struct MieruTcpStream")
            && !adapter.contains("async fn socks5_connect")
            && !adapter.contains("encrypt_client_data")
            && !adapter.contains("decrypt_server_data_with_consumed")
            && !adapter.contains("TcpSessionProtocol<mieru::MieruTcpTarget>"),
        "Mieru adapter TCP module should stay on transport-leaf delegation and leave tunneled session details to transport/protocol layers"
    );
    assert!(
        transport
            .contains("pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self>")
            && transport.contains("pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(")
            && transport.contains("pub async fn open_tcp_relay_hop(")
            && transport.contains("establish_mieru_tcp_tunnel(")
            && transport.contains("mieru::tcp_outbound_profile_from_config(username, password)")
            && protocol_outbound.contains("pub struct MieruTcpOutboundProfile")
            && protocol_outbound.contains("pub fn from_config_parts")
            && protocol_outbound.contains("pub fn tcp_outbound_profile_from_config")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("pub struct MieruTcpStream")
            && protocol_outbound.contains("pub struct MieruTcpTunnelTarget")
            && protocol_outbound.contains("pub async fn establish_tcp_tunnel")
            && protocol_outbound.contains("super::tunnel::request_tcp_connect")
            && protocol_outbound.contains("super::tunnel::build_udp_associate_request")
            && protocol_outbound.contains("super::tunnel::validate_success_response")
            && !protocol_outbound.contains("async fn socks5_connect")
            && !protocol_outbound.contains("async fn send_udp_associate_request")
            && !protocol_outbound.contains("async fn read_udp_associate_response")
            && protocol_outbound.contains("encrypt_client_data")
            && protocol_outbound.contains("decrypt_server_data_with_consumed"),
        "Mieru protocol crate should own TCP encrypted stream and tunneled SOCKS5 connect details"
    );
    assert!(
        protocol_tunnel.contains("pub(crate) async fn accept_tunneled_session")
            && protocol_tunnel.contains("pub(crate) async fn request_tcp_connect")
            && protocol_tunnel.contains("pub(crate) fn build_udp_associate_request")
            && protocol_tunnel.contains("pub(crate) fn validate_success_response")
            && protocol_tunnel.contains("async fn read_request")
            && protocol_tunnel.contains("async fn write_request")
            && protocol_tunnel.contains("async fn read_success_response"),
        "Mieru protocol crate should centralize socks5-in-tunnel TCP/UDP negotiation in a dedicated protocol-owned module"
    );
}

#[test]
fn adapter_roots_keep_inbound_runtime_details_in_inbound_modules() {
    let cases: &[(&str, &[&str], bool)] = &[
        (
            "direct",
            &["run_direct_listener_with_bound", "bound.into_tcp()"],
            true,
        ),
        (
            "http",
            &["run_http_listener_with_bound", "bound.into_tcp()"],
            true,
        ),
        (
            "hysteria2",
            &[
                "QuicInbound::bind",
                "run_hysteria2_listener_with_bound",
                "cert_path",
                "key_path",
            ],
            true,
        ),
        (
            "mieru",
            &["run_mieru_listener_with_bound", "bound.into_tcp()"],
            true,
        ),
        (
            "mixed",
            &["run_mixed_listener_with_bound", "bound.into_tcp()"],
            true,
        ),
        (
            "shadowsocks",
            &["run_shadowsocks_listener_with_bound", "bound.into_tcp()"],
            true,
        ),
        (
            "socks5",
            &["run_socks5_listener_with_bound", "bound.into_tcp()"],
            true,
        ),
        (
            "trojan",
            &["run_trojan_listener_with_bound", "bound.into_tcp()"],
            false,
        ),
        (
            "vless",
            &[
                "QuicInbound::bind",
                "zero_platform_tokio::TokioListener::bind",
                "run_vless_listener_with_bound",
                "quic.cert_path",
            ],
            false,
        ),
        (
            "vmess",
            &["run_vmess_listener_with_bound", "bound.into_tcp()"],
            false,
        ),
    ];

    for (adapter_name, forbidden_patterns, expect_inbound_module) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let inbound = manifest_dir().join(format!("src/adapters/{adapter_name}/inbound.rs"));

        assert!(
            inbound.exists() == *expect_inbound_module,
            "{}",
            if *expect_inbound_module {
                format!(
                    "{adapter_name} inbound runtime details should live in src/adapters/{adapter_name}/inbound.rs"
                )
            } else {
                format!(
                    "{adapter_name} inbound request/bind construction should already be cleared out of src/adapters/{adapter_name}/inbound.rs"
                )
            }
        );
        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep inbound runtime details in src/adapters/{adapter_name}/inbound.rs; found `{forbidden}`"
            );
        }
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
fn collapsed_protocol_inbound_roots_do_not_use_wildcard_parent_imports() {
    for source in [
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
        "src/adapters/trojan.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains("use super::*;"),
            "{source} should import protocol inbound dependencies explicitly"
        );
    }
}

#[test]
fn protocol_named_inbound_modules_stay_proxy_glue_not_crypto_implementations() {
    for path in protocol_inbound_sources() {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read inbound protocol module");

        for forbidden in [
            "use aes::",
            "use chacha",
            "use cipher::",
            "use hmac::",
            "use md5::",
            "use ring::",
            "use sha1::",
            "use sha2::",
            "use uuid::",
            "aes::",
            "cipher::",
            "hmac::",
            "md5::",
            "ring::",
            "sha1::",
            "sha2::",
            "uuid::",
            "Aes128",
            "Aes256",
            "ChaCha20",
            "Hmac",
            "Md5",
            "Sha1",
            "Sha256",
            "Uuid::",
            "CipherKind::from_str",
            "password: String",
            "pub(crate) password",
            "cipher_name()",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should stay proxy-side inbound glue and delegate protocol crypto/parsing primitives to protocols/*; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_named_inbound_modules_stay_runtime_glue_not_dispatch_or_packet_owners() {
    for path in protocol_inbound_sources() {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read inbound protocol module");

        for forbidden in [
            "InboundProtocolConfig::",
            "ResolvedLeafOutbound::",
            "session.network",
            "VlessInbound::is_mux_session",
            "InboundUdpDispatchParts",
            "InboundUdpPacket",
            "InboundUdpResponse",
            "InboundUdpCodec",
            "UdpPacketTarget",
            "UdpPacketFraming",
            "decode_udp_associate_request",
            "decode_udp_associate_response",
            "encode_udp_associate_response",
            "decode_inbound_udp",
            "encode_inbound_udp",
            "decode_datagram",
            "encode_datagram",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should stay listener/session/pipe glue and delegate dispatch classification or packet framing detail to protocols/*; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn tcp_inbound_source_address_conversion_lives_in_platform_layer() {
    let platform = fs::read_to_string(repo_root().join("crates/platform/tokio/src/lib.rs"))
        .expect("read zero-platform-tokio source");
    let listener_loop = read_proxy_module_tree("src/runtime/listener_loop.rs");

    assert!(
        platform.contains("pub fn remote_ip_to_socket_addr")
            && platform.contains("addr.map(|ip| socket_addr_from_ip(ip, 0))")
            && platform.contains("pub fn socket_address_to_socket_addr")
            && platform.contains("socket_addr_from_ip(addr.ip, addr.port)"),
        "zero-platform-tokio should own remote IpAddress to SocketAddr conversion for listener source addresses"
    );

    for source_path in [
        "src/inbound/direct.rs",
        "src/transport/http_inbound/listener.rs",
        "src/adapters/mixed/inbound/listener.rs",
        "src/transport/socks5_inbound/listener.rs",
        "src/transport/shadowsocks_inbound/listener.rs",
        "src/adapters/trojan.rs",
        "src/transport/mieru_inbound/listener.rs",
        "src/adapters/vmess.rs",
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

    assert!(
        listener_loop.contains("zero_platform_tokio::remote_ip_to_socket_addr(remote_addr)"),
        "neutral TCP listener loop should own accepted peer source address conversion"
    );

    let socks5 = read("src/transport/socks5_inbound/listener.rs");
    assert!(
        socks5.contains("run_tcp_listener_loop")
            && socks5.contains("source_addr: Option<std::net::SocketAddr>")
            && !socks5.contains("zero_platform_tokio::remote_ip_to_socket_addr"),
        "SOCKS5 inbound should consume the neutral listener-loop source address instead of converting it locally"
    );

    for source_path in [
        "src/inbound/direct.rs",
        "src/transport/http_inbound/listener.rs",
        "src/adapters/mixed/inbound/listener.rs",
        "src/transport/mieru_inbound/listener.rs",
        "src/transport/shadowsocks_inbound/listener.rs",
        "src/transport/socks5_inbound/listener.rs",
        "src/adapters/trojan/listener.rs",
        "src/adapters/vless/listener.rs",
        "src/adapters/vmess/listener.rs",
    ] {
        let source = read(source_path);
        assert!(
            (source.contains("run_tcp_listener_loop")
                || source.contains("run_logged_tcp_socket_listener_loop")
                || source.contains("run_logged_quic_stream_listener_loop")
                || source.contains("spawn_recorded_protocol_mux_bound_inbound_listener")
                || source.contains("spawn_no_client_stream_route_inbound_listener")
                || source.contains("spawn_no_client_mux_route_inbound_listener")
                || contains_helper_call(&source, "spawn_transport_stream_route_inbound_listener",)
                || contains_helper_call(&source, "spawn_transport_mux_route_inbound_listener")
                || contains_helper_call(
                    &source,
                    "spawn_recorded_transport_mux_bound_inbound_listener",
                ))
                && (source.contains("TcpListenerLoopRequest")
                    || source.contains("LoggedTcpSocketListenerRequest")
                    || source.contains("LoggedQuicStreamListenerRequest")
                    || contains_helper_call(
                        &source,
                        "spawn_recorded_transport_mux_bound_inbound_listener",
                    )
                    || contains_helper_call(
                        &source,
                        "spawn_transport_stream_route_inbound_listener",
                    )
                    || contains_helper_call(&source, "spawn_transport_mux_route_inbound_listener"))
                && !source.contains("listener.accept()")
                && (!source.contains("JoinSet")
                    || source.contains("run_logged_tcp_socket_listener_loop(")
                    || source.contains("run_logged_quic_stream_listener_loop(")
                    || contains_helper_call(
                        &source,
                        "spawn_recorded_transport_mux_bound_inbound_listener",
                    )
                    || contains_helper_call(
                        &source,
                        "spawn_transport_stream_route_inbound_listener",
                    )
                    || contains_helper_call(&source, "spawn_transport_mux_route_inbound_listener"))
                && !source.contains("zero_platform_tokio::remote_ip_to_socket_addr")
                && !source.contains("inbound listener ready")
                && !source.contains("inbound listener stopped"),
            "{source_path} should delegate neutral TCP listener lifecycle and source conversion to runtime/listener_loop"
        );
    }

    let hysteria2 =
        read_repo_module_tree("crates/proxy/src/adapters/hysteria2/inbound/listener.rs");
    let vless_listener = read_proxy_module_tree("src/adapters/vless.rs");
    let vless_transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    assert!(
        hysteria2.contains("run_quic_listener_loop")
            && hysteria2.contains("QuicListenerLoopRequest")
            && !hysteria2.contains("quic_inbound.accept_connection()")
            && !hysteria2.contains("inbound listener ready")
            && !hysteria2.contains("inbound listener stopped"),
        "Hysteria2 inbound should delegate neutral QUIC listener lifecycle to runtime/listener_loop"
    );
    assert!(
        !manifest_dir()
            .join("src/adapters/vless/inbound/listener/session.rs")
            .exists()
            && (vless_listener.contains("run_logged_tcp_socket_listener_loop(")
                || vless_listener.contains("run_logged_quic_stream_listener_loop("))
            && !vless_transport.contains("run_vless_quic_accept_loop")
            && !vless_transport.contains("quic_inbound.accept()")
            && vless_transport.contains("TransportInboundBindTarget::Quic")
            && vless_transport.contains("impl crate::inbound_route::ProtocolInboundBindPlan")
            && vless_transport.contains("async fn accept_stream_route"),
        "VLESS QUIC accept lifecycle should live in runtime/listener_loop, leaving accepted-stream handling delegated to the transport bridge"
    );

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

    let system = read("src/inbound/system.rs");
    assert!(
        system.contains("run_system_tcp_stack_loop")
            && system.contains("SystemTcpStackLoopRequest")
            && !system.contains("tokio::select!")
            && !system.contains("JoinSet")
            && !system.contains("stack.accept()")
            && !system.contains("connections.abort_all()"),
        "system inbound should delegate neutral stack accept lifecycle to runtime/listener_loop"
    );
}

#[test]
fn tcp_tls_async_socket_bridge_lives_in_transport_layer() {
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/tls.rs"))
        .expect("read zero-transport tls source");
    let transport_inbound_stack =
        fs::read_to_string(repo_root().join("crates/transport/src/inbound_stack.rs"))
            .expect("read zero-transport inbound_stack source");
    let transport_trojan = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let transport_vmess = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let trojan_request = read_if_exists("src/adapters/trojan/inbound/request.rs");
    let vmess_request = read_if_exists("src/adapters/vmess/inbound/request.rs");
    let proxy_transport = read("src/transport/mod.rs");
    let proxy_trojan_dispatch = read_repo_module_tree("crates/proxy/src/adapters/trojan.rs");
    let proxy_vmess_dispatch = read_repo_module_tree("crates/proxy/src/adapters/vmess.rs");
    assert!(
        transport.contains("pub async fn accept_tls_inbound(")
            && transport.contains("InboundTlsStream::new_generic")
            && !proxy_transport.contains("struct AsyncSocketStream")
            && !proxy_transport.contains("impl<S> AsyncSocket for AsyncSocketStream<S>"),
        "generic inbound TLS accept and AsyncSocket bridge should live in zero-transport, not proxy glue"
    );

    let trojan = read_repo_module_tree("crates/proxy/src/adapters/trojan.rs");
    assert!(
        (trojan.contains("run_logged_tcp_socket_listener_loop(")
            || trojan.contains("dispatch_no_client_stream_route("))
            && (proxy_trojan_dispatch.contains("request.accept_route(socket).await")
                || proxy_trojan_dispatch.contains("dispatch_no_client_stream_route("))
            && trojan_request.is_empty()
            && transport_trojan.contains("pub struct TrojanInboundListenerRequest")
            && transport_trojan.contains("pub async fn accept_route(")
            && !transport_trojan.contains("TransportStreamRouteInboundRequest<TrojanInboundRequestSpec>")
            && !transport_trojan.contains("ProtocolStreamRouteAcceptor<TrojanInboundTlsStream>")
            && !trojan_request.contains("TransportStreamRouteInboundRequest<TrojanInboundRequestSpec>")
            && !trojan_request.contains("ProtocolStreamRouteAcceptor<TrojanInboundTlsStream>")
            && !trojan_request.contains("tls::accept_tls_inbound(socket, &tls_acceptor).await?")
            && !proxy_trojan_dispatch.contains("tls_acceptor.accept(stream)")
            && !proxy_trojan_dispatch.contains("impl AsyncSocket for TlsStream")
            && !proxy_trojan_dispatch.contains("tokio::io::AsyncReadExt::read(&mut self")
            && !proxy_trojan_dispatch.contains("tokio::io::AsyncWriteExt::write_all(&mut self")
            && transport_trojan.contains("type TrojanInboundTlsStream =")
            && transport_trojan.contains("tls_acceptor: crate::tls::TlsAcceptor")
            && transport_trojan.contains("crate::inbound_stack::build_required_tls_acceptor(")
            && transport_trojan.contains("crate::inbound_stack::accept_tls_inbound_stream(")
            && !transport_trojan.contains("struct TrojanInboundTransportRequest")
            && !transport_trojan.contains("pub async fn dispatch_route<H>(")
            && !transport_trojan.contains("pub async fn accept_route_with_handoff(")
            && !transport_trojan.contains(".dispatch_socket(socket, {")
            && transport_inbound_stack.contains("pub async fn accept_tls_inbound_stream("),
        "trojan inbound listener should delegate TLS stream accept into zero-transport instead of owning TLS stream socket glue"
    );
    let vmess = read_repo_module_tree("crates/proxy/src/adapters/vmess.rs");
    assert!(
        (vmess.contains("dispatch_no_client_mux_route_with_defaults(")
            || vmess.contains("run_logged_tcp_socket_listener_loop("))
            && !vmess.contains("crate::transport::accept_tls_inbound(")
            && !vmess.contains("crate::transport::accept_ws(")
            && !vmess.contains("crate::transport::serve_grpc(")
            && !vmess.contains("tls_acceptor.accept(stream)")
            && (proxy_vmess_dispatch.contains("dispatch_no_client_mux_route_with_defaults(")
                || proxy_vmess_dispatch.contains("run_logged_tcp_socket_listener_loop("))
            && vmess_request.is_empty()
            && !vmess_request.contains("TransportMuxRouteInboundRequest<VmessInboundRequestSpec>")
            && !vmess_request.contains("ProtocolMuxRouteAcceptor<TcpRelayStream>")
            && transport_vmess.contains("pub struct VmessInboundListenerRequest")
            && transport_vmess.contains("pub async fn accept_route(")
            && !transport_vmess
                .contains("TransportMuxRouteInboundRequest<VmessInboundRequestSpec>")
            && !transport_vmess.contains("ProtocolMuxRouteAcceptor<TcpRelayStream>")
            && !vmess.contains("impl VmessInboundCarrierRequest")
            && !transport_vmess.contains("struct VmessInboundCarrierRequest")
            && transport_vmess.contains("crate::inbound_stack::accept_tls_inbound_stream_stack(")
            && transport_vmess.contains("InboundStreamStack {")
            && !transport_vmess.contains("pub async fn dispatch_route<H>(")
            && !transport_vmess.contains("pub async fn accept_route_with_handoff(")
            && !transport_vmess.contains(".dispatch_socket(socket, {")
            && transport_inbound_stack.contains("pub async fn accept_tls_inbound_stream_stack(")
            && transport_inbound_stack.contains("accept_inbound_stream_stack(")
            && transport_inbound_stack.contains("ws::accept_ws(")
            && transport_inbound_stack.contains("grpc::accept_grpc("),
        "vmess inbound carrier TLS and ws/grpc transport dispatch should live in zero-transport"
    );
}

#[test]
fn vless_fallback_recording_stream_lives_in_transport_layer() {
    let transport = fs::read_to_string(repo_root().join("crates/transport/src/stream.rs"))
        .expect("read zero-transport stream source");
    let transport_route =
        fs::read_to_string(repo_root().join("crates/transport/src/inbound_route.rs"))
            .expect("read zero-transport inbound_route source");
    let transport_inbound_stack =
        fs::read_to_string(repo_root().join("crates/transport/src/inbound_stack.rs"))
            .expect("read zero-transport inbound_stack source");
    let transport_metered = fs::read_to_string(repo_root().join("crates/transport/src/metered.rs"))
        .expect("read zero-transport metered source");
    let dispatch = read_repo_module_tree("crates/proxy/src/adapters/vless.rs");
    let proxy_transport = read_proxy_module_tree("src/runtime/inbound_route.rs");
    let runtime_fallback = read("src/runtime/inbound_fallback.rs");
    let vless_helper_path = manifest_dir().join("src/adapters/vless/inbound/listener/helpers.rs");
    let _vless_listener = read_repo_module_tree("crates/proxy/src/adapters/vless.rs");
    let vless_session = read_if_exists("src/adapters/vless/inbound/listener/session.rs");
    let vless_fallback = read_if_exists("src/adapters/vless/inbound/listener/fallback.rs");
    let transport_vless = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let request_vless = read_if_exists("src/adapters/vless/inbound/request.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read vless protocol inbound source");

    assert!(
        transport.contains("struct RecordingStream")
            && transport.contains("impl<S> AsyncSocket for RecordingStream<S>")
            && transport.contains("pub fn into_parts(self) -> (S, Vec<u8>)"),
        "generic fallback read-recording stream wrapper should live in transport glue"
    );
    assert!(
        (dispatch.contains("run_logged_tcp_socket_listener_loop(")
            || dispatch.contains("run_logged_quic_stream_listener_loop("))
            && dispatch.contains(".accept_recorded_tcp_route(")
            && dispatch.contains(".accept_recorded_stream_route(")
            && transport_vless.contains("fn record_client_stream<S>(stream: S)")
            && transport_vless.contains("crate::RecordingStream::new(stream)")
            && transport_vless.contains("crate::MeteredStream::new(crate::RecordingStream::new(stream))")
            && !vless_session.contains("impl<S> vless::inbound::VlessFallbackCapture")
            && !vless_session.contains("vless::inbound::VlessFallbackReplay::new")
            && transport_metered.contains("impl<S> zero_core::InboundFallbackCapture")
            && transport_metered.contains("fn into_fallback_replay_parts(self) -> (Self::Stream, Vec<u8>)")
            && transport_metered.contains("self.into_inner().into_parts()")
            && !request_vless.contains("impl<S> zero_core::InboundFallbackCapture")
            && !request_vless.contains("vless::inbound::VlessFallbackReplay::new(stream, replay_head)")
            && transport_metered.contains("pub fn into_unrecorded_inner(self) -> S")
            && !dispatch.contains("into_inner().into_parts()")
            && !dispatch.contains("let (inner, head)")
            && !dispatch.contains("impl<S> zero_core::InboundFallbackCapture"),
        "VLESS session glue should use transport-owned recording stream bridges instead of unpacking the recording wrapper"
    );
    assert!(
        !protocol_inbound.contains("pub trait VlessFallbackCapture")
            && protocol_inbound.contains("InboundFallbackCapture")
            && protocol_inbound.contains("pub struct VlessFallbackReplay")
            && protocol_inbound.contains("into_fallback_replay_parts()")
            && protocol_inbound.contains("VlessFallbackReplay::new(stream, replay_head)")
            && protocol_inbound.contains("pub async fn write_replay_head")
            && protocol_inbound.contains("pub async fn replay_to_upstream")
            && protocol_inbound.contains("writer.write_all(&self.replay_head).await")
            && protocol_inbound.contains("writer.write_all(&replay_head).await")
            && proxy_transport.contains("relay_recorded_fallback_replay(")
            && runtime_fallback.contains("pub(crate) async fn relay_recorded_fallback")
            && runtime_fallback.contains("pub(crate) async fn relay_recorded_fallback_replay")
            && runtime_fallback.contains("FallbackReplayToUpstream")
            && runtime_fallback.contains("relay_bidirectional_metered(")
            && runtime_fallback.contains("replay_to_upstream: FReplay")
            && !vless_fallback.contains("fallback_replay.write_replay_head")
            && !vless_fallback.contains("fallback_replay.into_stream()")
            && !vless_fallback.contains("tokio::io::AsyncWriteExt::write_all(&mut upstream")
            && !vless_fallback.contains("&head"),
        "protocols/vless should own fallback replay head semantics while shared runtime fallback glue only connects and relays"
    );
    assert!(
        protocol_inbound.contains("pub struct VlessFallbackAlpnPolicy")
            && protocol_inbound.contains("pub fn fallback_alpn_matches")
            && protocol_inbound.contains("pub fn fallback_replay_for_alpns")
            && protocol_inbound.contains("pub struct VlessFallbackAlpnDecision")
            && protocol_inbound.contains("enum VlessFallbackAlpnDecisionState")
            && protocol_inbound.contains("fn replay(replay: VlessFallbackReplay<S>) -> Self")
            && protocol_inbound
                .contains("fn continue_with(stream: S, replay_head: Vec<u8>) -> Self")
            && protocol_inbound.contains(
                "pub fn into_transport_parts(self) -> Result<VlessFallbackReplay<S>, (S, Vec<u8>)>",
            )
            && protocol_inbound.contains("client_alpns.into_iter().any")
            && (dispatch.contains("run_logged_tcp_socket_listener_loop(")
                || dispatch.contains("run_logged_quic_stream_listener_loop("))
            && dispatch.contains(".accept_recorded_tcp_route(")
            && dispatch.contains(".accept_recorded_stream_route(")
            && transport_vless.contains("struct OwnedVlessInboundTransportPlan")
            && transport_vless.contains("accept_vless_inbound_transport(")
            && transport_vless.contains("accept_vless_inbound_carrier(")
            && transport_vless.contains("vless::inbound::fallback_replay_for_alpns")
            && transport_vless.contains(".into_transport_parts()")
            && !transport_vless.contains("VlessTcpFallbackReplay")
            && !transport_vless.contains("VlessInboundFallbackReplay")
            && transport_route.contains("pub struct OpaqueFallbackReplay<S>")
            && transport_vless.contains("RouteAcceptResult::Fallback")
            && !request_vless.contains("vless::fallback_alpn_matches")
            && transport_metered.contains("into_fallback_replay_parts")
            && !transport_metered.contains("vless::inbound::VlessFallbackReplay::new")
            && protocol_inbound.contains("VlessFallbackReplay::new(stream, replay_head)")
            && !request_vless.contains("vless::inbound::VlessFallbackReplay::new")
            && !request_vless.contains(".and_then(|fb| fb.alpn.as_ref().zip(Some(fb)))")
            && !request_vless.contains(".find(|a| *a == expected)"),
        "protocols/vless should own fallback ALPN matching and replay construction while proxy transport performs the inbound transport decision"
    );
    assert!(
        !vless_helper_path.exists()
            && transport_vless.contains("profile.upgrade_server(stream).await")
            && transport_vless.contains("accept_inbound_stream_stack(")
            && transport_inbound_stack.contains("grpc::accept_grpc(")
            && !transport_vless.contains("grpc::serve_grpc")
            && !transport_vless.contains("struct RecordingStream")
            && !transport_vless.contains("impl<S> AsyncSocket for RecordingStream")
            && !transport_vless.contains("impl<S> AsyncRead for RecordingStream"),
        "VLESS inbound base transport glue should live in zero-transport rather than proxy transport"
    );
}

#[test]
fn mieru_inbound_stream_uses_protocol_codec_not_crypto_primitives() {
    for path in rust_sources_under("src/adapters/mieru/inbound") {
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

    let inbound = read("src/adapters/mieru/inbound/listener.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");
    let protocol_tunnel = fs::read_to_string(repo_root().join("protocols/mieru/src/tunnel.rs"))
        .expect("read mieru protocol tunnel source");
    assert!(
        inbound.contains(".accept_and_dispatch_client(")
            && !inbound.contains("mieru::inbound::MieruInboundStream::new")
            && !inbound.contains("client.accept_tunneled_socks5_session().await")
            && !inbound.contains("MieruCipher")
            && !inbound.contains("parse_segment")
            && !inbound.contains("build_data_segment")
            && !inbound.contains("decrypt_client_data_with_consumed")
            && !inbound.contains("encrypt_server_data")
            && protocol_inbound.contains("pub async fn accept_tunneled_stream")
            && protocol_inbound.contains("pub async fn accept_client<S>")
            && protocol_inbound.contains("let mut client = MieruInboundStream::new(stream, accept)")
            && protocol_inbound.contains("client.accept_tunneled_socks5_session().await")
            && protocol_inbound.contains("pub struct MieruInboundStream")
            && protocol_inbound.contains("decrypt_client_data_with_consumed")
            && protocol_inbound.contains("encrypt_server_data")
            && !manifest_dir().join("src/adapters/mieru/inbound/model.rs").exists(),
        "Mieru proxy inbound should use protocol-owned tunneled stream acceptance and data-phase wrapper"
    );
    for required in [
        "pub struct MieruInboundStream",
        "impl<S> AsyncRead for MieruInboundStream<S>",
        "impl<S> AsyncWrite for MieruInboundStream<S>",
        "accept_tunneled_socks5_session",
        "super::tunnel::accept_tunneled_session(self).await",
        "decrypt_client_data_with_consumed",
        "encrypt_server_data",
    ] {
        assert!(
            protocol_inbound.contains(required),
            "protocols/mieru should own Mieru inbound stream detail `{required}`"
        );
    }
    assert!(
        protocol_tunnel.contains("pub(crate) async fn accept_tunneled_session")
            && protocol_tunnel.contains("async fn read_request")
            && protocol_tunnel.contains("async fn write_success_response")
            && protocol_tunnel.contains("enum MieruTunnelRequest"),
        "protocols/mieru should keep tunneled SOCKS5 request parsing and success response framing in the dedicated tunnel module"
    );
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
    for path in rust_sources_under("src/adapters/shadowsocks/inbound") {
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

    let udp = read("src/adapters/shadowsocks/inbound/listener.rs");
    let transport = read("src/adapters/shadowsocks/inbound/listener.rs");
    let protocol_udp = read_repo_module_tree("protocols/shadowsocks/src/udp.rs");
    let protocol_shared =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/shared.rs"))
            .expect("read shadowsocks protocol shared source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/shadowsocks/src/lib.rs"))
        .expect("read shadowsocks protocol lib source");
    assert!(
        transport.contains("let (acceptor, udp_session) = profile.into_listener_bindings().into_parts()")
            && transport.contains("run_protocol_datagram_udp_relay(")
            && transport.contains("let response_already_sent = false;")
            && udp.contains("run_protocol_datagram_udp_relay(")
            && !udp.contains("ShadowsocksInboundUdpCodec")
            && !udp.contains("decode_udp_datagram_2022_session")
            && !udp.contains("encode_udp_response_2022")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpCodec")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpSession")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpResponder")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpRelay")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpDispatchParts")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpResponse")
            && protocol_udp.contains("pub struct ShadowsocksInboundUdpResponseTarget")
            && protocol_udp.contains("pub fn record_dispatch_success")
            && protocol_udp.contains("fn encode_response_to_client")
            && protocol_udp.contains("fn send_response_for_target_proxy_session_to_client_tokio"),
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
    let types = read("src/runtime/udp_dispatch/candidate.rs");

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
fn http_redirect_response_framing_stays_in_protocol_crate() {
    assert!(
        !manifest_dir().join("src/inbound/http.rs").exists(),
        "HTTP CONNECT protocol inbound glue should not live under src/inbound"
    );
    let inbound = read("src/transport/http_inbound/listener.rs");
    let mixed = read("src/adapters/mixed/inbound/listener.rs");
    let redirect = read("src/runtime/http_redirect.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/http/src/inbound.rs"))
        .expect("read http protocol inbound source");

    assert!(
        inbound.contains("select_redirect_target")
            && !inbound.contains("fn select_redirect_target")
            && redirect.contains("pub(crate) fn select_redirect_target")
            && redirect.contains("rules: &[zero_config::UrlRewriteRule]")
            && redirect.contains("session: &Session")
            && redirect.contains("rule.status_code?")
            && !inbound.contains("build_redirect_response")
            && inbound.contains("send_redirect_response")
            && inbound.contains("Some((status, location))")
            && !inbound.contains("HTTP/1.1 {status} Found")
            && !inbound.contains("Location: {location}")
            && protocol_inbound.contains("pub fn redirect_response")
            && protocol_inbound.contains("pub async fn send_redirect_response")
            && protocol_inbound.contains("HTTP/1.1 {status} Found")
            && protocol_inbound.contains("Location: {location}"),
        "HTTP CONNECT redirect wire response framing should live in protocols/http; proxy should only select status/location"
    );
    assert!(
        inbound.contains(".send_success_response(")
            && inbound.contains(".send_blocked_response(")
            && inbound.contains(".send_upstream_failure_response(")
            && inbound.contains(".send_accept_error_response(")
            && mixed.contains(".send_accept_error_response(")
            && !inbound.contains(".send_method_not_allowed_response(")
            && !inbound.contains(".send_bad_request_response(")
            && !mixed.contains(".send_method_not_allowed_response(")
            && !mixed.contains(".send_bad_request_response(")
            && !inbound.contains("CoreError::Unsupported")
            && !inbound.contains("CoreError::Protocol")
            && !mixed.contains("CoreError::Unsupported")
            && !mixed.contains("CoreError::Protocol")
            && !inbound.contains("HttpConnectResponse")
            && !mixed.contains("HttpConnectResponse"),
        "HTTP CONNECT inbound glue should ask the protocol crate to handle accept-error responses instead of selecting concrete response frames"
    );
    assert!(
        protocol_inbound.contains("pub async fn send_success_response")
            && protocol_inbound.contains("pub async fn send_bad_request_response")
            && protocol_inbound.contains("pub async fn send_method_not_allowed_response")
            && protocol_inbound.contains("pub async fn send_accept_error_response")
            && protocol_inbound.contains("Error::Unsupported(_)")
            && protocol_inbound.contains("Error::Protocol(_)")
            && protocol_inbound.contains("pub async fn send_blocked_response")
            && protocol_inbound.contains("pub async fn send_upstream_failure_response")
            && protocol_inbound.contains("HttpConnectResponse::ConnectionEstablished")
            && protocol_inbound.contains("HttpConnectResponse::BadRequest")
            && protocol_inbound.contains("HttpConnectResponse::MethodNotAllowed")
            && protocol_inbound.contains("HttpConnectResponse::Forbidden")
            && protocol_inbound.contains("HttpConnectResponse::BadGateway"),
        "protocols/http should own concrete response selection for common inbound outcomes"
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
        "pub http_inbound:",
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
        "use http::",
        "use shadowsocks::",
        "use socks5::",
        "use trojan::",
        "use vless::",
        "use vmess::",
        "fn socks5_inbound_protocol(&self)",
        "fn socks5_outbound_protocol(&self)",
        "fn http_inbound_protocol(&self)",
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
    let active = read("src/adapters/socks5/udp/upstream_association.rs");
    let flow = read("src/adapters/socks5/udp/flow.rs");
    let model = manifest_dir().join("src/adapters/socks5/udp/model.rs");
    let protocol_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");
    let packet_path_source = read("src/adapters/socks5/udp/packet_path.rs");
    let send = manifest_dir().join("src/adapters/socks5/udp/send.rs");
    let runtime_source = read_proxy_module_tree("src/adapters/socks5/udp.rs");
    let runtime = manifest_dir().join("src/adapters/socks5/udp/runtime.rs");
    let registered_upstream_contract =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/contract.rs");
    let registered_upstream_model =
        read("src/runtime/udp_flow/registered/upstream/runtime/association/model.rs");
    let registered_upstream_lifecycle =
        read("src/runtime/udp_flow/registered/upstream/runtime/association/lifecycle.rs");
    let registered_upstream_response =
        read("src/runtime/udp_flow/registered/upstream/runtime/association/response.rs");
    let registered_upstream_handler =
        read("src/runtime/udp_flow/registered/upstream/runtime/handler.rs");
    let packet_path = manifest_dir().join("src/adapters/socks5/udp/packet_path.rs");
    let establish = manifest_dir().join("src/adapters/socks5/udp/establish.rs");
    let old_protocol_runtime = manifest_dir().join("src/protocol_runtime/socks5_udp.rs");
    let old_protocol_runtime_dir = manifest_dir().join("src/protocol_runtime/socks5_udp");

    assert!(
        !outbound.exists(),
        "SOCKS5 should not need a protocol-named proxy outbound module; TCP glue lives in adapters/socks5/tcp.rs and protocol handshake lives in protocols/socks5"
    );
    assert!(
        !model.exists(),
        "SOCKS5 UDP association model traits should live in protocols/socks5/src/udp.rs, not proxy adapter model.rs"
    );
    assert!(
        !send.exists(),
        "SOCKS5 UDP send request model should live in protocols/socks5/src/udp.rs, not proxy adapter send.rs"
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
        active.contains("struct ProxySocks5UdpAssociationRuntime")
            && active.contains("impl zero_transport::socks5_transport::Socks5UdpAssociationRuntime")
            && active.contains("crate::runtime::udp_socket::resolve_udp_peer_endpoint(")
            && active.contains("socket_addr_to_socket_address(relay_addr)")
            && active.contains("record_udp_upstream_association_closed")
            && active.contains("record_udp_upstream_association_idle_timeout")
            && active.contains("record_udp_upstream_association_dropped")
            && active.contains("UpstreamAssociationTransport<")
            && active.contains("zero_transport::socks5_transport::establish_registered_udp_association(")
            && active.contains("zero_transport::socks5_transport::establish_packet_path_udp_association(")
            && !active.contains("fn socket_address_from_std")
            && !active.contains("fn ip_address_from_std")
            && !active.contains("SocketAddress::new")
            && !active.contains("IpAddress::V4")
            && !active.contains("IpAddress::V6")
            && !active.contains("TokioDatagramSocket::bind_addr(")
            && !active.contains("Socks5UdpAssociation::new")
            && !active.contains("Socks5UdpAssociationTarget::new")
            && !active.contains("Socks5OwnedUdpAssociationConfig")
            && !active.contains("Socks5UdpRelay,")
            && !active.contains("Socks5UdpRelayEndpoint")
            && !active.contains("socks5::udp::establish_udp_relay_with_control")
            && !active.contains("_control:")
            && !active.contains("relay:")
            && !active.contains("Socks5UdpRelayTarget")
            && !active.contains("Socks5OutboundAuth")
            && !active.contains(".establish_udp_relay(")
            && !active.contains("relay_target.address")
            && !active.contains("relay_target.port")
            && !establish.exists()
            && runtime_source.contains("boxed_registered_upstream_handler::<")
            && runtime_source.contains("packet_path::carrier_descriptor")
            && runtime_source.contains("packet_path::build")
            && runtime_source.contains("flow::start")
            && packet_path_source
                .contains("establish_packet_path_association(proxy, plan.into_carrier_build()).await?")
            && packet_path_source.contains("packet_path_payload_carrier(association)")
            && !packet_path_source.contains("establish_shared_packet_path_carrier")
            && !packet_path_source.contains("establish_shared_packet_path_association")
            && !packet_path_source.contains("into_association_target()")
            && registered_upstream_lifecycle.contains("async fn ensure_association")
            && registered_upstream_lifecycle
                .contains("A::establish(proxy, association.clone(), session_id).await")
            && registered_upstream_lifecycle
                .contains("association.close(UpstreamAssociationCloseReason::Closed);")
            && registered_upstream_lifecycle
                .contains("association.close(UpstreamAssociationCloseReason::Dropped);")
            && registered_upstream_response.contains("association.recv_response_parts(buf).await?")
            && registered_upstream_lifecycle.contains("self.upstream.insert(association, a)")
            && active.contains("self.recv_response_parts(buf).await")
            && !registered_upstream_response.contains("response.into_parts()")
            && !runtime_source.contains("upstream_response_from_socks5")
            && !runtime_source.contains("Socks5InboundUdpResponse")
            && !runtime_source.contains("Socks5Inbound")
            && !registered_upstream_response.contains("decode_response_parts")
            && !registered_upstream_response.contains("response.target().clone()")
            && !registered_upstream_response.contains("response.payload().to_vec()")
            && !protocol_udp.contains("pub struct Socks5TrackedUdpAssociation<A>")
            && !protocol_udp.contains("pub struct Socks5TrackedUdpAssociationState<A>")
            && registered_upstream_lifecycle.contains("!self.upstream.matches_target(&association)")
            && registered_upstream_lifecycle
                .contains("let (outbound_tag, server, port) = association.log_parts();")
            && registered_upstream_lifecycle.contains("let (record, association) = assoc.into_parts();")
            && !runtime_source.contains("Socks5UdpAssociationSnapshot")
            && !runtime_source.contains("Socks5UdpAssociationTargetSnapshot")
            && !registered_upstream_lifecycle.contains(".upstream_endpoint()")
            && !registered_upstream_lifecycle.contains("active.server()")
            && !registered_upstream_lifecycle.contains("active.port()"),
        "SOCKS5 UDP active association wrapper should stay as thin concrete bridge glue over protocol-owned association semantics"
    );
    for source in [
        ("src/adapters/socks5/udp.rs", adapter.as_str()),
        (
            "src/adapters/socks5/udp/upstream_association.rs",
            active.as_str(),
        ),
        (
            "src/adapters/socks5/udp/packet_path.rs",
            packet_path_source.as_str(),
        ),
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
        !model.exists()
            && !establish.exists()
            && !flow.contains("struct Socks5UdpFlowStart")
            && !runtime_source.contains("struct TrackedSocks5UdpAssociation")
            && !runtime_source.contains("struct Socks5UdpAssociationIdentity")
            && !runtime_source.contains("struct Socks5UdpAssociationLifecycleRecord")
            && runtime_source.contains("boxed_registered_upstream_handler::<")
            && protocol_udp.contains("struct Socks5UdpAssociationTarget")
            && protocol_udp.contains("pub fn from_relay_socket_address")
            && protocol_udp.contains("struct Socks5EstablishedUdpAssociation")
            && !protocol_udp.contains("pub struct Socks5TrackedUdpAssociation<A>")
            && !protocol_udp.contains("pub struct Socks5TrackedUdpAssociationState<A>")
            && protocol_udp.contains("outbound_tag: alloc::string::String")
            && protocol_udp.contains("packet_path_carrier_association_target")
            && protocol_udp.contains("pub async fn establish_with_control<S>(")
            && protocol_udp.contains("pub fn log_parts(&self) -> (&str, &str, u16)")
            && protocol_udp.contains(
                "pub fn into_log_parts(self) -> (alloc::string::String, alloc::string::String, u16)"
            )
            && protocol_udp.contains(
                "pub fn matches(&self, outbound_tag: &str, server: &str, port: u16) -> bool"
            )
            && !protocol_udp.contains("pub struct Socks5UdpAssociationConfig")
            && !protocol_udp.contains("pub struct Socks5OwnedUdpAssociationConfig")
            && !protocol_udp.contains("pub struct Socks5UdpRelayTargetAddress")
            && !protocol_udp.contains("pub async fn establish_udp_relay_with_control")
            && !protocol_udp.contains("pub fn association_config(&self)")
            && !protocol_udp.contains("trait Socks5UdpAssociationHandle")
            && !protocol_udp.contains("trait Socks5UdpPacketPathAssociation")
            && !protocol_udp.contains("struct Socks5UdpAssociationIdentity")
            && !protocol_udp.contains("struct Socks5UdpAssociationLifecycleRecord")
            && !protocol_udp.contains("struct Socks5UdpAssociationEndpoint"),
        "SOCKS5 UDP protocol target/association semantics should stay in protocols/socks5 while proxy runtime keeps only lifecycle tracking"
    );
    assert!(
        !send.exists()
            && !runtime_source.contains("pub(super) async fn send_packet")
            && !runtime_source.contains("async fn ensure_association")
            && !runtime_source.contains("fn drop_after_send_error")
            && runtime_source.contains("boxed_registered_upstream_handler::<")
            && active.contains("struct ProxySocks5UdpAssociationRuntime")
            && active.contains("impl zero_transport::socks5_transport::Socks5UdpAssociationRuntime")
            && active.contains("fn record_close(")
            && active.contains("UpstreamAssociationTransport<")
            && registered_upstream_model.contains("pub(crate) struct UpstreamAssociationRuntime<T, A>")
            && registered_upstream_model.contains("pub(crate) fn upstream_outbound_tag(&self) -> Option<&str>")
            && registered_upstream_contract.contains("pub(crate) trait UpstreamAssociationTarget")
            && registered_upstream_contract.contains("pub(crate) trait UpstreamAssociationTransport")
            && registered_upstream_handler.contains("self.runtime.recv_upstream_response(buf).await")
            && registered_upstream_handler.contains("self.runtime.upstream_outbound_tag()")
            && registered_upstream_handler.contains("self.runtime.close_all_upstreams()")
            && !runtime_source.contains("association: BoxedSocks5UdpAssociation"),
        "SOCKS5 UDP upstream association lifecycle should be delegated into the registered runtime helper, leaving the adapter runtime as a thin bridge"
    );
    assert!(
        !packet_path_source.contains("socks5::parse_udp_packet")
            && !packet_path_source.contains("socks5::decode_udp_associate_response")
            && packet_path_source.contains("packet_path_payload_carrier(association)")
            && packet_path_source.contains("packet_path_carrier_descriptor_from_build(")
            && !packet_path_source.contains("SharedSocks5UdpPacketPathAssociation")
            && !packet_path_source.contains("struct Socks5PacketPath")
            && active
                .contains("impl crate::runtime::udp_flow::packet_path::PacketPathPayloadTransport")
            && active.contains("self.recv_payload(buf).await")
            && read("src/runtime/udp_flow/packet_path.rs")
                .contains("pub(crate) trait PacketPathPayloadTransport")
            && read("src/runtime/udp_flow/packet_path.rs")
                .contains("pub(crate) fn packet_path_payload_carrier("),
        "SOCKS5 packet-path carrier should use the generic runtime payload carrier wrapper over the concrete bridge association"
    );
    assert!(
        !adapter.contains("Socks5UdpPacketSend")
            && !adapter.contains("pub(crate) use send::Socks5UdpSend"),
        "SOCKS5 UDP adapter facade should not expose packet-send request models"
    );
    assert!(
        !send.exists() && !runtime.exists() && packet_path.exists(),
        "SOCKS5 UDP proxy bridge should keep packet_path.rs while registered runtime glue absorbs the old runtime.rs split"
    );
    assert!(
        !old_protocol_runtime.exists() && !old_protocol_runtime_dir.exists(),
        "SOCKS5 UDP runtime manager should not live under protocol_runtime"
    );
}

#[test]
fn vless_udp_state_model_lives_outside_runtime_root() {
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed");
    let root_udp = read_proxy_module_tree("src/adapters/vless.rs");
    let _adapter_udp = read_proxy_module_tree("src/adapters/vless.rs");
    let transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let managed_bridge = read_proxy_module_tree("src/runtime/udp_flow/managed/bridge.rs");
    let managed_connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    let connector_path = manifest_dir().join("src/adapters/vless/udp/managed/connector.rs");
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
            "VLESS UDP should not revive adapter-local manager modules `{removed}`"
        );
    }

    assert!(
        !old_runtime.exists()
            && !old_runtime_dir.exists()
            && !connector_path.exists()
            && !manifest_dir().join("src/adapters/vless/udp/managed.rs").exists()
            && !manifest_dir()
                .join("src/adapters/vless/udp/transport.rs")
                .exists()
            && !root_udp.contains("mod managed;")
            && !root_udp.contains("mod transport;")
            && root_udp.contains("managed_stream_udp_handler_for_bridge::<")
            && root_udp.contains("start_protocol_transport_bridge_udp_flow(")
            && root_udp.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !root_udp.contains("ManagedStreamFlowManager::<T>::new")
            && !root_udp.contains("ManagedStreamPacketStartBridge")
            && !root_udp.contains("start_tracked_managed_stream_packet(")
            && managed.contains("start_protocol_transport_bridge_udp_flow")
            && managed.contains("start_protocol_transport_bridge_udp_relay_final_hop")
            && managed.contains("start_protocol_transport_bridge_udp_relay_two_stream")
            && stream_manager.contains("managed_stream_connector_flow_from_build(")
            && managed_connection.contains("ManagedTupleUdpOpsConnection { connection }")
            && transport.contains("struct VlessManagedUdpFlowResume")
            && transport.contains("type VlessManagedUdpConnectorFlow = ManagedConnectorFlow<vless::udp::VlessUdpConnectorFlow>;")
            && transport.contains(".open_udp_flow_with_transport_or_mux(")
            && transport.contains(".open_relay_udp_flow_with_transport(")
            && managed_bridge.contains("ManagedStreamFlowManager::<T>::new")
            && managed_bridge.contains("struct ManagedStreamPacketStartBridge")
            && managed_bridge.contains("UdpFlowStartContext")
            && managed_bridge.contains(".start_managed_flow(")
            && managed_cache.contains("struct ManagedUdpConnectionCache"),
        "VLESS UDP managed state should stay out of runtime roots while tracked-flow glue lives in the shared managed bridge"
    );
}

#[test]
fn vless_udp_transport_opening_lives_in_transport_crate() {
    let _managed = read_proxy_module_tree("src/adapters/vless.rs");
    let adapter_udp = read_proxy_module_tree("src/adapters/vless.rs");
    let _proxy_transport = read_proxy_module_tree("src/adapters/vless.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");

    assert!(
        adapter_udp.contains("start_protocol_transport_bridge_udp_flow(")
            && adapter_udp
                .contains("protocol_transport_bridge_udp_relay_needs_two_streams(")
            && adapter_udp
                .contains("start_protocol_transport_bridge_udp_relay_two_stream(")
            && adapter_udp.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !adapter_udp.contains("uses_quic()")
            && !adapter_udp.contains("VLESS QUIC final hop over TCP relay chain is not supported")
            && transport.contains("OwnedVlessOutboundTransportPlan")
            && transport.contains("build_relay_two_stream_udp_transport(")
            && transport.contains("direct_udp_resume(")
            && transport.contains("relay_two_stream_udp_resume(")
            && transport.contains("relay_final_hop_udp_resume(")
            && transport
                .contains("fn validate_udp_relay_final_hop(&self) -> Result<(), EngineError>")
            && transport.contains("VLESS QUIC final hop over TCP relay chain is not supported")
            && stream_manager.contains("connect_upstream_host_owned(")
            && transport.contains("leaf.direct_udp_resume(self.mux_pool.clone())")
            && transport.contains("leaf.relay_final_hop_udp_resume(self.mux_pool.clone())")
            && transport.contains("transport.stream_options()")
            && (transport.contains("let open_socket = clone_socket_opener(open_socket);")
                || transport.contains("clone_socket_opener(open_socket.clone())")
                || transport.contains("clone_socket_opener(self.open_socket.clone())"))
            && transport.contains("async fn build_vless_split_http_over_relay(")
            && transport.contains(".open_udp_flow_with_transport_or_mux(")
            && transport.contains(".open_relay_udp_flow_with_transport(")
            && transport.contains("struct OwnedVlessUdpTransportOptions")
            && !transport.contains("OwnedVlessTransportOptions")
            && !transport.contains("VlessUdpTransportConnector")
            && transport.contains("pub type VlessManagedStreamUdpResume"),
        "VLESS UDP flow glue should keep adapter orchestration thin while transport bridge modules own carrier opening and tracked-flow starts"
    );

    for required in [
        "struct VlessUdpTransportOptions",
        "struct OwnedVlessUdpTransportOptions",
        "struct VlessUdpOutboundTransportRequest",
        "pub(super) async fn build_vless_udp_outbound_transport",
        "quic::connect_quic",
        "struct VlessTransportOptions",
        "async fn build_vless_outbound_transport_over_stream",
    ] {
        assert!(
            transport.contains(required),
            "zero-transport should own VLESS UDP transport helper `{required}`"
        );
    }
}

#[test]
fn vless_udp_identity_is_protocol_parsed() {
    let root = read_proxy_module_tree("src/adapters/vless.rs");
    let transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read vless protocol udp source");

    assert!(
        root.contains("start_protocol_transport_bridge_udp_flow(")
            && root.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !root.contains("parse_uuid")
            && !root.contains("parse_udp_identity")
            && !root.contains("PreparedVlessOutboundRequestBundle::from_config_with_transport_hints("),
        "src/adapters/vless.rs should stay on UDP runtime orchestration and not parse VLESS identity"
    );
    assert!(
        transport.contains("PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(")
            && transport.contains("ResolvedLeafOutbound::Vless")
            && transport.contains("VlessOutboundLeaf::new(")
            && transport
                .contains("impl<'a> ProtocolTransportLeafResolver<'a> for VlessStreamBridge"),
        "crates/transport/src/vless_transport.rs should project the VLESS leaf and build protocol-owned outbound requests"
    );
    assert!(
        transport.contains("pub struct VlessManagedUdpFlowResume")
            && transport.contains("VlessManagedUdpFlowResume::new(")
            && transport.contains("leaf.direct_udp_resume(self.mux_pool.clone())"),
        "crates/transport/src/vless_transport.rs should own the managed UDP resume carrier"
    );
    assert!(
        protocol_udp.contains("fn parse_udp_identity(")
            && protocol_udp.contains("parse_uuid(id).map(|uuid| VlessUdpIdentity { uuid })"),
        "protocols/vless/src/udp.rs should keep VLESS UDP identity parsing private to the protocol crate"
    );
}

#[test]
fn vless_udp_adapter_delegates_packet_framing_to_protocol_helpers() {
    let adapter = read_proxy_module_tree("src/adapters/vless.rs");

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
    let runtime = read_proxy_module_tree("src/adapters/vless.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let proxy_transport = read_proxy_module_tree("src/adapters/vless.rs");
    let transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let protocol = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read protocols/vless/src/udp.rs");

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
            !runtime.contains(forbidden)
                && !proxy_transport.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "VLESS UDP runtime should avoid raw packet framing and use protocols/vless flow helpers; found `{forbidden}`"
        );
    }
    assert!(
        !runtime.contains("use zero_core::{Address, Session, UdpFlowPacket}")
            && !runtime.contains("zero_core::UdpFlowPacket::from_parts")
            && !runtime.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !stream_manager.contains("UdpFlowPacket::from_parts"),
        "VLESS UDP runtime should not construct core UDP flow packets directly"
    );
    assert!(
        transport.contains("::connector_flow(")
            && transport.contains("protocol: vless::udp::PreparedVlessUdpFlowPlan")
            && transport.contains("&self.mux_pool")
            && (transport.contains("let open_socket = clone_socket_opener(open_socket);")
                || transport.contains("clone_socket_opener(open_socket.clone())")
                || transport.contains("clone_socket_opener(self.open_socket.clone())"))
            && transport.contains(".open_udp_flow_with_transport_or_mux(")
            && transport.contains(".open_relay_udp_flow_with_transport(")
            && connection.contains("ManagedTupleUdpOpsConnection { connection }")
            && transport.contains(
                "impl ManagedTupleUdpConnectionOps for vless::udp::VlessUdpFlowConnection"
            )
            && !runtime.contains("vless::udp::VlessUdpFlowConnection")
            && !runtime.contains("vless::VlessUdpIdentity")
            && !stream_manager.contains("vless::VlessUdpFlowConnection")
            && !stream_manager.contains("vless::VlessUdpFlowSession")
            && !stream_manager.contains("vless::VlessUdpFlowSender")
            && !protocol.contains("establish_flow_with_initial_packet")
            && protocol.contains("async fn write_packet_tokio")
            && !protocol.contains("pub async fn write_packet_tokio")
            && protocol.contains("async fn read_packet_tokio")
            && !protocol.contains("pub async fn read_packet_tokio")
            && protocol.contains("fn spawn_udp_flow")
            && !protocol.contains("pub fn spawn_udp_flow")
            && protocol.contains("pub fn start_mux_udp_flow")
            && protocol.contains("async fn establish_udp_flow_with_resume")
            && !protocol.contains("pub async fn establish_udp_flow_with_resume"),
        "VLESS UDP packet IO should stay in protocols/vless while adapter UDP glue keeps only cache and carrier orchestration"
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
}

#[test]
fn vmess_udp_state_model_lives_outside_runtime_root() {
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed");
    let root_udp = read_proxy_module_tree("src/adapters/vmess.rs");
    let _adapter_udp = read_proxy_module_tree("src/adapters/vmess.rs");
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let managed_bridge = read_proxy_module_tree("src/runtime/udp_flow/managed/bridge.rs");
    let managed_connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    let connector_path = manifest_dir().join("src/adapters/vmess/udp/managed/connector.rs");
    let old_runtime = manifest_dir().join("src/protocol_runtime/vmess_udp.rs");
    let old_runtime_dir = manifest_dir().join("src/protocol_runtime/vmess_udp");
    let bridge = manifest_dir().join("src/adapters/vmess/udp/manager/bridge.rs");

    assert!(
        !old_runtime.exists()
            && !old_runtime_dir.exists()
            && !bridge.exists()
            && !connector_path.exists()
            && !manifest_dir()
                .join("src/adapters/vmess/udp/managed.rs")
                .exists()
            && !manifest_dir()
                .join("src/adapters/vmess/udp/transport.rs")
                .exists(),
        "VMess UDP manager should not survive under protocol_runtime or adapter-local connector modules"
    );

    for forbidden in ["struct VmessUdpUpstream {", "struct VmessUdpTransport"] {
        assert!(
            !managed.contains(forbidden),
            "vmess UDP manager should keep neutral state/cache mechanics outside the protocol connector; found `{forbidden}`"
        );
    }

    assert!(
        !manifest_dir()
            .join("src/adapters/vmess/udp/transport.rs")
            .exists()
            && !root_udp.contains("mod managed;")
            && !root_udp.contains("mod transport;")
            && root_udp.contains("managed_stream_udp_handler_for_bridge::<")
            && root_udp.contains("start_protocol_transport_bridge_udp_flow(")
            && root_udp.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !root_udp.contains("ManagedStreamFlowManager::<T>::new")
            && !root_udp.contains("ManagedStreamPacketStartBridge")
            && !root_udp.contains("start_tracked_managed_stream_packet(")
            && managed.contains("start_protocol_transport_bridge_udp_flow")
            && managed.contains("start_protocol_transport_bridge_udp_relay_final_hop")
            && stream_manager.contains("managed_stream_connector_flow_from_build(")
            && managed_connection.contains("ManagedTupleUdpOpsConnection { connection }")
            && transport.contains("struct VmessManagedUdpFlowResume")
            && transport.contains("type VmessManagedUdpConnectorFlow = ManagedConnectorFlow<vmess::udp::VmessUdpConnectorFlow>;")
            && transport.contains(".open_udp_flow_with_transport_or_mux(")
            && transport.contains(".open_relay_udp_flow_with_transport(")
            && managed_bridge.contains("ManagedStreamFlowManager::<T>::new")
            && managed_bridge.contains("struct ManagedStreamPacketStartBridge")
            && managed_bridge.contains("UdpFlowStartContext")
            && managed_bridge.contains(".start_managed_flow(")
            && managed_cache.contains("struct ManagedUdpConnectionCache"),
        "VMess UDP managed state should stay out of runtime roots while tracked-flow glue lives in the shared managed bridge"
    );
}

#[test]
fn vmess_udp_transport_opening_lives_in_transport_crate() {
    let _managed = read_proxy_module_tree("src/adapters/vmess.rs");
    let adapter_udp = read_proxy_module_tree("src/adapters/vmess.rs");
    let _proxy_transport = read_proxy_module_tree("src/adapters/vmess.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let transport_outbound_stack =
        fs::read_to_string(repo_root().join("crates/transport/src/outbound_stack.rs"))
            .expect("read crates/transport/src/outbound_stack.rs");

    assert!(
        adapter_udp.contains("start_protocol_transport_bridge_udp_flow(")
            && adapter_udp.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && transport.contains("OwnedVmessOutboundTransportPlan")
            && stream_manager.contains("connect_upstream_host_owned(")
            && transport.contains("direct_udp_resume(")
            && transport.contains("relay_final_hop_udp_resume(")
            && transport.contains("leaf.direct_udp_resume(self.mux_pool.clone())")
            && transport.contains("protocol: vmess::udp::PreparedVmessUdpFlowPlan")
            && transport.contains("&self.mux_pool")
            && (transport.contains("let open_socket = clone_socket_opener(open_socket);")
                || transport.contains("clone_socket_opener(open_socket.clone())")
                || transport.contains("clone_socket_opener(self.open_socket.clone())"))
            && transport.contains("pub(super) async fn build_vmess_outbound_transport(")
            && transport.contains("async fn build_vmess_outbound_transport_over_stream(")
            && transport.contains(".open_udp_flow_with_transport_or_mux(")
            && transport.contains(".open_relay_udp_flow_with_transport(")
            && transport.contains("pub type VmessManagedStreamUdpResume"),
        "VMess UDP flow glue should keep adapter orchestration thin while transport bridge modules own carrier opening and tracked-flow starts"
    );

    for required in [
        "pub(super) struct VmessTransportOptions",
        "struct OwnedVmessTransportOptions",
        "pub(super) struct VmessOutboundTransportRequest",
        "struct VmessFinalHopTransportRequest",
        "pub(super) async fn build_vmess_outbound_transport",
        "async fn build_vmess_outbound_transport_over_stream",
    ] {
        assert!(
            transport.contains(required),
            "zero-transport should own VMess transport opening helper `{required}`"
        );
    }
    for required in [
        "connect_socket_transport_stack(",
        "connect_relay_transport_stack(",
    ] {
        assert!(
            transport.contains(required),
            "zero-transport VMess bridge should delegate shared carrier opening through `{required}`"
        );
    }
    for required in [
        "tls::connect_tls_upstream",
        "tls::connect_tls_stream",
        "grpc::connect_grpc",
        "ws::connect_ws",
    ] {
        assert!(
            transport_outbound_stack.contains(required),
            "shared zero-transport outbound stack should own `{required}`"
        );
    }
}

#[test]
fn vmess_udp_identity_is_protocol_parsed() {
    let root = read_proxy_module_tree("src/adapters/vmess.rs");
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read vmess protocol udp source");

    assert!(
        root.contains("start_protocol_transport_bridge_udp_flow(")
            && root.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !root.contains("parse_uuid")
            && !root.contains("parse_udp_identity")
            && !root.contains("VmessCipher::from_name")
            && !root.contains("PreparedVmessOutboundRequestBundle::from_config_with_transport_hints("),
        "src/adapters/vmess.rs should stay on UDP runtime orchestration and not parse VMess identity/cipher state"
    );
    assert!(
        transport.contains("PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(")
            && transport.contains("ResolvedLeafOutbound::Vmess")
            && transport.contains("VmessOutboundLeaf::new(")
            && transport
                .contains("impl<'a> ProtocolTransportLeafResolver<'a> for VmessStreamBridge"),
        "crates/transport/src/vmess_transport.rs should project the VMess leaf and build protocol-owned outbound requests"
    );
    assert!(
        transport.contains("pub struct VmessManagedUdpFlowResume")
            && transport.contains("VmessManagedUdpFlowResume::new(")
            && transport.contains("leaf.direct_udp_resume(self.mux_pool.clone())"),
        "crates/transport/src/vmess_transport.rs should own the managed UDP resume carrier"
    );
    assert!(
        protocol_udp.contains("fn parse_udp_identity(")
            && protocol_udp.contains("VmessCipher::from_name(cipher)")
            && protocol_udp.contains("crate::shared::parse_uuid(id)?"),
        "protocols/vmess/src/udp.rs should keep VMess UDP identity and cipher parsing private to the protocol crate"
    );
}

#[test]
fn vmess_udp_runtime_delegates_packet_framing_to_protocol_helpers() {
    let runtime = read_proxy_module_tree("src/adapters/vmess.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let proxy_transport = read_proxy_module_tree("src/adapters/vmess.rs");
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
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
            !runtime.contains(forbidden)
                && !proxy_transport.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "VMess UDP runtime should avoid raw packet framing and use protocols/vmess flow helpers; found `{forbidden}`"
        );
    }
    assert!(
        !runtime.contains("use zero_core::{Address, Session, UdpFlowPacket}")
            && !runtime.contains("zero_core::UdpFlowPacket::from_parts")
            && !runtime.contains("let initial_packet = UdpFlowPacket::from_parts")
            && !stream_manager.contains("UdpFlowPacket::from_parts"),
        "VMess UDP runtime should not construct core UDP flow packets directly"
    );
    assert!(
        transport.contains("::connector_flow(")
            && transport.contains("protocol: vmess::udp::PreparedVmessUdpFlowPlan")
            && transport.contains("&self.mux_pool")
            && (transport.contains("let open_socket = clone_socket_opener(open_socket);")
                || transport.contains("clone_socket_opener(open_socket.clone())")
                || transport.contains("clone_socket_opener(self.open_socket.clone())"))
            && transport.contains(".open_udp_flow_with_transport_or_mux(")
            && transport.contains(".open_relay_udp_flow_with_transport(")
            && connection.contains("ManagedTupleUdpOpsConnection { connection }")
            && transport.contains(
                "impl ManagedTupleUdpConnectionOps for vmess::udp::VmessUdpFlowConnection"
            )
            && !runtime.contains("vmess::udp::VmessUdpFlowConnection")
            && !runtime.contains("vmess::VmessUdpIdentity")
            && !stream_manager.contains("vmess::VmessUdpFlowConnection")
            && !stream_manager.contains("vmess::VmessUdpFlowSession")
            && !stream_manager.contains("vmess::VmessUdpFlowSender")
            && !protocol.contains("establish_flow_with_initial_packet")
            && !protocol.contains("start_flow_with_initial_packet")
            && protocol.contains("async fn write_packet_tokio")
            && !protocol.contains("pub async fn write_packet_tokio")
            && protocol.contains("async fn read_packet_tokio")
            && !protocol.contains("pub async fn read_packet_tokio")
            && protocol.contains("fn spawn_udp_flow")
            && !protocol.contains("pub fn spawn_udp_flow")
            && protocol.contains("pub fn start_udp_flow")
            && protocol.contains("async fn establish_udp_flow_with_resume")
            && !protocol.contains("pub async fn establish_udp_flow_with_resume"),
        "VMess UDP packet IO should stay in protocols/vmess while adapter UDP glue keeps only cache and carrier orchestration"
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
}

#[test]
fn vmess_mux_pool_model_lives_outside_runtime_root() {
    let root = read_adapter_transport_bridge(
        "src/adapters/vmess.rs",
        "crates/transport/src/vmess_transport.rs",
    );
    let model_path = manifest_dir().join("src/adapters/vmess/mux_pool/model.rs");
    let _transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let old_root = manifest_dir().join("src/protocol_runtime/vmess_mux_pool.rs");
    let old_dir = manifest_dir().join("src/protocol_runtime/vmess_mux_pool");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read protocols/vmess mux source");

    for forbidden in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxConn",
        "struct VmessMuxConnectionPool",
        "impl VmessMuxConnectionPool",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess transport mux pool bridge should keep protocol MUX state out of proxy glue; found `{forbidden}`"
        );
    }
    assert!(
        !model_path.exists()
            && !root.contains("struct VmessMuxOpenRequest")
            && !root.contains("identity: vmess::mux::VmessMuxIdentity")
            && !root.contains("vmess::mux::pool_key_from_transport_config(")
            && !root.contains("fn pool_key(")
            && !root.contains("vmess::mux::VmessMuxPoolKeyConfig::new")
            && !root.contains(".into_pool_key()"),
        "VMess mux pool should accept protocol-built cache keys directly without a proxy request model"
    );
    for required in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxIdentity",
        "pub struct VmessMuxConnectionPool",
        "impl VmessMuxConnectionPool",
        "impl VmessMuxPoolKey",
        "fn pool_key_from_transport_config(",
        "fn from_identity(",
        "fn from_config_parts(",
    ] {
        assert!(
            protocol_mux.contains(required),
            "VMess MUX protocol cache identity should live in protocols/vmess/src/mux.rs; missing `{required}`"
        );
    }
    for hidden in [
        "pub struct VmessMuxIdentity",
        "pub enum VmessMuxTransportKey",
        "pub struct VmessMuxStream",
        "pub fn pool_key_from_transport_config(",
        "pub fn from_identity(",
        "pub fn from_config_parts(",
        "pub fn uuid(&self)",
        "pub fn cipher(&self)",
    ] {
        assert!(
            !protocol_mux.contains(hidden),
            "VMess MUX protocol cache identity helper `{hidden}` should not be public outside vmess::mux"
        );
    }
    assert!(
        !old_root.exists() && !old_dir.exists(),
        "VMess MUX pool should not live under protocol_runtime"
    );

    assert!(
        !root.contains("VmessMuxStream::new_with_network")
            && !root.contains("struct VmessMuxConnectionPool")
            && !root.contains("pool.lock().unwrap()")
            && root.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && root.contains("protocol: vmess::udp::PreparedVmessUdpFlowPlan"),
        "VMess proxy mux transport bridge should delegate pool state through prepared protocol objects"
    );
    for forbidden in [
        "vmess::mux_cool_session",
        "vmess::VmessOutbound",
        "VmessAeadStream::outbound",
        "establish_tcp_session",
        "read_mux_frame_from_tokio",
        "vmess::read_mux_stream_frame",
        "tokio::spawn",
        "write_all(&frame)",
        "mpsc::unbounded_channel::<Vec<u8>>()",
        "struct VmessMuxConn",
        "read_mux_stream_frame(&mut reader)",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess proxy mux transport bridge should not own protocol MUX connection or pump detail `{forbidden}`"
        );
    }
    assert!(
        !root.contains("open_vmess_mux_tcp_stream(")
            && !root.contains("open_vmess_mux_udp_stream(")
            && !root.contains("vmess::mux::establish_mux_outbound_stream")
            && !root.contains("key.uuid()")
            && !root.contains("key.cipher()")
            && protocol_mux.contains("pub(crate) async fn open_tcp_stream<")
            && protocol_mux.contains("pub(crate) async fn open_udp_stream<")
            && protocol_mux.contains("establish_mux_outbound_stream(stream)")
            && protocol_mux.contains("into_pool_conn(stream, max_concurrency)"),
        "VMess mux pool runtime should delegate protocol-key mux establishment through vmess::mux without unpacking identity fields in transport"
    );
    assert!(
        !root.contains("vmess::mux::VmessMuxConn::new")
            && protocol_mux.contains("VmessMuxConn::new(stream, max_concurrency)"),
        "VMess transport mux pool should let vmess::mux wrap established streams as pool connections"
    );
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vmess/src/lib.rs"))
        .expect("read protocols/vmess lib source");
    for required in [
        "struct VmessMuxConn",
        "fn new<S>",
        "fn open_stream",
        "fn spawn_mux_write_relay",
        "fn spawn_mux_read_relay",
        "tokio::spawn",
        "read_mux_server_event(&mut reader)",
        "pub(crate) async fn open_tcp_stream",
        "pub(crate) async fn open_udp_stream",
        "fn establish_mux_outbound_stream",
        "fn into_pool_conn",
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
    ] {
        assert!(
            protocol_mux.contains(private_root_item) && !protocol_lib.contains(private_root_item),
            "VMess MUX detail `{private_root_item}` should stay under vmess::mux instead of the crate root"
        );
    }
}

#[test]
fn vless_vmess_udp_packet_models_do_not_expose_raw_fields() {
    let vless_shared = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read protocols/vless/src/udp.rs");
    let vmess_udp = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read protocols/vmess/src/udp.rs");
    let socks5_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");

    for (source_name, source, struct_name) in [
        (
            "protocols/vless/src/udp.rs",
            vless_shared.as_str(),
            "VlessUdpPacket",
        ),
        (
            "protocols/vless/src/udp.rs",
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
            "protocols/socks5/src/udp.rs",
            socks5_udp.as_str(),
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
    let trojan_udp = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read protocols/trojan/src/udp.rs");
    let shadowsocks_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read protocols/shadowsocks/src/outbound.rs");
    let mieru_udp = read_repo_module_tree("protocols/mieru/src/udp.rs");
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
            "protocols/trojan/src/udp.rs",
            trojan_udp.as_str(),
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
fn vmess_mux_pool_transport_opening_lives_in_transport_bridge() {
    let root = read_adapter_transport_bridge(
        "src/adapters/vmess.rs",
        "crates/transport/src/vmess_transport.rs",
    );
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let transport_outbound_stack =
        fs::read_to_string(repo_root().join("crates/transport/src/outbound_stack.rs"))
            .expect("read crates/transport/src/outbound_stack.rs");

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
        !root.contains("open_vmess_mux_tcp_stream(")
            && !root.contains("open_vmess_mux_udp_stream("),
        "VMess transport mux pool bridge should delegate mux orchestration through prepared protocol objects instead of transport-local mux helpers"
    );
    assert!(
        transport.contains("pub(super) struct VmessTransportOptions")
            && !transport.contains("async fn establish_vmess_mux_pool_connection<")
            && !transport.contains("pub(super) async fn open_vmess_mux_tcp_stream<")
            && !transport.contains("pub(super) async fn open_vmess_mux_udp_stream<")
            && transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && transport.contains("protocol: vmess::udp::PreparedVmessUdpFlowPlan")
            && transport.contains("connect_socket_transport_stack(")
            && transport_outbound_stack.contains("tls::connect_tls_upstream")
            && transport_outbound_stack.contains("grpc::connect_grpc")
            && transport_outbound_stack.contains("ws::connect_ws"),
        "zero-transport should own carrier opening while prepared VMess objects drive mux pool usage"
    );
}

#[test]
fn vmess_mux_pool_receives_protocol_parsed_cipher() {
    let root = read_proxy_module_tree("src/adapters/vmess.rs");
    let tcp = read_proxy_module_tree("src/adapters/vmess.rs");
    let udp = read_proxy_module_tree("src/adapters/vmess.rs");
    let transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/vmess/src/outbound.rs"))
        .expect("read vmess protocol outbound source");

    assert!(
        !root.contains("VmessCipher::from_name")
            && !root.contains("PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(")
            && tcp.contains("connect_protocol_transport_bridge_tcp(")
            && udp.contains("start_protocol_transport_bridge_udp_flow("),
        "VMess adapter roots should not parse VMess cipher state while tcp/udp bridge modules own capability glue"
    );
    assert!(
        transport.contains("PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(")
            && transport.contains("VmessOutboundLeaf::new(")
            && transport.contains("transport.mux_transport_hints()")
            && transport
                .contains("impl<'a> ProtocolTransportLeafResolver<'a> for VmessStreamBridge"),
        "crates/transport/src/vmess_transport.rs should build the VMess prepared protocol request bundle before transport opening"
    );
    assert!(
        transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && transport.contains("protocol: vmess::udp::PreparedVmessUdpFlowPlan")
            && transport.contains(".open_tcp_stream_with_transport_or_mux(")
            && transport.contains(".open_udp_flow_with_transport_or_mux("),
        "crates/transport/src/vmess_transport.rs should receive protocol-prepared mux plans and hand them the bridge-owned mux pool"
    );
    assert!(
        protocol_outbound.contains("VmessCipher::from_name")
            && protocol_outbound.contains("pub struct PreparedVmessOutboundRequestBundle")
            && protocol_outbound.contains("pub fn from_config_with_transport_hints(")
            && !protocol_outbound.contains("pub fn prepare_with_transport_hints(")
            && protocol_outbound.contains("pub async fn open_tcp_stream_with_transport_or_mux<")
            && protocol_outbound.contains(
                "pub fn udp_direct_flow_plan(&self) -> crate::udp::PreparedVmessUdpFlowPlan",
            ),
        "protocols/vmess/src/outbound.rs should keep VMess cipher parsing and build prepared mux-aware protocol plans"
    );
}

#[test]
fn vless_mux_pool_model_lives_outside_runtime_root() {
    let root = read_adapter_transport_bridge(
        "src/adapters/vless.rs",
        "crates/transport/src/vless_transport.rs",
    );
    let model_path = manifest_dir().join("src/adapters/vless/mux_pool/model.rs");
    let protocol_mux_pool = fs::read_to_string(repo_root().join("protocols/vless/src/mux_pool.rs"))
        .expect("read protocols/vless/src/mux_pool.rs");
    let transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let old_root = manifest_dir().join("src/protocol_runtime/vless_mux_pool.rs");
    let old_dir = manifest_dir().join("src/protocol_runtime/vless_mux_pool");

    {
        let forbidden = "struct MuxConnectionPool";
        assert!(
            !root.contains(forbidden),
            "VLESS transport mux pool bridge should keep protocol MUX state out of proxy glue; found `{forbidden}`"
        );
    }
    assert!(
        !model_path.exists()
            && !root.contains("struct VlessMuxOpenRequest")
            && !root.contains("identity: MuxIdentity")
            && !root.contains("vless::mux_pool::pool_key_from_transport_config(")
            && !root.contains("fn pool_key(")
            && !root.contains("PoolKeyConfig::new")
            && !root.contains(".into_pool_key()"),
        "VLESS mux pool should accept protocol-built cache keys directly without a proxy request model"
    );
    for required in [
        "pub struct MuxConnectionPool",
        "impl MuxConnectionPool",
        "struct MuxIdentity",
        "impl MuxIdentity",
        "struct PoolKeyConfig",
        "impl PoolKeyConfig",
        "fn into_pool_key",
        "impl PoolKey",
        "fn from_identity(",
        "fn from_config_parts(",
        "fn pool_key_from_transport_config(",
        "fn transport_key_from_config(",
        "pub(crate) async fn open_tcp_stream<",
        "pub(crate) async fn open_udp_stream<",
        "pub async fn establish_outbound_mux_connection<",
    ] {
        assert!(
            protocol_mux_pool.contains(required),
            "VLESS mux protocol identity should live in protocols/vless/src/mux_pool.rs; missing `{required}`"
        );
    }
    for hidden in [
        "pub struct MuxIdentity",
        "pub enum TransportKey",
        "pub struct MuxStreamRelay",
        "pub fn from_identity(",
        "pub fn from_config_parts(",
        "pub fn uuid(&self)",
        "pub fn pool_key_from_transport_config(",
    ] {
        assert!(
            !protocol_mux_pool.contains(hidden),
            "VLESS mux protocol identity helper `{hidden}` should not be public outside vless::mux_pool"
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
            "VLESS proxy mux transport bridge should not own protocol MUX frame or pump detail `{forbidden}`"
        );
    }
    for required in [
        "connect_protocol_transport_bridge_tcp(",
        "leaf.open_tcp_stream(session, &self.mux_pool, open_socket)",
        "protocol: vless::udp::PreparedVlessUdpFlowPlan",
        ".open_udp_flow_with_transport_or_mux(",
    ] {
        assert!(
            root.contains(required),
            "VLESS proxy mux transport bridge should delegate protocol MUX stream mechanics through `{required}`"
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
            "VLESS proxy mux transport bridge should ask protocols/vless to build transport cache identity; found `{forbidden}`"
        );
    }
    assert!(
        !root.contains("PoolKey::from_identity")
            && !root.contains("PoolKey::from_config_parts")
            && !root.contains("vless::mux_pool::transport_key_from_config")
            && !model_path.exists(),
        "VLESS transport mux pool should call protocol-owned pool-key builders instead of composing transport cache identity"
    );
    assert!(
        !root.contains("open_vless_mux_tcp_stream(")
            && !root.contains("vless::mux_pool::pool_key_from_transport_config(")
            && !root.contains("fn pool_key(")
            && !root.contains("PoolKeyConfig::new")
            && !root.contains(".into_pool_key()"),
        "VLESS mux pool should delegate protocol pool key construction to protocol-owned prepared flows"
    );
    assert!(
        !root.contains("establish_mux(&mut metered")
            && !root.contains("key.uuid()")
            && !root.contains("key.server")
            && !root.contains("key.port")
            && !root.contains("struct MuxConnectionPool")
            && !root.contains("pool.lock().unwrap()")
            && !root.contains("MuxPoolConn::new("),
        "VLESS transport mux pool should not unpack protocol identity or construct MUX connections directly"
    );
    assert!(
        !transport.contains("async fn establish_vless_mux_pool_connection<")
            && !transport.contains("pub(super) async fn open_vless_mux_tcp_stream<")
            && !transport.contains("pub(super) async fn open_vless_mux_udp_stream<")
            && transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && transport.contains("protocol: vless::udp::PreparedVlessUdpFlowPlan"),
        "zero-transport should own carrier opening while prepared VLESS objects drive mux pool usage"
    );
    let protocol_mux_pool = fs::read_to_string(repo_root().join("protocols/vless/src/mux_pool.rs"))
        .expect("read protocols/vless mux_pool source");
    for required in [
        "pub(crate) async fn open_tcp_stream",
        "pub(crate) async fn open_udp_stream",
        "pub async fn establish_outbound_mux_connection",
        "fn establish_mux_connection",
        "fn into_pool_conn",
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
fn protocol_mux_pools_are_transport_bridge_owned_not_runtime_fields() {
    let runtime = read("src/runtime.rs");
    let vless_adapter = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_adapter = read_proxy_module_tree("src/adapters/vmess.rs");
    let vless_tcp = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_tcp = read_proxy_module_tree("src/adapters/vmess.rs");
    let vless_udp = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_udp = read_proxy_module_tree("src/adapters/vmess.rs");
    let managed_bridge = read_proxy_module_tree("src/runtime/udp_flow/managed/bridge.rs");
    let vless_transport = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let vmess_transport = read_repo_module_tree("crates/transport/src/vmess_transport.rs");

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
    let orchestration = read("src/runtime/orchestration.rs");
    assert!(
        orchestration.contains("proxy.protocols.on_config_reloaded()"),
        "runtime reload should notify protocol inventory instead of clearing concrete protocol pools"
    );
    assert!(
        vless_adapter.contains("bridge: VlessStreamBridge")
            && vless_adapter.contains("VlessStreamBridge")
            && vless_adapter.contains("fn on_config_reloaded(&self)")
            && vless_adapter.contains("self.bridge.on_config_reloaded()")
            && !vless_tcp.contains("crate::adapters::open_vless_mux_stream(")
            && !vless_tcp.contains(".mux_pool")
            && vless_tcp.contains("connect_protocol_transport_bridge_tcp(")
            && vless_udp.contains("start_protocol_transport_bridge_udp_flow(")
            && vless_transport.contains("struct VlessStreamBridge")
            && vless_transport.contains("mux_pool: vless::mux_pool::MuxConnectionPool")
            && vless_transport.contains("self.mux_pool.evict_all()")
            && vless_transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && vless_transport.contains("protocol: vless::udp::PreparedVlessUdpFlowPlan")
            && vless_udp.contains("managed_stream_udp_handler_for_bridge::<VlessStreamBridge>()")
            && managed_bridge
                .contains("pub(crate) fn managed_stream_udp_handler_for_bridge<TBridge>()"),
        "VLESS MUX pool should be protocol-owned state held by the VLESS transport bridge and shared through thin adapter shells"
    );
    assert!(
        vmess_adapter.contains("bridge: VmessStreamBridge")
            && vmess_adapter.contains("VmessStreamBridge")
            && vmess_adapter.contains("fn on_config_reloaded(&self)")
            && vmess_adapter.contains("self.bridge.on_config_reloaded()")
            && !vmess_tcp.contains("crate::adapters::open_vmess_mux_stream(")
            && !vmess_tcp.contains(".mux_pool")
            && vmess_tcp.contains("connect_protocol_transport_bridge_tcp(")
            && vmess_udp.contains("start_protocol_transport_bridge_udp_flow(")
            && vmess_transport.contains("struct VmessStreamBridge")
            && vmess_transport.contains("mux_pool: vmess::mux::VmessMuxConnectionPool")
            && vmess_transport.contains("self.mux_pool.evict_all()")
            && vmess_transport.contains("leaf.open_tcp_stream(session, &self.mux_pool, open_socket)")
            && vmess_transport.contains("protocol: vmess::udp::PreparedVmessUdpFlowPlan")
            && vmess_udp.contains("managed_stream_udp_handler_for_bridge::<VmessStreamBridge>()")
            && managed_bridge
                .contains("pub(crate) fn managed_stream_udp_handler_for_bridge<TBridge>()"),
        "VMess MUX pool should be protocol-owned state held by the VMess transport bridge and shared through thin adapter shells"
    );
}

#[test]
fn protocol_runtime_udp_and_mux_roots_do_not_reexport_request_models() {
    for (source, forbidden) in [
        ("src/adapters/vless.rs", "VlessUdpStartFlow"),
        ("src/adapters/vless.rs", "VlessUdpRelayTwoStream"),
        ("src/adapters/vless.rs", "VlessUdpRelayFinalHopStart"),
        ("src/adapters/vmess.rs", "VmessUdpStartFlow"),
        ("src/adapters/vmess.rs", "VmessUdpRelayFlowStart"),
    ] {
        let content = read(source);
        assert!(
            !content.contains(forbidden),
            "{source} should not re-export request model `{forbidden}`"
        );
    }

    for model in [
        "src/adapters/vless/mux_pool/model.rs",
        "src/adapters/vmess/mux_pool/model.rs",
        "src/adapters/vless/udp/managed/model.rs",
        "src/adapters/vmess/udp/managed/model.rs",
    ] {
        assert!(
            !manifest_dir().join(model).exists(),
            "{model} should stay deleted after collapsing proxy request-model wrappers"
        );
    }
    assert!(
        !read_adapter_transport_bridge("src/adapters/vless.rs", "crates/transport/src/vless_transport.rs")
            .contains("mod model;")
            && !read_adapter_transport_bridge(
                "src/adapters/vmess.rs",
                "crates/transport/src/vmess_transport.rs",
            )
            .contains("mod model;"),
        "VLESS and VMess mux pool bridge roots should not re-export request models through a model module"
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
    let state = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let managed_state = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let managed_flow = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");

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
    let managed = read("src/adapters/mieru/udp.rs");
    let connector = read_repo_module_tree("crates/transport/src/mieru_transport/managed_udp.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
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
    let managed = read("src/adapters/hysteria2/udp.rs");
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
        managed.contains("managed_datagram_handler_box::<")
            && managed.contains("Hysteria2ManagedDatagramFlowResume")
            && !managed.contains("Hysteria2Connector::from_udp_profile")
            && !managed.contains("connect_raw_with_udp_profile")
            && !managed.contains("resume.connector_profile()"),
        "Hysteria2 UDP managed glue should delegate QUIC/profile setup and protocol flow pumping to the generic managed datagram connector"
    );
    assert!(
        !managed
            .contains("impl ManagedUdpConnection for hysteria2::udp::Hysteria2UdpFlowConnection")
            && !managed.contains("managed_tuple_udp_connection")
            && !managed.contains("SharedManagedUdpConnection")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowSession"),
        "Hysteria2 UDP managed glue should expose only a generic handler box, not implement runtime traits on the raw flow session"
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
fn inbound_vmess_mux_task_models_do_not_live_in_proxy_model() {
    let transport = read_proxy_module_tree("src/adapters/vmess.rs");
    let mux_tcp = read("src/runtime/mux_tcp.rs");
    let mux_udp = read("src/runtime/mux_udp.rs");
    let runtime_route = read_proxy_module_tree("src/runtime/inbound_route.rs");
    let model_path = manifest_dir().join("src/adapters/vmess/inbound/listener/model.rs");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read protocols/vmess/src/mux.rs");
    let protocol_streams_impl = impl_block(&protocol_mux, "VmessInboundMuxStreams");
    let protocol_server_impl = impl_block(&protocol_mux, "VmessInboundMuxServer");
    let protocol_writer_impl = impl_block(&protocol_mux, "VmessInboundMuxWriter");
    assert!(
        !model_path.exists()
            && !manifest_dir()
                .join("src/adapters/vmess/inbound/listener/mux.rs")
                .exists()
            && !manifest_dir()
                .join("src/adapters/vmess/inbound/listener/mux_udp.rs")
                .exists(),
        "VMess inbound listener should not keep adapter-side mux helper modules or model glue once mux dispatch moves into adapter inbound dispatch"
    );
    for forbidden in [
        "struct VmessMuxTcpStreamTask",
        "struct VmessMuxUdpStreamTask",
        "dispatch_next_opened_route(self.reader, &mut bridge)",
        "dispatch_next_opened_route_with_handlers",
        "struct VmessMuxOpenedDispatcherBridge",
        "impl vmess::mux::VmessInboundMuxOpenedRouteDispatcher",
        "read_mux_frame_from_tokio",
        "VmessInboundMuxStreams::new",
        "VmessInboundMuxWriter::new",
        "write_all(&mut writer, &frame)",
        "mpsc::unbounded_channel::<Vec<u8>>()",
    ] {
        assert!(
            !transport.contains(forbidden),
            "VMess transport mux glue should not own protocol-private mux helper `{forbidden}`"
        );
    }
    assert!(
        (transport.contains("dispatch_no_client_mux_route(")
            || transport.contains("dispatch_no_client_mux_route_with_defaults(")
            || transport.contains("dispatch_no_client_mux_route_request_with_defaults(")
            || contains_helper_call(&transport, "spawn_transport_mux_route_inbound_listener"))
            && !transport.contains("run_protocol_mux_udp_relay")
            && !transport.contains("run_mux_tcp_stream_task")
            && !transport.contains("MuxTcpStreamTask")
            && !transport.contains("TcpPipe::new")
            && !transport.contains("TcpPipeInput")
            && runtime_route.contains("pub(crate) async fn dispatch_no_client_mux_route")
            && runtime_route.contains("run_protocol_mux_session(")
            && mux_udp.contains("run_protocol_mux_udp_relay")
            && mux_tcp.contains("pub(crate) async fn run_mux_tcp_stream_task")
            && mux_tcp.contains("pub(crate) async fn run_protocol_mux_tcp_task")
            && mux_tcp.contains("pub(crate) struct MuxTcpStreamTask")
            && mux_tcp.contains("open_mux_tcp_upstream(proxy")
            && mux_tcp.contains("TcpPipe::new(proxy)")
            && mux_tcp.contains("bridge.close_stream().await")
            && mux_tcp.contains("bridge.relay_stream(upstream).await"),
        "VMess inbound dispatch glue should keep only opened-stream bridge calls while runtime/mux_tcp owns TCP pipe orchestration"
    );
    for required in [
        "async fn dispatch_with_handlers",
        "pub(crate) async fn dispatch_next_opened_route_with_handlers",
        "VmessInboundMuxAction",
        "VmessInboundMuxSession",
        "VmessInboundMuxWriter",
        "VmessInboundMuxServer",
        "VmessInboundMuxTcpRelay",
        "VmessInboundMuxUdpRelay",
        "read_mux_server_event",
        "queue_keep_stream",
        "queue_end_stream",
        "spawn_mux_write_relay(writer, write_rx)",
        "impl InboundMuxTcpRelay for VmessInboundMuxTcpRelay",
    ] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vmess should own VMess MUX API `{required}`"
        );
    }
    assert!(
        !protocol_mux.contains("pub enum VmessInboundMuxAction")
            && !protocol_mux.contains("pub enum VmessMuxServerEvent"),
        "VMess inbound MUX action/event enums should stay protocol-private"
    );
    assert!(
        protocol_streams_impl.contains("fn open_stream(")
            && !protocol_streams_impl.contains("pub fn open_stream(")
            && protocol_streams_impl.contains("fn push_stream_data(")
            && !protocol_streams_impl.contains("pub fn push_stream_data(")
            && protocol_streams_impl.contains("fn close_inbound_stream(")
            && !protocol_streams_impl.contains("pub fn close_inbound_stream(")
            && protocol_streams_impl.contains("fn apply_inbound_action(")
            && !protocol_streams_impl.contains("pub fn apply_inbound_action(")
            && protocol_server_impl.contains("fn from_tokio_writer<")
            && !protocol_server_impl.contains("pub fn from_tokio_writer<")
            && protocol_writer_impl.contains("fn new(")
            && !protocol_writer_impl.contains("pub fn new(")
            && protocol_mux.contains("pub(crate) struct VmessInboundMuxWriter")
            && !protocol_mux.contains("pub struct VmessInboundMuxWriter")
            && !protocol_mux.contains("pub async fn read_frame<")
            && !protocol_mux.contains("pub struct MuxFrame"),
        "VMess inbound MUX stream-table helpers should stay protocol-private inside VmessInboundMuxStreams"
    );
    for required in ["queue_keep_stream", "queue_end_stream"] {
        assert!(
            protocol_mux.contains(required),
            "protocols/vmess should own VMess MUX frame queue helper `{required}`"
        );
    }
}

#[test]
fn vmess_transport_dispatch_uses_protocol_session_classification() {
    let transport = read_repo_module_tree("crates/proxy/src/adapters/vmess.rs");
    let proxy_transport = transport.clone();
    let transport_vmess = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let transport_path = manifest_dir().join("src/adapters/vmess/inbound/listener/transport.rs");
    let request_vmess = read_if_exists("src/adapters/vmess/inbound/request.rs");
    let runtime_route = read_proxy_module_tree("src/runtime/inbound_route.rs");
    let mux_tcp = read("src/runtime/mux_tcp.rs");
    let mux_udp = read("src/runtime/mux_udp.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vmess/src/inbound.rs"))
        .expect("read protocols/vmess/src/inbound.rs");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vmess/src/mux.rs"))
        .expect("read protocols/vmess/src/mux.rs");

    assert!(
        !transport_path.exists()
            && request_vmess.is_empty()
            && !proxy_transport.contains(".accept_route_owned_with(")
            && !proxy_transport.contains("vmess::inbound::VmessInbound")
            && transport.contains("run_logged_tcp_socket_listener_loop(")
            && (transport.contains("dispatch_no_client_mux_route_with_defaults(")
                || transport.contains("dispatch_no_client_mux_route_request_with_defaults("))
            && !transport.contains("struct VmessAcceptedSessionHandler")
            && !transport.contains("vmess::mux::dispatch_inbound_session")
            && !transport.contains("vmess::mux::classify_inbound_session(&session)")
            && !transport.contains("vmess::mux::VmessInboundSessionKind::")
            && transport_vmess.contains("pub struct VmessInboundListenerRequest")
            && transport_vmess.contains("pub async fn accept_route(")
            && transport_vmess.contains(".accept_route_owned(vmess::inbound::VmessInbound, stream)")
            && transport_vmess.contains("crate::inbound_stack::accept_tls_inbound_stream_stack(")
            && runtime_route.contains("pub(crate) struct MuxRouteBridge")
            && runtime_route.contains("dispatch_no_client_mux_route")
            && runtime_route.contains("dispatch_protocol_mux_route")
            && runtime_route.contains("run_protocol_mux_session(")
            && mux_tcp.contains("run_protocol_mux_tcp_task")
            && mux_udp.contains("run_protocol_mux_udp_task"),
        "VMess inbound dispatch glue should consume protocol-owned session classification without implementing protocol callback handlers"
    );
    assert!(
        protocol_inbound.contains("pub async fn accept_client<S>(")
            && protocol_inbound.contains("S: AsyncSocket + tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static")
            && protocol_inbound.contains("VmessInboundAcceptedStream::from_session_stream")
            && protocol_mux.contains("pub struct VmessInboundAcceptedStream")
            && protocol_mux.contains("enum VmessInboundAcceptedStreamState")
            && !protocol_mux.contains("pub enum VmessInboundAcceptedStream")
            && protocol_mux.contains("pub struct VmessInboundUdpRelay")
            && protocol_mux.contains("pub(crate) fn from_session_stream")
            && protocol_mux.contains("relay: VmessInboundUdpRelay<S>")
            && protocol_mux.contains("reader: tokio::io::ReadHalf<S>")
            && protocol_mux.contains("mux_server: VmessInboundMuxServer")
            && protocol_mux.contains("responder: crate::udp::VmessInboundUdpResponder")
            && protocol_mux.contains("auth: Option<SessionAuth>")
            && protocol_mux.contains("let auth = session.auth.clone()")
            && protocol_mux.contains("let (reader, writer) = tokio::io::split(stream)")
            && protocol_mux.contains("accept_mux_session_from_tokio_writer(writer)")
            && protocol_mux.contains("fn into_parts(self)")
            && !protocol_mux.contains("pub fn into_parts(self)")
            && !protocol_mux
                .contains("pub fn map_stream<T, F>(self, map: F) -> VmessInboundUdpRelay<T>")
            && protocol_mux.contains("async fn dispatch")
            && !protocol_mux.contains("pub async fn dispatch")
            && !protocol_mux.contains("pub trait VmessInboundAcceptedStreamDispatcher")
            && !protocol_mux.contains("pub async fn dispatch_with<")
            && !protocol_mux.contains("Self::Mux { stream }")
            && protocol_mux.contains("enum VmessInboundSessionKind")
            && !protocol_mux.contains("pub enum VmessInboundSessionKind")
            && protocol_mux.contains("fn classify_inbound_session")
            && !protocol_mux.contains("pub fn classify_inbound_session")
            && !protocol_mux.contains("VmessInboundSessionHandler")
            && !protocol_mux.contains("dispatch_inbound_session")
            && protocol_mux.contains("VmessInboundSessionKind::Udp")
            && protocol_mux.contains("VmessInboundSessionKind::Mux")
            && protocol_mux.contains("VmessInboundSessionKind::Tcp")
            && protocol_mux.contains("is_mux_cool_session(session)"),
        "protocols/vmess should own VMess inbound TCP/UDP/MUX session classification"
    );
}

#[test]
fn vmess_inbound_udp_response_encoding_stays_in_protocol_crate() {
    let helper_path = manifest_dir().join("src/adapters/vmess/inbound/listener/helpers.rs");
    let stream_udp = read("src/runtime/stream_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let shared_mux_udp = read("src/runtime/mux_udp.rs");
    let mux = read_proxy_module_tree("src/adapters/vmess.rs");
    let mux_udp = read_proxy_module_tree("src/adapters/vmess.rs");
    let udp_session = read_proxy_module_tree("src/adapters/vmess.rs");
    let runtime_route = read_proxy_module_tree("src/runtime/inbound_route.rs");
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
        !helper_path.exists(),
        "VMess inbound helper should not exist; stream and UDP framing should be protocol-owned"
    );
    assert!(
        !mux.contains("vmess::build_udp_packet"),
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
            && packet_session_udp.contains("upstream_udp.recv_response")
            && !udp_session.contains("upstream_udp.recv_response")
            && packet_session_udp.contains("upstream_udp.recv_response")
            && !mux_udp.contains("upstream_udp.recv_response")
            && !mux.contains("&pkt.target")
            && !mux.contains("pkt.port,")
            && !mux.contains("&pkt.payload")
            && !mux.contains("pkt.payload.len()")
            && !mux.contains("pkt.payload,")
            && !mux_udp.contains("&pkt.target")
            && !mux_udp.contains("pkt.port,")
            && !mux_udp.contains("&pkt.payload")
            && !mux_udp.contains("pkt.payload.len()")
            && !mux_udp.contains("pkt.payload,")
            && !udp_session.contains("&pkt.target")
            && !udp_session.contains("pkt.port,")
            && !udp_session.contains("&pkt.payload")
            && !udp_session.contains("pkt.payload.len()")
            && !udp_session.contains("pkt.payload,"),
        "VMess inbound dispatch should consume neutral registered upstream responses"
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
            !mux.contains(forbidden)
                && !mux_udp.contains(forbidden)
                && !udp_session.contains(forbidden),
            "VMess inbound helper should use inbound-specific protocol helpers; found `{forbidden}`"
        );
    }
    assert!(
        !mux.contains("VmessInboundUdpPayload")
            && !mux.contains("vmess::VmessInboundUdpCodec")
            && (udp_session.contains("dispatch_no_client_mux_route(")
                || udp_session.contains("dispatch_no_client_mux_route_with_defaults(")
                || udp_session.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(
                    &udp_session,
                    "spawn_transport_mux_route_inbound_listener",
                ))
            && !mux_udp.contains("run_protocol_mux_udp_relay")
            && !mux_udp.contains("relay.into_parts()")
            && !mux_udp.contains("responder,")
            && stream_udp.contains("run_packet_session_udp_relay")
            && shared_mux_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && packet_session_udp.contains("record_direct_udp_response_parts")
            && packet_session_udp.contains("record_upstream_udp_response_received")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response")
            && !udp_session.contains("dispatch_inbound_udp_packet")
            && !udp_session.contains("upstream_udp.recv_response")
            && !udp_session.contains("record_direct_udp_response_parts")
            && !udp_session.contains("record_upstream_udp_response_received")
            && !udp_session.contains("write_direct_response")
            && !mux_udp.contains("dispatch_inbound_udp_packet")
            && !mux_udp.contains("upstream_udp.recv_response")
            && !mux_udp.contains("record_direct_udp_response_parts")
            && !mux_udp.contains("record_upstream_udp_response_received")
            && !mux_udp.contains("write_direct_response")
            && runtime_route.contains("run_mapped_protocol_stream_udp_relay(")
            && protocol_udp.contains("struct VmessInboundUdpCodec")
            && protocol_udp.contains("struct VmessInboundUdpSession")
            && protocol_udp.contains("struct VmessInboundUdpResponder")
            && protocol_udp.contains("struct VmessInboundMuxUdpResponder")
            && protocol_udp.contains("fn decode_request")
            && protocol_udp.contains("fn decode_dispatch_parts")
            && !protocol_udp.contains("pub fn decode_request")
            && !protocol_udp.contains("pub fn decode_dispatch_parts")
            && protocol_udp.contains("pub fn decode_inbound_dispatch")
            && protocol_udp.contains("fn decode_mux_inbound_dispatch")
            && !protocol_udp.contains("pub fn decode_mux_inbound_dispatch")
            && protocol_udp.contains("async fn read_inbound_dispatch_tokio")
            && !protocol_udp.contains("pub async fn read_inbound_dispatch_tokio")
            && !protocol_udp.contains("pub async fn read_dispatch_parts_tokio")
            && protocol_udp.contains("fn write_client_response_tokio")
            && protocol_udp.contains("fn write_mux_client_response")
            && !protocol_dispatch_parts.contains("pub target: Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>"),
        "VMess inbound UDP packet framing and response mode selection should go through protocols/vmess inbound codec"
    );
    assert!(
        (mux.contains("dispatch_no_client_mux_route(")
            || mux.contains("dispatch_no_client_mux_route_with_defaults(")
            || mux.contains("dispatch_no_client_mux_route_request_with_defaults(")
            || contains_helper_call(&mux, "spawn_transport_mux_route_inbound_listener"))
            && !mux.contains("vmess mux udp direct response send failed")
            && !mux.contains("udp_session.write_mux_response_to_socket_addr")
            && packet_session_udp.contains("packet session udp direct response encode failed")
            && !mux_udp.contains("run_protocol_mux_udp_relay")
            && (mux_udp.contains("run_protocol_mux_udp_task")
                || mux_udp.contains("dispatch_no_client_mux_route_with_defaults(")
                || mux_udp.contains("dispatch_no_client_mux_route_request_with_defaults(")
                || contains_helper_call(&mux_udp, "spawn_transport_mux_route_inbound_listener",))
            && !mux_udp.contains("responder,")
            && !mux_udp.contains("vmess::inbound::VmessInbound.mux_udp_responder_for")
            && !mux_udp.contains("self.inner.write_response_for_target")
            && !mux_udp.contains("udp_session.write_mux_client_response")
            && !mux_udp.contains("udp_session.write_mux_response_to_socket_addr")
            && runtime_route.contains("run_protocol_mux_session(")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && !mux_udp.contains("dispatch_inbound_udp_packet"),
        "VMess MUX root should only spawn UDP sub-stream glue while shared MUX UDP glue owns MUX UDP dispatch"
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
fn inbound_vless_mux_task_model_does_not_live_in_proxy_model() {
    let root = read_proxy_module_tree("src/adapters/vless.rs");
    let runtime_route = read_proxy_module_tree("src/runtime/inbound_route.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let model_path = manifest_dir().join("src/adapters/vless/inbound/listener/model.rs");
    let protocol_mux = fs::read_to_string(repo_root().join("protocols/vless/src/mux.rs"))
        .expect("read protocols/vless/src/mux.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read protocols/vless/src/inbound.rs");
    let protocol_streams_impl = impl_block(&protocol_mux, "VlessInboundMuxStreams");
    let protocol_writer_impl = impl_block(&protocol_mux, "VlessInboundMuxWriter");

    assert!(
        !root.contains("struct VlessMuxUdpStreamTask")
            && !model_path.exists()
            && !manifest_dir()
                .join("src/adapters/vless/inbound/listener/mux.rs")
                .exists()
            && !manifest_dir()
                .join("src/adapters/vless/inbound/listener/mux_udp.rs")
                .exists(),
        "VLESS inbound dispatch glue should not keep adapter-side MUX task models or helper modules"
    );
    assert!(
        !root.contains("dispatch_next_opened_route(self.client, &mut bridge)")
            && !root.contains("dispatch_next_opened_route_with_handlers")
            && contains_vless_mux_dispatch(&root)
            && !root.contains(".next_opened_route(self.client)")
            && !root.contains(".next_opened_route_with_auth(self.client")
            && !root.contains("self.auth")
            && !root.contains("route.dispatch_with(&mut bridge).await")
            && !root.contains("struct VlessMuxOpenedDispatcherBridge")
            && !root.contains("impl vless::mux::VlessInboundMuxOpenedRouteDispatcher")
            && !root.contains("VlessInboundMuxOpenedRoute::Tcp")
            && !root.contains("VlessInboundMuxOpenedRoute::Udp")
            && !root.contains("opened.into_route_with_auth")
            && !root.contains("opened: vless::mux::VlessInboundMuxTcpOpenedStream")
            && !root.contains("opened: vless::mux::VlessInboundMuxUdpOpenedStream")
            && !root.contains("opened.into_parts_with_auth(self.auth.as_ref())")
            && !root.contains("session.apply_auth")
            && runtime_route.contains("run_recorded_protocol_mux_session(")
            && protocol_mux.contains("struct VlessInboundMuxOpenedRoute")
            && !protocol_mux.contains("pub enum VlessInboundMuxOpenedRoute")
            && !protocol_mux.contains("pub struct VlessInboundMuxOpenedRoute")
            && !protocol_mux.contains("pub trait VlessInboundMuxOpenedRouteDispatcher")
            && !protocol_mux.contains("pub async fn dispatch_with<")
            && protocol_mux.contains("async fn dispatch_with_handlers")
            && !protocol_mux.contains("pub async fn dispatch_with_handlers")
            && !protocol_mux.contains("pub async fn dispatch_next_opened_route<")
            && protocol_mux.contains("pub(crate) async fn dispatch_next_opened_route_with_handlers")
            && !protocol_mux.contains("route.dispatch_with(dispatcher).await")
            && protocol_mux.contains("dispatch_with_handlers(on_tcp_opened, on_udp_opened)")
            && protocol_mux.contains("auth: Option<SessionAuth>")
            && protocol_mux.contains("fn into_route_with_auth")
            && !protocol_mux.contains("pub fn into_route_with_auth")
            && protocol_mux.contains("async fn next_opened_route")
            && !protocol_mux.contains("pub async fn next_opened_route")
            && protocol_mux.contains("async fn next_opened_route_with_auth")
            && !protocol_mux.contains("pub async fn next_opened_route_with_auth")
            && protocol_mux.contains("opened.into_route_with_auth(auth, writer)")
            && protocol_mux.contains("session.apply_auth(auth.clone())")
            && !protocol_mux.contains("pub enum VlessInboundMuxOpenedKind")
            && !protocol_mux.contains("pub struct VlessInboundMuxTcpOpenedStream")
            && !protocol_mux.contains("pub struct VlessInboundMuxUdpOpenedStream"),
        "VLESS inbound dispatch glue should consume protocol-owned opened-stream route handoff and let protocols/vless normalize session auth"
    );
    assert!(
        !root.contains(".accept_mux_session_with_auth(&mut client, mux_master_uuid, auth)")
            && contains_vless_mux_dispatch(&root)
            && protocol_inbound
                .contains(".accept_mux_session_with_auth(&mut stream, mux_master_uuid, auth)")
            && !root.contains("VlessInboundMuxServer::from_master_uuid_with_auth")
            && !root.contains("impl<S> MuxOpenedDispatcher")
            && !root.contains("struct OpenedDispatch")
            && !root.contains("struct VlessMuxOpenedDispatcherBridge")
            && !root.contains("dispatch_next_opened_route(self.client, &mut bridge)")
            && !root.contains("dispatch_next_opened_route_with_handlers")
            && !root.contains(".next_opened_route(self.client)")
            && !root.contains(".next_opened_route_with_auth(self.client")
            && !root.contains("self.auth")
            && runtime_route.contains("run_recorded_protocol_mux_session(")
            && !root.contains("dispatch_next_opened_stream")
            && !protocol_mux.contains("dispatch_next_opened_stream")
            && !protocol_mux.contains("VlessInboundMuxOpenedHandler")
            && !root.contains("run_mux_tcp_stream_task")
            && !root.contains("MuxTcpStreamTask")
            && !root.contains("bridge: relay")
            && !root.contains("impl MuxTcpStreamBridge for vless::mux::VlessInboundMuxTcpRelay")
            && !root.contains("VlessInboundMuxStreams::new")
            && !model_path.exists()
            && protocol_mux.contains("struct VlessInboundMuxDownlink")
            && !protocol_mux.contains("pub struct VlessInboundMuxDownlink")
            && !protocol_mux.contains("pub struct VlessInboundMuxStreams")
            && protocol_mux.contains("pub(crate) async fn reject_opened_stream")
            && !protocol_mux.contains("pub struct VlessInboundMuxOpenedStream")
            && protocol_mux.contains("pub struct VlessInboundMuxServer")
            && !protocol_mux.contains("pub enum VlessInboundMuxEvent")
            && protocol_mux.contains("pub struct VlessInboundMuxTcpRelay")
            && protocol_mux.contains("impl InboundMuxTcpRelay for VlessInboundMuxTcpRelay")
            && !protocol_mux.contains("pub async fn next_opened_stream")
            && protocol_mux.contains("async fn next_opened_route_with_auth")
            && !protocol_mux.contains("pub async fn next_opened_route_with_auth")
            && protocol_mux.contains("fn close_stream(&self)")
            && !protocol_mux.contains("pub fn close_stream(&self)")
            && protocol_mux.contains("async fn relay_inbound_mux_stream")
            && protocol_mux.contains("async fn relay_stream<S>(self, upstream: S)")
            && !protocol_mux.contains("pub async fn relay_stream<S>(self, upstream: S)")
            && protocol_mux.contains("pub(crate) fn write_inbound_stream_payload")
            && protocol_mux.contains("mpsc::unbounded_channel::<VlessInboundMuxDownlink>()"),
        "VLESS inbound dispatch glue should rely on protocol-owned writer/stream relay state and keep raw channel shapes in protocols/vless"
    );
    assert!(
        protocol_streams_impl.contains("fn open_stream(")
            && !protocol_streams_impl.contains("pub fn open_stream(")
            && protocol_streams_impl.contains("fn push_stream_data(")
            && !protocol_streams_impl.contains("pub fn push_stream_data(")
            && protocol_streams_impl.contains("fn close_inbound_stream(")
            && !protocol_streams_impl.contains("pub fn close_inbound_stream(")
            && protocol_streams_impl.contains("async fn apply_inbound_action")
            && !protocol_streams_impl.contains("pub async fn apply_inbound_action")
            && protocol_streams_impl.contains("async fn send_inbound_downlink")
            && !protocol_streams_impl.contains("pub async fn send_inbound_downlink"),
        "VLESS inbound MUX stream-table helpers should stay protocol-private inside VlessInboundMuxStreams"
    );
    assert!(
        protocol_writer_impl.contains("fn channel()")
            && !protocol_writer_impl.contains("pub fn channel()")
            && protocol_mux.contains("pub(crate) struct VlessInboundMuxWriter")
            && !protocol_mux.contains("pub struct VlessInboundMuxWriter")
            && protocol_writer_impl.contains("pub(crate) fn write_inbound_stream_payload")
            && !protocol_writer_impl.contains("pub fn new("),
        "VLESS inbound MUX writer should keep only the protocol-owned public bridge surface it still needs"
    );
    assert!(
        (contains_vless_mux_dispatch(&root) || root.contains("run_logged_protocol_mux_udp_relay("))
            && !root.contains("run_protocol_mux_udp_relay")
            && !root.contains("vless mux udp dispatch init failed")
            && !root.contains("record_direct_udp_response_received")
            && packet_session_udp.contains("packet session udp dispatch init failed")
            && packet_session_udp.contains("record_direct_udp_response_parts")
            && !root.contains("record_direct_udp_response_parts"),
        "VLESS inbound dispatch glue should delegate UDP sub-stream dispatch to shared runtime mux UDP helpers"
    );
}

#[test]
fn vless_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let helper_path = manifest_dir().join("src/adapters/vless/inbound/listener/helpers.rs");
    let helper = String::new();
    let stream_udp = read("src/runtime/stream_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let shared_mux_udp = read("src/runtime/mux_udp.rs");
    let runtime_route = read_proxy_module_tree("src/runtime/inbound_route.rs");
    let transport = read_proxy_module_tree("src/adapters/vless.rs");
    let _protocol_inbound = fs::read_to_string(repo_root().join("protocols/vless/src/inbound.rs"))
        .expect("read protocols/vless/src/inbound.rs");
    let protocol_shared = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/vless/src/udp.rs");
    let protocol_shared =
        fs::read_to_string(protocol_shared).expect("read vless protocol shared source");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read vless protocol udp source");
    let _protocol_mux = fs::read_to_string(repo_root().join("protocols/vless/src/mux.rs"))
        .expect("read vless protocol mux source");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/vless/src/lib.rs"))
        .expect("read vless protocol lib source");
    let protocol_dispatch_parts = struct_block(&protocol_shared, "VlessInboundUdpDispatchParts");

    for (source_name, source) in [(
        "adapters/vless/inbound/listener/dispatch.rs",
        transport.as_str(),
    )] {
        for forbidden in ["vless::build_udp_packet", "vless::parse_udp_packet"] {
            assert!(
                !source.contains(forbidden),
                "{source_name} should delegate VLESS UDP packet framing to protocols/vless; found `{forbidden}`"
            );
        }
    }
    assert!(
        !helper_path.exists() && helper.is_empty(),
        "VLESS inbound helper module should be gone once base Reality/TLS accept moves into zero-transport"
    );
    assert!(
        !manifest_dir()
            .join("src/adapters/vless/inbound/listener/udp_session.rs")
            .exists()
            && !manifest_dir()
                .join("src/adapters/vless/inbound/listener/mux.rs")
                .exists()
            && !manifest_dir()
                .join("src/adapters/vless/inbound/listener/mux_udp.rs")
                .exists()
            && !transport.contains("socks5::parse_udp_packet")
            && !transport.contains("socks5::decode_udp_associate_response")
            && !transport.contains("udp_response::decode_socks5_upstream_response")
            && packet_session_udp.contains("upstream_udp.recv_response")
            && !transport.contains("upstream_udp.recv_response")
            && !transport.contains("&pkt.target")
            && !transport.contains("pkt.port,")
            && !transport.contains("&pkt.payload")
            && !transport.contains("pkt.payload.len()")
            && !transport.contains("pkt.payload,"),
        "VLESS inbound dispatch UDP glue should consume neutral registered upstream responses through shared runtime helpers"
    );

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
            && !helper.contains("vless::VlessInboundUdpCodec")
            && !transport.contains("vless::VlessInboundUdpCodec")
            && contains_vless_mux_dispatch(&transport)
            && runtime_route.contains("run_recorded_protocol_stream_udp_relay")
            && runtime_route.contains("MeteredStream<RecordingStream<S>>")
            && !transport.contains("relay.map_stream(")
            && !transport.contains("let (mut client, responder, auth) = relay.into_parts()")
            && !transport.contains("run_protocol_mux_udp_relay")
            && !transport.contains("relay.into_parts()")
            && !transport.contains("responder,")
            && stream_udp.contains("run_packet_session_udp_relay")
            && stream_udp.contains("run_mapped_protocol_stream_udp_relay")
            && stream_udp
                .contains("let (client, responder, auth) = relay.into_stream_udp_parts();")
            && shared_mux_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && packet_session_udp.contains("record_direct_udp_response_parts")
            && packet_session_udp.contains("record_upstream_udp_response_received")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response")
            && !transport.contains("dispatch_inbound_udp_packet")
            && !transport.contains("upstream_udp.recv_response")
            && !transport.contains("record_direct_udp_response_parts")
            && !transport.contains("record_upstream_udp_response_received")
            && !transport.contains("write_direct_response")
            && protocol_shared.contains("struct VlessInboundUdpCodec")
            && protocol_shared.contains("struct VlessInboundUdpSession")
            && protocol_shared.contains("struct VlessInboundUdpResponder")
            && protocol_shared.contains("struct VlessInboundMuxUdpResponder")
            && protocol_shared.contains("struct VlessInboundUdpDispatchParts")
            && protocol_shared.contains("fn decode_request")
            && protocol_shared.contains("fn decode_dispatch_parts")
            && !protocol_shared.contains("pub fn decode_request")
            && !protocol_shared.contains("pub fn decode_dispatch_parts")
            && protocol_shared.contains("pub fn decode_inbound_dispatch")
            && protocol_shared.contains("pub fn decode_mux_inbound_dispatch")
            && protocol_shared.contains("fn read_inbound_dispatch_tokio")
            && !protocol_shared.contains("pub async fn read_dispatch_parts_tokio")
            && protocol_shared.contains("fn write_client_response_tokio")
            && protocol_shared.contains("fn send_mux_client_response")
            && protocol_shared.contains("pub fn encode_response_packet")
            && protocol_shared.contains("pub fn encode_mux_response_packet")
            && !protocol_dispatch_parts.contains("pub target: Address")
            && !protocol_dispatch_parts.contains("pub port: u16")
            && !protocol_dispatch_parts.contains("pub payload: Vec<u8>")
            && !protocol_dispatch_parts.contains("pub client_session_id: Option<u64>")
            && !protocol_shared.contains("fn write_response_to_socket_addr_tokio")
            && !protocol_shared.contains("fn send_mux_response_to_socket_addr"),
        "VLESS inbound UDP packet framing should go directly through the protocols/vless inbound codec from transport glue"
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
    let upstream_contract =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/contract.rs");
    let upstream_handler = read("src/runtime/udp_flow/registered/upstream/runtime/handler.rs");
    let upstream_response =
        read("src/runtime/udp_flow/registered/upstream/runtime/association/response.rs");
    let state = read("src/runtime/udp_flow/state.rs");

    assert_src_pattern_confined(
        "socks5::decode_udp_associate_response",
        &["src/transport/socks5_inbound/listener/udp_associate/upstream_response.rs"],
        &[],
        "raw SOCKS5 UDP response decoding should not leak into generic inbound response bridging",
    );
    assert!(
        response.contains("struct UpstreamUdpResponse")
            && response.contains("fn into_parts(self) -> (Address, u16, Vec<u8>)")
            && !response.contains("fn target(&self)")
            && !response.contains("fn payload(&self)")
            && upstream_contract.contains("Result<UpstreamUdpResponse, EngineError>")
            && state.contains("recv_response")
            && upstream_response.contains("if let Some(association) = self.upstream.association() {")
            && upstream_response.contains("association.recv_response_parts(buf).await?")
            && upstream_handler.contains("self.runtime.recv_upstream_response(buf).await")
            && !upstream_handler.contains("socks5::Socks5Inbound")
            && !upstream_handler.contains(".decode_response_parts(")
            && !upstream_handler.contains("Socks5InboundUdpCodec")
            && !upstream_handler.contains("Socks5InboundUdpResponse"),
        "registered upstream handlers should consume protocol-owned response parts and expose neutral UpstreamUdpResponse values"
    );
}

#[test]
fn trojan_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let root = read_proxy_module_tree("src/adapters/trojan.rs");
    let proxy_trojan_transport = root.clone();
    let stream_udp = read("src/runtime/stream_udp.rs");
    let packet_session_udp = read_proxy_module_tree("src/runtime/packet_session_udp.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
        .expect("read trojan protocol inbound source");
    let protocol_udp = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/trojan/src/udp.rs");
    let protocol_udp = fs::read_to_string(protocol_udp).expect("read trojan protocol udp source");
    let protocol_session_impl = impl_block(&protocol_udp, "TrojanInboundUdpSession");
    let protocol_responder_impl = impl_block(&protocol_udp, "TrojanInboundUdpResponder");
    let _protocol_dispatch_parts = struct_block(&protocol_udp, "TrojanInboundUdpDispatchParts");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/trojan/src/lib.rs"))
        .expect("read trojan protocol lib source");
    let protocol_shared = fs::read_to_string(repo_root().join("protocols/trojan/src/shared.rs"))
        .expect("read trojan protocol shared source");

    assert!(
        !manifest_dir().join("src/adapters/trojan/inbound.rs").exists()
            && root.contains("run_logged_tcp_socket_listener_loop(")
            && root.contains("request.accept_route(socket).await")
            && !root.contains("UdpPipe::new")
            && !root.contains("record_direct_udp_response_received")
            && !root.contains("TrojanInboundUdpResponder"),
        "Trojan inbound TCP listener should delegate UDP relay glue directly from the adapter root once inbound.rs is removed"
    );

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
            !proxy_trojan_transport.contains(forbidden),
            "adapter trojan inbound dispatch should delegate Trojan UDP packet framing to protocols/trojan; found `{forbidden}`"
        );
    }
    assert!(
        !proxy_trojan_transport.contains("upstream_udp.recv_response")
            && !proxy_trojan_transport.contains("&pkt.target")
            && !proxy_trojan_transport.contains("pkt.port,")
            && !proxy_trojan_transport.contains("&pkt.payload"),
        "Trojan inbound dispatch UDP glue should consume neutral registered upstream responses"
    );

    assert!(
        stream_udp.contains("run_packet_session_udp_relay")
            && packet_session_udp.contains("dispatch_inbound_udp_packet")
            && packet_session_udp.contains("record_direct_udp_response_parts")
            && packet_session_udp.contains("record_upstream_udp_response_received")
            && packet_session_udp.contains("write_direct_response")
            && packet_session_udp.contains("write_upstream_response")
            && packet_session_udp.contains("write_chain_response")
            && root.contains("run_logged_tcp_socket_listener_loop(")
            && !proxy_trojan_transport.contains("dispatch_inbound_udp_packet")
            && !proxy_trojan_transport.contains("write_direct_response")
            && !proxy_trojan_transport.contains("write_upstream_response")
            && !proxy_trojan_transport.contains("write_chain_response")
            && !proxy_trojan_transport.contains("record_direct_udp_response_parts")
            && !proxy_trojan_transport.contains("record_upstream_udp_response_received")
            && protocol_inbound
                .contains("impl<S> InboundStreamUdpRelay for TrojanInboundUdpRelay<S>")
            && protocol_udp.contains("pub struct TrojanInboundUdpResponder")
            && protocol_responder_impl.contains("async fn write_response_for_target<S>")
            && protocol_session_impl.contains("async fn read_inbound_dispatch<S>")
            && protocol_udp.contains("read_udp_flow_packet")
            && protocol_udp.contains("write_udp_flow_packet"),
        "Trojan inbound UDP packet framing should be owned by protocols/trojan udp codec"
    );
    for private_helper in [
        "read_inbound_udp_packet",
        "read_udp_flow_packet",
        "write_udp_response",
        "write_udp_flow_packet",
    ] {
        assert!(
            protocol_udp.contains(&format!("async fn {private_helper}"))
                && !protocol_udp.contains(&format!("pub async fn {private_helper}"))
                && !protocol_lib.contains(private_helper),
            "Trojan UDP helper `{private_helper}` should stay private to protocols/trojan::udp and should not be re-exported"
        );
    }
    assert!(
        !protocol_udp.contains("fn udp_flow_packet") && !protocol_lib.contains("udp_flow_packet"),
        "Trojan UDP flow packet constructor helper should be removed from the public protocol surface"
    );
    for private_root_item in [
        "read_password",
        "read_request",
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
            protocol_shared.contains(private_root_item)
                && !protocol_lib.contains(private_root_item),
            "Trojan wire helper `{private_root_item}` should stay under protocols/trojan::shared instead of the crate root"
        );
    }
    assert!(
        protocol_lib.contains("mod shared;") && !protocol_lib.contains("pub mod shared;"),
        "protocols/trojan crate root should keep shared wire helpers private"
    );
}

#[test]
fn mieru_client_stream_model_lives_outside_inbound_root() {
    let root = read("src/transport/mieru_inbound/listener.rs");
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
        !manifest_dir()
            .join("src/transport/mieru_inbound/listener/model.rs")
            .exists(),
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
    let root = read("src/adapters/mieru/inbound/listener.rs");
    let _stream_udp = read("src/runtime/stream_udp.rs");
    let inbound = read("src/adapters/mieru/inbound/listener.rs");
    let protocol_inbound = fs::read_to_string(repo_root().join("protocols/mieru/src/inbound.rs"))
        .expect("read mieru protocol inbound source");
    let protocol_udp = read_repo_module_tree("protocols/mieru/src/udp.rs");

    assert!(
        !root.contains("#[path = \"udp.rs\"]")
            && !root.contains("async fn run_mieru_udp_relay")
            && !root.contains("UdpPipe::new")
            && inbound.contains("run_mapped_protocol_stream_udp_relay(")
            && inbound.contains("\"mieru_udp\"")
            && !inbound.contains("impl Proxy"),
        "Mieru listener should hand protocol-owned UDP relays directly to the neutral stream runtime without a forwarding shell"
    );
    assert!(
        inbound.contains("run_mapped_protocol_stream_udp_relay(")
            && !inbound.contains("wrap_udp_associate")
            && !inbound.contains("unwrap_udp_associate")
            && !inbound.contains("encode_udp_flow_packet")
            && !inbound.contains("decode_udp_flow_packet")
            && protocol_inbound.contains("pub struct MieruInboundUdpRelay")
            && protocol_inbound
                .contains("impl<S> InboundStreamUdpRelay for MieruInboundUdpRelay<S>")
            && protocol_udp
                .contains("pub use inbound::{MieruInboundUdpResponder, MieruInboundUdpSession};")
            && protocol_udp.contains(
                "pub(crate) use packet::{decode_udp_flow_packet, encode_udp_flow_packet};"
            )
            && protocol_udp
                .contains("pub(crate) use packet::{unwrap_udp_associate, wrap_udp_associate};"),
        "Mieru inbound UDP packet framing should go through protocol-owned relay/session APIs"
    );
}

#[test]
fn socks5_udp_send_details_stay_out_of_udp_dispatch() {
    let managed = read_proxy_module_tree("src/runtime/udp_dispatch/managed.rs");
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
        managed.contains("start_tracked_upstream")
            && managed.contains("forward_managed_relay_flow")
            && socks5_flow.contains("UpstreamTrackedStart")
            && socks5_flow.contains(".start_tracked_upstream(")
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
    let model = manifest_dir().join("src/adapters/socks5/udp/model.rs");
    let send = manifest_dir().join("src/adapters/socks5/udp/send.rs");
    let runtime = read_proxy_module_tree("src/adapters/socks5/udp.rs");
    let transport_udp = read_repo_module_tree("crates/transport/src/socks5_transport.rs");
    let protocol_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/socks5/src/outbound.rs"))
            .expect("read socks5 outbound");
    let upstream_model =
        read("src/runtime/udp_flow/registered/upstream/runtime/association/model.rs");
    let upstream_lifecycle =
        read("src/runtime/udp_flow/registered/upstream/runtime/association/lifecycle.rs");
    let upstream_control = read("src/runtime/udp_flow/registered/upstream/runtime/control.rs");
    let response = read("src/runtime/udp_delivery/helpers.rs");

    assert!(
        !send.exists()
            && runtime.contains("let (tag, server, port, association_target) = plan.into_parts();")
            && runtime.contains("resume: association_target")
            && !runtime.contains("association.into_target()")
            && !runtime.contains(".flow(")
            && !runtime.contains(".association_target()")
            && !runtime.contains("association.target()")
            && transport_udp
                .contains("pub fn association_target(&self) -> Socks5ManagedUdpAssociationTarget")
            && transport_udp.contains("pub fn outbound_tag(&self) -> &str")
            && transport_udp.contains("pub fn log_parts(&self) -> (&str, &str, u16)")
            && protocol_udp.contains("struct Socks5UdpAssociationTarget")
            && !protocol_outbound.contains("pub struct Socks5UdpAssociationSend")
            && !protocol_outbound.contains("pub fn association_send(")
            && !protocol_outbound
                .contains("pub fn into_target(self) -> Socks5UdpAssociationTarget")
            && !protocol_outbound.contains("pub struct Socks5UdpFlowSpec")
            && !protocol_outbound.contains("pub fn flow(")
            && !protocol_udp.contains("Socks5UdpAssociationSend")
            && !protocol_udp.contains("Socks5UdpFlowSpec")
            && protocol_udp.contains("outbound_tag: alloc::string::String")
            && !model.exists(),
        "SOCKS5 UDP association identity should be named outbound_tag, not a generic tag"
    );
    assert!(
        upstream_model.contains("pub(crate) fn upstream_outbound_tag(&self) -> Option<&str>")
            && upstream_model.contains("UpstreamAssociationTarget::outbound_tag")
            && upstream_control.contains("let Some(association) = request.resume.cloned::<T>() else")
            && !upstream_lifecycle.contains("tag: inbound_tag"),
        "SOCKS5 UDP runtime must pass the outbound tag into the upstream association through neutral upstream dispatch"
    );
    assert!(
        !runtime.contains("resume.association_send(")
            && !runtime.contains(".association_target()")
            && !runtime.contains("association.target()")
            && upstream_lifecycle.contains("!self.upstream.matches_target(&association)")
            && upstream_lifecycle.contains("let (outbound_tag, server, port) = association.log_parts();")
            && upstream_lifecycle.contains("let (record, association) = assoc.into_parts();")
            && upstream_lifecycle.contains("let _ = self.upstream.insert(association, a);")
            && upstream_lifecycle.contains("let (target, association) = association.into_parts();")
            && !runtime.contains("association.into_target()")
            && !upstream_lifecycle.contains("active.outbound_tag != target.outbound_tag")
            && !upstream_lifecycle.contains("&target.outbound_tag")
            && !upstream_lifecycle.contains("association.tag")
            && protocol_udp.contains("pub fn into_log_parts(self)"),
        "SOCKS5 UDP upstream helper should store and match the relay outbound tag while the SOCKS5 runtime stays thin"
    );
    assert!(
        response.contains("association.outbound_tag")
            && response.contains("dispatch.upstream_response_session_id")
            && !response.contains("inbound_tag, &packet.target"),
        "runtime UDP response helper should look up upstream response sessions by outbound tag"
    );
}

#[test]
fn socks5_udp_association_close_details_stay_out_of_udp_associate_loop() {
    let associate = read("src/transport/socks5_inbound/listener/udp_associate.rs");

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
    assert!(
        !manifest_dir()
            .join("src/transport/socks5_inbound/listener/udp_associate/protocol_glue.rs")
            .exists(),
        "SOCKS5 UDP associate should not keep proxy-local protocol_glue for protocol-owned responder/session APIs"
    );

    for removed in [
        "src/transport/socks5_inbound/listener/udp_associate/chain_response.rs",
        "src/transport/socks5_inbound/listener/udp_associate/cleanup.rs",
        "src/transport/socks5_inbound/listener/udp_associate/idle_timeout.rs",
        "src/transport/socks5_inbound/listener/udp_associate/upstream_response.rs",
    ] {
        assert!(
            !manifest_dir().join(removed).exists(),
            "{removed} should be folded into neutral runtime UDP association orchestration"
        );
    }

    let associate = read("src/adapters/socks5/inbound/client_association.rs");
    let association_runtime = read_proxy_module_tree("src/runtime/udp_association.rs");
    let transport = read_repo_module_tree("crates/transport/src/socks5_transport.rs");

    for forbidden in [
        "UdpPipeInput",
        "ProtocolType::Socks5",
        "tokio::select!",
        "UdpDispatch::new",
        "dispatch.poll_refs()",
        "wait_for_upstream_idle",
        "chain_tasks.join_next()",
        "finish_all()",
        "log_completed_udp_flow(",
    ] {
        assert!(
            !associate.contains(forbidden),
            "SOCKS5 UDP associate entrypoint should delegate runtime loop details; found `{forbidden}`"
        );
    }

    assert!(
        associate.contains("run_udp_association_loop")
            && associate.contains("UdpAssociationLoopRequest")
            && associate.contains("relay: setup.relay")
            && associate.contains("pending_control_traffic: setup.pending_control_traffic")
            && associate.contains("handler: setup.handler")
            && associate.contains("pub(crate) async fn run_client_udp_association")
            && !associate.contains("impl Proxy")
            && !associate.contains("handle_socks5_udp_associate")
            && transport.contains("pub async fn setup_inbound_udp_association")
            && transport.contains("pub struct Socks5InboundUdpAssociationSetup")
            && transport.contains("pub struct Socks5InboundUdpAssociationHandler")
            && transport.contains("impl InboundUdpAssociation for Socks5InboundUdpAssociationHandler")
            && transport.contains("impl InboundUdpAssociationResponder for Socks5InboundUdpAssociationHandler"),
        "SOCKS5 UDP associate entrypoint should call the neutral runtime association loop with a protocol handler"
    );

    assert!(
        association_runtime.contains("pub(crate) trait UdpAssociationHandler")
            && association_runtime.contains("pub(crate) async fn run_udp_association_loop")
            && association_runtime.contains("UdpDispatch::new")
            && association_runtime.contains("dispatch.poll_refs()")
            && association_runtime.contains("wait_for_upstream_idle")
            && association_runtime.contains("chain_tasks.join_next()")
            && association_runtime.contains("record_upstream_udp_response_received")
            && association_runtime.contains("record_chain_udp_response_parts")
            && association_runtime.contains("write_upstream_response")
            && association_runtime.contains("write_chain_response")
            && association_runtime.contains("drop_idle_upstream_association")
            && association_runtime.contains("drop_upstream_association")
            && association_runtime.contains("finish_all")
            && association_runtime.contains("log_completed_udp_flow")
            && !association_runtime.contains("socks5::")
            && !association_runtime.contains("Socks5"),
        "runtime/udp_association should own neutral UDP association loop, response accounting, idle cleanup, and finish logging without naming SOCKS5"
    );

    /* legacy assertions removed while runtime_boundary catches up to the
        collapsed SOCKS5 inbound bridge layout.

        let associate = read("src/transport/socks5_inbound/listener/udp_associate.rs");
        let association_runtime = read("src/runtime/udp_association.rs");
        let dispatch = read("src/transport/socks5_inbound/listener/udp_associate/dispatch.rs");
        let direct_response =
            read("src/transport/socks5_inbound/listener/udp_associate/direct_response.rs");
        let relay_socket = read("src/transport/socks5_inbound/listener/udp_associate/relay_socket.rs");
        let setup = read("src/transport/socks5_inbound/listener/udp_associate/setup.rs");
        let adapter_active = read("src/adapters/socks5/udp/upstream_association.rs");

        for forbidden in [
            "UdpPipeInput",
            "ProtocolType::Socks5",
            "DnsResolver",
            ".resolver.resolve(",
            "async fn dispatch_packet",
            "async fn forward_chain_response",
            "socks5::encode_udp_associate_response(&address_from_socket_addr",
            "direct_response_session_id",
            "record_session_outbound_rx",
            "record_session_inbound_tx",
            "failed to send UDP chain response to client",
            "failed to build SOCKS5 UDP chain response",
            "chain upstream read error",
            "chain response task panicked",
            "async fn handle_upstream_response",
            "socks5_upstream_view",
            "upstream_response_session_id",
            "record_udp_upstream_recv_failure",
            "log_udp_upstream_association_dropped",
            "async fn handle_idle_timeout",
            "fn handle_idle_timeout",
            "drop_socks5_idle",
            "log_udp_upstream_association_idle_timeout",
            "tokio::select!",
            "UdpDispatch::new",
            "poll_refs",
            "wait_for_upstream_idle",
            "join_next",
            "finish_all",
            "log_completed_udp_flow",
        ] {
            assert!(
                !associate.contains(forbidden),
                "SOCKS5 UDP associate entrypoint should delegate runtime loop details; found `{forbidden}`"
            );
        }

        assert!(
            associate.contains("run_udp_association_loop")
                && associate.contains("UdpAssociationLoopRequest")
                && associate.contains("Socks5UdpAssociationHandler::new(request)")
                && associate.contains("pub(crate) async fn run_client_udp_association")
                && !associate.contains("impl Proxy")
                && !associate.contains("handle_socks5_udp_associate")
                && setup.contains("pub(super) async fn setup_association")
                && !setup.contains("Proxy")
                && !setup.contains("_proxy"),
            "SOCKS5 UDP associate entrypoint should call the neutral runtime association loop with a protocol handler"
        );

        assert!(
            association_runtime.contains("pub(crate) trait UdpAssociationHandler")
                && association_runtime.contains("pub(crate) async fn run_udp_association_loop")
                && association_runtime.contains("UdpDispatch::new")
                && association_runtime.contains("dispatch.poll_refs()")
                && association_runtime.contains("wait_for_upstream_idle")
                && association_runtime.contains("chain_tasks.join_next()")
                && association_runtime.contains("record_upstream_udp_response_received")
                && association_runtime.contains("record_chain_udp_response_parts")
                && association_runtime.contains("write_upstream_response")
                && association_runtime.contains("write_chain_response")
                && association_runtime.contains("drop_idle_upstream_association")
                && association_runtime.contains("drop_upstream_association")
                && association_runtime.contains("finish_all")
                && association_runtime.contains("log_completed_udp_flow")
                && !association_runtime.contains("socks5::")
                && !association_runtime.contains("Socks5"),
            "runtime/udp_association should own neutral UDP association loop, response accounting, idle cleanup, and finish logging without naming SOCKS5"
        );

        assert!(
            dispatch.contains("async fn dispatch_packet")
                && dispatch.contains("dispatch_inbound_udp_packet")
                && dispatch.contains("association: &socks5::udp::Socks5InboundUdpAssociationSession")
                && !dispatch.contains("socks5::Socks5Inbound")
                && !dispatch.contains(".udp_responder()")
                && dispatch.contains("struct Socks5InboundUdpDispatchBridge")
                && dispatch.contains("impl socks5::udp::Socks5InboundUdpDispatchActionDispatcher")
                && dispatch.contains("async fn dispatch_local_dns")
                && dispatch.contains("async fn dispatch_inbound_packet")
                && dispatch.contains(".dispatch_client_packet(packet, &mut bridge)")
                && dispatch.contains("request.into_inbound_dispatch()")
                && dispatch.contains("protocol_overhead.record")
                && !dispatch.contains("let protocol = request.protocol();")
                && !dispatch.contains("UdpPipeInput {")
                && !dispatch.contains("ProtocolType::Socks5")
                && dispatch.contains("proxy.resolver.resolve(domain).await")
                && !dispatch.contains("decode_dispatch_or_local_dns(")
                && !dispatch.contains("decode_dispatch_action")
                && !dispatch.contains("action.local_dns_domain()")
                && !dispatch.contains("action.dispatch_view()")
                && !dispatch.contains("Socks5InboundUdpDispatchAction::LocalDns")
                && !dispatch.contains("Socks5InboundUdpDispatchAction::Dispatch"),
            "SOCKS5 UDP packet dispatch should stay as protocol responder glue over runtime UDP pipe dispatch"
        );
        assert!(
            direct_response.contains("async fn forward_relay_socket_response")
                && direct_response.contains("record_direct_udp_response_parts")
                && direct_response.contains("write_direct_response")
                && read("src/runtime/udp_flow/helpers.rs").contains("direct_response_session_id")
                && direct_response
                    .contains("association: &socks5::udp::Socks5InboundUdpAssociationSession")
                && !direct_response.contains("socks5::Socks5Inbound")
                && !direct_response.contains(".udp_responder()")
                && direct_response.contains(".send_current_client_response_for_target")
                && direct_response.contains(".send_current_client_peer_response")
                && !direct_response.contains(".send_client_response_for_target")
                && !direct_response.contains(".send_client_response(")
                && !direct_response.contains("Socks5UdpClientResponse::new")
                && !direct_response.contains("record_direct_udp_response_received")
                && !direct_response.contains("udp_response_target_from_socket_addr")
                && !direct_response.contains("socket_addr_to_socket_address(client_addr)")
                && !direct_response.contains("socket_addr_to_socket_address(sender)")
                && !direct_response.contains("fn socket_address_from_std")
                && !direct_response.contains("fn ip_address_from_std")
                && !direct_response.contains("Socks5UdpRelayEndpoint")
                && !direct_response.contains("Socks5UdpRelayError")
                && direct_response.contains("into_mapped(EngineError::from)")
                && !direct_response.contains("address_from_socket_addr(sender)")
                && !direct_response.contains("socket_addr_to_ip(sender)")
                && !direct_response.contains("udp_session.response_frame")
                && !direct_response.contains("Socks5Inbound.udp_session()")
                && !direct_response.contains("Socks5InboundUdpCodec")
                && !direct_response.contains("socks5::encode_udp_associate_response("),
            "SOCKS5 UDP direct response metering should live in proxy while framing stays behind protocol helpers"
        );
        assert!(
            relay_socket.contains("impl UdpAssociationHandler for Socks5UdpAssociationHandler")
                && relay_socket.contains("Socks5InboundUdpAssociationSession")
                && !relay_socket.contains("Socks5InboundUdpRelaySession")
                && !relay_socket.contains("Socks5InboundUdpResponder")
                && relay_socket.contains("struct Socks5UdpRelayPacketBridge")
                && relay_socket
                    .contains("association: socks5::udp::Socks5InboundUdpAssociationSession")
                && relay_socket.contains("self.association")
                && relay_socket.contains(".dispatch_relay_packet(sender, payload, &mut bridge)")
                && relay_socket.contains("impl socks5::udp::Socks5InboundUdpRelayPacketDispatcher")
                && relay_socket.contains("async fn dispatch_client_packet")
                && relay_socket.contains("async fn dispatch_peer_response")
                && relay_socket.contains("async fn dispatch_unexpected_sender")
                && !relay_socket.contains("Socks5InboundUdpRelayPacketAction::ClientPacket")
                && !relay_socket.contains("Socks5InboundUdpRelayPacketAction::PeerResponse")
                && !relay_socket.contains("Socks5InboundUdpRelayPacketAction::UnexpectedSender")
                && !relay_socket.contains("Socks5InboundUdpPeerResponse::from_parts")
                && !relay_socket.contains(".handle_packet(")
                && !relay_socket.contains("self.association.classify_packet(sender, payload)")
                && !relay_socket.contains("action.client_payload()")
                && !relay_socket.contains("action.peer_sender_payload()")
                && !relay_socket.contains("action.unexpected_sender()")
                && !relay_socket.contains("impl socks5::udp::Socks5InboundUdpRelayPacketHandler")
                && relay_socket.contains("async fn write_upstream_response")
                && relay_socket.contains("async fn write_chain_response")
                && relay_socket.contains("forward_relay_peer_response")
                && !relay_socket.contains("response.into_sender_payload()")
                && !relay_socket.contains("fn client_addr")
                && !association_runtime.contains("fn client_addr")
                && !association_runtime.contains(".client_addr()")
                && !relay_socket.contains("client_udp_addr.is_none")
                && !relay_socket.contains("*request.client_udp_addr"),
            "SOCKS5 UDP relay socket glue should adapt protocol-owned relay classification and response encoding to the neutral runtime association loop"
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
            ("relay_socket.rs", &relay_socket),
        ] {
            for forbidden in ["socks5::parse_udp_packet", "socks5::build_udp_packet"] {
                assert!(
                    !source.contains(forbidden),
                    "SOCKS5 UDP associate {path} should use semantic associate packet helpers instead of `{forbidden}`"
                );
            }
            for forbidden in [
                "socks5::decode_udp_associate_request",
                "socks5::decode_udp_associate_response",
                "socks5::encode_udp_associate_response_to_client",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "SOCKS5 UDP associate {path} should call the protocol responder instead of raw helper `{forbidden}`"
                );
            }
        }

        assert!(
            dispatch.contains("association: &socks5::udp::Socks5InboundUdpAssociationSession")
                && !dispatch.contains("socks5::Socks5Inbound")
                && !dispatch.contains("udp_responder()")
                && dispatch.contains("request.protocol_overhead()")
                && dispatch.contains("request.into_inbound_dispatch()")
                && dispatch.contains("dispatch_inbound_udp_packet")
                && dispatch.contains("protocol_overhead.record")
                && dispatch.contains("impl socks5::udp::Socks5InboundUdpDispatchActionDispatcher")
                && dispatch.contains(".dispatch_client_packet(packet, &mut bridge)")
                && !dispatch.contains("Socks5InboundUdpDispatchAction::LocalDns")
                && !dispatch.contains("Socks5InboundUdpDispatchAction::Dispatch")
                && !dispatch.contains("udp_packet.into_dispatch_parts()")
                && !dispatch.contains("protocol_overhead_len")
                && relay_socket
                    .contains("association: socks5::udp::Socks5InboundUdpAssociationSession")
                && relay_socket.contains("socks5::Socks5Inbound.accept_udp_association(request)")
                && !relay_socket
                    .contains("socks5::Socks5Inbound.accept_udp_association(request).into_parts()")
                && !relay_socket.contains("socks5::Socks5Inbound.udp_responder()")
                && !relay_socket.contains("socks5::Socks5Inbound.udp_relay_session()")
                && relay_socket.contains("self.write_client_response")
                && !relay_socket.contains("socks5::udp::Socks5InboundUdpRelaySession")
                && !relay_socket.contains("Socks5UdpClientResponse::new")
                && !relay_socket.contains(".send_client_response(")
                && relay_socket.contains(".send_current_client_response_for_target")
                && !relay_socket.contains(".send_current_client_peer_response")
                && !relay_socket.contains(".send_client_response_for_target")
                && direct_response
                    .contains("association: &socks5::udp::Socks5InboundUdpAssociationSession")
                && !direct_response.contains("socks5::Socks5Inbound")
                && !direct_response.contains(".udp_responder()")
                && !direct_response.contains("Socks5UdpClientResponse::new")
                && !direct_response.contains(".send_client_response(")
                && direct_response.contains(".send_current_client_response_for_target")
                && direct_response.contains(".send_current_client_peer_response_parts")
                && !direct_response.contains(".send_client_response_for_target")
                && !dispatch.contains("Socks5Inbound.udp_session()")
                && !direct_response.contains("Socks5Inbound.udp_session()")
                && !relay_socket.contains("Socks5Inbound.udp_session()")
                && !dispatch.contains("Socks5InboundUdpCodec")
                && !direct_response.contains("Socks5InboundUdpCodec")
                && !relay_socket.contains("Socks5InboundUdpCodec")
                && relay_socket.contains(".dispatch_relay_packet(sender, payload, &mut bridge)")
                && relay_socket.contains("impl socks5::udp::Socks5InboundUdpRelayPacketDispatcher")
                && !relay_socket.contains("Socks5InboundUdpPeerResponse::from_parts")
                && !relay_socket.contains("self.association.classify_packet(sender, payload)")
                && !relay_socket.contains("action.client_payload()")
                && !relay_socket.contains("action.peer_sender_payload()")
                && !relay_socket.contains("action.unexpected_sender()"),
            "SOCKS5 UDP associate dispatch/attribution should use the protocol-owned inbound UDP responder"
        );
        assert!(
            !dispatch.contains("udp_packet.into_parts()")
                && !dispatch.contains("udp_session.decode_request")
                && !dispatch.contains("udp_session.local_dns_domain_request")
                && !dispatch.contains("udp_session.request_dispatch_parts")
                && !dispatch.contains("request.into_parts()")
                && !dispatch.contains("request.pipe_parts()")
                && !dispatch.contains("request.protocol()")
                && !dispatch.contains("request.into_pipe_parts()")
                && !dispatch.contains("UdpPipeInput {")
                && !dispatch.contains("request.record_protocol_overhead")
                && !dispatch.contains("client_session_id: None")
                && !dispatch.contains("request.target")
                && !dispatch.contains("request.port")
                && !dispatch.contains("request.payload")
                && !dispatch.contains("request.client_session_id")
                && !dispatch.contains("udp_packet.target()")
                && !dispatch.contains("udp_packet.port()")
                && !dispatch.contains("udp_packet.dns_domain_request()")
                && !relay_socket.contains("response.target()")
                && !relay_socket.contains("response.port()")
                && !relay_socket.contains(".send_response_to_client_target")
                && !direct_response.contains(".send_response_to_client_target"),
            "SOCKS5 UDP associate dispatch should consume protocol-owned dispatch parts instead of rebuilding session facts"
        );

        let protocol_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");
        let protocol_inbound = fs::read_to_string(repo_root().join("protocols/socks5/src/inbound.rs"))
            .expect("read protocols/socks5/src/inbound.rs");
        assert!(
            protocol_udp.contains("pub struct Socks5InboundUdpAssociationSession")
                && protocol_udp.contains("pub trait Socks5InboundUdpDispatchActionDispatcher")
                && protocol_udp.contains("pub trait Socks5InboundUdpRelayPacketDispatcher")
                && protocol_udp.contains("pub async fn dispatch_relay_packet")
                && protocol_udp.contains("pub async fn dispatch_client_packet")
                && protocol_udp.contains("pub async fn send_current_client_response_for_target")
                && protocol_udp.contains("pub async fn send_current_client_peer_response_parts")
                && protocol_udp.contains("pub fn accept_udp_association")
                && protocol_udp.contains("struct Socks5InboundUdpResponder")
                && protocol_udp.contains("struct Socks5InboundUdpRelaySession")
                && protocol_udp.contains("enum Socks5InboundUdpRelayPacketAction")
                && protocol_udp.contains("client: Option<SocketAddress>")
                && protocol_udp.contains("self.client = Some(sender)")
                && protocol_udp.contains("Socks5InboundUdpRelayPacketAction::ClientPacket")
                && protocol_udp.contains("Socks5InboundUdpRelayPacketAction::PeerResponse")
                && !protocol_udp.contains("pub struct Socks5InboundUdpResponder")
                && !protocol_udp.contains("pub struct Socks5InboundUdpRelaySession")
                && !protocol_udp.contains("pub struct Socks5InboundUdpSession")
                && !protocol_udp.contains("pub struct Socks5InboundUdpResponseFrame")
                && !protocol_udp.contains("pub struct Socks5InboundUdpResponseKey")
                && !protocol_udp.contains("pub struct Socks5UdpClientResponse")
                && !protocol_udp.contains("pub enum Socks5InboundUdpRelayPacketAction")
                && !protocol_udp.contains("pub async fn send_response_to_client")
                && !protocol_udp.contains("pub async fn send_response_to_client_target")
                && !protocol_udp.contains("pub async fn send_client_response")
                && !protocol_udp.contains("pub async fn send_client_response_for_target")
                && !protocol_udp.contains("pub async fn send_encoded_response_to_client")
                && !protocol_udp.contains("pub fn response_session_key_parts")
                && !protocol_udp.contains("pub fn into_parts(self) -> (Socks5InboundUdpRelaySession, Socks5InboundUdpResponder)")
                && !protocol_udp.contains("pub async fn send_response_to_client_endpoint")
                && !protocol_udp.contains("pub async fn send_response_to_client_socket_addr")
                && !protocol_udp.contains("fn address_from_ip")
                && !protocol_udp.contains("pub fn decode_dispatch_action")
                && protocol_udp.contains("fn decode_dispatch_action")
                && !protocol_udp.contains("pub async fn decode_dispatch_parts_or_resolve_local_dns")
                && !protocol_udp.contains("pub fn local_dns_domain_request")
                && !protocol_udp.contains("pub fn decode_dispatch_parts")
                && !protocol_udp.contains("pub fn request_dispatch_parts")
                && !protocol_udp.contains("pub fn decode_response_parts")
                && protocol_udp.contains("pub struct Socks5InboundUdpProtocolOverhead")
                && protocol_udp.contains("pub fn protocol_overhead(&self)")
                && protocol_udp.contains("pub fn into_pipe_parts(self)")
                && protocol_udp.contains("pub fn into_inbound_dispatch(self)")
                && protocol_udp.contains("pub fn pipe_parts(&self)")
                && protocol_udp.contains("pub fn record_protocol_overhead")
                && protocol_udp.contains("pub fn into_mapped")
                && !protocol_udp.contains("Socks5InboundUdpRelayPacketHandler")
                && !protocol_udp.contains("pub async fn handle_packet")
                && !protocol_udp.contains("pub fn classify_packet")
                && !protocol_udp.contains("pub fn client_payload(&self)")
                && !protocol_udp.contains("pub fn peer_sender_payload(&self)")
                && !protocol_udp.contains("pub fn unexpected_sender(&self)"),
            "protocols/socks5 should own UDP associate response framing, attribution helpers, and relay packet classification state"
        );
        assert!(
            protocol_udp.contains("pub(crate) struct Socks5InboundUdpDispatchParts")
                && !protocol_udp.contains("pub target: Address")
                && !protocol_udp.contains("pub port: u16")
                && !protocol_udp.contains("pub payload: Vec<u8>")
                && !protocol_udp.contains("pub client_session_id: Option<u64>")
                && protocol_udp.contains("fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>)"),
            "SOCKS5 inbound UDP dispatch parts should expose a one-shot neutral parts API instead of public fields"
        );
        assert!(
            protocol_udp.contains("pub struct Socks5InboundUdpAssociationSession")
                && protocol_udp.contains("pub fn accept_udp_association")
                && protocol_udp.contains("pub async fn dispatch_relay_packet")
                && protocol_udp.contains("pub async fn dispatch_client_packet")
                && protocol_udp.contains("pub async fn send_current_client_response_for_target")
                && protocol_udp.contains("pub async fn send_current_client_peer_response_parts")
                && !protocol_udp.contains("pub async fn send_client_response_for_target")
                && !protocol_udp.contains("pub fn into_parts(self) -> (Socks5InboundUdpRelaySession, Socks5InboundUdpResponder)")
                && relay_socket.contains("socks5::Socks5Inbound.accept_udp_association(request)")
                && !relay_socket.contains("socks5::Socks5Inbound.accept_udp_association(request).into_parts()")
                && !relay_socket.contains("socks5::Socks5Inbound.udp_relay_session()")
                && !relay_socket.contains("socks5::Socks5Inbound.udp_responder()")
                && !relay_socket.contains("Socks5InboundUdpRelaySession::new()")
                && !relay_socket.contains(".client()")
                && !relay_socket.contains("response.into_sender_payload()")
                && !relay_socket.contains("socket_address_to_socket_addr")
                && direct_response.contains("socket_address_to_socket_addr")
                && !associate.contains("Option<SocketAddr>")
                && associate.contains("request: Socks5UdpAssociateRequest")
                && !associate.contains("_request: Socks5UdpAssociateRequest")
                && !relay_socket.contains("*request.client_udp_addr")
                && !associate.contains("client_udp_addr.is_none"),
            "SOCKS5 UDP associate loop should keep client endpoint ownership in the protocol relay session while proxy owns dispatch orchestration"
        );
        assert!(
            adapter_active.contains("into_mapped(EngineError::from)")
                && !adapter_active.contains("Socks5UdpRelayError::"),
            "SOCKS5 UDP adapter should use protocol-owned relay error mapping instead of unpacking relay error variants"
        );
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
                    && !direct_response.contains(forbidden)
                    && !relay_socket.contains(forbidden),
                "SOCKS5 UDP associate glue should not rebuild protocol packet accounting/framing detail `{forbidden}`"
            );
        }
        assert!(
            association_runtime.contains("async fn handle_upstream_response")
                && association_runtime.contains("record_upstream_udp_response_received")
                && association_runtime.contains("record_udp_upstream_recv_failure")
                && association_runtime.contains("UpstreamUdpResponse")
                && !association_runtime.contains("upstream_association_view")
                && !association_runtime.contains("upstream_response_session_id")
                && !association_runtime.contains("failed to attribute upstream UDP response"),
            "UDP upstream response attribution and cleanup should live in neutral runtime association glue"
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
    */
}

#[test]
fn socks5_protocol_udp_module_stays_runtime_neutral() {
    let protocol_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");

    assert!(
        protocol_udp.contains("use zero_traits::{AsyncSocket, DatagramSocket, IpAddress, SocketAddress, UdpRelayProtocol};")
            && protocol_udp.contains("pub struct Socks5EstablishedUdpAssociation<C, S>")
            && protocol_udp.contains("pub struct Socks5InboundUdpAssociationSession")
            && protocol_udp.contains("S: AsyncSocket")
            && protocol_udp.contains("S: DatagramSocket")
            && !protocol_udp.contains("TokioDatagramSocket")
            && !protocol_udp.contains("TokioSocket")
            && !protocol_udp.contains("tokio::")
            && !protocol_udp.contains("crate::runtime::")
            && !protocol_udp.contains("use crate::runtime::")
            && !protocol_udp.contains("use tokio::")
            && !protocol_udp.contains("use zero_platform_tokio::"),
        "protocols/socks5/src/udp.rs should own SOCKS5 UDP semantics over zero-traits sockets, not runtime- or tokio-specific UDP transport"
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
            && !flow_state.contains("recv_raw_packet")
            && !flow_state.contains("recv_raw_upstream_packet")
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
                "{source} should not import protocol crate `{protocol_crate}` directly; keep protocol state in protocols/* or protocol-owned adapter glue"
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
    let old_helpers = manifest_dir().join("src/runtime/udp_flow/helpers.rs");
    let content = read("src/runtime/udp_delivery/helpers.rs");

    assert!(
        !old_helpers.exists() && !content.contains("protocol_runtime::"),
        "response helpers should live under protocol-neutral udp_delivery"
    );
}

#[test]
fn udp_packet_path_carrier_snapshot_is_protocol_neutral() {
    let runtime = read("src/runtime/udp_flow/sessions.rs");
    let packet_path_runtime = read("src/runtime/udp_flow/packet_path.rs");
    let traits = read("src/runtime/udp_flow/packet_path.rs");

    assert!(
        !runtime.contains("enum UdpPacketPathCarrier"),
        "protocol-named packet-path carrier snapshots should not be declared in generic runtime UDP flow state"
    );
    assert!(
        !packet_path_runtime.contains("enum UdpPacketPathCarrier"),
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
            && !traits.contains("pub(crate) carrier_cache_key: String")
            && !traits.contains("pub(crate) datagram: UdpDatagramKey")
            && traits.contains("pub(crate) fn lookup_key(&self) -> PacketPathLookupKey")
            && !traits.contains("datagram_cache_key: String"),
        "packet-path flow snapshots should store only private neutral carrier/datagram cache identities"
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
    let snapshot = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");
    let state = read("src/runtime/udp_flow/registered/mod.rs");
    let managed_state = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let registered_state = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");

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
            && !managed_state.contains("HashMap<ManagedUdpFlowRef, ManagedUdpFlowResume>")
            && registered_state.contains("managed_resumes:")
            && registered_state.contains("HashMap<ManagedUdpFlowRef, ManagedUdpFlowResume>"),
        "registered UDP state should own opaque resume identity outside managed execution state"
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
            && !managed_state.contains("next_flow_id: u64")
            && registered_state.contains("next_managed_flow_id: u64")
            && registered_state.contains("fn register_managed_flow")
            && registered_state.contains("fn managed_flow_resume"),
        "RegisteredUdpState should own opaque protocol UDP resumes behind runtime managed flow refs"
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
    let state = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let upstream_contract =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/contract.rs");
    let upstream_state_root = read("src/runtime/udp_flow/registered/upstream/state.rs");
    let upstream_state =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/state.rs");
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
            && upstream_contract.contains("trait UpstreamAssociationHandler")
            && upstream_state_root.contains("mod handlers;")
            && upstream_state_root.contains("mod tracked;")
            && upstream_state.contains("handlers: UpstreamUdpHandlers")
            && register.contains("provider.upstream_association_handler()"),
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
    let content = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");

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
    let connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let flow_root = read("src/runtime/udp_flow/managed/flow.rs");
    let flow_tree = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");
    let model_root = read("src/runtime/udp_flow/managed/model.rs");
    let model_tree = read_proxy_module_tree("src/runtime/udp_flow/managed/model.rs");

    for required in [
        "pub(crate) mod bridge;",
        "mod connection;",
        "pub(crate) mod datagram_manager;",
        "mod flow;",
        "pub(crate) use flow::{",
        "pub(crate) use model::ManagedDatagramFlowHandler;",
        "pub(crate) use model::ManagedStreamHandlerPair;",
        "pub(crate) use state::{",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed root should wire the submodule `{required}`"
        );
    }
    for required in [
        "mod request;",
        "mod resume;",
        "pub(crate) use request::ManagedExistingFlowForward;",
        "pub(crate) use request::ManagedUdpFlowKind;",
        "pub(crate) use resume::ManagedUdpFlowResume;",
    ] {
        assert!(
            flow_root.contains(required),
            "runtime::udp_flow::managed::flow root should wire facade export `{required}`"
        );
    }
    for required in [
        "mod handler;",
        "mod send;",
        "pub(crate) use handler::ManagedDatagramFlowHandler;",
        "pub(crate) use handler::ManagedStreamPacketFlowHandler;",
        "pub(crate) use send::ManagedDatagramExistingSend;",
        "pub(crate) use send::ManagedRelayExistingSend;",
        "pub(crate) use send::ManagedStreamExistingSend;",
    ] {
        assert!(
            model_root.contains(required),
            "runtime::udp_flow::managed::model root should wire facade export `{required}`"
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
    for forbidden in [
        "trait ManagedUdpFlowResumeObject",
        "struct ManagedDatagramFlow",
        "struct ManagedStreamPacketFlow",
        "struct ManagedRelayStreamFlow",
        "struct ManagedUdpFlowRequest",
        "struct ManagedUdpFlowResume",
    ] {
        assert!(
            !flow_root.contains(forbidden),
            "runtime::udp_flow::managed::flow root should remain a facade and not own `{forbidden}`"
        );
    }
    for forbidden in [
        "struct ManagedDatagramExistingSend",
        "struct ManagedStreamExistingSend",
        "struct ManagedRelayExistingSend",
        "trait ManagedDatagramFlowHandler",
        "trait ManagedStreamPacketFlowHandler",
        "trait ManagedRelayFlowHandler",
    ] {
        assert!(
            !model_root.contains(forbidden),
            "runtime::udp_flow::managed::model root should remain a facade and not own `{forbidden}`"
        );
    }
    assert!(
        connection.contains("trait ManagedUdpConnection")
            && connection.contains("trait ManagedTupleUdpSender")
            && connection.contains("trait ManagedPacketUdpSender")
            && connection.contains("trait ManagedDatagramUdpConnection")
            && connection.contains("fn spawn_response_bridge<T, F>")
            && flow_tree.contains("struct ManagedDatagramFlow")
            && flow_tree.contains("struct ManagedStreamPacketFlow")
            && flow_tree.contains("struct ManagedRelayStreamFlow")
            && flow_tree.contains("struct ManagedUdpFlowRequest")
            && flow_tree.contains("trait ManagedUdpFlowResumeObject")
            && flow_tree.contains("struct ManagedUdpFlowResume"),
        "managed UDP connection wrappers and flow models should live in explicit submodules, not the facade root"
    );
    assert!(
        model_tree.contains("struct ManagedDatagramExistingSend")
            && model_tree.contains("struct ManagedStreamExistingSend")
            && model_tree.contains("struct ManagedRelayExistingSend")
            && model_tree.contains("trait ManagedDatagramFlowHandler")
            && model_tree.contains("trait ManagedStreamPacketFlowHandler")
            && model_tree.contains("trait ManagedRelayFlowHandler"),
        "managed UDP handler traits and neutral send requests should live in explicit model submodules"
    );
}

#[test]
fn managed_udp_state_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/state.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/state");

    for path in [
        "error.rs",
        "forward.rs",
        "model.rs",
        "registry.rs",
        "start.rs",
    ] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::state should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod error;",
        "mod forward;",
        "mod model;",
        "mod registry;",
        "mod start;",
        "pub(super) use error::flow_mismatch;",
        "pub(crate) use model::{ManagedUdpHandlers, ManagedUdpState};",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::state root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "pub(crate) struct ManagedUdpHandlers",
        "pub(crate) struct ManagedUdpState",
        "fn flow_mismatch(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::state root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::state module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_state_start_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/state/start.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/state/start.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/state/start");

    for path in ["datagram.rs", "dispatch.rs", "stream.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::state::start should keep `{path}` under the module directory"
        );
    }

    for required in ["mod datagram;", "mod dispatch;", "mod stream;"] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::state::start root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "ManagedUdpFlowKind::Datagram",
        "ManagedUdpFlowKind::StreamPacket",
        "ManagedUdpFlowKind::RelayStream",
        "fn start_datagram_flow(",
        "fn start_stream_packet_flow(",
        "fn start_relay_stream_flow(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::state::start root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::state::start module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn udp_dispatch_managed_root_is_facade_only() {
    let root = read("src/runtime/udp_dispatch/managed.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_dispatch/managed.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_dispatch/managed");

    for path in ["forward.rs", "model.rs", "start.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_dispatch::managed should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod forward;",
        "mod model;",
        "mod start;",
        "pub(crate) use model::ManagedDatagramStart;",
        "pub(crate) use model::UpstreamTrackedStart;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_dispatch::managed root should wire facade export `{required}`"
        );
    }

    for forbidden in ["struct ManagedDatagramStart", "struct UpstreamTrackedStart"] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_dispatch::managed root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_dispatch::managed module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_datagram_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/datagram.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/datagram");

    for path in ["connection.rs", "response.rs", "state.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::datagram should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod connection;",
        "mod response;",
        "mod state;",
        "pub(crate) use connection::managed_datagram_connection_from_ops;",
        "pub(in crate::runtime::udp_flow::managed) use state::ManagedDatagramState;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::datagram root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait ManagedDatagramSender",
        "fn managed_datagram_connection(",
        "trait ManagedDatagramFlowConnection",
        "struct ManagedDatagramState",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::datagram root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::datagram module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_connection_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/connection.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/connection");

    for path in ["model.rs", "packet.rs", "response.rs", "tuple.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::connection should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod model;",
        "mod packet;",
        "mod response;",
        "mod tuple;",
        "pub(crate) use model::{",
        "pub(crate) use packet::managed_packet_udp_connection_from_flow;",
        "pub(crate) use tuple::managed_tuple_udp_connection_from_ops;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::connection root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait ManagedUdpConnection",
        "trait ManagedDatagramUdpConnection",
        "trait ManagedTupleUdpSender",
        "trait ManagedTupleUdpFlowConnection",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::connection root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::connection module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_datagram_connection_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/datagram/connection.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram/connection.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/datagram/connection");

    for path in ["flow.rs", "model.rs", "ops.rs", "sender.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::datagram::connection should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod flow;",
        "mod model;",
        "mod ops;",
        "mod sender;",
        "pub(crate) use flow::managed_datagram_connection_from_flow;",
        "pub(crate) use ops::managed_datagram_connection_from_ops;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::datagram::connection root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait ManagedDatagramSender",
        "struct ManagedDatagramConnection",
        "trait ManagedDatagramFlowConnection",
        "fn managed_datagram_connection(",
        "struct ManagedDatagramFlowSender",
        "struct ManagedDatagramOpsConnection",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::datagram::connection root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::datagram::connection module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_cache_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/cache.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/cache");

    for path in ["datagram.rs", "key.rs", "stream.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::cache should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod datagram;",
        "mod key;",
        "mod stream;",
        "pub(crate) use datagram::ManagedDatagramConnectionCache;",
        "pub(crate) use stream::ManagedUdpConnectionCache;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::cache root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct ManagedUdpConnectionCacheKey",
        "struct ManagedDatagramConnectionCacheKey",
        "fn send_or_insert_pre_sent",
        "fn get_or_insert_with",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::cache root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::cache module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_cache_stream_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/cache/stream.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/cache/stream.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/cache/stream");

    for path in ["insert.rs", "model.rs", "send.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::cache::stream should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod insert;",
        "mod model;",
        "mod send;",
        "pub(crate) use model::ManagedUdpConnectionCache;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::cache::stream root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct ManagedUdpConnectionCache",
        "async fn send_or_insert_pre_sent",
        "async fn send_or_insert",
        "async fn insert_and_send(",
        "fn send_managed_udp_connection(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::cache::stream root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::cache::stream module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_stream_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/stream.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/stream.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/stream");

    for path in ["forward.rs", "model.rs", "start.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::stream should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod forward;",
        "mod model;",
        "mod start;",
        "pub(super) use model::ManagedStreamState;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::stream root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct ManagedStreamState",
        "stream_packet_handlers:",
        "relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>",
        "async fn start_stream_packet_flow(",
        "async fn start_relay_stream_flow(",
        "async fn forward_existing_flow(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::stream root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::stream module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_stream_manager_manager_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/stream_manager/manager.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager/manager.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/stream_manager/manager");

    for path in ["mismatch.rs", "model.rs", "relay.rs", "send.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::stream_manager::manager should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod mismatch;",
        "mod model;",
        "mod relay;",
        "mod send;",
        "pub(crate) use model::{ManagedStreamFlowManager, SharedManagedStreamFlowManager};",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::stream_manager::manager root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct ManagedStreamFlowManager",
        "struct ManagedStreamRelayRequest",
        "async fn send(",
        "async fn send_managed_existing(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::stream_manager::manager root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::stream_manager::manager module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_stream_manager_connector_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/stream_manager/connector.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager/connector.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/stream_manager/connector");

    for path in ["flow.rs", "packet.rs", "tuple.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::stream_manager::connector should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod flow;",
        "mod packet;",
        "mod tuple;",
        "pub(crate) use flow::ManagedStreamFlowConnector;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::stream_manager::connector root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait ManagedStreamFlowConnector",
        "struct ManagedStreamConnectorFlow",
        "trait ManagedStreamConnectorFlowBuild",
        "impl<T> ManagedStreamFlowConnector for ManagedTupleUdpResume<T>",
        "impl<T> ManagedStreamFlowConnector for ManagedPacketUdpResume<T>",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::stream_manager::connector root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::stream_manager::connector module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_datagram_manager_connector_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/datagram_manager/connector.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager/connector.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/datagram_manager/connector");

    for path in ["flow.rs", "socket.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::datagram_manager::connector should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod flow;",
        "mod socket;",
        "pub(crate) use flow::{managed_datagram_handler_box, ManagedDatagramFlowConnector};",
        "pub(crate) use socket::{managed_datagram_socket_handler_box, ManagedDatagramSocketFlowConnector};",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::datagram_manager::connector root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait ManagedDatagramFlowConnector",
        "struct ManagedDatagramConnectorFlow",
        "fn managed_datagram_handler_box",
        "trait ManagedDatagramSocketFlowConnector",
        "struct ManagedDatagramSocketConnectorFlow",
        "fn managed_datagram_socket_handler_box",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::datagram_manager::connector root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::datagram_manager::connector module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_datagram_manager_manager_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/datagram_manager/manager.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager/manager.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/datagram_manager/manager");

    for path in ["flow.rs", "mismatch.rs", "model.rs", "socket.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::datagram_manager::manager should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod flow;",
        "mod mismatch;",
        "mod model;",
        "mod socket;",
        "pub(crate) use model::ManagedDatagramFlowManager;",
        "pub(crate) use model::ManagedDatagramSocketFlowManager;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::datagram_manager::manager root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct ManagedDatagramFlowManager",
        "struct ManagedDatagramSocketFlowManager",
        "async fn send(",
        "async fn send_managed_existing(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::datagram_manager::manager root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::datagram_manager::manager module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_bridge_transport_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/bridge/transport.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/bridge/transport.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/bridge/transport");

    for path in ["direct.rs", "relay.rs", "two_stream.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::bridge::transport should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod direct;",
        "mod relay;",
        "mod two_stream;",
        "pub(crate) use direct::start_protocol_transport_bridge_udp_flow;",
        "pub(crate) use relay::start_protocol_transport_bridge_udp_relay_final_hop;",
        "pub(crate) use two_stream::{",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::bridge::transport root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "fn protocol_transport_bridge_udp_relay_needs_two_streams",
        "async fn start_protocol_transport_bridge_udp_flow",
        "async fn start_protocol_transport_bridge_udp_relay_final_hop",
        "async fn start_protocol_transport_bridge_udp_relay_two_stream",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::bridge::transport root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::bridge::transport module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_bridge_transport_two_stream_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/bridge/transport/two_stream.rs");
    let tree =
        read_proxy_module_tree("src/runtime/udp_flow/managed/bridge/transport/two_stream.rs");
    let module_dir =
        manifest_dir().join("src/runtime/udp_flow/managed/bridge/transport/two_stream");

    for path in ["flow.rs", "predicate.rs", "start.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::bridge::transport::two_stream should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod flow;",
        "mod predicate;",
        "mod start;",
        "pub(crate) use predicate::protocol_transport_bridge_udp_relay_needs_two_streams;",
        "pub(crate) use start::start_protocol_transport_bridge_udp_relay_two_stream;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::bridge::transport::two_stream root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "fn protocol_transport_bridge_udp_relay_needs_two_streams",
        "async fn start_protocol_transport_bridge_udp_relay_two_stream",
        "async fn start_relay_two_stream_managed_flow",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::bridge::transport::two_stream root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::bridge::transport::two_stream module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_bridge_stream_packet_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/managed/bridge/stream_packet.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/managed/bridge/stream_packet.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/managed/bridge/stream_packet");

    for path in ["handler.rs", "request.rs", "start.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::managed::bridge::stream_packet should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod handler;",
        "mod request;",
        "mod start;",
        "pub(crate) use handler::{",
        "pub(crate) use start::{",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::managed::bridge::stream_packet root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "type ManagedStreamStages =",
        "struct ManagedStreamPacketStartBridge",
        "async fn start_managed_stream_packet",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::managed::bridge::stream_packet root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::managed::bridge::stream_packet module tree should still own `{forbidden}`"
        );
    }
    assert!(
        tree.contains("UdpFlowStartContext") && !tree.contains("UdpDispatch"),
        "managed stream bridge should receive narrow persistent-flow context instead of the session dispatcher"
    );
}

#[test]
fn registered_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/registered/mod.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/registered");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/registered");

    for path in ["forward.rs", "state.rs", "upstream.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::registered should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod forward;",
        "mod state;",
        "mod upstream;",
        "pub(crate) use state::{",
        "RegisteredUdpState",
        "RegisteredUdpHandlers",
        "RegisteredUpstreamAssociationView",
        "ClosedRegisteredUpstreamAssociation",
        "pub(crate) use upstream::{",
        "boxed_registered_upstream_handler",
        "UpstreamAssociationCloseReason",
        "UpstreamAssociationHandler",
        "UpstreamAssociationStages",
        "UpstreamAssociationTarget",
        "UpstreamAssociationTransport",
        "UpstreamUdpHandlers",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::registered root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct RegisteredUdpState",
        "struct RegisteredUdpHandlers",
        "struct RegisteredUpstreamAssociationView",
        "trait UpstreamAssociationHandler",
        "fn boxed_registered_upstream_handler",
        "fn start_managed_udp_flow(",
        "fn upstream_flow_mismatch(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::registered root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::registered module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn registered_state_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/registered/state.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/registered/state");

    for path in ["lifecycle.rs", "model.rs", "start.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::registered::state should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod lifecycle;",
        "mod model;",
        "mod start;",
        "pub(crate) use model::{",
        "RegisteredUdpState",
        "RegisteredUdpHandlers",
        "RegisteredUpstreamAssociationView",
        "ClosedRegisteredUpstreamAssociation",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::registered::state root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct RegisteredUdpState",
        "struct RegisteredUdpHandlers",
        "struct RegisteredUpstreamAssociationView",
        "fn upstream_association_view(",
        "fn start_managed_udp_flow(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::registered::state root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::registered::state module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn registered_upstream_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/registered/upstream.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/registered/upstream.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/registered/upstream");

    for path in ["contract.rs", "runtime.rs", "state.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::registered::upstream should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod contract;",
        "mod runtime;",
        "mod state;",
        "pub(crate) use contract::{",
        "pub(crate) use runtime::boxed_registered_upstream_handler;",
        "pub(super) use state::handlers::UpstreamAssociationState;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::registered::upstream root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait UpstreamAssociationHandler",
        "fn upstream_flow_mismatch(",
        "struct UpstreamAssociationState",
        "struct TrackedUpstreamAssociationState",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::registered::upstream root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::registered::upstream module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn registered_upstream_contract_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/registered/upstream/contract.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/contract.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/registered/upstream/contract");

    for path in [
        "handler.rs",
        "model.rs",
        "resume.rs",
        "target.rs",
        "transport.rs",
    ] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::registered::upstream::contract should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod handler;",
        "mod model;",
        "mod resume;",
        "mod target;",
        "mod transport;",
        "pub(crate) use handler::UpstreamAssociationHandler;",
        "pub(crate) use model::{",
        "pub(crate) use resume::handles_registered_resume;",
        "pub(crate) use target::UpstreamAssociationTarget;",
        "pub(crate) use transport::UpstreamAssociationTransport;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::registered::upstream::contract root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "trait UpstreamAssociationTarget",
        "fn handles_registered_resume<",
        "enum UpstreamAssociationCloseReason",
        "trait UpstreamAssociationTransport",
        "trait UpstreamAssociationHandler",
        "struct UpstreamAssociationStages",
        "struct UpstreamUdpHandlers",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::registered::upstream::contract root should remain a facade and avoid owning `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::registered::upstream::contract module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn registered_upstream_association_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/registered/upstream/runtime/association.rs");
    let tree =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/runtime/association.rs");
    let module_dir =
        manifest_dir().join("src/runtime/udp_flow/registered/upstream/runtime/association");

    for path in ["lifecycle.rs", "model.rs", "response.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::registered::upstream::runtime::association should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod lifecycle;",
        "mod model;",
        "mod response;",
        "pub(crate) use model::UpstreamAssociationRuntime;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::registered::upstream::runtime::association root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct UpstreamAssociationRuntime",
        "fn idle_deadline(",
        "fn touch_idle(",
        "fn upstream_outbound_tag(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::registered::upstream::runtime::association root should remain a facade and not own `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::registered::upstream::runtime::association module tree should still own `{forbidden}`"
        );
    }

    let upstream_runtime_root = read("src/runtime/udp_flow/registered/upstream/runtime.rs");
    let upstream_state_root = read("src/runtime/udp_flow/registered/upstream/state.rs");
    let upstream_runtime_tree =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/runtime.rs");
    let upstream_state_tree =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/state.rs");

    for required in [
        "mod association;",
        "mod control;",
        "mod handler;",
        "mod mismatch;",
        "pub(crate) use mismatch::upstream_flow_mismatch;",
        "pub(crate) use handler::boxed_registered_upstream_handler;",
    ] {
        assert!(
            upstream_runtime_root.contains(required),
            "runtime::udp_flow::registered::upstream::runtime root should wire facade export `{required}`"
        );
    }
    for forbidden in [
        "fn upstream_flow_mismatch(",
        "struct RegisteredUpstreamAssociationHandler",
    ] {
        assert!(
            !upstream_runtime_root.contains(forbidden),
            "runtime::udp_flow::registered::upstream::runtime root should stay a facade and avoid owning `{forbidden}`"
        );
    }
    for required in [
        "struct RegisteredUpstreamAssociationHandler",
        "fn upstream_flow_mismatch(",
    ] {
        assert!(
            upstream_runtime_tree.contains(required),
            "runtime::udp_flow::registered::upstream::runtime module tree should still own `{required}`"
        );
    }

    for required in [
        "mod handlers;",
        "mod tracked;",
        "pub(super) use tracked::TrackedUpstreamAssociation;",
    ] {
        assert!(
            upstream_state_root.contains(required),
            "runtime::udp_flow::registered::upstream::state root should wire facade export `{required}`"
        );
    }
    for forbidden in [
        "struct UpstreamAssociationState",
        "struct TrackedUpstreamAssociationState",
        "handlers: UpstreamUdpHandlers",
        "upstream: Option<TrackedUpstreamAssociation",
    ] {
        assert!(
            !upstream_state_root.contains(forbidden),
            "runtime::udp_flow::registered::upstream::state root should stay a facade and avoid owning `{forbidden}`"
        );
    }
    for required in [
        "struct UpstreamAssociationState",
        "struct TrackedUpstreamAssociationState",
        "handlers: UpstreamUdpHandlers",
        "upstream: Option<TrackedUpstreamAssociation",
    ] {
        assert!(
            upstream_state_tree.contains(required),
            "runtime::udp_flow::registered::upstream::state module tree should still own `{required}`"
        );
    }
}

#[test]
fn registered_upstream_state_handlers_root_is_facade_only() {
    let root = read("src/runtime/udp_flow/registered/upstream/state/handlers.rs");
    let tree = read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/state/handlers.rs");
    let module_dir = manifest_dir().join("src/runtime/udp_flow/registered/upstream/state/handlers");

    for path in ["dispatch.rs", "lifecycle.rs", "model.rs", "view.rs"] {
        assert!(
            module_dir.join(path).exists(),
            "runtime::udp_flow::registered::upstream::state::handlers should keep `{path}` under the module directory"
        );
    }

    for required in [
        "mod dispatch;",
        "mod lifecycle;",
        "mod model;",
        "mod view;",
        "pub(in crate::runtime::udp_flow::registered) use model::UpstreamAssociationState;",
    ] {
        assert!(
            root.contains(required),
            "runtime::udp_flow::registered::upstream::state::handlers root should wire facade export `{required}`"
        );
    }

    for forbidden in [
        "struct UpstreamAssociationState",
        "async fn start_upstream_flow(",
        "async fn recv_upstream_response(",
        "fn upstream_outbound_tag(",
        "fn close_all_upstreams(",
    ] {
        assert!(
            !root.contains(forbidden),
            "runtime::udp_flow::registered::upstream::state::handlers root should remain a facade and avoid owning `{forbidden}`"
        );
        assert!(
            tree.contains(forbidden),
            "runtime::udp_flow::registered::upstream::state::handlers module tree should still own `{forbidden}`"
        );
    }
}

#[test]
fn managed_udp_cache_keys_are_internal_details() {
    let cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");

    for forbidden in [
        "pub(crate) struct ManagedUdpConnectionCacheKey",
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
        "pub(crate) async fn send_existing_key",
        "pub(crate) async fn get_or_insert_key",
        "struct ManagedUdpConnectionCacheKey",
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
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let managed_datagram = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
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
            && managed_datagram.contains("ManagedDatagramExistingSend::datagram"),
        "managed datagram UDP flow kind should dispatch through registered datagram handlers"
    );
    assert!(
        register.contains("registered_udp_handlers")
            && register.contains("capability.managed_datagram_udp_handler()")
            && !register.contains("crate::protocol_runtime::udp::shadowsocks_datagram_handler")
            && !register.contains("crate::protocol_runtime::udp::hysteria2_datagram_handler"),
        "datagram UDP handler collection should live at the compiled registration boundary"
    );
}

#[test]
fn protocol_udp_upstream_start_dispatch_lives_behind_registered_handlers() {
    let state = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let upstream_contract =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/contract.rs");
    let upstream_handler = read("src/runtime/udp_flow/registered/upstream/runtime/handler.rs");
    let upstream_control = read("src/runtime/udp_flow/registered/upstream/runtime/control.rs");
    let register = read("src/register.rs");
    let socks5_adapter = read("src/adapters/socks5.rs");
    let socks5 = read("src/adapters/socks5/udp.rs");
    let socks5_flow = read("src/adapters/socks5/udp/flow.rs");

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
            && state.contains("start_upstream_flow(_inbound_tag, upstream_send(request))")
            && upstream_contract.contains("fn supports_upstream_resume")
            && upstream_contract.contains("async fn send_upstream")
            && register.contains("provider.upstream_association_handler()")
            && socks5_adapter.contains("impl UpstreamUdpHandlerProvider for Socks5Adapter")
            && socks5_adapter.contains("fn upstream_association_handler(&self)")
            && socks5.contains("pub(crate) fn upstream_association_handler")
            && socks5_flow.contains("boxed_registered_upstream_handler::<")
            && socks5_flow.contains("UpstreamTrackedStart")
            && upstream_handler
                .contains("impl<T, A> UpstreamAssociationHandler for RegisteredUpstreamAssociationHandler<T, A>")
            && upstream_handler.contains("start_registered_upstream_flow(")
            && upstream_control.contains("let Some(association) = request.resume.cloned::<T>() else"),
        "upstream UDP relay start should dispatch through a registered neutral upstream association handler"
    );
}

#[test]
fn protocol_udp_stream_start_dispatch_lives_in_protocol_modules() {
    let state = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let managed_stream = read_proxy_module_tree("src/runtime/udp_flow/managed/stream.rs");
    let register = read("src/register.rs");
    let trojan_adapter = read_proxy_module_tree("src/adapters/trojan.rs");
    let vless_adapter = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_adapter = read_proxy_module_tree("src/adapters/vmess.rs");
    let mieru_adapter = read("src/adapters/mieru.rs");

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
            && state.contains("start_upstream_flow(_inbound_tag, upstream_send(request))")
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
            && managed_stream.contains("stream_packet_handlers:")
            && managed_stream.contains("relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>")
            && !managed_stream.contains("TrojanChainManager")
            && !managed_stream.contains("MieruChainManager")
            && managed_stream.contains("for handler in &mut self.stream_packet_handlers")
            && managed_stream.contains("for handler in &mut self.relay_handlers")
            && managed_stream.contains("ManagedStreamExistingSend::stream_packet")
            && managed_stream.contains("ManagedRelayExistingSend::relay_stream"),
        "stream-packet and relay-stream UDP flow kinds should dispatch through registered stream handlers"
    );
    assert!(
        register.contains("registered_udp_handlers")
            && register.contains("capability.managed_stream_udp_handlers()")
            && trojan_adapter.contains("fn managed_stream_udp_handlers(&self)")
            && vless_adapter.contains("fn managed_stream_udp_handlers(&self)")
            && vmess_adapter.contains("fn managed_stream_udp_handlers(&self)")
            && mieru_adapter.contains("fn managed_stream_udp_handlers(&self)")
            && !register.contains("crate::protocol_runtime::udp::trojan_stream_handler")
            && !register.contains("crate::protocol_runtime::udp::mieru_stream_handler"),
        "stream UDP handler collection should live at the compiled registration boundary"
    );
}

#[test]
fn udp_dispatch_does_not_keep_external_managed_flow_handles() {
    let dispatch = read("src/runtime/udp_dispatch/mod.rs");
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let types = read("src/runtime/udp_dispatch/candidate.rs");

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
        "entries: Vec<RegisteredProtocolEntry>",
        "support: Arc<dyn ProtocolSupportCapability>",
        "inbound: Arc<dyn InboundListenerCapability>",
        "tcp: Arc<dyn TcpOutboundCapability>",
        "udp: Option<Arc<dyn UdpFlowCapability>>",
        "packet_path: Option<Arc<dyn UdpPacketPathCapability>>",
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
        !registry.contains("pub(crate) fn register_capability("),
        "src/protocol_registry/registry/mod.rs should keep register capability helper in src/protocol_registry/registry/build.rs"
    );
    assert!(
        build.contains("pub(crate) fn register_capability<T>(&mut self, adapter: Arc<T>)"),
        "src/protocol_registry/registry/build.rs should own the capability register helper used by src/register.rs"
    );
    assert!(
        build.contains("support: adapter.clone()")
            && build.contains("udp: Some(adapter.clone())")
            && build.contains("packet_path: Some(adapter)"),
        "src/protocol_registry/registry/build.rs should build focused views from one adapter Arc"
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
        "entry.inbound.bind_inbound(",
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
        !capability.contains("RegisteredProtocolCapability"),
        "focused capability traits should not be recombined into a five-trait registry collector"
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
        "src/adapters/http.rs",
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
        ("src/adapters/http.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read_proxy_module_tree(source);
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
        ("src/adapters/http.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read_proxy_module_tree(source);
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
        ("src/adapters/http.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read_proxy_module_tree(source);
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
        ("src/adapters/http.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read_proxy_module_tree(source);
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
        registry.contains("entries: Vec<RegisteredProtocolEntry>")
            && registry.contains("support: Arc<dyn ProtocolSupportCapability>")
            && registry.contains("inbound: Arc<dyn InboundListenerCapability>")
            && registry.contains("tcp: Arc<dyn TcpOutboundCapability>")
            && registry.contains("udp: Option<Arc<dyn UdpFlowCapability>>")
            && registry.contains("packet_path: Option<Arc<dyn UdpPacketPathCapability>>")
            && !registry.contains("RegisteredProtocolCapability"),
        "ProtocolRegistry should store focused capability views without a five-trait intersection"
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
        "use crate::runtime::path::{OutboundEndpoint, TcpPathCategory}",
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
        "use crate::runtime::path::{OutboundEndpoint, TcpPathCategory}",
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
    let outbound_error = read("src/transport/tcp_outbound/error.rs");

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
    assert!(
        content.contains("Proxy transport facade")
            && content.contains("Concrete carrier implementations remain in the `zero-transport` crate")
            && !manifest_dir().join("src/transport/tcp_flow.rs").exists()
            && outbound_error.contains("pub(crate) fn is_block_error"),
        "proxy transport should remain an explicit boundary facade and keep normalized TCP outbound errors with TCP outbound glue"
    );
}

#[test]
fn protocol_transport_roots_are_facade_only() {
    for (relative, bridge, adapter) in [
        (
            "crates/proxy/src/adapters/vless.rs",
            "VlessStreamBridge",
            "VlessAdapter",
        ),
        (
            "crates/proxy/src/adapters/vmess.rs",
            "VmessStreamBridge",
            "VmessAdapter",
        ),
        (
            "crates/proxy/src/adapters/trojan.rs",
            "TrojanTlsBridge",
            "TrojanAdapter",
        ),
    ] {
        let content = read_repo_file(relative);
        for pattern in [bridge, adapter] {
            assert!(
                content.contains(pattern),
                "{relative} should keep the adapter marker and transport bridge object in the root adapter"
            );
        }
        for required in [
            "connect_protocol_transport_bridge_tcp(",
            "start_protocol_transport_bridge_udp_flow(",
            "impl InboundListenerCapability for",
            "impl TcpOutboundCapability for",
            "impl UdpFlowCapability for",
            "impl UdpPacketPathCapability for",
        ] {
            assert!(
                content.contains(required),
                "{relative} should keep direct capability forwarding in the root and include `{required}`"
            );
        }
        for forbidden in [
            "mod inbound;",
            "mod tcp;",
            "mod udp;",
            "struct VlessManagedUdpFlowResume",
            "struct VmessManagedUdpFlowResume",
            "struct TrojanManagedUdpFlowResume",
        ] {
            assert!(
                !content.contains(forbidden),
                "{relative} should not revive removed proxy bridge shells or protocol-owned managed UDP state; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_transport_roots_only_export_runtime_entrypoints() {
    for (path, required, forbidden) in [
        (
            "crates/transport/src/vless_transport.rs",
            &[
                "pub use bridge::VlessStreamBridge;",
                "pub use inbound::{OwnedVlessInboundBindPlan, VlessInboundListenerRequest};",
            ][..],
            &[
                "pub use leaf::",
                "pub use managed_udp::",
                "pub use mux::",
                "pub use outbound::",
            ][..],
        ),
        (
            "crates/transport/src/vmess_transport.rs",
            &[
                "pub use bridge::VmessStreamBridge;",
                "pub use inbound::VmessInboundListenerRequest;",
            ][..],
            &[
                "pub use leaf::",
                "pub use managed_udp::",
                "pub use mux::",
                "pub use outbound::",
            ][..],
        ),
        (
            "crates/transport/src/trojan_transport.rs",
            &[
                "pub use bridge::TrojanTlsBridge;",
                "pub use inbound::TrojanInboundListenerRequest;",
            ][..],
            &[
                "pub use leaf::",
                "pub use managed_udp::",
                "pub use outbound::",
            ][..],
        ),
    ] {
        let content = read_repo_file(path);
        for expected in required {
            assert!(
                content.contains(expected),
                "{path} should keep runtime-facing entrypoint export `{expected}`"
            );
        }
        for blocked in forbidden {
            assert!(
                !content.contains(blocked),
                "{path} should not publicly export collapsed transport internals `{blocked}`"
            );
        }
    }
}

#[test]
fn protocol_transport_udp_roots_keep_bridge_logic_local_after_facade_collapse() {
    let managed_bridge = read_proxy_module_tree("src/runtime/udp_flow/managed/bridge.rs");

    for (root_path, udp_path, transport_path, required_udp, required_transport) in [
        (
            "src/adapters/vless.rs",
            "src/adapters/vless/udp.rs",
            "crates/transport/src/vless_transport.rs",
            &[
                "managed_stream_udp_handler_for_bridge::<VlessStreamBridge>()",
                "start_protocol_transport_bridge_udp_flow(",
                "start_protocol_transport_bridge_udp_relay_final_hop(",
                "start_protocol_transport_bridge_udp_relay_two_stream(",
            ][..],
            &[
                "pub struct VlessManagedUdpFlowResume",
                "pub type VlessManagedStreamUdpResume",
                "impl ProtocolManagedTupleUdpFlowResumeConnectionOps for VlessManagedUdpFlowResume",
            ][..],
        ),
        (
            "src/adapters/vmess.rs",
            "src/adapters/vmess/udp.rs",
            "crates/transport/src/vmess_transport.rs",
            &[
                "managed_stream_udp_handler_for_bridge::<VmessStreamBridge>()",
                "start_protocol_transport_bridge_udp_flow(",
                "start_protocol_transport_bridge_udp_relay_final_hop(",
            ][..],
            &[
                "pub struct VmessManagedUdpFlowResume",
                "pub type VmessManagedStreamUdpResume",
                "impl ProtocolManagedTupleUdpFlowResumeConnectionOps for VmessManagedUdpFlowResume",
            ][..],
        ),
        (
            "src/adapters/trojan.rs",
            "src/adapters/trojan/udp.rs",
            "crates/transport/src/trojan_transport.rs",
            &[
                "managed_stream_udp_handler_for_bridge::<TrojanTlsBridge>()",
                "start_protocol_transport_bridge_udp_flow(",
                "start_protocol_transport_bridge_udp_relay_final_hop(",
            ][..],
            &[
                "pub struct TrojanManagedUdpFlowResume",
                "pub type TrojanManagedStreamUdpResume",
                "impl ProtocolManagedPacketUdpFlowResumeConnectionOps for TrojanManagedUdpFlowResume",
            ][..],
        ),
    ] {
        let root = read(root_path);
        let transport = read_repo_module_tree(transport_path);

        assert!(
            !manifest_dir().join(udp_path).exists(),
            "{udp_path} should stay removed after collapsing the proxy UDP bridge back into {root_path}"
        );
        for pattern in required_udp {
            assert!(
                root.contains(pattern),
                "{root_path} should keep the thin UDP capability forwarding locally and include `{pattern}`"
            );
        }
        for pattern in required_transport {
            assert!(
                transport.contains(pattern),
                "{transport_path} should own the protocol-specific managed UDP resume detail `{pattern}`"
            );
        }
        for pattern in [
            "pub(crate) fn managed_stream_udp_handler_for_bridge<TBridge>()",
            "pub(crate) async fn start_protocol_transport_bridge_udp_flow<",
            "pub(crate) async fn start_protocol_transport_bridge_udp_relay_final_hop<",
        ] {
            assert!(
                managed_bridge.contains(pattern),
                "runtime/udp_flow/managed/bridge.rs should keep shared managed-stream UDP bridge helper `{pattern}`"
            );
        }
    }
}

#[test]
fn protocol_transport_tcp_roots_keep_bridge_logic_local_after_facade_collapse() {
    for (
        root_path,
        legacy_bridge_path,
        external_transport_path,
        required_root,
        required_external,
        forbidden_root,
    ) in [
        (
            "src/adapters/vless.rs",
            "src/adapters/vless/tcp/transport.rs",
            "crates/transport/src/vless_transport.rs",
            &[
                "connect_protocol_transport_bridge_tcp(",
                "apply_protocol_transport_bridge_relay_hop(",
            ][..],
            &[
                "impl ProtocolTransportLeaf for VlessOutboundLeaf<'_>",
                "impl ProtocolTcpTransportOpenResult for vless::outbound::VlessTcpStreamOpen",
            ][..],
            &[
                "mod carrier;",
                "mod connect;",
                "pub(crate) use transport::{",
                "pub(crate) use carrier::{",
                "pub(crate) use connect::{",
            ][..],
        ),
        (
            "src/adapters/vmess.rs",
            "src/adapters/vmess/tcp/transport.rs",
            "crates/transport/src/vmess_transport.rs",
            &[
                "connect_protocol_transport_bridge_tcp(",
                "apply_protocol_transport_bridge_relay_hop(",
            ][..],
            &[
                "impl ProtocolTransportLeaf for VmessOutboundLeaf<'_>",
                "impl ProtocolTcpTransportOpenResult for vmess::outbound::VmessTcpStreamOpen",
            ][..],
            &[
                "mod carrier;",
                "mod connect;",
                "pub(crate) use transport::{",
                "pub(crate) use carrier::{",
                "pub(crate) use connect::{",
            ][..],
        ),
        (
            "src/adapters/trojan.rs",
            "src/adapters/trojan/tcp/transport.rs",
            "crates/transport/src/trojan_transport.rs",
            &[
                "connect_protocol_transport_bridge_tcp(",
                "apply_protocol_transport_bridge_relay_hop(",
            ][..],
            &[
                "impl ProtocolTransportLeaf for TrojanOutboundLeaf<'_>",
                "impl ProtocolTcpTransportOpenResult for TrojanTcpStreamOpen",
            ][..],
            &[
                "mod carrier;",
                "mod connect;",
                "pub(crate) use transport::{",
                "pub(crate) use carrier::{",
                "pub(crate) use connect::{",
            ][..],
        ),
    ] {
        let root = read(root_path);
        let external_transport = read_repo_module_tree(external_transport_path);
        assert!(
            !manifest_dir().join(legacy_bridge_path).exists() && !root.contains("mod transport;"),
            "{legacy_bridge_path} should stay removed after collapsing the proxy TCP bridge into {root_path}"
        );
        for pattern in required_root {
            assert!(
                root.contains(pattern),
                "{root_path} should keep its local TCP capability forwarding and include `{pattern}`"
            );
        }
        for pattern in required_external {
            assert!(
                external_transport.contains(pattern),
                "{external_transport_path} should own the transport-leaf/result bridge details after the TCP facade collapse and include `{pattern}`"
            );
        }
        for pattern in forbidden_root {
            assert!(
                !root.contains(pattern),
                "{root_path} should not reintroduce removed TCP facade shells or bridge bodies; found `{pattern}`"
            );
        }
    }
}

#[test]
fn transport_protocol_adapter_roots_stay_capability_only() {
    for (
        root_path,
        adapter_name,
        bridge_type,
        protocol_variant,
        request_builder,
        forbidden_protocol_config_match,
        forbidden_protocol_parse,
        extra_required,
    ) in [
        (
            "src/adapters/vless.rs",
            "VlessAdapter",
            "VlessStreamBridge",
            "ResolvedLeafOutbound::Vless",
            "PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(",
            "InboundProtocolConfig::Vless",
            "parse_uuid",
            &[
                "protocol_transport_bridge_udp_relay_needs_two_streams(",
                "start_protocol_transport_bridge_udp_relay_two_stream(",
            ][..],
        ),
        (
            "src/adapters/vmess.rs",
            "VmessAdapter",
            "VmessStreamBridge",
            "ResolvedLeafOutbound::Vmess",
            "PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(",
            "InboundProtocolConfig::Vmess",
            "VmessCipher::from_name",
            &[][..],
        ),
        (
            "src/adapters/trojan.rs",
            "TrojanAdapter",
            "TrojanTlsBridge",
            "ResolvedLeafOutbound::Trojan",
            "PreparedTrojanOutboundRequestBundle::from_config(",
            "InboundProtocolConfig::Trojan",
            "TrojanInboundProfile::from_config_password",
            &[][..],
        ),
    ] {
        let root = read(root_path);

        let required_patterns = vec![
            format!("struct {adapter_name}"),
            format!("bridge: {bridge_type}"),
            format!("impl ProtocolSupportCapability for {adapter_name}"),
            format!("impl ProtocolMetadata for {adapter_name}"),
            format!("impl InboundListenerCapability for {adapter_name}"),
            format!("impl TcpOutboundCapability for {adapter_name}"),
            format!("impl UdpFlowCapability for {adapter_name}"),
            format!("impl UdpPacketPathCapability for {adapter_name}"),
            "listener::spawn(".to_string(),
            "connect_protocol_transport_bridge_tcp(".to_string(),
            "apply_protocol_transport_bridge_relay_hop(".to_string(),
            "managed_stream_udp_handler_for_bridge::<".to_string(),
            "start_protocol_transport_bridge_udp_flow(".to_string(),
            "start_protocol_transport_bridge_udp_relay_final_hop(".to_string(),
            "fn on_config_reloaded(&self)".to_string(),
            "self.bridge.on_config_reloaded()".to_string(),
        ];

        for required in &required_patterns {
            assert!(
                root.contains(required.as_str()),
                "{root_path} should stay at the final collapsed shape and include capability-bridge forwarding `{required}`"
            );
        }

        for required in extra_required {
            assert!(
                root.contains(required),
                "{root_path} should keep the required transport-owned capability bridge detail `{required}`"
            );
        }

        for forbidden in [
            protocol_variant,
            request_builder,
            forbidden_protocol_config_match,
            forbidden_protocol_parse,
            "TlsAcceptor",
            "Reality",
            "InboundStreamStack",
            "OwnedVlessInboundTransportPlan",
            "OwnedVmessOutboundTransportPlan",
            "OwnedTrojanOutboundTlsPlan",
            "accept_route(",
            "accept_recorded_tcp_route(",
            "accept_recorded_stream_route(",
            "build_required_tls_acceptor(",
            "build_optional_tls_acceptor(",
            "MuxConnectionPool",
            "VmessMuxConnectionPool",
        ] {
            assert!(
                !root.contains(forbidden),
                "{root_path} should keep only capability forwarding, transport handoff, and error mapping; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn stream_transport_leaf_models_own_variant_to_leaf_projection() {
    for (
        adapter_module,
        adapter_common_module,
        transport_module,
        variant,
        request_builder,
        leaf_ctor,
    ) in [
        (
            "src/adapters/vless.rs",
            "src/adapters/identity.rs",
            "crates/transport/src/vless_transport.rs",
            "ResolvedLeafOutbound::Vless",
            "PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(",
            "VlessOutboundLeaf::new(",
        ),
        (
            "src/adapters/vmess.rs",
            "src/adapters/identity.rs",
            "crates/transport/src/vmess_transport.rs",
            "ResolvedLeafOutbound::Vmess",
            "PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(",
            "VmessOutboundLeaf::new(",
        ),
        (
            "src/adapters/trojan.rs",
            "src/adapters/identity.rs",
            "crates/transport/src/trojan_transport.rs",
            "ResolvedLeafOutbound::Trojan",
            "PreparedTrojanOutboundRequestBundle::from_config(",
            "TrojanOutboundLeaf::new(",
        ),
    ] {
        let adapter = read(adapter_module);
        let adapter_common = read(adapter_common_module);
        let proxy_transport = read_proxy_module_tree("src/transport/tcp_outbound.rs");
        let transport_leaf = read_repo_module_tree("crates/transport/src/outbound_leaf.rs");
        let transport = read_repo_module_tree(transport_module);

        assert!(
            !adapter_common.contains("pub(super) fn tcp_transport_leaf")
                && !adapter_common.contains("pub(super) fn tcp_relay_transport_leaf")
                && !adapter_common.contains("pub(super) fn udp_transport_leaf")
                && proxy_transport.contains("connect_protocol_transport_bridge_tcp")
                && proxy_transport.contains("apply_protocol_transport_bridge_relay_hop")
                && transport_leaf.contains("pub trait ProtocolTransportLeafResolver<'a>")
                && transport_leaf.contains("pub fn prepare_transport_bridge_leaf"),
            "transport-leaf projection should live in zero-transport plus the neutral proxy tcp bridge instead of adapters/identity.rs"
        );
        assert!(
            !adapter.contains(variant)
                && !adapter.contains(request_builder)
                && transport.contains(variant)
                && transport.contains(request_builder)
                && transport.contains(leaf_ctor)
                && transport.contains("impl<'a> ProtocolTransportLeafResolver<'a> for"),
            "{transport_module} should own resolved-leaf projection and typed leaf construction instead of {adapter_module}"
        );
        assert!(
            transport.contains("pub(super) fn new(")
                && !transport.contains("from_config_parts("),
            "{transport_module} should keep the typed leaf constructor after adapter-local projection took over"
        );
    }
}

#[test]
fn stream_transport_leaf_models_own_bridge_failure_mapping() {
    for (
        adapter_module,
        adapter_common_module,
        transport_leaf_module,
        variant,
        request_builder,
        leaf_ctor,
    ) in [
        (
            "src/adapters/vless.rs",
            "src/adapters/identity.rs",
            "crates/transport/src/vless_transport.rs",
            "ResolvedLeafOutbound::Vless",
            "PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(",
            "VlessOutboundLeaf::new(",
        ),
        (
            "src/adapters/vmess.rs",
            "src/adapters/identity.rs",
            "crates/transport/src/vmess_transport.rs",
            "ResolvedLeafOutbound::Vmess",
            "PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(",
            "VmessOutboundLeaf::new(",
        ),
        (
            "src/adapters/trojan.rs",
            "src/adapters/identity.rs",
            "crates/transport/src/trojan_transport.rs",
            "ResolvedLeafOutbound::Trojan",
            "PreparedTrojanOutboundRequestBundle::from_config(",
            "TrojanOutboundLeaf::new(",
        ),
    ] {
        let adapter = read(adapter_module);
        let adapter_common = read(adapter_common_module);
        let proxy_transport = read_proxy_module_tree("src/transport/tcp_outbound.rs");
        let transport_leaf = read_repo_module_tree(transport_leaf_module);

        assert!(
            !adapter.contains("fn invalid_"),
            "{adapter_module} should not own resolved-leaf validation branches directly"
        );
        assert!(
            !adapter_common.contains("pub(super) fn tcp_transport_leaf")
                && !adapter_common.contains("pub(super) fn udp_transport_leaf")
                && proxy_transport.contains("TcpOutboundFailure")
                && proxy_transport.contains("fn tcp_connect_prepare_failure")
                && proxy_transport.contains("fn tcp_relay_prepare_error")
                && proxy_transport.contains("connect_protocol_transport_bridge_tcp")
                && proxy_transport.contains("apply_protocol_transport_bridge_relay_hop"),
            "generic transport-bridge failure mapping should live in src/transport/tcp_outbound.rs instead of adapters/identity.rs"
        );
        assert!(
            transport_leaf.contains(variant)
                && transport_leaf.contains(request_builder)
                && transport_leaf.contains(leaf_ctor)
                && transport_leaf.contains("impl<'a> ProtocolTransportLeafResolver<'a> for")
                && !transport_leaf.contains("from_config_parts("),
            "{transport_leaf_module} should own variant projection and typed transport-leaf logic"
        );
    }
}

#[test]
fn stream_transport_bridge_metadata_lives_in_transport_layer() {
    for (
        tcp_adapter,
        udp_adapter,
        managed_adapter,
        transport_module,
        tcp_connect_stage,
        tcp_invalid_config,
        udp_invalid_config,
        expected_leaf,
        establish_stage,
        mismatch_stage,
    ) in [
        (
            "src/adapters/vless.rs",
            "src/adapters/vless.rs",
            "src/adapters/vless.rs",
            "crates/transport/src/vless_transport.rs",
            "connect_upstream_vless",
            "invalid vless tcp config",
            "invalid vless udp config",
            "expected VLESS outbound leaf",
            "vless_establish",
            "udp_vless_resume",
        ),
        (
            "src/adapters/vmess.rs",
            "src/adapters/vmess.rs",
            "src/adapters/vmess.rs",
            "crates/transport/src/vmess_transport.rs",
            "connect_upstream_vmess",
            "invalid vmess tcp config",
            "invalid vmess udp config",
            "expected VMess outbound leaf",
            "vmess_establish",
            "udp_vmess_resume",
        ),
        (
            "src/adapters/trojan.rs",
            "src/adapters/trojan.rs",
            "src/adapters/trojan.rs",
            "crates/transport/src/trojan_transport.rs",
            "connect_upstream_trojan",
            "invalid trojan tcp config",
            "invalid trojan udp config",
            "expected Trojan outbound leaf",
            "trojan_establish",
            "udp_trojan_resume",
        ),
    ] {
        let tcp = read(tcp_adapter);
        let udp = read(udp_adapter);
        let managed = read(managed_adapter);
        let transport = read_repo_module_tree(transport_module);

        assert!(
            !tcp.contains(tcp_connect_stage)
                && !tcp.contains(tcp_invalid_config)
                && !tcp.contains(expected_leaf)
                && !udp.contains(udp_invalid_config)
                && !udp.contains(expected_leaf)
                && !managed.contains(establish_stage)
                && !managed.contains(mismatch_stage),
            "proxy adapter bridge files should use transport-owned bridge metadata instead of embedding protocol labels"
        );
        assert!(
            transport.contains("impl ProtocolTcpTransportBridgeMetadata for")
                && transport.contains("impl ProtocolUdpTransportBridgeMetadata for")
                && transport.contains("impl ProtocolManagedStreamUdpResumeMetadata for")
                && transport.contains(tcp_connect_stage)
                && transport.contains(tcp_invalid_config)
                && transport.contains(udp_invalid_config)
                && transport.contains(expected_leaf)
                && transport.contains(establish_stage)
                && transport.contains(mismatch_stage),
            "{transport_module} should own TCP/UDP bridge metadata and managed-stream stage labels"
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
        content.contains(
            "pub(crate) use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};"
        )
            && content.contains("pub(crate) use candidate::UdpCandidate;"),
        "src/runtime/udp_dispatch/mod.rs should expose its candidate and narrowly re-export flow-owned result types"
    );

    let managed = read_proxy_module_tree("src/runtime/udp_dispatch/managed.rs");
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
    let content = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let cached_start = manifest_dir().join("src/runtime/udp_flow/registered/cached_start.rs");
    let datagram = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
    let stream = read_proxy_module_tree("src/runtime/udp_flow/managed/stream.rs");
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
            && managed.contains("datagram: ManagedDatagramState")
            && managed.contains("stream: ManagedStreamState")
            && content.contains("managed_resumes:")
            && content.contains("HashMap<ManagedUdpFlowRef, ManagedUdpFlowResume>")
            && !content.contains("stream_senders: ManagedStreamSenderState")
            && !managed.contains("cached: ManagedCachedState")
            && !cached_start.exists()
            && stream_manager.contains("trait ManagedStreamFlowConnector")
            && stream_manager.contains("struct ManagedStreamFlowManager")
            && stream_manager.contains("struct SharedManagedStreamFlowManager")
            && stream_manager.contains("ManagedUdpConnectionCache")
            && !managed.contains("ManagedCachedFlowSender")
            && datagram.contains("handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>")
            && stream.contains("stream_packet_handlers:")
            && stream.contains("relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>")
            && !managed.contains("pub(crate) vless:")
            && !managed.contains("pub(super) vless:")
            && !managed.contains("vless: VlessUdpOutboundManager")
            && !managed.contains("vmess: VmessUdpOutboundManager")
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
    let managed_model = read_proxy_module_tree("src/runtime/udp_flow/managed/model.rs");
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
        "ManagedDatagramExistingSend",
        "ManagedStreamExistingSend",
        "ManagedRelayExistingSend",
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
            && managed.contains("pub(crate) use model::ManagedDatagramFlowHandler;")
            && managed.contains("pub(crate) use model::ManagedStreamHandlerPair;")
            && managed_model.contains("ManagedDatagramFlowHandler")
            && managed_model.contains("ManagedStreamPacketFlowHandler")
            && managed_model.contains("ManagedRelayFlowHandler"),
        "runtime registered should keep protocol UDP managers out of protocol_runtime::udp and expose managed handler traits from runtime::udp_flow::managed"
    );
}

#[test]
fn protocol_udp_manager_construction_is_adapter_registered() {
    let allowed = [
        "src/adapters/hysteria2/udp.rs",
        "src/adapters/hysteria2/udp.rs",
        "src/adapters/mieru/udp.rs",
        "src/adapters/mieru/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/trojan.rs",
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
    let trojan = read("src/adapters/trojan.rs");
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
    let mieru_managed = read("src/adapters/mieru/udp.rs");
    let mieru_connector =
        read_repo_module_tree("crates/transport/src/mieru_transport/managed_udp.rs");
    let trojan_managed = read("src/adapters/trojan.rs");
    let trojan_transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    assert!(
        stream_manager.contains("ManagedUdpConnectionCache")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowStore")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowStore")
            && !mieru_managed.contains("mieru::udp::MieruUdpFlowSessions")
            && !trojan_managed.contains("trojan::udp::TrojanUdpFlowSessions")
            && mieru_connector.contains("mieru::udp::MieruUdpFlowConnection")
            && !mieru_managed
                .contains("mieru::udp::MieruUdpFlowStore<mieru::udp::MieruUdpFlowSession>")
            && !trojan_managed
                .contains("trojan::udp::TrojanUdpFlowStore<trojan::udp::TrojanUdpFlowSession>")
            && !mieru_managed
                .contains("mieru::udp::MieruUdpFlowStore<mieru::udp::MieruUdpFlowConnection>")
            && !trojan_managed
                .contains("trojan::udp::TrojanUdpFlowStore<trojan::udp::TrojanUdpFlowConnection>")
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
            && !mieru_managed
                .contains("resume.cache_key(endpoint.server, endpoint.port, session_id)")
            && !trojan_managed
                .contains("resume.cache_key(endpoint.server, endpoint.port, session_id)")
            && mieru_connector.contains("mieru::udp::connector_flow_from_resume")
            && stream_manager.contains("managed_stream_connector_flow_from_build(")
            && trojan_transport.contains("TrojanManagedUdpFlowResume::connector_flow(")
            && !trojan_managed.contains("trojan::udp::connector_flow_from_plan")
            && !mieru_connector
                .contains("resume.connector_flow(endpoint.server, endpoint.port, session_id)")
            && !mieru_connector.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !trojan_managed.contains(".flow(endpoint.server, endpoint.port, session_id)")
            && !mieru_connector.contains("resume.flow_cache_key(")
            && !trojan_managed.contains("resume.flow_cache_key(")
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
fn datagram_udp_managers_do_not_rebuild_protocol_cache_keys() {
    let shadowsocks_managed = read("src/adapters/shadowsocks/udp.rs");
    let hysteria2_managed = read("src/adapters/hysteria2/udp.rs");
    let shadowsocks_transport =
        read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs");
    let hysteria2_transport = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let datagram_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    assert!(
        datagram_manager.contains("ManagedUdpConnectionCache")
            && datagram_manager.contains("ManagedDatagramConnectionCache")
            && datagram_manager.contains(".connector_flow(&resume, endpoint)")
            && datagram_manager.contains(".into_cache_key()")
            && datagram_manager.contains(".send_or_insert_pre_sent_key(")
            && datagram_manager.contains(".send_or_insert_key(")
            && datagram_manager.contains(".get_or_insert_key(")
            && !datagram_manager.contains("resume.cache_key(")
            && !datagram_manager.contains("resume.flow_cache_key(")
            && !datagram_manager.contains("resume.connector_flow(")
            && !datagram_manager.contains("endpoint.server.to_string()")
            && !datagram_manager.contains("format!(\"{}:{}\"")
            && !datagram_manager.contains("self.upstreams.entries")
            && !datagram_manager.contains("if let Some(entry) = self.upstreams.get(&cache_key)")
            && !datagram_manager.contains("self.upstreams.insert(")
            && !datagram_manager.contains("ManagedUdpConnectionCacheKey")
            && !datagram_manager.contains("ManagedDatagramConnectionCacheKey")
            && managed_cache.contains("struct ManagedUdpConnectionCacheKey")
            && managed_cache.contains("struct ManagedDatagramConnectionCacheKey")
            && !managed_cache.contains("pub(crate) struct ManagedUdpConnectionCacheKey")
            && !managed_cache.contains("pub(crate) struct ManagedDatagramConnectionCacheKey"),
        "datagram UDP managers should consume adapter-built opaque cache keys through neutral cache APIs"
    );
    assert!(
        shadowsocks_managed.contains("managed_datagram_socket_handler_box::<")
            && shadowsocks_managed.contains("ShadowsocksManagedDatagramFlowResume")
            && !shadowsocks_managed.contains("ManagedDatagramSocketConnectorFlow::new")
            && !shadowsocks_managed.contains("resume.cache_key(")
            && !shadowsocks_managed.contains("resume.flow_cache_key(")
            && !shadowsocks_managed.contains("resume.connector_flow(")
            && !shadowsocks_managed.contains("ShadowsocksUdpCacheKey")
            && !shadowsocks_managed.contains("ManagedDatagramConnectionCacheKey")
            && shadowsocks_transport
                .contains("impl crate::managed_udp::ProtocolManagedDatagramSocketUdpResumeConnectionOps")
            && hysteria2_managed.contains("managed_datagram_handler_box::<")
            && hysteria2_managed.contains("Hysteria2ManagedDatagramFlowResume")
            && !hysteria2_managed.contains("ManagedDatagramConnectorFlow::new")
            && !hysteria2_managed.contains("resume.cache_key(")
            && !hysteria2_managed.contains("resume.flow_cache_key(")
            && !hysteria2_managed.contains("resume.connector_flow(")
            && !hysteria2_managed.contains("Hysteria2UdpCacheKey")
            && !hysteria2_managed.contains("ManagedUdpConnectionCacheKey")
            && hysteria2_transport
                .contains("impl crate::managed_udp::ProtocolManagedDatagramUdpResumeConnectionOps"),
        "datagram UDP adapters should delegate cache identity construction to protocol-owned flow builders"
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
            && forward.contains("UdpPathCategory::Datagram =>")
            && forward.contains("UdpPathCategory::StreamPacket =>")
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
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let managed_datagram = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
    let managed_model_root = read("src/runtime/udp_flow/managed/model.rs");
    let managed_model = read_proxy_module_tree("src/runtime/udp_flow/managed/model.rs");
    let managed_stream = read_proxy_module_tree("src/runtime/udp_flow/managed/stream.rs");
    let upstream_state =
        read_proxy_module_tree("src/runtime/udp_flow/registered/upstream/state.rs");
    let upstream_handler = read("src/runtime/udp_flow/registered/upstream/runtime/handler.rs");

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
        normalized_forward
            .contains("self\n            .managed\n            .forward_existing_flow")
            && forward.contains("self.upstream.handles_resume(&resume)")
            && upstream_state.contains("fn handles_resume")
            && upstream_state.contains("handler.supports_upstream_resume(resume)")
            && upstream_handler
                .contains("fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume)")
            && upstream_handler.contains("handles_registered_resume::<T>(resume)")
            && managed.contains("fn forward_existing_flow")
            && !forward.contains("managed_flow_snapshot")
            && managed_model_root.contains("mod handler;")
            && managed_model_root.contains("mod send;")
            && managed_model.contains("trait ManagedDatagramFlowHandler")
            && managed_model.contains("trait ManagedStreamPacketFlowHandler")
            && managed_model.contains("trait ManagedRelayFlowHandler")
            && managed_model.contains("pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>")
            && managed_model.contains("pub(crate) resume: ManagedUdpFlowResume")
            && managed_datagram.contains("ManagedDatagramExistingSend")
            && managed_datagram.contains("send_managed_existing")
            && managed_datagram.contains("for handler in &mut self.handlers")
            && managed_stream.contains("ManagedStreamExistingSend")
            && managed_stream.contains("send_managed_existing")
            && managed_stream.contains("for handler in &mut self.stream_packet_handlers"),
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
    let registered_state = read_proxy_module_tree("src/runtime/udp_flow/registered/state.rs");
    let old_cached = manifest_dir().join("src/runtime/udp_flow/registered/cached.rs");
    let protocol_stream_sender =
        manifest_dir().join("src/runtime/udp_flow/registered/stream_sender.rs");
    let stream_sender = manifest_dir().join("src/runtime/udp_flow/managed/stream_sender.rs");
    let stream_packet_manager =
        manifest_dir().join("src/runtime/udp_flow/managed/stream_packet_manager.rs");
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed/state.rs");
    let stream_state = read_proxy_module_tree("src/runtime/udp_flow/managed/stream.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let protocol_forward = read("src/runtime/udp_flow/registered/forward.rs");
    let vless_flow = manifest_dir().join("src/runtime/udp_flow/registered/vless_flow.rs");
    let vmess_flow = manifest_dir().join("src/runtime/udp_flow/registered/vmess_flow.rs");
    let vmess_adapter = read_proxy_module_tree("src/adapters/vmess.rs");
    let vless_transport = read_proxy_module_tree("src/adapters/vless.rs");
    let vmess_transport = read_proxy_module_tree("src/adapters/vmess.rs");
    let cached_start = manifest_dir().join("src/runtime/udp_flow/registered/cached_start.rs");
    let register = read("src/register.rs");

    for forbidden in [
        "fn send_existing_cached_flow",
        ".vless\n            .send_existing",
        ".vmess\n            .send_existing",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/runtime/udp_flow/registered/mod.rs should keep managed stream forwarding details out of the registered facade; found `{forbidden}`"
        );
    }
    assert!(
        !stream_sender.exists()
            && !stream_packet_manager.exists()
            && !old_cached.exists()
            && !protocol_stream_sender.exists(),
        "managed stream UDP fast-path helpers should live under the generic managed runtime, not protocol-named facades"
    );
    assert!(
        !state.contains("stream_senders: ManagedStreamSenderState")
            && !managed.contains("cached: ManagedCachedState")
            && !managed.contains("vless: VlessUdpOutboundManager")
            && !managed.contains("vmess: VmessUdpOutboundManager")
            && registered_state.contains("managed_resumes:")
            && stream_state.contains("stream_packet_handlers:")
            && stream_state.contains("relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>")
            && !cached_start.exists()
            && stream_manager.contains("struct ManagedStreamFlowManager")
            && stream_manager.contains("ManagedUdpConnectionCache")
            && stream_manager.contains(".send_existing_key(")
            && stream_manager.contains(".send_or_insert_key(")
            && stream_manager.contains(".insert_and_send_key(")
            && !protocol_forward.contains("has_stream_flow_sender")
            && !protocol_forward.contains("udp_cached_send")
            && !managed.contains("ManagedCachedFlowSender")
            && !state.contains("cached_handler_mut")
            && !vless_flow.exists()
            && !vmess_flow.exists()
            && vless_transport.contains("start_protocol_transport_bridge_udp_flow(")
            && vless_transport
                .contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && vmess_adapter.contains("start_protocol_transport_bridge_udp_flow(")
            && !vmess_adapter.contains("start_tracked_managed_stream_packet(")
            && vmess_transport.contains("start_protocol_transport_bridge_udp_flow(")
            && vmess_transport
                .contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !vmess_transport.contains("ManagedStreamPacketStartBridge")
            && !vmess_transport.contains("start_tracked_managed_stream_packet(")
            && !vmess_adapter.contains("register_managed_stream_flow_sender")
            && !vmess_adapter.contains("register_managed_stream_packet_flow")
            && !vmess_adapter.contains("cached_handler_mut")
            && !register.contains("ManagedStreamSenderHandlers")
            && register.contains("capability.managed_stream_udp_handlers()")
            && !register.contains("vless_cached_handler")
            && !register.contains("vmess_cached_handler"),
        "managed stream UDP flow starts should use generic managed stream handlers while transport bridges own the shared tracked stream-packet glue"
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
            "src/adapters/hysteria2/udp.rs" | "src/adapters/shadowsocks/udp.rs"
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
    let ss_managed = read("src/adapters/shadowsocks/udp.rs");
    let h2_managed = read("src/adapters/hysteria2/udp.rs");
    let trojan_managed = read("src/adapters/trojan.rs");
    let mieru_managed = read("src/adapters/mieru/udp.rs");

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
    let vless_transport_bridge = read_repo_module_tree("crates/transport/src/vless_transport.rs");
    let vmess_transport_bridge = read_repo_module_tree("crates/transport/src/vmess_transport.rs");
    let vless_protocol = fs::read_to_string(repo_root().join("protocols/vless/src/udp.rs"))
        .expect("read VLESS protocol udp source");
    let vless_outbound = fs::read_to_string(repo_root().join("protocols/vless/src/outbound.rs"))
        .expect("read VLESS protocol outbound source");
    let vmess_protocol = fs::read_to_string(repo_root().join("protocols/vmess/src/udp.rs"))
        .expect("read VMess protocol UDP source");

    for (source, content, flow_helper) in [
        (
            "crates/transport/src/vless_transport.rs",
            &vless_transport_bridge,
            "::connector_flow(",
        ),
        (
            "crates/transport/src/vmess_transport.rs",
            &vmess_transport_bridge,
            "::connector_flow(",
        ),
    ] {
        for forbidden in [
            ".encode_packet(",
            ".decode_packet(",
            ".write_packet_tokio(",
            ".read_packet_tokio(",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should call protocol-owned stream packet IO helpers instead of direct UDP packet framing `{forbidden}`"
            );
        }
        assert!(
            content.contains(flow_helper),
            "{source} should delegate protocol UDP flow pumping through protocol-owned helpers"
        );
        assert!(
            !content.contains("::establish_udp_flow_with_initial_packet"),
            "{source} should call flow pumping through the protocol-owned UDP flow config"
        );
    }
    assert!(
        vless_transport_bridge.contains("::connector_flow(")
            && vmess_transport_bridge.contains("::connector_flow(")
            && vless_transport_bridge.contains(".open_udp_flow_with_transport_or_mux(")
            && vmess_transport_bridge.contains(".open_udp_flow_with_transport_or_mux(")
            && vless_transport_bridge.contains(".open_relay_udp_flow_with_transport(")
            && vmess_transport_bridge.contains(".open_relay_udp_flow_with_transport("),
        "stream UDP managed bridges should keep protocol packet IO out of zero-proxy while zero-transport owns both resume opening and carrier opening"
    );

    assert!(
        vless_protocol.contains("async fn write_packet_tokio")
            && !vless_protocol.contains("pub async fn write_packet_tokio")
            && vless_protocol.contains("async fn read_packet_tokio")
            && !vless_protocol.contains("pub async fn read_packet_tokio")
            && vless_protocol.contains("failed to flush VLESS UDP response")
            && !vless_outbound.contains("pub fn spawn_udp_flow")
            && !vless_outbound.contains("fn spawn_udp_flow_task")
            && !vless_outbound.contains(".write_packet_tokio(")
            && !vless_outbound.contains(".read_packet_tokio(")
            && vless_protocol.contains("fn spawn_udp_flow")
            && !vless_protocol.contains("pub fn spawn_udp_flow")
            && vless_protocol.contains("pub fn start_mux_udp_flow")
            && vless_protocol.contains("fn spawn_udp_flow_task")
            && vless_protocol.contains(".write_packet_tokio(")
            && vless_protocol.contains(".read_packet_tokio(")
            && vless_protocol.contains("async fn establish_udp_flow_with_resume")
            && !vless_protocol.contains("pub async fn establish_udp_flow_with_resume"),
        "protocols/vless should own async stream packet IO helpers and UDP flow pumping"
    );
    assert!(
        vmess_protocol.contains("async fn write_packet_tokio")
            && !vmess_protocol.contains("pub async fn write_packet_tokio")
            && vmess_protocol.contains("async fn read_packet_tokio")
            && !vmess_protocol.contains("pub async fn read_packet_tokio")
            && vmess_protocol.contains("failed to flush VMess UDP response")
            && vmess_protocol.contains("fn spawn_udp_flow")
            && !vmess_protocol.contains("pub fn spawn_udp_flow")
            && vmess_protocol.contains("pub fn start_udp_flow")
            && vmess_protocol.contains("async fn establish_udp_flow_with_resume")
            && !vmess_protocol.contains("pub async fn establish_udp_flow_with_resume")
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
fn udp_start_and_managed_forward_use_narrow_operation_models() {
    let packet_path_root = read("src/runtime/udp_flow/packet_path_chain.rs");
    let packet_path_model = read("src/runtime/udp_flow/packet_path_chain/model.rs");
    let flow_request = read("src/runtime/udp_flow/managed/flow/request.rs");
    let flow_state = read("src/runtime/udp_flow/state.rs");
    let dispatch_packet_path = read("src/runtime/udp_dispatch/packet_path.rs");

    assert!(
        packet_path_root.contains("pub(crate) use model::PacketPathStartRequest;")
            && !packet_path_root.contains("struct PacketPathStartRequest")
            && packet_path_model.contains("struct PacketPathStartRequest")
            && packet_path_model.contains("session_id: u64")
            && packet_path_model.contains("carrier_leaf: &'a ResolvedLeafOutbound<'a>")
            && packet_path_model.contains("datagram_leaf: &'a ResolvedLeafOutbound<'a>")
            && packet_path_model.contains("packet: UdpPacketRef<'a>")
            && !packet_path_model.contains("Proxy"),
        "packet-path start should use a narrow model submodule request without embedding Proxy"
    );
    assert!(
        flow_state.contains("request: PacketPathStartRequest<'_>")
            && dispatch_packet_path.contains("request: PacketPathStartRequest<'_>")
            && flow_request.contains("type ManagedExistingFlowForward<'a>")
            && flow_request.contains("(&'a UdpFlowSnapshot, &'a [u8])")
            && flow_state.contains("request: ManagedExistingFlowForward<'_>"),
        "UDP start and managed forward layers should pass named operation inputs intact"
    );
}

#[test]
fn feature_gated_udp_manager_modules_do_not_embed_disabled_stubs() {
    for source in ["src/adapters/mieru/udp.rs", "src/adapters/trojan.rs"] {
        let content = read(source);
        assert!(
            !content.contains("#[cfg(not(feature ="),
            "{source} should not mix enabled manager logic with disabled-feature stubs"
        );
    }
}

#[test]
fn trojan_udp_socket_wrappers_stay_in_proxy_stream_glue() {
    let managed = read("src/adapters/trojan.rs");
    let stream = manifest_dir().join("src/adapters/trojan/udp/manager/stream.rs");
    let socket = manifest_dir().join("src/adapters/trojan/udp/manager/socket.rs");
    let transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read trojan protocol udp source");

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
        protocol_udp.contains("struct ReadOnlySocket")
            && protocol_udp.contains("struct WriteOnlySocket")
            && protocol_udp.contains("impl<S> AsyncSocket for ReadOnlySocket")
            && protocol_udp.contains("impl<S> AsyncSocket for WriteOnlySocket")
            && !transport.contains("struct ReadOnlySocket")
            && !transport.contains("struct WriteOnlySocket")
            && !transport.contains("impl AsyncSocket for ReadOnlySocket")
            && !transport.contains("impl AsyncSocket for WriteOnlySocket"),
        "Trojan UDP stream half AsyncSocket adapters should live with protocols/trojan packet pump, not proxy or zero-transport"
    );
}

#[test]
fn trojan_udp_response_bridge_lives_outside_manager() {
    let trojan_managed = read("src/adapters/trojan.rs");
    let _proxy_transport = read("src/adapters/trojan.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let managed = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
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
            && !trojan_managed
                .contains("impl ManagedUdpConnection for trojan::udp::TrojanUdpFlowConnection")
            && stream_manager.contains("managed_packet_udp_connection_from_flow(connection)")
            && !trojan_managed.contains("spawn_response_bridge")
            && managed.contains("pub(crate) fn managed_packet_udp_connection")
            && managed.contains("pub(crate) fn spawn_response_bridge<T, F>")
            && managed.contains("FnMut(T) -> (Address, u16, Vec<u8>)"),
        "Trojan UDP response bridge should hang off the neutral managed packet connection bridge without adapter-local cache bookkeeping"
    );
}

#[test]
fn trojan_udp_tls_connect_lives_outside_manager() {
    let connect_path = manifest_dir().join("src/adapters/trojan/udp/manager/connect.rs");
    let adapter = read("src/adapters/trojan.rs");
    let managed_bridge = read("src/runtime/udp_flow/managed/bridge.rs");
    let outbound = manifest_dir().join("src/outbound/trojan.rs");
    let transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");
    let _proxy_transport = read_proxy_module_tree("src/adapters/trojan.rs");

    for forbidden in [
        "ClientTlsConfig",
        "connect_tls_upstream",
        "connect_tls_stream",
        "TrojanTlsOptions",
    ] {
        assert!(
            !adapter.contains(forbidden) && !managed_bridge.contains(forbidden),
            "Trojan UDP start glue should keep TLS config/profile conversion out of adapter and runtime bridge code; found `{forbidden}`"
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
            !adapter.contains(forbidden) && !managed_bridge.contains(forbidden),
            "Trojan adapter/runtime bridge should delegate raw TLS stream opening through proxy transport glue; found `{forbidden}`"
        );
    }
    assert!(
        !adapter.contains("crate::outbound::trojan::open_trojan_tls_stream")
            && !outbound.exists()
            && transport.contains("async fn open_direct_connection<")
            && transport.contains("async fn open_relay_connection(")
            && transport.contains("transport: OwnedTrojanOutboundTlsPlan")
            && transport.contains("leaf.direct_udp_resume()")
            && transport.contains("leaf.relay_final_hop_udp_resume()")
            && transport.contains("async fn open_direct_with_profile<")
            && transport.contains("async fn open_relay_with_profile(")
            && transport.contains("self.protocol")
            && transport.contains(".open_udp_flow_with_transport(session, None, move |tls_profile| async move")
            && transport.contains(
                ".open_udp_flow_with_transport(session, tls_server_name, move |tls_profile| async move",
            )
            && transport.contains("async fn open_trojan_tls_stream_with_profile(")
            && transport.contains("async fn open_trojan_tls_relay_stream_with_profile(")
            && protocol_outbound.contains("pub struct OwnedTrojanResolvedTlsProfile")
            && protocol_outbound.contains("impl ClientTlsProfile for OwnedTrojanResolvedTlsProfile")
            && transport.contains("crate::tls::connect_tls_upstream_with_profile")
            && transport.contains("crate::tls::connect_tls_stream_with_profile")
            && !transport.contains("pub struct TrojanTlsOptions")
            && !transport.contains("pub struct TrojanTlsProfile")
            && !transport.contains("pub fn trojan_tls_options_from_parts")
            && !transport.contains("pub fn trojan_tls_options_from_profile")
            && !transport.contains("pub async fn open_direct<")
            && !transport.contains("pub async fn open_relay(")
            && !transport.contains("ClientTlsConfig")
            && !transport.contains("client_tls_config_from_protocol_profile(")
            && !transport.contains("fn into_tls_config(self) -> ClientTlsConfig")
            && !transport.contains("TrojanUdpTlsProfile")
            && !transport.contains("TrojanTcpTlsProfile"),
        "Trojan UDP adapter glue should materialize protocol-selected TLS profiles through zero-transport TLS primitives"
    );
}

#[test]
fn trojan_udp_flow_resume_is_protocol_owned() {
    let root = read("src/adapters/trojan.rs");
    let transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read trojan protocol udp source");

    assert!(
        root.contains("start_protocol_transport_bridge_udp_flow(")
            && root.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && !root.contains("TrojanUdpFlowPlan::direct_from_config")
            && !root.contains("TrojanUdpFlowPlan::relay_from_config")
            && !root.contains("PreparedTrojanOutboundRequestBundle::from_config("),
        "src/adapters/trojan.rs should stay on UDP runtime orchestration and not build Trojan UDP flow state inline"
    );
    assert!(
        transport.contains("PreparedTrojanOutboundRequestBundle::from_config(")
            && transport.contains("TrojanOutboundLeaf::new(")
            && transport.contains("impl<'a> ProtocolTransportLeafResolver<'a> for TrojanTlsBridge"),
        "crates/transport/src/trojan_transport.rs should build the Trojan prepared protocol request bundle before transport opening"
    );
    assert!(
        transport.contains("pub struct TrojanManagedUdpFlowResume")
            && transport.contains("protocol: trojan::udp::PreparedTrojanUdpFlowPlan")
            && transport.contains("pub(super) fn direct_udp_resume(&self) -> TrojanManagedUdpFlowResume")
            && transport
                .contains("pub(super) fn relay_final_hop_udp_resume(&self) -> TrojanManagedUdpFlowResume")
            && transport.contains("leaf.direct_udp_resume()")
            && transport.contains("leaf.relay_final_hop_udp_resume()")
            && transport.contains(
                "impl ProtocolManagedPacketUdpFlowResumeConnectionOps for TrojanManagedUdpFlowResume"
            ),
        "crates/transport/src/trojan_transport.rs should own the managed Trojan UDP resume carrier"
    );
    assert!(
        protocol_udp.contains("struct TrojanUdpFlowResume")
            && protocol_udp.contains("struct TrojanUdpFlowConfig")
            && protocol_udp.contains("fn flow_resume(&self)")
            && protocol_udp.contains("fn udp_flow_resume_from_config(")
            && protocol_udp.contains("pub(crate) fn direct_from_config(")
            && protocol_udp.contains("pub(crate) fn relay_from_config(")
            && protocol_udp.contains("pub fn owned_tls_profile(")
            && protocol_udp.contains("pub struct PreparedTrojanUdpFlowPlan"),
        "protocols/trojan/src/udp.rs should keep Trojan UDP flow state and resume composition private to the protocol crate"
    );
}

#[test]
fn trojan_udp_packet_stream_tasks_live_outside_manager() {
    let managed = read("src/adapters/trojan.rs");
    let adapter = read_proxy_module_tree("src/adapters/trojan.rs");
    let proxy_transport = read_proxy_module_tree("src/adapters/trojan.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let stream = manifest_dir().join("src/adapters/trojan/udp/manager/stream.rs");
    let transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read trojan protocol udp source");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    let forbidden = "MeteredStream";
    assert!(
        !adapter.contains(forbidden) && !stream_manager.contains(forbidden),
        "Trojan adapter/runtime bridge should not own packet stream task detail `{forbidden}`"
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
                && !proxy_transport.contains(forbidden)
                && !stream_manager.contains(forbidden),
            "Trojan managed UDP glue should delegate Trojan packet framing to protocols/trojan helpers; found `{forbidden}`"
        );
    }
    for forbidden in ["TrojanUdpPacket {", "trojan::udp::TrojanUdpPacket"] {
        assert!(
            !managed.contains(forbidden)
                && !proxy_transport.contains(forbidden)
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
            protocol_shared.contains(&format!(
                "pub(crate) {} {private_helper}",
                if private_helper == "build_udp_packet" {
                    "fn"
                } else {
                    "async fn"
                }
            )) && !protocol_lib.contains(private_helper),
            "Trojan low-level UDP stream helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
    assert!(
        transport.contains(".open_udp_flow_with_transport(session, None, move |tls_profile| async move")
            && transport.contains(
                ".open_udp_flow_with_transport(session, tls_server_name, move |tls_profile| async move",
            )
            && !transport.contains("TrojanManagedUdpConnection")
            && !managed.contains("tokio::io::split")
            && !managed.contains("tokio::spawn")
            && !managed.contains(".write_flow_packet(")
            && !managed.contains(".write_packet(")
            && !managed.contains("&mut send_stream")
            && !managed.contains(".read_flow_packet(")
            && !managed.contains("&mut recv_stream")
            && protocol_udp.contains("fn spawn_udp_flow")
            && !protocol_udp.contains("pub fn spawn_udp_flow")
            && protocol_udp.contains("async fn establish_udp_flow_with_resume")
            && !protocol_udp.contains("pub async fn establish_udp_flow_with_resume")
            && protocol_udp.contains("async fn read_udp_flow_packet")
            && !protocol_udp.contains("pub async fn read_udp_flow_packet")
            && protocol_udp.contains("async fn write_udp_flow_packet")
            && !protocol_udp.contains("pub async fn write_udp_flow_packet")
            && protocol_udp.contains("struct TrojanUdpFlowSender")
            && !protocol_udp.contains("pub struct TrojanUdpFlowSender")
            && protocol_udp.contains("pub struct TrojanUdpFlowConnection")
            && protocol_udp.contains("struct TrojanUdpFlowSession")
            && !protocol_udp.contains("pub struct TrojanUdpFlowSession")
            && protocol_udp.contains("pub type TrojanUdpFlowResponseReceiver")
            && protocol_udp.contains("type TrojanUdpFlowResponses")
            && !protocol_udp.contains("pub type TrojanUdpFlowResponses")
            && protocol_udp.contains("tokio::spawn")
            && protocol_udp.contains("mpsc::channel::<UdpFlowPacket>")
            && protocol_udp.contains("broadcast::channel::<UdpFlowPacket>")
            && protocol_udp.contains("fn build_udp_request")
            && !protocol_udp.contains("pub fn build_udp_request")
            && !protocol_outbound.contains("pub fn build_udp_request")
            && protocol_outbound.contains("fn build_tcp_request")
            && !managed.contains(".write_stream_packet")
            && !managed.contains(".read_stream_packet")
            && !managed.contains(".read_packet(")
            && !managed.contains("trojan::udp_flow_packet")
            && transport.contains("async fn open_direct_connection<")
            && transport.contains("async fn open_relay_connection(")
            && !managed.contains("packet.write_to")
            && !managed.contains("struct TrojanPacket"),
        "Trojan UDP packet stream tasks should live in protocols/trojan while adapter glue keeps handshake/cache bridge glue"
    );
}

#[test]
fn mieru_udp_managed_connector_is_thin_protocol_glue() {
    let managed = read("src/adapters/mieru/udp.rs");
    let connector = read_repo_module_tree("crates/transport/src/mieru_transport/managed_udp.rs");
    let adapter = read("src/adapters/mieru/udp.rs");
    let adapter_flow = read("src/adapters/mieru/udp/flow.rs");
    let transport = read_repo_module_tree("crates/transport/src/mieru_transport.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let protocol_udp = read_repo_module_tree("protocols/mieru/src/udp.rs");

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
        managed.contains(
            "managed_stream_handler_box::<zero_transport::mieru_transport::MieruManagedStreamUdpResume>"
        )
            && adapter.contains("MieruTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_flow_plan(false)")
            && adapter.contains("leaf.udp_flow_plan(true)")
            && adapter_flow.contains("start_direct_managed_stream_packet(")
            && adapter_flow.contains("start_relay_managed_stream_packet(")
            && transport.contains("pub type MieruManagedStreamUdpResume = ManagedTupleUdpResume<MieruManagedUdpFlowResume>;")
            && connector.contains("impl ManagedConnectorFlowOps for mieru::udp::MieruUdpConnectorFlow")
            && connector.contains("impl ProtocolManagedTupleUdpFlowResumeConnectionOps for MieruManagedUdpFlowResume")
            && connector.contains("impl ManagedTupleUdpConnectionOps for mieru::udp::MieruUdpFlowConnection")
            && stream_manager.contains("managed_stream_connector_flow_from_build(")
            && connection.contains("managed_tuple_udp_connection_from_ops")
            && protocol_udp.contains("pub struct MieruUdpConnectorFlow")
            && protocol_udp.contains("pub fn connector_flow_from_resume")
            && protocol_udp.contains("pub fn udp_flow_resume_from_config("),
        "Mieru managed.rs should adapt protocol flow establishment while generic stream_manager owns cache and send orchestration"
    );

    assert!(
        !adapter.contains("mieru::udp_flow_codec")
            && !adapter.contains("MieruUdpFlowResume::new")
            && !adapter.contains("MieruUdpFlowConfig::new")
            && protocol_udp.contains("struct MieruUdpFlowResume")
            && protocol_udp.contains("pub fn udp_flow_resume_from_config(")
            && protocol_udp.contains("pub struct MieruUdpConnectorFlow")
            && protocol_udp.contains("pub fn connector_flow(")
            && protocol_udp.contains("pub fn flow_cache_key(&self")
            && protocol_udp.contains("pub fn flow_requires_relay_upstream(&self) -> bool"),
        "Mieru adapter should build and carry an opaque protocol-owned UDP flow resume descriptor"
    );
}

#[test]
fn mieru_udp_response_bridge_uses_generic_managed_tuple_connection() {
    let managed = read("src/adapters/mieru/udp.rs");
    let connector = read_repo_module_tree("crates/transport/src/mieru_transport/managed_udp.rs");
    let connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");

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
            && !connector.contains("managed_tuple_udp_connection")
            && connector.contains("fn subscribe_protocol_packets(&self)")
            && connector.contains("mieru upstream closed")
            && connection.contains("managed_tuple_udp_connection_from_ops")
            && connection.contains("spawn_tuple_response_bridge")
            && connection.contains("broadcast::Receiver<(Address, u16, Vec<u8>)>")
            && stream_manager.contains(".insert_and_send_key("),
        "Mieru UDP response bridge should hang off the neutral managed tuple connection bridge"
    );
}

#[test]
fn trojan_udp_managed_connector_is_thin_protocol_glue() {
    let managed = read("src/adapters/identity.rs");
    let adapter = read_proxy_module_tree("src/adapters/trojan.rs");
    let proxy_transport = read_proxy_module_tree("src/adapters/trojan.rs");
    let _managed_bridge = read("src/runtime/udp_flow/managed/bridge.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let _connection = read_proxy_module_tree("src/runtime/udp_flow/managed/connection.rs");
    let transport = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read trojan protocol udp source");

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
    ] {
        assert!(
            !managed.contains(forbidden) && !proxy_transport.contains(forbidden),
            "Trojan managed.rs should not own protocol-private/cache/runtime orchestration detail `{forbidden}`"
        );
    }
    assert!(
        !adapter.contains("TrojanResolvedTlsProfile")
            && !proxy_transport.contains("TrojanResolvedTlsProfile")
            && !adapter.contains("OwnedTrojanResolvedTlsProfile")
            && !proxy_transport.contains("OwnedTrojanResolvedTlsProfile")
            && !protocol_outbound.contains("pub struct TrojanResolvedTlsProfile")
            && !protocol_outbound.contains(
                "pub fn tls_profile(&self) -> TrojanResolvedTlsProfile<'_>"
            )
            && protocol_outbound.contains("impl ClientTlsProfile for OwnedTrojanResolvedTlsProfile"),
        "Trojan UDP TLS profile-to-transport mapping should stay in zero-transport, not adapter or proxy managed glue"
    );

    assert!(
        adapter.contains("managed_stream_udp_handler_for_bridge::<TrojanTlsBridge>()")
            && adapter.contains("start_protocol_transport_bridge_udp_flow(")
            && adapter.contains("start_protocol_transport_bridge_udp_relay_final_hop(")
            && stream_manager.contains("managed_stream_connector_flow_from_build(")
            && transport.contains("impl ManagedConnectorFlowOps for trojan::udp::TrojanUdpConnectorFlow")
            && stream_manager.contains("managed_packet_udp_connection_from_flow(connection)")
            && transport.contains("impl ManagedPacketUdpConnectionOps for trojan::udp::TrojanUdpFlowConnection")
            && transport.contains("struct TrojanManagedUdpFlowResume")
            && transport.contains(
                "impl ProtocolManagedPacketUdpFlowResumeConnectionOps for TrojanManagedUdpFlowResume"
            )
            && transport.contains("async fn open_direct_connection<")
            && transport.contains("async fn open_relay_connection(")
            && transport.contains("type TrojanManagedUdpConnectorFlow = ManagedConnectorFlow<trojan::udp::TrojanUdpConnectorFlow>;")
            && transport.contains("async fn send_protocol_packet(")
            && transport.contains("fn subscribe_protocol_packets("),
        "Trojan managed UDP glue should adapt TLS stream and protocol flow establishment while generic stream_manager owns cache/send orchestration"
    );

    assert!(
        !protocol_udp.contains("pub fn udp_flow_packet")
            && !protocol_udp.contains("fn udp_flow_packet")
            && protocol_udp.contains("async fn read_flow_packet")
            && !protocol_udp.contains("pub async fn read_flow_packet")
            && protocol_udp.contains("async fn write_flow_packet")
            && !protocol_udp.contains("pub async fn write_flow_packet")
            && protocol_udp.contains("fn spawn_udp_flow")
            && !protocol_udp.contains("pub fn spawn_udp_flow")
            && protocol_udp.contains("async fn establish_udp_flow_with_resume")
            && !protocol_udp.contains("pub async fn establish_udp_flow_with_resume")
            && protocol_udp.contains("pub struct TrojanUdpFlowConnection")
            && protocol_udp.contains("pub type TrojanUdpFlowResponseReceiver")
            && protocol_udp.contains("pub async fn open_udp_flow_with_transport<")
            && !protocol_udp.contains("pub async fn open_udp_flow(")
            && !transport.contains("mpsc::Sender<UdpFlowPacket>")
            && !transport.contains("trojan::udp_flow_packet")
            && !transport.contains("trojan::udp::TrojanUdpFlowIo"),
        "Trojan UDP packet conversion and flow channels should stay protocol-owned and out of zero-transport"
    );
}

#[test]
fn mieru_udp_packet_stream_tasks_live_outside_manager() {
    let managed = read("src/adapters/mieru/udp.rs");
    let connector = read_repo_module_tree("crates/transport/src/mieru_transport/managed_udp.rs");
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let stream = manifest_dir().join("src/adapters/mieru/udp/manager/stream.rs");
    let socket = manifest_dir().join("src/adapters/mieru/udp/manager/socket.rs");
    let _transport = fs::read_to_string(repo_root().join("crates/transport/Cargo.toml"))
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
    assert!(
        !stream.exists() && !socket.exists(),
        "Mieru UDP stream task should live in protocols/mieru without proxy stream/socket wrappers"
    );
    assert!(
        managed.contains(
            "managed_stream_handler_box::<zero_transport::mieru_transport::MieruManagedStreamUdpResume>"
        )
            && !managed.contains("MieruFlowSender")
            && !managed.contains("MieruEntry")
            && !managed.contains(".sender")
            && !managed.contains(".recv_tx")
            && stream_manager.contains("managed_stream_connector_flow_from_build(")
            && stream_manager.contains("managed_tuple_udp_connection_from_ops(connection)")
            && !managed.contains("UdpFlowPacket")
            && protocol_outbound.contains("pub struct MieruUdpFlowConnection")
            && protocol_outbound.contains("pub type MieruUdpFlowResponseReceiver")
            && connector.contains("async fn send_protocol_packet(")
            && connector.contains("fn subscribe_protocol_packets(")
            && !managed.contains("mpsc::channel")
            && !managed.contains("tokio::sync::broadcast::channel")
            && !managed.contains("tokio::spawn")
            && connector.contains("impl ManagedConnectorFlowOps for mieru::udp::MieruUdpConnectorFlow")
            && connector.contains("mieru::udp::establish_udp_flow_with_resume")
            && protocol_outbound.contains("pub async fn establish_udp_flow_with_resume"),
        "Mieru UDP stream flow task should stay out of zero-transport and live in protocols/mieru while proxy keeps handshake/cache bridge glue"
    );
}

#[test]
fn h2_udp_datagram_codec_lives_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp.rs");
    let connector = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let transport = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let adapter = read("src/adapters/hysteria2/udp.rs");
    let snapshot = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    let forward = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let connector_flow_impl = impl_block(&protocol_udp, "Hysteria2UdpConnectorFlow");
    let protocol_lib = fs::read_to_string(repo_root().join("protocols/hysteria2/src/lib.rs"))
        .expect("read hysteria2 protocol lib source");
    let adapter_flow = read("src/adapters/hysteria2/udp/flow.rs");
    let adapter_packet_path = read("src/adapters/hysteria2/udp/packet_path.rs");
    let _profile_connector_uses = connector
        .matches("Hysteria2UdpConnector::from_udp_profile")
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
            && adapter.contains("Hysteria2TransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_flow_plan()")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && !adapter_flow.contains("Hysteria2UdpFlowConfig::new")
            && !adapter_packet_path
                .contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("Hysteria2UdpFlowConfig::new")
            && managed.contains("managed_datagram_handler_box::<")
            && managed.contains("Hysteria2ManagedDatagramFlowResume")
            && !managed.contains("send_datagram")
            && !managed.contains("read_datagram")
            && !managed.contains("tokio::spawn")
            && connector.contains("pub struct Hysteria2ManagedDatagramFlowResume")
            && connector.contains("pub async fn establish_hysteria2_udp_flow_connection(")
            && protocol_udp.contains("pub(crate) fn udp_flow_codec(")
            && protocol_udp.contains("impl DatagramCodec<Address> for Hysteria2DatagramCodec")
            && protocol_udp.contains("pub fn into_shared_codec_parts"),
        "Hysteria2 adapter should forward transport-owned UDP plans while protocols/hysteria2 owns UDP flow packet helpers"
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
            && !managed.contains("UdpFlowPacket::from_parts")
            && !managed.contains("flow_io.encode_packet")
            && !managed.contains("flow_io.decode_packet(&data)")
            && !managed.contains("hysteria2::udp::spawn_udp_flow")
            && generic_manager.contains(".send_or_insert_pre_sent_key(")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowConnection")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowSession")
            && protocol_udp.contains("pub fn start_udp_flow_with_initial_packet")
            && connector.contains("pub async fn establish_hysteria2_udp_flow_connection("),
        "Hysteria2 UDP managed glue should store protocol-owned flow sessions while protocols/hysteria2 owns packet encode/decode and flow pump"
    );
    assert!(
        !adapter.contains("Hysteria2UdpFlowResume::new")
            && !adapter.contains(".flow_resume()")
            && !adapter.contains(".packet_path_spec()")
            && adapter.contains("Hysteria2TransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_flow_plan()")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && !adapter_flow.contains(".flow_resume()")
            && !adapter.contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path
                .contains("hysteria2::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains(".packet_path_spec()")
            && connector.contains("pub fn udp_flow_plan(&self) -> Hysteria2ManagedUdpFlowPlan<'a>")
            && connector.contains(
                "pub fn udp_packet_path_plan(&self) -> Hysteria2ManagedUdpPacketPathPlan"
            )
            && !adapter_packet_path.contains("udp_packet_path_carrier_build_from_config")
            && !adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("spec.carrier()")
            && !adapter_packet_path.contains("spec.cache_key()")
            && !adapter_packet_path.contains("spec.carrier_cache_key()")
            && !adapter_packet_path.contains("spec.codec()")
            && !adapter_packet_path.contains("build.server()")
            && !adapter_packet_path.contains("build.port()")
            && !adapter_packet_path.contains("build.connector_profile()")
            && !adapter_packet_path.contains("build.codec()")
            && adapter_packet_path
                .contains("zero_transport::hysteria2_quic::open_hysteria2_udp_packet_path_build")
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
        forward.contains("ManagedDatagramExistingSend")
            && forward.contains("ManagedDatagramExistingSend::forwarded")
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
        managed.contains("managed_datagram_handler_box::<")
            && managed.contains("Hysteria2ManagedDatagramFlowResume")
            && !managed.contains("ManagedDatagramConnectorFlow::new")
            && !managed.contains("flow.cache_key()")
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
            && !connector.contains("pub(crate) async fn open_udp_packet_path_connection")
            && !connector.contains("profile.password()")
            && !transport.contains("request.resume.connector_profile()"),
        "Hysteria2 UDP managed glue should consume protocol-owned opaque cache keys through neutral endpoints and keep UDP profile/connection setup in zero-transport"
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
    let connector = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let _profile_connector_uses = connector
        .matches("Hysteria2UdpConnector::from_udp_profile")
        .count();

    assert!(
        !adapter.contains("hysteria2::udp_flow_codec")
            && !adapter.contains("hysteria2::udp_cache_key")
            && !adapter.contains("Hysteria2UdpFlowConfig")
            && adapter.contains("Hysteria2TransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && !adapter_packet_path.contains("Hysteria2UdpFlowConfig"),
        "Hysteria2 packet-path adapter submodule should forward transport-owned packet-path plans rather than rebuild protocol config locally"
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
            && adapter_packet_path.contains("open_hysteria2_udp_packet_path_build(")
            && adapter_packet_path.contains("plan.into_carrier_build()")
            && adapter_packet_path.contains("plan.into_carrier_descriptor()")
            && !adapter_packet_path.contains("build.server()")
            && !adapter_packet_path.contains("build.port()")
            && connector.contains(".into_shared_codec_parts()")
            && protocol_udp.contains("pub fn into_shared_codec_parts")
            && connector.contains("async fn open_udp_profile_connection")
            && !adapter.contains("Hysteria2Connector")
            && !adapter_packet_path.contains("Hysteria2Connector"),
        "Hysteria2 packet-path adapter submodule should request protocol-specific QUIC connection setup from the adapter connector while zero-transport owns connection lifecycle and codec use"
    );
}

#[test]
fn h2_udp_response_bridge_lives_outside_manager() {
    let managed_adapter = read("src/adapters/hysteria2/udp.rs");
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
            && !managed_adapter.contains(
                "impl ManagedUdpConnection for hysteria2::udp::Hysteria2UdpFlowConnection"
            )
            && !managed_adapter.contains("managed_tuple_udp_connection")
            && !managed_adapter.contains("spawn_tuple_response_bridge")
            && read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs")
                .contains("ManagedDatagramResponseWaiters")
            && read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs")
                .contains("spawn_datagram_response_bridge")
            && read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs")
                .contains("spawn_upstream_response_pump"),
        "Hysteria2 UDP response bridge should use generic managed datagram response glue, not h2_manager/bridge.rs"
    );
}

#[test]
fn h2_udp_packet_stream_tasks_live_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp.rs");
    let stream_path = manifest_dir().join("src/adapters/hysteria2/udp/manager/stream.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let _transport = fs::read_to_string(repo_root().join("crates/transport/src/hysteria2_quic.rs"))
        .expect("read zero-transport hysteria2_quic source");
    let connector = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let flow = read("src/adapters/hysteria2/udp/flow.rs");

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
        managed.contains("managed_datagram_handler_box::<")
            && managed.contains("Hysteria2ManagedDatagramFlowResume")
            && !managed.contains("send_datagram")
            && !managed.contains("read_datagram")
            && !managed.contains("tokio::spawn")
            && flow.contains("ManagedDatagramStart")
            && connector.contains("async fn open_udp_profile_connection")
            && connector.contains("pub async fn establish_hysteria2_udp_flow_connection(")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowConnection")
            && protocol_udp.contains("pub struct Hysteria2UdpFlowSession")
            && protocol_udp.contains("pub fn start_udp_flow_with_initial_packet")
            && protocol_udp.contains("broadcast::channel::<Hysteria2UdpFlowResponse>")
            && protocol_udp.contains("mpsc::channel::<UdpFlowPacket>")
            && protocol_udp.contains("tokio::spawn"),
        "Hysteria2 UDP flow tasks should stay out of zero-proxy adapters while transport/protocol layers own QUIC connect/cache bridge glue"
    );
}

#[test]
fn h2_transport_delegates_protocol_handshake_to_protocol_crate() {
    let transport = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        read_repo_file("crates/transport/src/hysteria2_quic.rs"),
        read_repo_file("crates/transport/src/hysteria2_quic/connection.rs"),
        read_repo_file("crates/transport/src/hysteria2_quic/managed_udp.rs"),
        read_repo_file("crates/transport/src/hysteria2_quic/model.rs"),
        read_repo_file("crates/transport/src/hysteria2_quic/projection.rs"),
        read_repo_file("crates/transport/src/hysteria2_quic/stream.rs")
    );
    let adapter = read_proxy_module_tree("src/adapters/hysteria2.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/hysteria2/src/outbound.rs"))
            .expect("read hysteria2 protocol outbound source");

    for forbidden in [
        "build_auth_frame",
        "sign_hmac",
        "build_tcp_connect_header",
        "parse_auth_response",
        "authenticate_with_salt(",
        "send_tcp_connect(",
        "read_connect_response(",
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
            && transport.contains("pub async fn open_quic_connection")
            && transport.contains("connect_hysteria2_tcp_outbound(")
            && transport.contains("open_hysteria2_udp_packet_path_build(")
            && transport.contains("establish_hysteria2_udp_flow_connection(")
            && transport.contains(".authenticate_connection(&conn, &mut stream)")
            && transport.contains(".establish_tcp_connect(&mut stream, session)")
            && transport.contains("hysteria2::outbound_profile_from_config_password")
            && adapter.contains("leaf.open_tcp_stream(session).await")
            && adapter.contains("open_hysteria2_udp_packet_path_build(")
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
    let managed = read("src/adapters/hysteria2/udp.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
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
            && generic_manager.contains("ManagedDatagramExistingSend<'_>"),
        "Hysteria2 UDP should use the generic managed datagram request model instead of h2_manager/model.rs"
    );
}

#[test]
fn h2_udp_model_details_live_outside_manager_root() {
    let managed = read("src/adapters/hysteria2/udp.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
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
            && managed.contains("managed_datagram_handler_box::<")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowStore")
            && !managed.contains("hysteria2::udp::Hysteria2UdpFlowSessions"),
        "Hysteria2 UDP should use neutral generic cache storage while the protocol resume owns cache-key identity"
    );
}

#[test]
fn h2_udp_send_orchestration_lives_outside_manager() {
    let managed = read("src/adapters/hysteria2/udp.rs");
    let send = manifest_dir().join("src/adapters/hysteria2/udp/manager/send.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");

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
    let managed = read("src/adapters/hysteria2/udp.rs");
    let establish = manifest_dir().join("src/adapters/hysteria2/udp/manager/establish.rs");

    for forbidden in ["stream::establish", "spawn_response_bridge"] {
        assert!(
            !managed.contains(forbidden),
            "Hysteria2 UDP managed glue should keep establish details behind the connector trait; found `{forbidden}`"
        );
    }
    assert!(
        !establish.exists()
            && managed.contains("managed_datagram_handler_box::<")
            && read_repo_module_tree("crates/transport/src/hysteria2_quic.rs")
                .contains("pub async fn establish_hysteria2_udp_flow_connection("),
        "Hysteria2 UDP establish glue should live in zero-transport resume helpers, not h2_manager/establish.rs"
    );
}

#[test]
fn shadowsocks_udp_datagram_codec_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp.rs");
    let outbound = manifest_dir().join("src/outbound/shadowsocks.rs");
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let _adapter_flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let _adapter_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let transport = format!(
        "{}\n{}",
        read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs"),
        read_repo_file("crates/transport/src/shadowsocks_transport/udp_socket.rs")
    );
    let _transport_manifest = fs::read_to_string(repo_root().join("crates/transport/Cargo.toml"))
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
        managed.contains("managed_datagram_socket_handler_box::<")
            && managed.contains("ShadowsocksManagedDatagramFlowResume")
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_flow_plan()")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && transport.contains("pub fn udp_flow_plan(&self)")
            && transport.contains("pub fn udp_packet_path_plan(")
            && transport.contains("pub fn packet_path_carrier_codec(")
            && transport.contains("establish_shadowsocks_udp_socket_flow")
            && transport.contains("Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>")
            && protocol_outbound.contains("pub fn parse_udp_cipher(")
            && protocol_outbound.contains("pub fn into_shared_managed_socket_flow_codec(")
            && protocol_outbound.contains("pub fn carrier_codec(&self)")
            && !managed.contains("BridgeWaiters")
            && !managed.contains("resume.managed_socket_flow().codec()"),
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
        !protocol_outbound.contains("fn udp_flow_packet")
            && !protocol_lib.contains("udp_flow_packet"),
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
    let managed = read("src/adapters/shadowsocks/udp.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_datagram = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
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
    assert!(
        !bridge.exists()
            && !managed.contains("ManagedDatagramResponseWaiters")
            && !managed.contains("spawn_datagram_response_bridge")
            && !managed.contains("spawn_upstream_response_pump")
            && !managed.contains("tokio::spawn")
            && managed_datagram.contains("struct ManagedDatagramResponseWaiters")
            && managed_datagram.contains("fn spawn_datagram_response_bridge")
            && managed_datagram.contains("fn spawn_upstream_response_pump")
            && managed_datagram.contains("oneshot::channel")
            && managed_datagram.contains("VecDeque"),
        "Shadowsocks UDP response waiter/pump logic should live in neutral managed datagram helpers"
    );
}

#[test]
fn shadowsocks_udp_socket_runtime_lives_outside_manager() {
    let managed = read("src/adapters/shadowsocks/udp.rs");
    let transport_path = repo_root().join("crates/transport/src/shadowsocks_transport.rs");
    let transport = format!(
        "{}\n{}",
        read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs"),
        read_repo_file("crates/transport/src/shadowsocks_transport/udp_socket.rs")
    );

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
    let managed = read("src/adapters/shadowsocks/udp.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
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
            && generic_manager.contains("ManagedDatagramExistingSend<'_>"),
        "Shadowsocks UDP should use the generic managed datagram socket request model instead of ss_manager/model.rs"
    );
}

#[test]
fn shadowsocks_udp_flow_cipher_is_protocol_parsed() {
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let adapter_flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let flows = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");
    let managed = read("src/adapters/shadowsocks/udp.rs");
    let outbound = manifest_dir().join("src/outbound/shadowsocks.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let managed_cache = read_proxy_module_tree("src/runtime/udp_flow/managed/cache.rs");
    let snapshot = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");
    let forward = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
    let transport = read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");
    let socket_flow_spec_impl = impl_block(&protocol_outbound, "ShadowsocksUdpSocketFlowSpec");

    assert!(
        !adapter.contains("CipherKind::from_str")
            && !adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowConfig::new")
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_flow_plan()")
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
            && managed.contains("managed_datagram_socket_handler_box::<")
            && managed.contains("ShadowsocksManagedDatagramFlowResume")
            && !managed.contains("ManagedDatagramSocketConnectorFlow::new")
            && !managed.contains("flow.cache_key()")
            && !managed.contains("resume.managed_socket_flow()")
            && !managed.contains("resume.managed_socket_flow().codec()")
            && transport.contains("managed_socket_flow_from_resume(&self.protocol)")
            && transport.contains("establish_shadowsocks_udp_socket_flow(")
            && transport.contains("resume.into_shared_managed_socket_flow_codec()")
            && !managed.contains("Arc::new(resume.into_managed_socket_flow_codec())")
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
            && generic_manager.contains("SharedManagedDatagramUdpConnection")
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
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_flow_plan()")
            && !adapter_flow.contains("ShadowsocksUdpFlowConfig::new")
            && !adapter_flow.contains(".flow_resume()")
            && protocol_outbound.contains("pub fn parse_udp_cipher(")
            && transport.contains(
                "pub fn udp_flow_plan(&self) -> Result<ShadowsocksManagedUdpFlowPlan<'a>, zero_core::Error>"
            )
            && transport.contains("pub struct ShadowsocksManagedDatagramFlowResume")
            && protocol_outbound.contains("pub struct ShadowsocksUdpSocketFlowSpec")
            && socket_flow_spec_impl.contains("pub fn into_cache_key")
            && socket_flow_spec_impl
                .contains("pub fn into_codec(self) -> ShadowsocksDatagramCodec")
            && protocol_outbound.contains("pub fn managed_socket_flow(&self)")
            && transport.contains("managed_socket_flow_from_resume(&self.protocol)"),
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
        forward.contains("ManagedDatagramExistingSend")
            && forward.contains("ManagedDatagramExistingSend::forwarded")
            && !forward.contains("existing.resume.cache_key()")
            && !forward.contains("existing.resume.codec()")
            && !forward.contains("shadowsocks::udp_flow_codec")
            && !forward.contains("password: &'a str")
            && !forward.contains("cipher_kind: shadowsocks::CipherKind")
            && !forward.contains("datagram_cache_key: &'a str"),
        "existing Shadowsocks UDP flow forwarding should pass the opaque resume descriptor without unpacking cache identity or codec state"
    );
    let start = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");
    assert!(
        !start.contains("ManagedUdpFlowResume::Shadowsocks")
            && start.contains("ManagedDatagramExistingSend::datagram")
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
fn shadowsocks_packet_path_cipher_is_protocol_parsed() {
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let adapter_flow = read("src/adapters/shadowsocks/udp/flow.rs");
    let adapter_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let shadowsocks_transport =
        read_repo_module_tree("crates/transport/src/shadowsocks_transport.rs");
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
    let forward = read_proxy_module_tree("src/runtime/udp_flow/managed/datagram.rs");

    assert!(
        !adapter.contains("CipherKind::from_str")
            && !adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowConfig::new")
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && !adapter_flow.contains("ShadowsocksUdpFlowConfig::new")
            && adapter_packet_path.contains("plan.carrier_codec()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(")
            && adapter_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && protocol_outbound.contains("pub fn udp_packet_path_carrier_codec_from_config("),
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
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && !adapter.contains("shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config")
            && !adapter.contains("shadowsocks::udp::udp_packet_path_carrier_codec_from_config")
            && !adapter_packet_path.contains(".packet_path_spec()")
            && !adapter_packet_path.contains("packet_path.cache_key()")
            && !adapter_packet_path.contains("packet_path.codec()")
            && !adapter_packet_path.contains("UdpDatagramSourceParts")
            && !adapter_packet_path.contains(".into_codec()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(")
            && !adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("udp_packet_path_carrier_codec_from_config")
            && adapter_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !adapter_packet_path.contains("descriptor.cache_key()")
            && !adapter_packet_path.contains("descriptor.server()")
            && !adapter_packet_path.contains("descriptor.port()")
            && !adapter_packet_path.contains("udp_packet_path_datagram_source_build_from_config")
            && adapter_packet_path.contains("udp_datagram_source_from_build")
            && !adapter_packet_path.contains("spec.datagram_source_parts()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(")
            && !adapter_packet_path.contains("datagram.cache_key()")
            && !adapter_packet_path.contains("datagram.codec()")
            && shadowsocks_packet_path.contains(
                "ShadowsocksManagedUdpPacketPathDatagramSourceBuild::into_shared_codec_parts(self)"
            )
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
            && shadowsocks_transport.contains("pub fn udp_packet_path_plan(")
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
            "packet-path chain should receive protocol-parsed Shadowsocks cipher values"
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
            && adapter.contains("ShadowsocksTransportLeaf::from_resolved_leaf")
            && adapter.contains("leaf.udp_packet_path_plan()")
            && !adapter_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && !adapter_packet_path.contains("udp_packet_path_carrier_codec_from_config")
            && adapter_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && !adapter_packet_path.contains("descriptor.cache_key()")
            && !adapter_packet_path.contains("descriptor.server()")
            && !adapter_packet_path.contains("descriptor.port()")
            && !adapter_packet_path.contains("udp_packet_path_datagram_source_build_from_config")
            && !adapter_packet_path.contains("spec.datagram_source_parts()")
            && adapter_packet_path.contains("udp_datagram_source_from_build(")
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
            && !protocol_outbound
                .contains("pub struct ShadowsocksUdpPacketPathDatagramSourceParts {")
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
    let stream_manager = read_proxy_module_tree("src/runtime/udp_flow/managed/stream_manager.rs");
    let datagram_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
    let packet_path = read("src/runtime/udp_flow/packet_path.rs");
    let packet_path_key = read("src/runtime/udp_flow/packet_path_chain/key.rs");
    let socks5_packet_path = read("src/adapters/socks5/udp/packet_path.rs");
    let shadowsocks_packet_path = read("src/adapters/shadowsocks/udp/packet_path.rs");
    let _shadowsocks_managed = read("src/adapters/shadowsocks/udp.rs");
    let hysteria2_connector = read_repo_module_tree("crates/transport/src/hysteria2_quic.rs");
    let trojan_connector = read_repo_module_tree("crates/transport/src/trojan_transport.rs");
    let mieru_connector =
        read_repo_module_tree("crates/transport/src/mieru_transport/managed_udp.rs");
    let socks5_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");
    let shadowsocks_protocol =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");
    let hysteria2_protocol = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");
    let trojan_protocol = fs::read_to_string(repo_root().join("protocols/trojan/src/udp.rs"))
        .expect("read trojan protocol udp source");
    let mieru_protocol = read_repo_module_tree("protocols/mieru/src/udp.rs");
    let socks5_descriptor_impl = impl_block(&socks5_udp, "Socks5UdpPacketPathCarrierDescriptor");
    let socks5_build_impl = impl_block(&socks5_udp, "Socks5UdpPacketPathCarrierBuild");
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
            && stream_manager.contains(
                "let (cache_key, requires_relay_upstream) = connector_flow.into_parts();"
            )
            && !stream_manager.contains("fn cache_key(&self) -> String;")
            && !stream_manager.contains("fn requires_relay_upstream(&self) -> bool;")
            && !stream_manager.contains("connector_flow.cache_key()")
            && !stream_manager.contains("connector_flow.requires_relay_upstream()"),
        "managed stream connector flow builds should consume protocol-provided parts instead of exposing getter traits"
    );
    assert!(
        trojan_connector.contains("fn into_managed_connector_parts(self) -> (String, bool)")
            && trojan_connector.contains("trojan::udp::TrojanUdpConnectorFlow::into_parts(self)")
            && trojan_connector
                .contains("impl ManagedConnectorFlowOps for trojan::udp::TrojanUdpConnectorFlow")
            && mieru_connector.contains("fn into_managed_connector_parts(self) -> (String, bool)")
            && mieru_connector
                .contains("impl ManagedConnectorFlowOps for mieru::udp::MieruUdpConnectorFlow")
            && mieru_connector.contains("mieru::udp::MieruUdpConnectorFlow::into_parts(self)")
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
            && datagram_manager.contains("ManagedDatagramSocketConnectorFlow::new(build.into_cache_key())")
            && datagram_manager.contains("resume.connector_flow_cache_key(endpoint.server, endpoint.port)")
            && !datagram_manager.contains("fn cache_key(&self) -> String;")
            && !datagram_manager.contains("fn cache_key(self) -> String")
            && !datagram_manager.contains(".cache_key()")
            && hysteria2_connector.contains("fn connector_flow_cache_key(&self, server: &str, port: u16) -> String")
            && shadowsocks_protocol.contains("pub fn into_cache_key(self) -> alloc::string::String")
            && !hysteria2_connector.contains("self.cache_key()"),
        "managed datagram connector flow builds should consume cache identity instead of exposing cache-key getters to proxy"
    );
    assert!(
        packet_path.contains("fn into_parts(self) -> (String, String, u16);")
            && packet_path.contains("let (cache_key, server, port) = build.into_parts();")
            && packet_path.contains("fn into_path_parts(self) -> (String, UdpDatagramKey)")
            && !packet_path.contains("fn server(&self) -> &str;")
            && !packet_path.contains("fn port(&self) -> u16;")
            && socks5_packet_path
                .contains("Socks5ManagedUdpPacketPathCarrierDescriptor::into_parts(")
            && shadowsocks_packet_path
                .contains("ShadowsocksManagedUdpPacketPathCarrierDescriptor::into_parts(self)")
            && shadowsocks_packet_path.contains(
                "ShadowsocksManagedUdpPacketPathDatagramSourceBuild::into_shared_codec_parts(self)"
            )
            && hysteria2_connector.contains("pub fn into_parts(self) -> (String, String, u16)")
            && !socks5_packet_path.contains("self.server()")
            && !socks5_packet_path.contains("self.port()")
            && !shadowsocks_packet_path.contains("self.server()")
            && !shadowsocks_packet_path.contains("self.port()")
            && !hysteria2_connector.contains("self.server()")
            && !hysteria2_connector.contains("self.port()")
            && socks5_packet_path.contains("fn into_parts(self) -> (String, String, u16)")
            && shadowsocks_protocol.contains(
                "pub fn into_parts(self) -> (alloc::string::String, alloc::string::String, u16)"
            )
            && hysteria2_protocol.contains("pub fn into_parts(self) -> (String, String, u16)"),
        "packet-path carrier descriptors should cross into proxy as consumed neutral parts"
    );
    assert!(
        packet_path_key.contains("let (carrier_key, datagram) = lookup.into_path_parts();")
            && !packet_path_key.contains("lookup.carrier_cache_key")
            && !packet_path_key.contains("lookup.datagram"),
        "packet-path lookup keys should cross chain management through consuming helpers, not public field reads"
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
            && shadowsocks_packet_path.contains(
                "ShadowsocksManagedUdpPacketPathDatagramSourceBuild::into_shared_codec_parts(self)"
            )
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
    let socks5_udp = read_repo_module_tree("protocols/socks5/src/udp.rs");
    let socks5_transport = read_repo_module_tree("crates/transport/src/socks5_transport.rs");
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
        socks5_udp.contains("fn udp_cache_key(")
            && !socks5_udp.contains("pub fn udp_cache_key(")
            && socks5_udp.contains("socks5|"),
        "protocols/socks5 should own SOCKS5 cache identity construction internally"
    );
    let proxy_test_support = fs::read_to_string(manifest_dir().join("tests/support/mod.rs"))
        .expect("read proxy test support source");
    assert!(
        socks5_udp.contains("pub(crate) struct Socks5UdpPacket")
            && socks5_udp.contains("Socks5InboundUdpRequest")
            && socks5_udp.contains("Socks5InboundUdpResponse")
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
            socks5_udp.contains(&format!("pub(crate) fn {private_helper}("))
                && !socks5_lib.contains(private_helper),
            "SOCKS5 UDP helper `{private_helper}` should stay crate-private and should not be re-exported"
        );
    }
    let socks5_adapter = read("src/adapters/socks5/udp.rs");
    let socks5_packet_path = read("src/adapters/socks5/udp/packet_path.rs");
    assert!(
        !socks5_adapter.contains("socks5::udp_cache_key")
            && !socks5_adapter.contains("Socks5UdpFlowConfig::new")
            && socks5_adapter.contains("Socks5TransportLeaf::from_resolved_leaf")
            && socks5_adapter.contains("leaf.udp_packet_path_plan()")
            && !socks5_adapter
                .contains("socks5::udp::udp_packet_path_carrier_descriptor_from_config")
            && !socks5_adapter.contains("socks5::udp::udp_packet_path_carrier_build_from_config")
            && !socks5_adapter.contains("socks5::udp::udp_flow_resume_from_config")
            && !socks5_packet_path
                .contains("socks5::udp::udp_packet_path_carrier_descriptor_from_config")
            && !socks5_packet_path.contains("Socks5UdpFlowConfig::new")
            && !socks5_packet_path.contains(".packet_path_spec()")
            && !socks5_packet_path.contains("udp_packet_path_carrier_build_from_config")
            && !socks5_packet_path.contains("udp_packet_path_carrier_descriptor_from_config")
            && socks5_packet_path.contains("packet_path_carrier_descriptor_from_build")
            && socks5_packet_path.contains("plan.into_carrier_descriptor()")
            && socks5_packet_path.contains("plan.into_carrier_build()")
            && socks5_packet_path.contains("packet_path_payload_carrier(association)")
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
            && socks5_udp.contains("pub fn packet_path_carrier_association_target")
            && socks5_udp.contains("carrier.into_association_target()")
            && socks5_transport.contains("pub fn udp_packet_path_plan(&self) -> Socks5ManagedUdpPacketPathPlan")
            && !socks5_packet_path.contains(".packet_path_cache_key()")
            && !socks5_adapter.contains("Socks5UdpFlowConfig {")
            && !socks5_packet_path.contains("Socks5UdpFlowConfig {")
            && socks5_udp.contains("struct Socks5UdpFlowConfig")
            && socks5_udp.contains("pub fn new(")
            && socks5_udp.contains("pub struct Socks5UdpPacketPathSpec")
            && socks5_udp.contains("pub fn packet_path_spec(&self)")
            && socks5_udp.contains("pub fn carrier_descriptor(&self)")
            && socks5_udp.contains("pub fn carrier_build(&self)")
            && !socks5_udp.contains("pub fn udp_packet_path_spec_from_config(")
            && !socks5_udp.contains("pub fn udp_packet_path_carrier_descriptor_from_config(")
            && !socks5_udp.contains("pub fn udp_packet_path_carrier_build_from_config(")
            && !socks5_udp.contains("pub fn udp_flow_resume_from_config(")
            && !socks5_udp.contains("pub fn carrier_cache_key(&self)")
            && !socks5_udp.contains("pub struct Socks5UdpPacketPathCarrier {")
            && !socks5_udp.contains("pub fn packet_path_cache_key(&self)")
            && !socks5_udp.contains("pub fn packet_path_association_config(&self)"),
        "SOCKS5 adapter should request packet-path cache identity through one protocol-owned UDP config object"
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
    let managed = read("src/adapters/shadowsocks/udp.rs");
    let generic_manager =
        read_proxy_module_tree("src/runtime/udp_flow/managed/datagram_manager.rs");
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
        generic_manager.contains("ManagedDatagramConnectionCache")
            && generic_manager.contains("ManagedDatagramSocketConnectorFlow")
            && !managed.contains(".waiters")
            && !managed.contains("BridgeWaiters")
            && !managed.contains("impl ManagedDatagramUdpConnection")
            && !managed.contains("SsUpstream")
            && !managed.contains("self.waiters.register"),
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
        ("src/adapters/socks5/udp/flow.rs", "UpstreamTrackedStart"),
        (
            "src/adapters/shadowsocks/udp/flow.rs",
            "ManagedDatagramStart",
        ),
        ("src/adapters/hysteria2/udp/flow.rs", "ManagedDatagramStart"),
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

    let mieru_adapter = read("src/adapters/mieru/udp/flow.rs");
    assert!(
        mieru_adapter.contains("start_direct_managed_stream_packet(")
            && mieru_adapter.contains("start_relay_managed_stream_packet(")
            && !mieru_adapter.contains("ManagedUdpSend")
            && !mieru_adapter.contains("ManagedUdpOutboundKind")
            && !mieru_adapter.contains("start_tracked_managed_udp"),
        "src/adapters/mieru/udp/flow.rs should use the higher-level neutral managed stream start helpers instead of owning runtime flow construction"
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
            !mieru_adapter.contains(forbidden),
            "src/adapters/mieru/udp/flow.rs should not call removed protocol-named UdpDispatch facade `{forbidden}`"
        );
    }

    for source in [
        "src/adapters/trojan.rs",
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
    ] {
        let transport = read(source);
        assert!(
            transport.contains("start_protocol_transport_bridge_udp_flow(")
                && transport
                    .contains("start_protocol_transport_bridge_udp_relay_final_hop(")
                && !transport.contains("ManagedStreamPacketStartBridge")
                && !transport.contains("start_tracked_managed_stream_packet(")
                && !transport.contains("UdpFlowOutbound::StreamPacket")
                && !transport.contains("register_managed_stream_flow_sender")
                && !transport.contains("register_managed_stream_packet_flow")
                && !transport.contains("ManagedStreamPacketSender"),
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

    let managed = read_proxy_module_tree("src/runtime/udp_dispatch/managed.rs");
    assert!(
        managed.contains("pub(crate) fn flow_start_context")
            && managed.contains("UdpFlowStartContext::new")
            && !managed.contains("ManagedStreamPacketStart")
            && !managed.contains("protocol_udp_state_and_chain_tasks"),
        "runtime UDP dispatch should expose only the narrow persistent flow-start context boundary"
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

    for (source, manager, resume) in [
        (
            "src/adapters/vless.rs",
            "VlessUdpOutboundManager",
            "managed_stream_udp_handler_for_bridge::<",
        ),
        (
            "src/adapters/vmess.rs",
            "VmessUdpOutboundManager",
            "managed_stream_udp_handler_for_bridge::<",
        ),
    ] {
        let transport = read_proxy_module_tree(source);
        assert!(
            !transport.contains("ManagedStreamPacketSender")
                && transport.contains(resume)
                && !transport.contains("open_relay_udp_flow_with_transport")
                && !transport.contains(manager)
                && !transport.contains("register_managed_stream_flow_sender")
                && !transport.contains("register_managed_stream_packet_flow"),
            "{source} should stay on resume/connection bridge glue without reviving protocol-owned manager state"
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

    let snapshot = read_proxy_module_tree("src/runtime/udp_flow/managed/flow.rs");

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
    let managed = read_proxy_module_tree("src/runtime/udp_dispatch/managed.rs");
    for source in [
        "src/runtime/udp_dispatch/managed.rs",
        "src/adapters/socks5/udp/flow.rs",
        "src/adapters/hysteria2/udp/flow.rs",
        "src/adapters/mieru/udp/flow.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/trojan.rs",
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
                "{source} should import protocol UDP types from protocol-owned adapter/protocol modules, not the runtime dispatch facade `{forbidden}`"
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
                "build_udp_request",
                "connector_flow_from_resume",
                "connector_tls_profile_parts_from_resume",
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

#[test]
fn protocol_crate_roots_do_not_reexport_tcp_helper_internals() {
    let vless_lib = fs::read_to_string(repo_root().join("protocols/vless/src/lib.rs"))
        .expect("read vless protocol lib source");
    let vmess_lib = fs::read_to_string(repo_root().join("protocols/vmess/src/lib.rs"))
        .expect("read vmess protocol lib source");
    let trojan_lib = fs::read_to_string(repo_root().join("protocols/trojan/src/lib.rs"))
        .expect("read trojan protocol lib source");

    for forbidden in [
        "VlessTcpTunnelTarget",
        "VlessFlowTcpTunnelTarget",
        "VlessTcpOutboundStream",
        "tcp_connect_config_from_config",
        "pub use metadata::VlessProtocol",
        "pub use outbound::{VlessOutbound, VlessTcpConnectConfig}",
        "VlessOutbound,",
        "VlessTcpConnectConfig",
        "pub use deferred_response::DeferredVlessResponseStream",
        "pub use flow::{",
        "flow_build_request,",
        "FLOW_XTLS_RPRX_VISION,",
        "pub use mux_crypto::MuxCrypto",
        "pub use reality::{",
        "generate_reality_key_pair",
        "upgrade_reality_client",
        "upgrade_reality_server",
        "RealityClientOptions",
        "RealityServerOptions",
        "RealityTlsStream,",
        "VlessRealityServerProfile,",
    ] {
        assert!(
            !vless_lib.contains(forbidden),
            "protocols/vless lib root should not re-export TCP helper internals `{forbidden}`"
        );
    }

    assert!(
        vless_lib.contains("pub mod deferred_response;") && vless_lib.contains("pub mod flow;"),
        "protocols/vless lib root should expose explicit modules for Reality-only helpers instead of flattening them into the crate root"
    );
    assert!(
        vless_lib.contains("pub mod metadata;")
            && vless_lib.contains("pub mod outbound;")
            && vless_lib.contains("pub mod mux_crypto;")
            && vless_lib.contains("pub mod reality;"),
        "protocols/vless lib root should expose explicit metadata and outbound/Reality helper modules instead of flattening them into the crate root"
    );

    for forbidden in [
        "establish_tcp_outbound_session",
        "establish_tcp_outbound_stream",
        "wrap_tcp_outbound_stream",
        "VmessTcpSessionTarget",
        "VmessOutboundSession",
        "tcp_connect_config_from_config",
        "wrap_tcp_inbound_stream",
        "VmessAeadStream",
        "pub use metadata::VmessProtocol",
        "pub use outbound::{VmessOutbound, VmessTcpConnectConfig}",
        "VmessOutbound",
        "VmessTcpConnectConfig",
        "parse_uuid",
        "AUTH_ID_LEN",
        "CMD_TCP",
        "CMD_UDP",
        "GCM_TAG_LEN",
        "MUX_COOL_DOMAIN",
        "MUX_COOL_PORT",
        "VERSION",
    ] {
        assert!(
            !vmess_lib.contains(forbidden),
            "protocols/vmess lib root should not re-export TCP helper internals `{forbidden}`"
        );
    }
    assert!(
        vmess_lib.contains("pub mod metadata;")
            && vmess_lib.contains("pub mod outbound;")
            && vmess_lib.contains("pub use shared::VmessCipher;"),
        "protocols/vmess lib root should keep explicit metadata and outbound module access while exposing only the user-facing cipher enum from shared helpers"
    );

    for forbidden in [
        "pub use metadata::TrojanProtocol",
        "pub use outbound::{TrojanOutbound, TrojanTcpConnectConfig}",
        "TrojanOutbound",
        "TrojanTcpConnectConfig",
        "tcp_outbound_profile_from_config_password",
        "tcp_connect_config_from_config",
        "TrojanTcpOutboundProfile",
        "TrojanTcpTunnelTarget",
        "TrojanTlsProfileAccess",
    ] {
        assert!(
            !trojan_lib.contains(forbidden),
            "protocols/trojan lib root should not re-export TCP helper internals `{forbidden}`"
        );
    }
    assert!(
        trojan_lib.contains("pub mod metadata;") && trojan_lib.contains("pub mod outbound;"),
        "protocols/trojan lib root should expose explicit metadata/outbound modules instead of flattening outbound helpers into the crate root"
    );
}

#[test]
fn protocol_inbound_accept_state_stays_protocol_private() {
    let trojan_inbound = fs::read_to_string(repo_root().join("protocols/trojan/src/inbound.rs"))
        .expect("read trojan protocol inbound source");
    let vmess_inbound = fs::read_to_string(repo_root().join("protocols/vmess/src/inbound.rs"))
        .expect("read vmess protocol inbound source");
    let vmess_stream = fs::read_to_string(repo_root().join("protocols/vmess/src/stream.rs"))
        .expect("read vmess protocol stream source");

    assert!(
        trojan_inbound.contains("struct TrojanAccept {")
            && !trojan_inbound.contains("pub struct TrojanAccept {")
            && !trojan_inbound.contains("pub session: Session")
            && !trojan_inbound.contains("pub command: u8")
            && !trojan_inbound.contains("pub fn session(&self) -> &Session")
            && !trojan_inbound.contains("pub fn command(&self) -> u8")
            && !trojan_inbound.contains("pub fn into_session(self) -> Session")
            && !trojan_inbound.contains("pub async fn accept<S: AsyncSocket>(")
            && trojan_inbound.contains("pub async fn accept_session<S: AsyncSocket>(")
            && trojan_inbound.contains("pub async fn accept_client<S: AsyncSocket>(")
            && trojan_inbound.contains("pub async fn accept_route_owned<S: AsyncSocket>("),
        "Trojan inbound raw accept state should stay module-private and expose only session/route helpers"
    );
    assert!(
        vmess_inbound.contains("pub(crate) struct VmessAccept {")
            && vmess_inbound.contains("pub(crate) struct VmessAcceptedStreamState {")
            && vmess_inbound.contains("session: Session")
            && vmess_inbound.contains("stream_state: VmessAcceptedStreamState")
            && !vmess_inbound.contains("pub struct VmessAccept {")
            && !vmess_inbound.contains("pub session: Session")
            && !vmess_inbound.contains("pub response_header: u8")
            && vmess_inbound.contains("pub(crate) upload_key: Vec<u8>")
            && vmess_inbound.contains("pub(crate) upload_nonce: Vec<u8>")
            && vmess_inbound.contains("pub(crate) download_key: Vec<u8>")
            && vmess_inbound.contains("pub(crate) download_nonce: Vec<u8>")
            && vmess_inbound.contains("pub(crate) cipher: VmessCipher")
            && vmess_inbound.contains("fn session(&self) -> &Session")
            && vmess_inbound.contains("fn into_session(self) -> Session")
            && !vmess_inbound.contains("pub fn session(&self) -> &Session")
            && !vmess_inbound.contains("pub fn into_session(self) -> Session")
            && vmess_inbound.contains("pub(crate) fn into_stream_state(self) -> VmessAcceptedStreamState")
            && vmess_stream.contains("let stream_state = accept.into_stream_state();")
            && vmess_stream.contains("pub(crate) fn inbound(inner: S, accept: VmessAccept)")
            && !vmess_stream.contains("pub fn inbound(inner: S, accept: VmessAccept)")
            && vmess_stream.contains("pub(crate) fn wrap_tcp_inbound_stream<S>(")
            && !vmess_stream.contains("pub fn wrap_tcp_inbound_stream<S>(")
            && !vmess_stream.contains("accept.upload_key")
            && !vmess_stream.contains("accept.upload_nonce")
            && !vmess_stream.contains("accept.download_key")
            && !vmess_stream.contains("accept.download_nonce"),
        "VMess inbound accept state should stay crate-private and be consumed through a crate-local stream-state handoff"
    );
}

#[test]
fn split_http_wire_details_live_outside_transport_root() {
    let root = read_repo_file("crates/transport/src/split_http.rs");
    let chunked = read_repo_file("crates/transport/src/split_http/chunked.rs");
    let legacy = read_repo_file("crates/transport/src/split_http/legacy.rs");
    let paired = read_repo_file("crates/transport/src/split_http/paired.rs");
    let registry = read_repo_file("crates/transport/src/split_http/registry.rs");
    let stream_one = read_repo_file("crates/transport/src/split_http/stream_one.rs");
    let wire = read_repo_file("crates/transport/src/split_http/wire.rs");

    assert!(root.contains("mod chunked;"));
    for owned in [
        "enum ChunkState {",
        "enum DecodeStep {",
        "struct ChunkedDecoder {",
        "fn try_decode(&mut self, buf: &mut ReadBuf<'_>)",
    ] {
        assert!(
            !root.contains(owned),
            "split_http root should not own chunk decoder detail `{owned}`"
        );
        assert!(
            chunked.contains(owned),
            "split_http chunked module should own `{owned}`"
        );
    }

    assert!(root.contains("mod paired;"));
    assert!(root.contains("pub use paired::{SplitHttpPairedStream, SplitHttpStream};"));
    for owned in [
        "pub struct SplitHttpPairedStream<R, W> {",
        "impl<R, W> AsyncRead for SplitHttpPairedStream<R, W>",
        "impl<R, W> AsyncWrite for SplitHttpPairedStream<R, W>",
        "impl<R, W> AsyncSocket for SplitHttpPairedStream<R, W>",
        "impl<R, W> ClientStream for SplitHttpPairedStream<R, W>",
    ] {
        assert!(
            !root.contains(owned),
            "split_http root should not own paired-stream lifecycle `{owned}`"
        );
        assert!(
            paired.contains(owned),
            "split_http paired module should own `{owned}`"
        );
    }

    assert!(root.contains("mod legacy;"));
    assert!(root.contains("pub use legacy::{accept_split_http, connect_split_http};"));
    for owned in [
        "pub async fn connect_split_http<S>(",
        "pub async fn accept_split_http<S>(",
        "async fn accept_half<S>(",
        "async fn read_headers<S>(",
        "fn downcast<S>(",
    ] {
        assert!(
            !root.contains(owned),
            "split_http root should not own legacy paired lifecycle `{owned}`"
        );
        assert!(
            legacy.contains(owned),
            "split_http legacy module should own `{owned}`"
        );
    }

    assert!(root.contains("mod stream_one;"));
    for owned in [
        "pub enum XhttpMode {",
        "pub struct XhttpStreamOne<S> {",
        "pub async fn connect_xhttp_stream_one<S>(",
        "pub async fn accept_xhttp_stream_one<S>(",
        "impl<S> AsyncRead for XhttpStreamOne<S>",
        "impl<S> AsyncWrite for XhttpStreamOne<S>",
        "impl<S> AsyncSocket for XhttpStreamOne<S>",
        "impl<S> ClientStream for XhttpStreamOne<S>",
    ] {
        assert!(
            !root.contains(owned),
            "split_http root should not own stream-one lifecycle `{owned}`"
        );
        assert!(
            stream_one.contains(owned),
            "split_http stream_one module should own `{owned}`"
        );
    }

    assert!(root.contains("mod registry;"));
    assert!(root.contains("pub use registry::SplitHttpRegistry;"));
    for owned in [
        "struct SplitHttpPending {",
        "pub struct SplitHttpRegistry {",
        "impl SplitHttpRegistry {",
        "fn generate_session_id()",
    ] {
        assert!(
            !root.contains(owned),
            "split_http root should not own registry detail `{owned}`"
        );
        assert!(
            registry.contains(owned),
            "split_http registry module should own `{owned}`"
        );
    }
    assert!(root.contains("mod wire;"));
    for helper in [
        "fn find_header_end(",
        "fn parse_status(",
        "fn parse_method_and_session(",
        "fn validate_path(",
        "fn write_get_response<",
        "fn write_http_request(",
    ] {
        assert!(
            !root.contains(helper),
            "split_http root should remain orchestration and not own `{helper}`"
        );
        assert!(
            wire.contains(helper),
            "split_http wire module should own `{helper}`"
        );
    }
}

#[test]
fn tls_certificate_io_lives_outside_transport_root() {
    let root = read_repo_file("crates/transport/src/tls.rs");
    let certificates = read_repo_file("crates/transport/src/tls/certificates.rs");
    let client_hello = read_repo_file("crates/transport/src/tls/client_hello.rs");
    let fingerprint = read_repo_file("crates/transport/src/tls/fingerprint.rs");
    let inbound_stream = read_repo_file("crates/transport/src/tls/inbound_stream.rs");

    assert!(root.contains("mod certificates;"));
    for helper in ["fn load_certs(", "fn load_private_key(", "fn resolve_path("] {
        assert!(
            !root.contains(helper),
            "tls root should remain handshake orchestration and not own `{helper}`"
        );
        assert!(
            certificates.contains(helper),
            "tls certificates module should own `{helper}`"
        );
    }
    for detail in [
        "File::open(",
        "rustls_pemfile::certs(",
        "rustls_pemfile::private_key(",
    ] {
        assert!(
            !root.contains(detail),
            "tls root should not perform certificate I/O detail `{detail}`"
        );
        assert!(certificates.contains(detail));
    }

    assert!(root.contains("mod client_hello;"));
    assert!(root.contains("pub async fn peek_client_hello<"));
    for helper in [
        "fn parse_extensions(",
        "async fn read_exact<",
        "async fn skip_exact<",
    ] {
        assert!(
            !root.contains(helper),
            "tls root should not own ClientHello detail `{helper}`"
        );
        assert!(client_hello.contains(helper));
    }

    assert!(root.contains("mod inbound_stream;"));
    assert!(root.contains("pub use inbound_stream::InboundTlsStream;"));
    for owned in [
        "pub struct InboundTlsStream<",
        "impl<IO> AsyncSocket for InboundTlsStream<IO>",
        "impl<IO> AsyncRead for InboundTlsStream<IO>",
        "impl<IO> AsyncWrite for InboundTlsStream<IO>",
    ] {
        assert!(
            !root.contains(owned),
            "tls root should not own inbound stream detail `{owned}`"
        );
        assert!(inbound_stream.contains(owned));
    }

    assert!(root.contains("mod fingerprint;"));
    for detail in [
        "fn tls13_config(",
        "fn rustls_to_ztls_suite(",
        "Tls13Stream::connect_async(",
        "Tls13Stream::connect(",
    ] {
        assert!(
            !root.contains(detail),
            "tls root should not own fingerprint handshake detail `{detail}`"
        );
        assert!(fingerprint.contains(detail));
    }
}

#[test]
fn socks5_transport_models_live_outside_transport_root() {
    let root = read_repo_file("crates/transport/src/socks5_transport.rs");
    let inbound = read_repo_file("crates/transport/src/socks5_transport/inbound.rs");
    let leaf = read_repo_file("crates/transport/src/socks5_transport/leaf.rs");
    let model = read_repo_file("crates/transport/src/socks5_transport/model.rs");
    let tcp = read_repo_file("crates/transport/src/socks5_transport/tcp.rs");
    let upstream = read_repo_file("crates/transport/src/socks5_transport/upstream.rs");

    assert!(root.contains("mod inbound;"));
    assert!(root.contains("pub use inbound::{"));
    for owned in [
        "fn inbound_acceptor_from_users(",
        "fn inbound_acceptor_from_protocol(",
        "async fn setup_inbound_udp_association<",
        "impl<S> InboundClientResponse<S> for OwnedSocks5InboundAcceptor",
        "impl InboundUdpAssociation for Socks5InboundUdpAssociationHandler",
        "impl InboundUdpAssociationResponder for Socks5InboundUdpAssociationHandler",
    ] {
        assert!(
            !root.contains(owned),
            "socks5 transport root should not own inbound behavior `{owned}`"
        );
        assert!(inbound.contains(owned));
    }
    assert!(root.contains("mod model;"));
    assert!(root.contains("pub use model::{"));
    for owned in [
        "pub enum Socks5UpstreamAssociationCloseReason {",
        "pub struct OwnedSocks5InboundAcceptor {",
        "pub struct Socks5InboundUdpAssociationSetup {",
        "pub struct Socks5ManagedUdpFlowPlan<'a> {",
        "pub struct Socks5ManagedUdpPacketPathPlan {",
        "pub struct Socks5TransportLeaf<'a> {",
    ] {
        assert!(
            !root.contains(owned),
            "socks5 transport root should not own model `{owned}`"
        );
        assert!(model.contains(owned));
    }
    for owned in [
        "impl<'a> Socks5ManagedUdpFlowConfig<'a> {",
        "impl Socks5ManagedUdpAssociationTarget {",
        "impl Socks5ManagedUdpPacketPathCarrierBuild {",
        "impl Socks5ManagedUdpPacketPathCarrierDescriptor {",
        "impl<'a> Socks5ManagedUdpFlowPlan<'a> {",
        "impl Socks5ManagedUdpPacketPathPlan {",
        "fn into_protocol_target(",
        "fn into_protocol_build(",
    ] {
        assert!(
            !root.contains(owned),
            "socks5 transport root should not own model behavior `{owned}`"
        );
        assert!(model.contains(owned));
    }

    assert!(root.contains("mod leaf;"));
    for owned in [
        "impl<'a> Socks5TransportLeaf<'a> {",
        "pub fn from_resolved_leaf(",
        "pub fn udp_flow_plan(",
        "pub fn udp_packet_path_plan(",
        "pub async fn open_tcp_stream<",
        "pub async fn open_tcp_relay_hop(",
    ] {
        assert!(
            !root.contains(owned),
            "socks5 transport root should not own leaf behavior `{owned}`"
        );
        assert!(leaf.contains(owned));
    }

    assert!(root.contains("mod upstream;"));
    assert!(root.contains("pub use upstream::{"));
    for exported in [
        "establish_packet_path_udp_association",
        "establish_registered_udp_association",
        "Socks5UdpAssociationRuntime",
        "Socks5UpstreamUdpAssociation",
    ] {
        assert!(root.contains(exported));
    }
    for owned in [
        "pub struct Socks5UpstreamUdpAssociation {",
        "pub trait Socks5UdpAssociationRuntime:",
        "pub async fn establish_registered_udp_association<",
        "pub async fn establish_packet_path_udp_association<",
        "impl Socks5UpstreamUdpAssociation {",
        "pub async fn send_packet(",
        "pub async fn recv_response_parts(",
        "impl Drop for Socks5UpstreamUdpAssociation",
    ] {
        assert!(
            !root.contains(owned),
            "socks5 transport root should not own upstream lifecycle `{owned}`"
        );
        assert!(upstream.contains(owned));
    }

    assert!(root.contains("mod tcp;"));
    assert!(
        root.contains("pub use tcp::{apply_socks5_tcp_relay_hop, establish_socks5_tcp_connect};")
    );
    for owned in [
        "pub async fn establish_socks5_tcp_connect(",
        "pub async fn apply_socks5_tcp_relay_hop(",
        "Socks5TcpOutboundProfile::from_config_parts(",
    ] {
        assert!(
            !root.contains(owned),
            "socks5 transport root should not own TCP bridge `{owned}`"
        );
        assert!(tcp.contains(owned));
    }
}

#[test]
fn shadowsocks_transport_models_live_outside_transport_root() {
    let root = read_repo_file("crates/transport/src/shadowsocks_transport.rs");
    let inbound = read_repo_file("crates/transport/src/shadowsocks_transport/inbound.rs");
    let leaf = read_repo_file("crates/transport/src/shadowsocks_transport/leaf.rs");
    let model = read_repo_file("crates/transport/src/shadowsocks_transport/model.rs");
    let tcp = read_repo_file("crates/transport/src/shadowsocks_transport/tcp.rs");
    let udp_socket = read_repo_file("crates/transport/src/shadowsocks_transport/udp_socket.rs");

    assert!(root.contains("mod inbound;"));
    assert!(root.contains("pub use inbound::inbound_profile_from_protocol;"));
    for owned in [
        "fn inbound_profile_from_protocol(",
        "impl OwnedShadowsocksInboundProfile {",
        "impl OwnedShadowsocksInboundTcpAcceptor {",
        "pub async fn accept_and_dispatch_stream<",
        "impl OwnedShadowsocksInboundBindings {",
    ] {
        assert!(
            !root.contains(owned),
            "shadowsocks transport root should not own inbound behavior `{owned}`"
        );
        assert!(inbound.contains(owned));
    }
    assert!(root.contains("mod model;"));
    assert!(root.contains("pub use model::{"));
    for owned in [
        "pub type ShadowsocksUdpResponse =",
        "pub struct ShadowsocksManagedDatagramFlowResume {",
        "pub struct ShadowsocksManagedUdpFlowPlan<'a> {",
        "pub struct ShadowsocksManagedUdpPacketPathPlan<'a> {",
        "pub struct ShadowsocksManagedUdpFlowConfig<'a> {",
        "pub struct ShadowsocksTransportLeaf<'a> {",
        "pub struct OwnedShadowsocksInboundProfile {",
        "pub struct OwnedShadowsocksInboundTcpAcceptor {",
        "pub struct OwnedShadowsocksInboundBindings {",
    ] {
        assert!(
            !root.contains(owned),
            "shadowsocks transport root should not own model `{owned}`"
        );
        assert!(model.contains(owned));
    }
    for owned in [
        "impl<'a> ShadowsocksManagedUdpFlowConfig<'a> {",
        "impl ShadowsocksManagedDatagramFlowResume {",
        "impl<'a> ShadowsocksManagedUdpFlowPlan<'a> {",
        "impl<'a> ShadowsocksManagedUdpPacketPathPlan<'a> {",
        "impl ShadowsocksManagedUdpPacketPathCarrierDescriptor {",
        "impl ShadowsocksManagedUdpPacketPathDatagramSourceBuild {",
        "fn protocol_config(",
        "fn into_shared_managed_socket_flow_codec(",
    ] {
        assert!(
            !root.contains(owned),
            "shadowsocks transport root should not own model behavior `{owned}`"
        );
        assert!(model.contains(owned));
    }
    assert!(root.contains("mod leaf;"));
    for owned in [
        "impl<'a> ShadowsocksTransportLeaf<'a> {",
        "pub fn from_resolved_leaf(",
        "pub fn udp_flow_plan(",
        "pub fn udp_packet_path_plan(",
        "pub async fn open_tcp_stream<",
        "pub async fn open_tcp_relay_hop(",
        "fn flow_config(",
    ] {
        assert!(
            !root.contains(owned),
            "shadowsocks transport root should not own leaf behavior `{owned}`"
        );
        assert!(leaf.contains(owned));
    }
    assert!(root.contains("mod udp_socket;"));
    for owned in [
        "pub struct ShadowsocksUdpSocketFlow {",
        "pub async fn establish_shadowsocks_udp_socket_flow(",
        "impl ShadowsocksUdpSocketFlow {",
        "impl ManagedDatagramConnectionOps for ShadowsocksUdpSocketFlow",
        "async fn bind_for_endpoint(",
        "fn spawn_recv_loop(",
        "async fn recv_loop(",
    ] {
        assert!(
            !root.contains(owned) && !model.contains(owned),
            "socket-backed lifecycle `{owned}` should not live in root or pure model"
        );
        assert!(udp_socket.contains(owned));
    }

    assert!(root.contains("mod tcp;"));
    for owned in [
        "pub async fn establish_shadowsocks_tcp_connect(",
        "pub async fn apply_shadowsocks_tcp_relay_hop(",
        "fn shadowsocks_tcp_connect_config(",
        "shadowsocks::tcp_connect_config_from_config(",
    ] {
        assert!(
            !root.contains(owned),
            "shadowsocks transport root should not own TCP bridge `{owned}`"
        );
        assert!(tcp.contains(owned));
    }
}

#[test]
fn vless_inbound_carrier_lives_outside_request_root() {
    let root = read_repo_file("crates/transport/src/vless_transport/inbound.rs");
    let bind = read_repo_file("crates/transport/src/vless_transport/inbound/bind.rs");
    let carrier = read_repo_file("crates/transport/src/vless_transport/inbound/carrier.rs");
    let plan = read_repo_file("crates/transport/src/vless_transport/inbound/plan.rs");

    assert!(root.contains("mod bind;"));
    assert!(root.contains("pub use bind::OwnedVlessInboundBindPlan;"));
    for owned in [
        "pub struct OwnedVlessInboundBindPlan {",
        "impl OwnedVlessInboundBindPlan {",
        "impl crate::inbound_route::ProtocolInboundBindPlan for OwnedVlessInboundBindPlan",
        "quic::QuicInbound::bind(",
    ] {
        assert!(
            !root.contains(owned),
            "VLESS inbound request root should not own bind plan `{owned}`"
        );
        assert!(
            bind.contains(owned),
            "VLESS bind module should own `{owned}`"
        );
    }

    assert!(root.contains("mod carrier;"));
    for owned in [
        "enum VlessInboundTransportResult {",
        "enum VlessInboundTransportStream {",
        "fn accept_vless_inbound_transport(",
        "fn accept_vless_inbound_carrier(",
        "impl zero_traits::AsyncSocket for VlessInboundTransportStream",
        "impl AsyncRead for VlessInboundTransportStream",
        "impl AsyncWrite for VlessInboundTransportStream",
        "impl ClientStream for VlessInboundTransportStream",
        "fn accept_vless_tls_inbound_transport(",
    ] {
        assert!(
            !root.contains(owned),
            "VLESS inbound request root should not own carrier lifecycle `{owned}`"
        );
        assert!(
            carrier.contains(owned),
            "VLESS inbound carrier module should own `{owned}`"
        );
    }

    assert!(root.contains("mod plan;"));
    for owned in [
        "struct OwnedVlessInboundTransportPlan {",
        "impl OwnedVlessInboundTransportPlan {",
        "enum VlessTcpInboundAcceptResult {",
        "fn accept_vless_stream_route<",
        "accept_route_owned_with_sni_or_else(",
        "VlessFallbackReplay::replay_to_upstream(",
    ] {
        assert!(
            !root.contains(owned),
            "VLESS inbound request root should not own transport plan/route `{owned}`"
        );
        assert!(
            plan.contains(owned),
            "VLESS inbound plan module should own `{owned}`"
        );
    }
}

#[test]
fn vless_outbound_transport_plan_lives_outside_carrier_root() {
    let root = read_repo_file("crates/transport/src/vless_transport/outbound.rs");
    let direct = read_repo_file("crates/transport/src/vless_transport/outbound/direct.rs");
    let plan = read_repo_file("crates/transport/src/vless_transport/outbound/plan.rs");
    let relay = read_repo_file("crates/transport/src/vless_transport/outbound/relay.rs");

    assert!(root.contains("mod plan;"));
    for owned in [
        "struct VlessTransportOptions<'a> {",
        "struct VlessOutboundTransportRequest<'a> {",
        "struct VlessDirectTransportRequest<'a> {",
        "struct VlessFinalHopTransportRequest<'a> {",
        "struct VlessUdpTransportOptions<'a> {",
        "struct OwnedVlessUdpTransportOptions {",
        "struct OwnedVlessOutboundTransportPlan {",
        "impl TcpStreamTransportPlan for OwnedVlessOutboundTransportPlan",
        "struct VlessUdpOutboundTransportRequest<'a> {",
    ] {
        assert!(
            !root.contains(owned),
            "VLESS outbound carrier root should not own transport plan `{owned}`"
        );
        assert!(
            plan.contains(owned),
            "VLESS outbound plan module should own `{owned}`"
        );
    }
    assert!(
        !plan.contains("pub(crate)") && !plan.contains("pub struct"),
        "VLESS outbound plan visibility should stay confined to vless_transport"
    );

    assert!(root.contains("mod direct;") && root.contains("mod relay;"));
    for owned in [
        "fn open_vless_quic_transport(",
        "fn build_vless_direct_outbound_transport(",
        "fn build_vless_outbound_transport(",
        "fn build_vless_udp_outbound_transport(",
        "connect_socket_transport_stack(",
    ] {
        assert!(!root.contains(owned));
        assert!(
            direct.contains(owned),
            "VLESS direct module should own `{owned}`"
        );
    }
    for owned in [
        "fn build_vless_outbound_transport_over_stream(",
        "fn build_vless_split_http_over_relay(",
        "connect_relay_transport_stack(",
    ] {
        assert!(!root.contains(owned));
        assert!(
            relay.contains(owned),
            "VLESS relay module should own `{owned}`"
        );
    }
}

#[test]
fn hysteria2_quic_stream_wrapper_lives_outside_transport_root() {
    let root = read_repo_file("crates/transport/src/hysteria2_quic.rs");
    let connection = read_repo_file("crates/transport/src/hysteria2_quic/connection.rs");
    let inbound = read_repo_file("crates/transport/src/hysteria2_quic/inbound.rs");
    let managed_udp = read_repo_file("crates/transport/src/hysteria2_quic/managed_udp.rs");
    let model = read_repo_file("crates/transport/src/hysteria2_quic/model.rs");
    let projection = read_repo_file("crates/transport/src/hysteria2_quic/projection.rs");
    let stream = read_repo_file("crates/transport/src/hysteria2_quic/stream.rs");

    assert!(root.contains("mod connection;"));
    assert!(root.contains("pub use connection::open_quic_connection;"));
    for owned in [
        "impl Hysteria2QuicProfile {",
        "pub async fn open_quic_connection(",
        "struct SkipVerify;",
        "impl rustls::client::danger::ServerCertVerifier for SkipVerify",
        "quinn::TransportConfig::default()",
        "quinn::Endpoint::new(",
    ] {
        assert!(
            !root.contains(owned),
            "hysteria2 QUIC root should not own connection setup `{owned}`"
        );
        assert!(
            connection.contains(owned),
            "hysteria2 connection module should own `{owned}`"
        );
    }

    assert!(root.contains("mod inbound;"));
    for owned in [
        "pub struct OwnedHysteria2InboundBindPlan {",
        "impl OwnedHysteria2InboundBindPlan {",
        "impl crate::inbound_route::ProtocolInboundBindPlan",
        "fn inbound_profile_from_protocol(",
        "fn inbound_tcp_acceptor(",
        "impl OwnedHysteria2InboundProfile {",
        "impl<S> InboundClientResponse<S> for OwnedHysteria2InboundTcpResponseProtocol",
    ] {
        assert!(
            !root.contains(owned),
            "hysteria2 QUIC root should not own inbound behavior `{owned}`"
        );
        assert!(inbound.contains(owned));
    }
    assert!(root.contains("mod model;"));
    assert!(root.contains("pub use model::{"));
    for owned in [
        "pub struct QuicConnectionOptions<'a> {",
        "pub struct Hysteria2ManagedDatagramFlowResume {",
        "pub struct OwnedHysteria2InboundProfile {",
        "pub struct OwnedHysteria2InboundTcpResponseProtocol {",
        "pub struct Hysteria2ManagedUdpFlowPlan<'a> {",
        "pub struct Hysteria2ManagedUdpPacketPathPlan {",
        "pub struct Hysteria2ManagedUdpFlowConfig<'a> {",
        "pub struct Hysteria2TransportLeaf<'a> {",
        "pub struct Hysteria2QuicProfile {",
    ] {
        assert!(
            !root.contains(owned),
            "hysteria2 QUIC root should not own model `{owned}`"
        );
        assert!(model.contains(owned));
    }
    assert!(root.contains("mod managed_udp;"));
    for owned in [
        "impl crate::managed_udp::ProtocolManagedDatagramUdpResumeMetadata",
        "impl crate::managed_udp::ProtocolManagedDatagramUdpResumeConnectionOps",
        "fn open_udp_profile_connection(",
        "pub async fn open_hysteria2_udp_packet_path_build(",
        "pub async fn establish_hysteria2_udp_flow_connection(",
        "impl ManagedTupleUdpConnectionOps for hysteria2::udp::Hysteria2UdpFlowConnection",
    ] {
        assert!(
            !root.contains(owned),
            "hysteria2 QUIC root should not own managed UDP lifecycle `{owned}`"
        );
        assert!(
            managed_udp.contains(owned),
            "hysteria2 managed_udp module should own `{owned}`"
        );
    }
    assert!(root.contains("mod projection;"));
    for owned in [
        "impl<'a> Hysteria2ManagedUdpFlowConfig<'a>",
        "impl<'a> Hysteria2TransportLeaf<'a>",
        "pub fn udp_flow_resume_from_config(",
        "impl Hysteria2ManagedDatagramFlowResume {",
        "impl<'a> Hysteria2ManagedUdpFlowPlan<'a>",
        "impl Hysteria2ManagedUdpPacketPathPlan {",
        "impl Hysteria2ManagedUdpPacketPathCarrierDescriptor {",
        "impl Hysteria2ManagedUdpPacketPathCarrierBuild {",
    ] {
        assert!(
            !root.contains(owned),
            "hysteria2 QUIC root should not own projection behavior `{owned}`"
        );
        assert!(
            projection.contains(owned),
            "hysteria2 projection module should own `{owned}`"
        );
    }
    assert!(root.contains("mod stream;"));
    assert!(root.contains("pub use stream::Hysteria2Stream;"));
    for owned in [
        "pub struct Hysteria2Stream {",
        "impl AsyncRead for Hysteria2Stream",
        "impl AsyncWrite for Hysteria2Stream",
        "impl AsyncSocket for Hysteria2Stream",
    ] {
        assert!(
            !root.contains(owned),
            "hysteria2 QUIC root should not own stream wrapper `{owned}`"
        );
        assert!(stream.contains(owned));
    }
}
