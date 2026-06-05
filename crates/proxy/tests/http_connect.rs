mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use zero_config::RuntimeConfig;
use zero_proxy::Proxy as Engine;

use support::{free_port, spawn_engine, wait_for_listener};

#[tokio::test]
async fn relays_tcp_through_http_connect_direct_outbound() {
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
                    "tag": "http-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{ "type": "http_connect" }}
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

    let mut client = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
    let request =
        format!("CONNECT 127.0.0.1:{echo_port} HTTP/1.1\r\nHost: 127.0.0.1:{echo_port}\r\n\r\n");
    client
        .write_all(request.as_bytes())
        .await
        .expect("write request");

    let mut response = vec![0_u8; 39];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(&response, b"HTTP/1.1 200 Connection Established\r\n\r\n");

    client.write_all(b"pong").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"pong");

    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn rejects_http_connect_blocked_domain_via_route_rule() {
    let proxy_port = free_port();

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "http-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{ "type": "http_connect" }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [
                    {{
                        "condition": {{
                            "type": "domain",
                            "values": ["blocked.example"]
                        }},
                        "action": {{ "type": "reject" }}
                    }}
                ],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    ))
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut client = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
    let request = "CONNECT blocked.example:443 HTTP/1.1\r\nHost: blocked.example:443\r\n\r\n";
    client
        .write_all(request.as_bytes())
        .await
        .expect("write request");

    let mut response = vec![0_u8; 64];
    let read = client.read(&mut response).await.expect("read response");
    let response = &response[..read];
    assert_eq!(
        response,
        b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );

    engine_handle.shutdown().await.expect("shutdown engine");
}
