use super::*;

#[tokio::test]
async fn vless_tls_with_alpn_config() {
    let echo_port = free_port();
    let proxy_port = free_port();
    let tls = test_tls_material();

    let echo_task = spawn_echo_server(echo_port).await;

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vless-tls-alpn-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{
                        "type": "vless",
                        "users": [{{ "id": "{USER_ID}" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}",
                            "alpn": ["http/1.1", "h2"]
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

    let mut roots = rustls::RootCertStore::empty();
    roots.add(tls.cert_der.clone()).expect("trust test cert");
    let mut client_config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let connector = TlsConnector::from(Arc::new(client_config));
    let server_name = rustls::pki_types::ServerName::try_from("localhost")
        .expect("server name")
        .to_owned();
    let stream = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");

    let mut tls_stream = connector
        .connect(server_name, stream)
        .await
        .expect("tls handshake with alpn");

    tls_stream
        .write_all(&vless_request_for_ipv4(USER_ID, [127, 0, 0, 1], echo_port))
        .await
        .expect("write vless request");

    let mut response = [0_u8; 2];
    tls_stream
        .read_exact(&mut response)
        .await
        .expect("read vless response");
    assert_eq!(response, [0x00, 0x00]);

    tls_stream.write_all(b"alpn").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    tls_stream
        .read_exact(&mut echoed)
        .await
        .expect("read payload");
    assert_eq!(&echoed, b"alpn");

    engine_handle.shutdown().await.expect("shutdown engine");
    echo_task.await.expect("echo task");
}
