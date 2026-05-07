use super::*;

#[tokio::test]
async fn relays_tcp_through_vless_ws_inbound() {
    let echo_port = free_port();
    let proxy_port = free_port();

    let echo_task = spawn_echo_server(echo_port).await;

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vless-ws-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [{{ "id": "{USER_ID}", "principal_key": "user:ws" }}],
                        "ws": {{
                            "path": "/vless"
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
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let stream = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");

    let (mut ws_stream, _) = tokio_tungstenite::client_async("ws://localhost/vless", stream)
        .await
        .expect("websocket handshake");

    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            vless_request_for_ipv4(USER_ID, [127, 0, 0, 1], echo_port),
        ))
        .await
        .expect("write vless request over ws");

    match ws_stream.next().await.expect("read vless response") {
        Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
            assert_eq!(data, [0x00, 0x00]);
        }
        other => panic!("unexpected ws message: {other:?}"),
    }

    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            b"vles".to_vec(),
        ))
        .await
        .expect("write payload over ws");

    match ws_stream.next().await.expect("read echoed payload") {
        Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
            assert_eq!(&data, b"vles");
        }
        other => panic!("unexpected ws message: {other:?}"),
    }

    drop(ws_stream);
    wait_for("completed vless ws flow", || {
        !engine_handle.completed_sessions().is_empty()
    })
    .await;

    let completed = engine_handle.completed_sessions();
    assert_eq!(
        completed[0]
            .auth
            .as_ref()
            .and_then(|auth| auth.principal_key.as_deref()),
        Some("user:ws")
    );

    engine_handle.shutdown().await.expect("shutdown engine");
    echo_task.await.expect("echo task");
}
