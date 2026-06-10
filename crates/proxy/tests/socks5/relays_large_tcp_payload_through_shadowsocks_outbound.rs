use super::*;

const PASSWORD: &str = "test-password";
const CIPHER: &str = "chacha20-ietf-poly1305";
const PAYLOAD_LEN: usize = 96 * 1024;

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
async fn relays_large_tcp_payload_through_shadowsocks_outbound() {
    let echo_port = free_port();
    let upstream_port = free_port();
    let outer_port = free_port();

    let payload = deterministic_payload(PAYLOAD_LEN);
    let expected = payload.clone();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = vec![0_u8; expected.len()];
        stream.read_exact(&mut buf).await.expect("read large echo");
        assert_eq!(buf, expected);
        stream.write_all(&buf).await.expect("write large echo");
    });

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-ss-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "shadowsocks",
                        "password": "{PASSWORD}",
                        "cipher": "{CIPHER}"
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
    let upstream_handle = spawn_engine(Engine::new(upstream_config).expect("build upstream"));
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
                        "password": "{PASSWORD}",
                        "cipher": "{CIPHER}"
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
    let outer_handle = spawn_engine(Engine::new(outer_config).expect("build outer"));
    wait_for_listener(outer_port).await;

    let mut client = socks5_connect(outer_port, echo_port).await;
    client
        .write_all(&payload)
        .await
        .expect("write large payload");
    client.shutdown().await.expect("shutdown upload");

    let mut echoed = Vec::new();
    client
        .read_to_end(&mut echoed)
        .await
        .expect("read large payload");
    assert_eq!(echoed, payload);

    outer_handle.shutdown().await.expect("shutdown outer");
    upstream_handle.shutdown().await.expect("shutdown upstream");
    let _ = echo_task.await;
}

async fn socks5_connect(proxy_port: u16, target_port: u16) -> TcpStream {
    let mut client = TcpStream::connect(("127.0.0.1", proxy_port))
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
        ((target_port >> 8) & 0xff) as u8,
        (target_port & 0xff) as u8,
    ];
    client.write_all(&request).await.expect("write request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response[1], 0x00);

    client
}

fn deterministic_payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * 31 + 7) % 251) as u8).collect()
}
