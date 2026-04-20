mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use zero_config::RuntimeConfig;
use zero_engine::Engine;

use support::{free_port, spawn_engine, wait_for_listener};

#[tokio::test]
async fn exports_serializable_engine_status_view() {
    let echo_port = free_port();
    let proxy_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "chain",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    ))
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    let handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut client = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
    client
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    client.read_exact(&mut auth).await.expect("read auth");

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

    let exported = handle.export_status();
    assert_eq!(exported.config.rule_count, 0);
    assert_eq!(exported.config.inbounds.len(), 1);
    assert_eq!(exported.config.inbounds[0].tag, "socks-in");
    assert_eq!(exported.config.inbounds[0].protocol, "socks5");
    assert_eq!(exported.config.outbounds.len(), 1);
    assert_eq!(exported.config.outbounds[0].tag, "chain");
    assert_eq!(exported.config.outbounds[0].protocol, "socks5");
    assert_eq!(exported.runtime.active_sessions.len(), 1);
    assert_eq!(exported.runtime.active_sessions[0].target.family, "ipv4");
    assert_eq!(exported.runtime.active_sessions[0].protocol, "socks5");

    let json = serde_json::to_value(&exported).expect("serialize export");
    assert_eq!(json["config"]["inbounds"][0]["tag"], "socks-in");
    assert_eq!(json["config"]["outbounds"][0]["server"], "127.0.0.1");
    assert_eq!(
        json["runtime"]["active_sessions"][0]["target"]["family"],
        "ipv4"
    );

    drop(client);
    handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}
