use super::*;

const USERNAME: &str = "alice";
const PASSWORD: &str = "secret";

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "mieru"))]
async fn relays_tcp_through_socks5_to_mieru_to_socks5_relay_chain() {
    let echo_port = free_port();
    let first_hop_port = free_port();
    let final_hop_port = free_port();
    let outer_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });

    let first_hop_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "first-mieru-in",
                    "listen": {{ "address": "127.0.0.1", "port": {first_hop_port} }},
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
    .expect("parse first hop config");
    let first_hop_engine = Engine::new(first_hop_config).expect("build first hop engine");
    let first_hop_handle = spawn_engine(first_hop_engine);

    wait_for_listener(first_hop_port).await;

    let final_hop_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "final-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {final_hop_port} }},
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
                    "tag": "first-mieru",
                    "protocol": {{
                        "type": "mieru",
                        "server": "127.0.0.1",
                        "port": {first_hop_port},
                        "username": "{USERNAME}",
                        "password": "{PASSWORD}"
                    }}
                }},
                {{
                    "tag": "final-socks",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {final_hop_port}
                    }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "tcp-relay-chain",
                    "type": "relay",
                    "proxies": ["first-mieru", "final-socks"]
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "tcp-relay-chain" }}
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

    client.write_all(b"mhop").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"mhop");

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
