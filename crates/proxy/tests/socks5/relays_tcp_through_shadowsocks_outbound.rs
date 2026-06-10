use super::*;

const CIPHERS: &[&str] = &[
    "aes-128-gcm",
    "aes-256-gcm",
    "chacha20-ietf-poly1305",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
];

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
async fn relays_tcp_through_shadowsocks_outbound() {
    for cipher in CIPHERS {
        relays_tcp_through_shadowsocks_outbound_for_cipher(cipher).await;
    }
}

async fn relays_tcp_through_shadowsocks_outbound_for_cipher(cipher: &str) {
    let password = password_for_cipher(cipher);
    let echo_port = free_port();
    let upstream_port = free_port();
    let outer_port = free_port();
    let payload = format!("tcp:{cipher}");

    let expected = payload.clone();
    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = vec![0_u8; expected.len()];
        stream.read_exact(&mut buf).await.expect("read echo");
        assert_eq!(buf, expected.as_bytes());
        stream.write_all(&buf).await.expect("write echo");
    });

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-ss-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "shadowsocks",
                        "password": "{password}",
                        "cipher": "{cipher}"
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
                    "tag": "ss-out",
                    "protocol": {{
                        "type": "shadowsocks",
                        "server": "127.0.0.1",
                        "port": {upstream_port},
                        "password": "{password}",
                        "cipher": "{cipher}"
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "ss-out" }}
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
    assert_eq!(response[1], 0x00, "cipher: {cipher}");

    client
        .write_all(payload.as_bytes())
        .await
        .expect("write payload");
    let mut echoed = vec![0_u8; payload.len()];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(echoed, payload.as_bytes(), "cipher: {cipher}");

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

fn password_for_cipher(cipher: &str) -> &'static str {
    match cipher {
        "2022-blake3-aes-128-gcm" => "MDEyMzQ1Njc4OWFiY2RlZg==",
        "2022-blake3-aes-256-gcm" | "2022-blake3-chacha20-poly1305" => {
            "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY="
        }
        _ => "test-password",
    }
}
