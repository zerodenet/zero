use super::*;

#[tokio::test]
async fn relays_tcp_through_vless_tls_direct_outbound() {
    let echo_port = free_port();
    let proxy_port = free_port();
    let tls = test_tls_material();

    let echo_task = spawn_echo_server(echo_port).await;

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vless-tls-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [
                            {{ "id": "{USER_ID}", "principal_key": "user:tls" }}
                        ],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
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
    .expect("parse engine config");

    let engine = Engine::new(config).expect("build engine");
    let engine_handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut client = connect_tls_client(proxy_port, tls.cert_der.clone()).await;
    client
        .write_all(&vless_request_for_ipv4(USER_ID, [127, 0, 0, 1], echo_port))
        .await
        .expect("write request");

    let mut response = [0_u8; 2];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response, [0x00, 0x00]);

    client.write_all(b"vtls").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"vtls");

    drop(client);
    wait_for("completed vless tls flow", || {
        !engine_handle.completed_sessions().is_empty()
    })
    .await;

    let completed = engine_handle.completed_sessions();
    assert_eq!(
        completed[0]
            .auth
            .as_ref()
            .and_then(|auth| auth.principal_key.as_deref()),
        Some("user:tls")
    );

    engine_handle.shutdown().await.expect("shutdown engine");
    echo_task.await.expect("echo task");
}
