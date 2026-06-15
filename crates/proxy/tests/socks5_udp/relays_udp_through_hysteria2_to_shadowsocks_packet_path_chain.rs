use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

const PASSWORD: &str = "test-password";
static NEXT_TLS_DIR: AtomicUsize = AtomicUsize::new(1);

/// Regression coverage for the Hysteria2 QUIC packet-path carrier
/// (`udp_relay_chain_quic_path`): a relay group `[hysteria2-hop, ss-final]`
/// must carry inner Shadowsocks UDP datagrams over a Hysteria2 QUIC
/// connection. UDP enters via the outer SOCKS5, is encoded as an inner SS
/// datagram, carried by the Hysteria2 QUIC carrier to the Hysteria2 server,
/// forwarded to the inner Shadowsocks inbound, and finally reaches the echo
/// target. The second packet exercises the cached carrier connection.
#[tokio::test]
#[cfg(all(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
async fn relays_udp_through_hysteria2_to_shadowsocks_packet_path_chain() {
    let tls = test_tls_material();
    let echo_port = free_udp_port();
    let ss_port = free_port();
    let h2_port = free_port();
    let outer_port = free_port();

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

    // Inner Shadowsocks inbound (final hop) — decodes the inner SS datagram
    // and forwards the payload to the echo target.
    let ss_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "ss-in",
                    "listen": {{ "address": "127.0.0.1", "port": {ss_port} }},
                    "protocol": {{
                        "type": "shadowsocks",
                        "password": "{PASSWORD}",
                        "cipher": "chacha20-ietf-poly1305"
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#
    ))
    .expect("parse ss config");
    let ss_engine = Engine::new(ss_config).expect("build ss engine");
    let ss_handle = spawn_engine(ss_engine);
    wait_for_listener(ss_port).await;

    // Hysteria2 inbound (carrier server) — forwards UDP datagrams to the
    // address encoded in each datagram (here: the inner SS server).
    let h2_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "h2-in",
                    "listen": {{ "address": "127.0.0.1", "port": {h2_port} }},
                    "protocol": {{
                        "type": "hysteria2",
                        "password": "{PASSWORD}",
                        "cert_path": "{}",
                        "key_path": "{}"
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse h2 config");
    let h2_engine = Engine::new(h2_config).expect("build h2 engine");
    let h2_handle = spawn_engine(h2_engine);
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Outer SOCKS5 with relay group [hysteria2-hop, ss-final].
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
                    "tag": "hysteria2-hop",
                    "protocol": {{
                        "type": "hysteria2",
                        "server": "127.0.0.1",
                        "port": {h2_port},
                        "password": "{PASSWORD}",
                        "insecure": true
                    }}
                }},
                {{
                    "tag": "ss-final",
                    "protocol": {{
                        "type": "shadowsocks",
                        "server": "127.0.0.1",
                        "port": {ss_port},
                        "password": "{PASSWORD}",
                        "cipher": "chacha20-ietf-poly1305"
                    }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "udp-relay-chain",
                    "type": "relay",
                    "proxies": ["hysteria2-hop", "ss-final"]
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "udp-relay-chain" }}
            }}
        }}"#
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_probe = outer_engine.clone();
    let outer_handle = spawn_engine(outer_engine);
    wait_for_listener(outer_port).await;

    let mut control = timeout(
        Duration::from_secs(3),
        TcpStream::connect(("127.0.0.1", outer_port)),
    )
    .await
    .expect("connect outer proxy timeout")
    .expect("connect outer proxy");
    timeout(
        Duration::from_secs(3),
        control.write_all(&[0x05, 0x01, 0x00]),
    )
    .await
    .expect("write auth timeout")
    .expect("write auth");

    let mut auth = [0_u8; 2];
    timeout(Duration::from_secs(3), control.read_exact(&mut auth))
        .await
        .expect("read auth timeout")
        .expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    timeout(
        Duration::from_secs(3),
        control.write_all(&[
            0x05, 0x03, 0x00, 0x01, // udp associate + ipv4
            0, 0, 0, 0, 0x00, 0x00,
        ]),
    )
    .await
    .expect("write udp associate timeout")
    .expect("write udp associate");

    let mut response = [0_u8; 10];
    timeout(Duration::from_secs(3), control.read_exact(&mut response))
        .await
        .expect("read udp associate response timeout")
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"h2ss1")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = match timeout(Duration::from_secs(5), client.recv_from(&mut buf)).await {
        Ok(result) => result.expect("recv udp response"),
        Err(error) => {
            panic!(
                "udp recv timeout: {error}; active={:?}; completed={:?}; stats={:?}",
                outer_probe.active_sessions(),
                outer_probe.completed_sessions(),
                outer_probe.stats_snapshot()
            );
        }
    };
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");
    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"h2ss1");

    // Second packet exercises the cached carrier QUIC connection.
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"h2ss2")
        .expect("build second udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send second udp packet");
    let (read, _) = timeout(Duration::from_secs(5), client.recv_from(&mut buf))
        .await
        .expect("second udp recv timeout")
        .expect("recv second udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse second udp response");
    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"h2ss2");

    wait_for(
        "outer udp session to record ss-final carrier chain outbound",
        || {
            outer_probe
                .active_sessions()
                .first()
                .map(|session| {
                    session.network == zero_core::Network::Udp
                        && session.outbound_tag.as_deref() == Some("ss-final")
                        && session.bytes_up > 0
                        && session.bytes_down > 0
                })
                .unwrap_or(false)
        },
    )
    .await;

    drop(control);
    wait_for("outer udp carrier chain session to complete", || {
        outer_probe
            .completed_sessions()
            .first()
            .map(|session| {
                session.network == zero_core::Network::Udp
                    && session.outbound_tag.as_deref() == Some("ss-final")
                    && session.outcome.kind() == "chained_relayed"
                    && session.bytes_up > 0
                    && session.bytes_down > 0
            })
            .unwrap_or(false)
    })
    .await;

    timeout(Duration::from_secs(3), outer_handle.shutdown())
        .await
        .expect("shutdown outer engine timeout")
        .expect("shutdown outer engine");
    timeout(Duration::from_secs(3), h2_handle.shutdown())
        .await
        .expect("shutdown h2 engine timeout")
        .expect("shutdown h2 engine");
    timeout(Duration::from_secs(3), ss_handle.shutdown())
        .await
        .expect("shutdown ss engine timeout")
        .expect("shutdown ss engine");
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
        "zero-h2-ss-carrier-tls-{}-{}-{}",
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
