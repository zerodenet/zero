mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout, Duration};
use zero_config::RuntimeConfig;
use zero_engine::Engine;

use support::{
    free_port, spawn_engine, spawn_http_probe_server, wait_for_group_selection, wait_for_listener,
};

#[tokio::test]
async fn relays_tcp_through_socks5_direct_outbound() {
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

    client.write_all(b"ping").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"ping");

    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

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

    engine_handle.shutdown().await.expect("shutdown engine");
}

#[tokio::test]
async fn relays_tcp_through_socks5_chained_outbound() {
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
                    "tag": "upstream-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{ "type": "socks5" }}
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
    let upstream_engine = Engine::new(upstream_config).expect("build upstream engine");
    let upstream_handle = spawn_engine(upstream_engine);

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
                    "tag": "chain",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {upstream_port}
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "chain" }}
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

    client.write_all(b"mesh").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"mesh");

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

#[tokio::test]
async fn relays_tcp_through_selector_group_in_global_mode() {
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
                    "tag": "upstream-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{ "type": "socks5" }}
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
    let upstream_engine = Engine::new(upstream_config).expect("build upstream engine");
    let upstream_handle = spawn_engine(upstream_engine);

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
                    "tag": "node-a",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {upstream_port}
                    }}
                }},
                {{
                    "tag": "node-b",
                    "protocol": {{ "type": "block" }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["node-a", "node-b"],
                    "selected": "node-a"
                }}
            ],
            "mode": {{
                "type": "global",
                "outbound": "proxy"
            }},
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
                "final": {{ "type": "reject" }}
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

    client.write_all(b"mode").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"mode");

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

#[tokio::test]
async fn relays_tcp_through_fallback_group_when_primary_unreachable() {
    let echo_port = free_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });

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
                    "tag": "chain-a",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {unreachable_port}
                    }}
                }},
                {{
                    "tag": "direct",
                    "protocol": {{ "type": "direct" }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "proxy",
                    "type": "fallback",
                    "outbounds": ["chain-a", "direct"]
                }}
            ],
            "mode": {{
                "type": "global",
                "outbound": "proxy"
            }},
            "route": {{
                "rules": [],
                "final": {{ "type": "reject" }}
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

    client.write_all(b"back").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"back");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_tcp_through_urltest_group_after_probe_selects_direct() {
    let echo_port = free_port();
    let probe_port = free_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });
    let probe_task = spawn_http_probe_server(probe_port);
    wait_for_listener(probe_port).await;

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
                    "tag": "chain-a",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {unreachable_port}
                    }}
                }},
                {{
                    "tag": "direct",
                    "protocol": {{ "type": "direct" }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "proxy",
                    "type": "urltest",
                    "outbounds": ["chain-a", "direct"],
                    "url": "http://127.0.0.1:{probe_port}/",
                    "interval_seconds": 1
                }}
            ],
            "mode": {{
                "type": "global",
                "outbound": "proxy"
            }},
            "route": {{
                "rules": [],
                "final": {{ "type": "reject" }}
            }}
        }}"#
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_handle = spawn_engine(outer_engine);

    wait_for_listener(outer_port).await;
    wait_for_group_selection(&outer_handle, "proxy", "direct").await;

    let status = outer_handle.export_status();
    let group = status
        .config
        .outbound_groups
        .iter()
        .find(|group| group.tag == "proxy")
        .expect("find urltest group");
    assert_eq!(group.selected.as_deref(), Some("direct"));
    assert!(group.latency_ms.is_some());
    assert!(group.last_checked_unix_ms.is_some());
    assert_eq!(
        group.effective_chains,
        vec![vec!["proxy".to_owned(), "direct".to_owned()]]
    );
    assert_eq!(group.urltest_members.len(), 2);
    let chain_a = group
        .urltest_members
        .iter()
        .find(|member| member.member_tag == "chain-a")
        .expect("find chain-a probe");
    assert!(!chain_a.healthy);
    assert!(chain_a.last_error.is_some());
    let direct = group
        .urltest_members
        .iter()
        .find(|member| member.member_tag == "direct")
        .expect("find direct probe");
    assert!(direct.healthy);
    assert!(direct.latency_ms.is_some());
    assert_eq!(direct.effective_chains, vec![vec!["direct".to_owned()]]);

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

    client.write_all(b"fast").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"fast");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    probe_task.abort();
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_tcp_through_urltest_group_with_nested_group_member() {
    let echo_port = free_port();
    let probe_port = free_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });
    let probe_task = spawn_http_probe_server(probe_port);
    wait_for_listener(probe_port).await;

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
                    "tag": "chain-a",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {unreachable_port}
                    }}
                }},
                {{
                    "tag": "direct",
                    "protocol": {{ "type": "direct" }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "fallback-proxy",
                    "type": "fallback",
                    "outbounds": ["chain-a", "direct"]
                }},
                {{
                    "tag": "proxy",
                    "type": "urltest",
                    "outbounds": ["fallback-proxy"],
                    "url": "http://127.0.0.1:{probe_port}/",
                    "interval_seconds": 1
                }}
            ],
            "mode": {{
                "type": "global",
                "outbound": "proxy"
            }},
            "route": {{
                "rules": [],
                "final": {{ "type": "reject" }}
            }}
        }}"#
    ))
    .expect("parse outer config");
    let outer_engine = Engine::new(outer_config).expect("build outer engine");
    let outer_handle = spawn_engine(outer_engine);

    wait_for_listener(outer_port).await;
    wait_for_group_selection(&outer_handle, "proxy", "fallback-proxy").await;
    timeout(Duration::from_secs(3), async {
        loop {
            let status = outer_handle.export_status();
            let group = status
                .config
                .outbound_groups
                .iter()
                .find(|group| group.tag == "proxy")
                .expect("find urltest group");

            if group.selected.as_deref() == Some("fallback-proxy") && group.latency_ms.is_some() {
                break;
            }

            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("wait for nested urltest probe");

    let status = outer_handle.export_status();
    let group = status
        .config
        .outbound_groups
        .iter()
        .find(|group| group.tag == "proxy")
        .expect("find nested urltest group");
    assert_eq!(group.selected.as_deref(), Some("fallback-proxy"));
    assert_eq!(
        group.effective_chains,
        vec![
            vec![
                "proxy".to_owned(),
                "fallback-proxy".to_owned(),
                "chain-a".to_owned(),
            ],
            vec![
                "proxy".to_owned(),
                "fallback-proxy".to_owned(),
                "direct".to_owned(),
            ],
        ]
    );
    let nested_member = group
        .urltest_members
        .iter()
        .find(|member| member.member_tag == "fallback-proxy")
        .expect("find nested member probe");
    assert!(nested_member.healthy);
    assert!(nested_member.latency_ms.is_some());
    assert_eq!(
        nested_member.effective_chains,
        vec![
            vec!["fallback-proxy".to_owned(), "chain-a".to_owned()],
            vec!["fallback-proxy".to_owned(), "direct".to_owned()],
        ]
    );

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

    client.write_all(b"nest").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"nest");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    probe_task.abort();
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_tcp_through_nested_group_target() {
    let echo_port = free_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });

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
                    "tag": "chain-a",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": {unreachable_port}
                    }}
                }},
                {{
                    "tag": "direct",
                    "protocol": {{ "type": "direct" }}
                }}
            ],
            "outbound_groups": [
                {{
                    "tag": "fallback-proxy",
                    "type": "fallback",
                    "outbounds": ["chain-a", "direct"]
                }},
                {{
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["fallback-proxy", "direct"],
                    "selected": "fallback-proxy"
                }}
            ],
            "mode": {{
                "type": "global",
                "outbound": "proxy"
            }},
            "route": {{
                "rules": [],
                "final": {{ "type": "reject" }}
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

    client.write_all(b"nest").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"nest");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    let _ = echo_task.await;
}
