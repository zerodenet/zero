mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsConnector;
use zero_api::{event_type, EventFilter, EventSource};
use zero_config::RuntimeConfig;
use zero_protocol_vless::parse_uuid;
use zero_proxy::Proxy as Engine;

use support::{free_port, spawn_engine, wait_for, wait_for_listener};

const USER_ID: &str = "11111111-2222-3333-4444-555555555555";

#[tokio::test]
async fn relays_tcp_through_vless_direct_outbound_and_records_principal() {
    let echo_port = free_port();
    let proxy_port = free_port();

    let echo_task = spawn_echo_server(echo_port).await;

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vless-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [
                            {{
                                "id": "{USER_ID}",
                                "credential_id": "node-user-1",
                                "principal_key": "user:10001"
                            }}
                        ]
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    ))
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut client = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
    client
        .write_all(&vless_request_for_ipv4(USER_ID, [127, 0, 0, 1], echo_port))
        .await
        .expect("write request");

    let mut response = [0_u8; 2];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response, [0x00, 0x00]);

    client.write_all(b"vles").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"vles");

    drop(client);
    wait_for("completed vless flow", || {
        !engine_handle.completed_sessions().is_empty()
    })
    .await;

    let completed = engine_handle.completed_sessions();
    assert_eq!(completed[0].auth.as_ref().expect("auth").scheme, "vless");
    assert_eq!(
        completed[0]
            .auth
            .as_ref()
            .and_then(|auth| auth.principal_key.as_deref()),
        Some("user:10001")
    );

    let events = engine_handle
        .subscribe(EventFilter {
            principal_keys: vec!["user:10001".to_owned()],
            ..EventFilter::default()
        })
        .expect("subscribe events");
    let completed_event = events
        .iter()
        .find(|event| event.event_type == event_type::FLOW_COMPLETED)
        .expect("flow completed event");
    assert_eq!(completed_event.principal_key.as_deref(), Some("user:10001"));
    assert_eq!(
        completed_event.payload["auth"]["credential_id"],
        "node-user-1"
    );

    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_tcp_through_vless_tls_direct_outbound() {
    let echo_port = free_port();
    let proxy_port = free_port();
    let tls = test_tls_material();

    let echo_task = spawn_echo_server(echo_port).await;

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vless-tls-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [
                            {{ "id": "{USER_ID}", "principal_key": "user:tls" }}
                        ],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut client = connect_tls_client(proxy_port, tls.cert_der.clone()).await;
    client
        .write_all(&vless_request_for_ipv4(USER_ID, [127, 0, 0, 1], echo_port))
        .await
        .expect("write request");

    let mut response = [0_u8; 2];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response, [0x00, 0x00]);

    client.write_all(b"vtls").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"vtls");

    drop(client);
    wait_for("completed vless tls flow", || {
        !engine_handle.completed_sessions().is_empty()
    })
    .await;

    let completed = engine_handle.completed_sessions();
    assert_eq!(
        completed[0]
            .auth
            .as_ref()
            .and_then(|auth| auth.principal_key.as_deref()),
        Some("user:tls")
    );

    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_tcp_through_vless_chained_outbound() {
    let echo_port = free_port();
    let upstream_port = free_port();
    let outer_port = free_port();

    let echo_task = spawn_echo_server(echo_port).await;

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-vless-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [
                            {{ "id": "{USER_ID}", "principal_key": "node:upstream" }}
                        ]
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    ))
    .expect("parse upstream config");
    let upstream_engine = Engine::new(upstream_config).expect("build upstream engine");
    let upstream_handle = spawn_engine(upstream_engine);

    wait_for_listener(upstream_port).await;

    let outer_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "outer-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {outer_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vless-chain",
                    "protocol": {{
                        "type": "vless",
                        "server": "127.0.0.1",
                        "port": {upstream_port},
                        "id": "{USER_ID}"
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "vless-chain" }}
            }}
        }}"#
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_handle = spawn_engine(outer_engine);

    wait_for_listener(outer_port).await;

    let mut client = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    client
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    client.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    let request = [
        0x05,
        0x01,
        0x00,
        0x01,
        127,
        0,
        0,
        1,
        ((echo_port >> 8) & 0xff) as u8,
        (echo_port & 0xff) as u8,
    ];
    client.write_all(&request).await.expect("write request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response[1], 0x00);

    client.write_all(b"mesh").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"mesh");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    upstream_handle
        .shutdown()
        .await
        .expect("shutdown upstream engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn vless_tls_with_alpn_config() {
    let echo_port = free_port();
    let proxy_port = free_port();
    let tls = test_tls_material();

    let echo_task = spawn_echo_server(echo_port).await;

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vless-tls-alpn-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [{{ "id": "{USER_ID}" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}",
                            "alpn": ["http/1.1", "h2"]
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    // 客户端也配置 ALPN 连接
    let mut roots = rustls::RootCertStore::empty();
    roots.add(tls.cert_der.clone()).expect("trust test cert");
    let mut client_config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let connector = TlsConnector::from(Arc::new(client_config));
    let server_name = rustls::pki_types::ServerName::try_from("localhost")
        .expect("server name")
        .to_owned();
    let stream = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");

    // TLS + ALPN 握手成功
    let mut tls_stream = connector
        .connect(server_name, stream)
        .await
        .expect("tls handshake with alpn");

    // 验证 VLESS 握手也能正常完成
    tls_stream
        .write_all(&vless_request_for_ipv4(USER_ID, [127, 0, 0, 1], echo_port))
        .await
        .expect("write vless request");

    let mut response = [0_u8; 2];
    tls_stream
        .read_exact(&mut response)
        .await
        .expect("read vless response");
    assert_eq!(response, [0x00, 0x00]);

    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn vless_outbound_tls_insecure_skip_verification() {
    let echo_port = free_port();
    let upstream_port = free_port();
    let outer_port = free_port();
    let tls = test_tls_material();

    let echo_task = spawn_echo_server(echo_port).await;

    // 上游服务端用自签名证书（正常情况下客户端会拒绝）
    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-vless-tls-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [{{ "id": "{USER_ID}" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse upstream config");
    let upstream_engine = Engine::new(upstream_config).expect("build upstream engine");
    let upstream_handle = spawn_engine(upstream_engine);

    wait_for_listener(upstream_port).await;

    // 外部代理客户端用 insecure: true 跳过证书验证，连接到自签名的上游
    let outer_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "outer-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {outer_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vless-tls-chain",
                    "protocol": {{
                        "type": "vless",
                        "server": "127.0.0.1",
                        "port": {upstream_port},
                        "id": "{USER_ID}",
                        "tls": {{
                            "server_name": "localhost",
                            "insecure": true
                        }}
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "vless-tls-chain" }}
            }}
        }}"#
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_handle = spawn_engine(outer_engine);

    wait_for_listener(outer_port).await;

    // 通过 SOCKS5 -> VLESS(TLS, insecure) -> 目标
    let mut client = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    client
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    client.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    let request = [
        0x05,
        0x01,
        0x00,
        0x01,
        127,
        0,
        0,
        1,
        ((echo_port >> 8) & 0xff) as u8,
        (echo_port & 0xff) as u8,
    ];
    client.write_all(&request).await.expect("write request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response[1], 0x00);

    client.write_all(b"isky").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"isky");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    upstream_handle
        .shutdown()
        .await
        .expect("shutdown upstream engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_tcp_through_vless_tls_chained_outbound() {
    let echo_port = free_port();
    let upstream_port = free_port();
    let outer_port = free_port();
    let tls = test_tls_material();

    let echo_task = spawn_echo_server(echo_port).await;

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-vless-tls-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [
                            {{ "id": "{USER_ID}", "principal_key": "node:upstream-tls" }}
                        ],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse upstream config");
    let upstream_engine = Engine::new(upstream_config).expect("build upstream engine");
    let upstream_handle = spawn_engine(upstream_engine);

    wait_for_listener(upstream_port).await;

    let outer_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "outer-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {outer_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vless-tls-chain",
                    "protocol": {{
                        "type": "vless",
                        "server": "127.0.0.1",
                        "port": {upstream_port},
                        "id": "{USER_ID}",
                        "tls": {{
                            "server_name": "localhost",
                            "ca_cert_path": "{}"
                        }}
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "vless-tls-chain" }}
            }}
        }}"#,
        escape_json_path(&tls.cert_path),
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_handle = spawn_engine(outer_engine);

    wait_for_listener(outer_port).await;

    let mut client = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    client
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    client.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    let request = [
        0x05,
        0x01,
        0x00,
        0x01,
        127,
        0,
        0,
        1,
        ((echo_port >> 8) & 0xff) as u8,
        (echo_port & 0xff) as u8,
    ];
    client.write_all(&request).await.expect("write request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response[1], 0x00);

    client.write_all(b"vto1").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"vto1");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    upstream_handle
        .shutdown()
        .await
        .expect("shutdown upstream engine");
    let _ = echo_task.await;
}

fn vless_request_for_ipv4(id: &str, address: [u8; 4], port: u16) -> Vec<u8> {
    let id = parse_uuid(id).expect("uuid");
    let mut request = vec![0x00];
    request.extend_from_slice(&id);
    request.extend_from_slice(&[
        0x00, // addon length
        0x01, // tcp command
        ((port >> 8) & 0xff) as u8,
        (port & 0xff) as u8,
        0x01, // ipv4
    ]);
    request.extend_from_slice(&address);
    request
}

async fn spawn_echo_server(port: u16) -> tokio::task::JoinHandle<()> {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .expect("bind echo");
        let _ = ready_tx.send(());
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });
    ready_rx.await.expect("echo server ready");
    task
}

async fn connect_tls_client(
    port: u16,
    cert: rustls::pki_types::CertificateDer<'static>,
) -> tokio_rustls::client::TlsStream<TcpStream> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert).expect("trust test cert");
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::try_from("localhost")
        .expect("server name")
        .to_owned();
    let stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect proxy");

    connector
        .connect(server_name, stream)
        .await
        .expect("tls handshake")
}

struct TestTlsMaterial {
    dir: PathBuf,
    cert_path: PathBuf,
    key_path: PathBuf,
    cert_der: rustls::pki_types::CertificateDer<'static>,
}

impl Drop for TestTlsMaterial {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn test_tls_material() -> TestTlsMaterial {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
        .expect("generate self-signed cert");
    let dir = std::env::temp_dir().join(format!(
        "zero-vless-tls-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create tls temp dir");
    let cert_path = dir.join("server.crt");
    let key_path = dir.join("server.key");
    std::fs::write(&cert_path, certified.cert.pem()).expect("write cert");
    std::fs::write(&key_path, certified.signing_key.serialize_pem()).expect("write key");

    TestTlsMaterial {
        dir,
        cert_path,
        key_path,
        cert_der: certified.cert.der().clone(),
    }
}

fn escape_json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
