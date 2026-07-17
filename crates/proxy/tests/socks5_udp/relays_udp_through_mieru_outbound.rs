use super::*;

const USERNAME: &str = "alice";
const PASSWORD: &str = "secret";

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "mieru"))]
async fn relays_udp_through_mieru_outbound() {
    let echo_port = free_udp_port();
    let upstream_port = free_port();
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

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-mieru-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "mieru",
                        "users": [
                            {{ "username": "{USERNAME}", "password": "{PASSWORD}" }}
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
                    "tag": "mieru-udp-chain",
                    "protocol": {{
                        "type": "mieru",
                        "server": "127.0.0.1",
                        "port": {upstream_port},
                        "username": "{USERNAME}",
                        "password": "{PASSWORD}"
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "mieru-udp-chain" }}
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
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"miup")
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
    assert_eq!(response.payload, b"miup");

    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"miu2")
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
    assert_eq!(response.payload, b"miu2");

    wait_for("outer udp session to record mieru outbound", || {
        outer_probe
            .active_sessions()
            .first()
            .map(|session| {
                session.network == zero_core::Network::Udp
                    && session.outbound_tag.as_deref() == Some("mieru-udp-chain")
                    && session.protocol == zero_core::ProtocolType::new("socks5")
                    && session.bytes_up > 0
                    && session.bytes_down > 0
            })
            .unwrap_or(false)
    })
    .await;

    drop(control);
    wait_for("outer udp mieru session to complete", || {
        outer_probe
            .completed_sessions()
            .first()
            .map(|session| {
                session.network == zero_core::Network::Udp
                    && session.outbound_tag.as_deref() == Some("mieru-udp-chain")
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
    timeout(Duration::from_secs(3), upstream_handle.shutdown())
        .await
        .expect("shutdown upstream engine timeout")
        .expect("shutdown upstream engine");
    timeout(Duration::from_secs(3), echo_task)
        .await
        .expect("join echo timeout")
        .expect("join echo task");
}
