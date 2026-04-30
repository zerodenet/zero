#![cfg(feature = "inbound-socks5")]

mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{timeout, Duration};
use zero_config::RuntimeConfig;
use zero_core::Address;
use zero_protocol_socks5::{build_udp_packet, parse_udp_packet};
use zero_proxy::Proxy as Engine;

use support::{
    free_port, free_udp_port, spawn_engine, spawn_http_probe_server, wait_for,
    wait_for_group_selection, wait_for_listener,
};

#[tokio::test]
async fn relays_udp_through_socks5_udp_associate_direct_outbound() {
    let echo_port = free_udp_port();
    let proxy_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
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

    let mut control = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, // udp associate + ipv4
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"ping")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"ping");

    drop(control);
    engine_handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_udp_through_upstream_socks5_udp_associate() {
    let echo_port = free_udp_port();
    let upstream_port = free_port();
    let outer_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
    });

    let upstream_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "upstream-socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {upstream_port} }},
                    "protocol": {{
                        "type": "socks5",
                        "users": [
                            {{ "username": "upstream", "password": "secret" }}
                        ]
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
                        "port": {upstream_port},
                        "username": "upstream",
                        "password": "secret"
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
    let outer_probe = outer_engine.clone();
    let outer_handle = spawn_engine(outer_engine);

    wait_for_listener(outer_port).await;

    let mut control = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, // udp associate + ipv4
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"mesh")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"mesh");

    wait_for(
        "outer udp session to record upstream response bytes",
        || {
            outer_probe
                .active_sessions()
                .first()
                .map(|session| {
                    session.network == zero_core::Network::Udp
                        && session.outbound_tag.as_deref() == Some("chain")
                        && session.bytes_up == 71
                        && session.bytes_down == 54
                })
                .unwrap_or(false)
        },
    )
    .await;

    drop(control);
    wait_for("outer udp chained session to complete", || {
        outer_probe
            .completed_sessions()
            .first()
            .map(|session| {
                session.network == zero_core::Network::Udp
                    && session.outbound_tag.as_deref() == Some("chain")
                    && session.outcome.kind() == "chained-relayed"
                    && session.bytes_up == 71
                    && session.bytes_down == 54
            })
            .unwrap_or(false)
    })
    .await;

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
async fn relays_udp_through_fallback_group_when_primary_unreachable() {
    let echo_port = free_udp_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
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

    let mut control = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, //
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"fall")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"fall");

    drop(control);
    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_udp_through_urltest_group_after_probe_selects_direct() {
    let echo_port = free_udp_port();
    let probe_port = free_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
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

    let mut control = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, //
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"best")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"best");

    drop(control);
    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    probe_task.abort();
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_udp_through_urltest_group_with_nested_group_member() {
    let echo_port = free_udp_port();
    let probe_port = free_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
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

    let mut control = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, //
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"nest")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"nest");

    drop(control);
    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    probe_task.abort();
    let _ = echo_task.await;
}

#[tokio::test]
async fn relays_udp_through_nested_group_target() {
    let echo_port = free_udp_port();
    let outer_port = free_port();
    let unreachable_port = free_port();

    let echo_task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind udp echo");
        let mut buf = [0_u8; 1024];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
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

    let mut control = TcpStream::connect(("127.0.0.1", outer_port))
        .await
        .expect("connect outer proxy");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await.expect("read auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, //
            0, 0, 0, 0, 0x00, 0x00,
        ])
        .await
        .expect("write udp associate");

    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00);
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), echo_port, b"nest")
        .expect("build udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 1024];
    let (read, _) = timeout(Duration::from_secs(3), client.recv_from(&mut buf))
        .await
        .expect("udp recv timeout")
        .expect("recv udp response");
    let response = parse_udp_packet(&buf[..read]).expect("parse udp response");

    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, echo_port);
    assert_eq!(response.payload, b"nest");

    drop(control);
    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    let _ = echo_task.await;
}
