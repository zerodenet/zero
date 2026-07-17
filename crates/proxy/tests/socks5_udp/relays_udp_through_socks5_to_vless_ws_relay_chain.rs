use super::*;

const USER_ID: &str = "11111111-2222-3333-4444-555555555555";

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "vless"))]
async fn relays_udp_through_socks5_to_vless_ws_relay_chain() {
    let echo_port = free_udp_port();
    let first_hop_port = free_port();
    let final_hop_port = free_port();
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

    let first_hop_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "first-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {first_hop_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    ))
    .expect("parse first hop config");
    let first_hop_engine = Engine::new(first_hop_config).expect("build first hop engine");
    let first_hop_handle = spawn_engine(first_hop_engine);

    wait_for_listener(first_hop_port).await;

    let final_hop_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "final-vless-ws-in",
                    "listen": {{ "address": "127.0.0.1", "port": {final_hop_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [
                            {{ "id": "{USER_ID}", "principal_key": "node:final-vless-ws" }}
                        ],
                        "ws": {{
                            "path": "/vless-udp-relay"
                        }}
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
    .expect("parse final hop config");
    let final_hop_engine = Engine::new(final_hop_config).expect("build final hop engine");
    let final_hop_handle = spawn_engine(final_hop_engine);

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
                    "tag": "first-socks",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {first_hop_port}
                    }}
                }},
                {{
                    "tag": "final-vless-ws",
                    "protocol": {{
                        "type": "vless",
                        "server": "127.0.0.1",
                        "port": {final_hop_port},
                        "id": "{USER_ID}",
                        "ws": {{
                            "path": "/vless-udp-relay"
                        }}
                    }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "udp-relay-chain",
                    "type": "relay",
                    "proxies": ["first-socks", "final-vless-ws"]
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
            0x05, 0x03, 0x00, 0x01, // udp associate + ipv4
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"ws1")
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
    assert_eq!(response.payload, b"ws1");

    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"ws2")
        .expect("build second udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send second udp packet");

    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("second udp recv timeout")
        .expect("recv second udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse second udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"ws2");

    wait_for(
        "outer udp session to record vless ws relay chain outbound",
        || {
            outer_probe
                .active_sessions()
                .first()
                .map(|session| {
                    session.network == zero_core::Network::Udp
                        && session.outbound_tag.as_deref() == Some("final-vless-ws")
                        && session.protocol == zero_core::ProtocolType::new("socks5")
                        && session.bytes_up > 0
                        && session.bytes_down > 0
                })
                .unwrap_or(false)
        },
    )
    .await;

    drop(control);
    wait_for("outer udp vless ws relay chain session to complete", || {
        outer_probe
            .completed_sessions()
            .first()
            .map(|session| {
                session.network == zero_core::Network::Udp
                    && session.outbound_tag.as_deref() == Some("final-vless-ws")
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
