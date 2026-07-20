use super::*;

#[tokio::test]
async fn reports_upstream_connection_failure_to_client_and_session_history() {
    let proxy_port = free_port();
    let unavailable_port = free_port();
    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [{{
                "tag": "socks-failure-in",
                "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                "protocol": {{ "type": "socks5" }}
            }}],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#
    ))
    .expect("failure config");
    let engine = Engine::new(config).expect("build engine");
    let probe = engine.clone();
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
    assert_eq!(auth, [0x05, 0x00]);

    client
        .write_all(&[
            0x05,
            0x01,
            0x00,
            0x01,
            127,
            0,
            0,
            1,
            (unavailable_port >> 8) as u8,
            unavailable_port as u8,
        ])
        .await
        .expect("write connect request");
    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read failure response");
    assert_eq!(response[0], 0x05);
    assert_eq!(response[1], 0x04);

    support::wait_for("failed session history", || {
        probe.completed_sessions().first().is_some_and(|session| {
            session.inbound_tag.as_deref() == Some("socks-failure-in")
                && session.outcome.kind() == "failed"
                && session.close_reason.as_deref() == Some("upstream_error")
        })
    })
    .await;

    let events = probe
        .subscribe(EventFilter {
            event_types: vec![event_type::FLOW_COMPLETED.to_owned()],
            ..EventFilter::default()
        })
        .expect("read completed flow events");
    let completed = events.first().expect("completed flow event");
    assert_eq!(completed.payload["record"]["state"], "completed");
    assert_eq!(completed.payload["record"]["result"]["outcome"], "failed");
    assert_eq!(
        completed.payload["record"]["result"]["failure"]["stage"],
        "route_or_establish"
    );
    assert!(completed.payload["record"]["result"]["failure"]["code"]
        .as_str()
        .is_some());
    assert!(completed.payload["record"]["result"]["failure"]["message"]
        .as_str()
        .is_some_and(|message| !message.is_empty()));

    handle.shutdown().await.expect("shutdown engine");
}
