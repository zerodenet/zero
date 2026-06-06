use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

const PASSWORD: &str = "test-password";
static NEXT_TLS_DIR: AtomicUsize = AtomicUsize::new(1);

#[tokio::test]
#[cfg(all(feature = "socks5", feature = "hysteria2"))]
async fn relays_tcp_through_hysteria2_outbound() {
    let tls = test_tls_material();
    let echo_port = free_port();
    let upstream_port = free_port();
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

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-hysteria2-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "hysteria2",
                        "password": "{PASSWORD}",
                        "cert_path": "{}",
                        "key_path": "{}"
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

    // Hysteria2 QUIC listener needs a bit more time to start.
    sleep(Duration::from_millis(100)).await;

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
                    "tag": "hysteria2-out",
                    "protocol": {{
                        "type": "hysteria2",
                        "server": "127.0.0.1",
                        "port": {upstream_port},
                        "password": "{PASSWORD}",
                        "insecure": true
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "hysteria2-out" }}
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

    client.write_all(b"h2tc").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"h2tc");

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

struct TestTlsMaterial {
    dir: std::path::PathBuf,
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
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
        "zero-hysteria2-tcp-tls-{}-{}-{}",
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
