use super::*;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const USER_ID_A: &str = "11111111-2222-3333-4444-555555555555";
const USER_ID_B: &str = "66666666-7777-8888-9999-aaaaaaaaaaaa";
static NEXT_TLS_DIR: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "vmess"))]
async fn relays_udp_through_vmess_to_vmess_relay_chain() {
    let echo_port = free_udp_port();
    let first_hop_port = free_port();
    let final_hop_port = free_port();
    let outer_port = free_port();
    let tls = test_tls_material();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        for _ in 0..2 {
            let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
            socket
                .send_to(&buf[..read], peer)
                .await
                .expect("send udp echo");
        }
    });

    let first_hop_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "first-vmess-in",
                    "listen": {{ "address": "127.0.0.1", "port": {first_hop_port} }},
                    "protocol": {{
                        "type": "vmess",
                        "users": [
                            {{
                                "id": "{USER_ID_A}",
                                "cipher": "chacha20-poly1305",
                                "principal_key": "node:first-vmess"
                            }}
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
    .expect("parse first hop config");
    let first_hop_handle = spawn_engine(Engine::new(first_hop_config).expect("build first hop"));
    wait_for_listener(first_hop_port).await;

    let final_hop_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "final-vmess-in",
                    "listen": {{ "address": "127.0.0.1", "port": {final_hop_port} }},
                    "protocol": {{
                        "type": "vmess",
                        "users": [
                            {{
                                "id": "{USER_ID_B}",
                                "cipher": "chacha20-poly1305",
                                "principal_key": "node:final-vmess"
                            }}
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
    .expect("parse final hop config");
    let final_hop_handle = spawn_engine(Engine::new(final_hop_config).expect("build final hop"));
    wait_for_listener(final_hop_port).await;

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
                    "tag": "first-vmess",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {first_hop_port},
                        "id": "{USER_ID_A}",
                        "cipher": "chacha20-poly1305",
                        "tls": {{
                            "server_name": "localhost",
                            "ca_cert_path": "{}"
                        }}
                    }}
                }},
                {{
                    "tag": "final-vmess",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {final_hop_port},
                        "id": "{USER_ID_B}",
                        "cipher": "chacha20-poly1305",
                        "tls": {{
                            "server_name": "localhost",
                            "ca_cert_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "vmess-relay-chain",
                    "type": "relay",
                    "proxies": ["first-vmess", "final-vmess"]
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "vmess-relay-chain" }}
            }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.cert_path),
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_probe = outer_engine.clone();
    let outer_handle = spawn_engine(outer_engine);
    wait_for_listener(outer_port).await;

    let mut control = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, // UDP ASSOCIATE + IPv4
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut associate = [0_u8; 10];
    control
        .read_exact(&mut associate)
        .await
        .expect("read udp associate response");
    assert_eq!(associate[1], 0x00);
    let relay_port = u16::from_be_bytes([associate[8], associate[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    send_and_assert_udp_echo(&client, relay_port, echo_port, b"vmess-chain").await;
    send_and_assert_udp_echo(&client, relay_port, echo_port, b"vmess-again").await;

    wait_for(
        "outer udp session to record vmess relay chain outbound",
        || {
            outer_probe
                .active_sessions()
                .first()
                .map(|session| {
                    session.network == zero_core::Network::Udp
                        && session.outbound_tag.as_deref() == Some("final-vmess")
                        && session.protocol == zero_core::ProtocolType::Socks5
                        && session.bytes_up > 0
                        && session.bytes_down > 0
                })
                .unwrap_or(false)
        },
    )
    .await;

    drop(control);
    wait_for("outer udp vmess relay chain session to complete", || {
        outer_probe
            .completed_sessions()
            .first()
            .map(|session| {
                session.network == zero_core::Network::Udp
                    && session.outbound_tag.as_deref() == Some("final-vmess")
                    && session.outcome.kind() == "chained_relayed"
                    && session.bytes_up > 0
                    && session.bytes_down > 0
            })
            .unwrap_or(false)
    })
    .await;

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    final_hop_handle
        .shutdown()
        .await
        .expect("shutdown final hop engine");
    first_hop_handle
        .shutdown()
        .await
        .expect("shutdown first hop engine");
    let _ = echo_task.await;
}

struct TestTlsMaterial {
    dir: PathBuf,
    cert_path: PathBuf,
    key_path: PathBuf,
}

impl Drop for TestTlsMaterial {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn test_tls_material() -> TestTlsMaterial {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
        .expect("generate self-signed cert");
    let unique = NEXT_TLS_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "zero-vmess-udp-chain-tls-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos(),
        unique
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
    }
}

fn escape_json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

async fn send_and_assert_udp_echo(
    client: &UdpSocket,
    relay_port: u16,
    echo_port: u16,
    payload: &[u8],
) {
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, payload)
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, payload);
}
