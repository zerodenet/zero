use super::*;

#[tokio::test]
async fn rejects_udp_when_inbound_policy_disabled() {
    let echo_port = free_udp_port();
    let proxy_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        if let Ok(Ok((read, peer))) =
            timeout(Duration::from_millis(300), socket.recv_from(&mut buf)).await
        {
            let _ = socket.send_to(&buf[..read], peer).await;
        }
    });

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "udp": {{ "enabled": false }},
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
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut control = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
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
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"ping")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    assert!(
        timeout(Duration::from_millis(300), client.recv_from(&mut buf))
            .await
            .is_err(),
        "udp response should be suppressed when inbound udp is disabled"
    );

    drop(control);
    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}
