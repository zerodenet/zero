use super::*;

#[tokio::test]
async fn rejects_blocked_domain_via_route_rule() {
    let proxy_port = free_port();

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{ "type": "socks5" }}
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
    let probe = engine.clone();
    let engine_handle = spawn_engine(engine);

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
    assert_eq!(auth, [0x05, 0x00]);

    let mut request = vec![0x05, 0x01, 0x00, 0x03, 0x0f];
    request.extend_from_slice(b"blocked.example");
    request.extend_from_slice(&443_u16.to_be_bytes());
    client.write_all(&request).await.expect("write request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response[1], 0x02);

    wait_for("blocked flow completion", || {
        !probe.completed_sessions().is_empty()
    })
    .await;
    let events = probe
        .subscribe(EventFilter {
            event_types: vec![event_type::FLOW_COMPLETED.to_owned()],
            ..EventFilter::default()
        })
        .expect("read completed event");
    let record = &events[0].payload["record"];
    assert_eq!(record["route"]["action"], "reject");
    assert_eq!(record["route"]["matched_rule"]["index"], 0);
    assert_eq!(
        record["route"]["matched_rule"]["condition"],
        "domain: blocked.example"
    );
    assert_eq!(record["result"]["outcome"], "blocked");

    engine_handle.shutdown().await.expect("shutdown engine");
}
