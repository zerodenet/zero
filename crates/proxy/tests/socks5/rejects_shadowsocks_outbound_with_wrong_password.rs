use super::*;

use std::io::ErrorKind;

const SERVER_PASSWORD: &str = "server-password";
const CLIENT_PASSWORD: &str = "client-password";
const CIPHER: &str = "aes-256-gcm";

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
async fn rejects_shadowsocks_outbound_with_wrong_password() {
    let echo_port = free_port();
    let upstream_port = free_port();
    let outer_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let accept_result = timeout(Duration::from_secs(1), listener.accept()).await;
        assert!(
            accept_result.is_err(),
            "wrong-password shadowsocks flow must not reach target"
        );
    });

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-ss-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "shadowsocks",
                        "password": "{SERVER_PASSWORD}",
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
                        "password": "{CLIENT_PASSWORD}",
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
        .write_all(b"must-not-arrive")
        .await
        .expect("write payload");
    let mut echoed = [0_u8; 1];
    let read_result = timeout(Duration::from_secs(3), client.read(&mut echoed))
        .await
        .expect("wrong-password flow should close in time");
    match read_result {
        Ok(0) => {}
        Err(error) if error.kind() == ErrorKind::ConnectionReset => {}
        Ok(read) => panic!("wrong-password flow returned {read} bytes"),
        Err(error) => panic!("unexpected wrong-password close error: {error}"),
    }

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
