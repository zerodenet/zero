mod support;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use support::{
    free_port, http_connect_tunnel, remove_temp_file, socks5_connect, spawn_zero, stop_child,
    wait_for_port, write_temp_config,
};

#[test]
fn zero_binary_relays_tcp_through_socks5_direct() {
    let proxy_port = free_port();
    let echo_port = free_port();

    let config = format!(
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
    );
    let config_path = write_temp_config(&config, "proxy-smoke-socks5-direct");

    let echo_thread = spawn_echo_server(echo_port);
    let mut child = spawn_zero(&["run", config_path.to_str().expect("utf-8 config path")]);

    wait_for_port(proxy_port);

    let mut stream = socks5_connect(proxy_port, "127.0.0.1", echo_port);
    stream.write_all(b"ping").expect("write payload");

    let mut echoed = [0_u8; 4];
    stream.read_exact(&mut echoed).expect("read payload");
    assert_eq!(&echoed, b"ping");

    stop_child(&mut child);
    let _ = echo_thread.join();
    remove_temp_file(&config_path);
}

#[test]
fn zero_binary_relays_tcp_through_http_connect() {
    let proxy_port = free_port();
    let echo_port = free_port();

    let config = format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "http-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{ "type": "http-connect" }}
                }}
            ],
            "outbounds": [],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    );
    let config_path = write_temp_config(&config, "proxy-smoke-http-connect");

    let echo_thread = spawn_echo_server(echo_port);
    let mut child = spawn_zero(&["run", config_path.to_str().expect("utf-8 config path")]);

    wait_for_port(proxy_port);

    let mut stream = http_connect_tunnel(proxy_port, &format!("127.0.0.1:{echo_port}"));
    stream.write_all(b"pong").expect("write payload");

    let mut echoed = [0_u8; 4];
    stream.read_exact(&mut echoed).expect("read payload");
    assert_eq!(&echoed, b"pong");

    stop_child(&mut child);
    let _ = echo_thread.join();
    remove_temp_file(&config_path);
}

#[test]
fn zero_binary_relays_tcp_through_chained_socks5_outbound() {
    let outer_port = free_port();
    let upstream_port = free_port();
    let echo_port = free_port();

    let upstream_config = format!(
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
    );
    let upstream_config_path = write_temp_config(&upstream_config, "proxy-smoke-upstream");

    let outer_config = format!(
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
    );
    let outer_config_path = write_temp_config(&outer_config, "proxy-smoke-outer");

    let echo_thread = spawn_echo_server(echo_port);
    let mut upstream = spawn_zero(&[
        "run",
        upstream_config_path.to_str().expect("utf-8 config path"),
    ]);
    wait_for_port(upstream_port);

    let mut outer = spawn_zero(&[
        "run",
        outer_config_path.to_str().expect("utf-8 config path"),
    ]);
    wait_for_port(outer_port);

    let mut stream = socks5_connect(outer_port, "127.0.0.1", echo_port);
    stream.write_all(b"mesh").expect("write payload");

    let mut echoed = [0_u8; 4];
    stream.read_exact(&mut echoed).expect("read payload");
    assert_eq!(&echoed, b"mesh");

    stop_child(&mut outer);
    stop_child(&mut upstream);
    let _ = echo_thread.join();
    remove_temp_file(&outer_config_path);
    remove_temp_file(&upstream_config_path);
}

fn spawn_echo_server(port: u16) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind echo");
        let (mut stream, _) = listener.accept().expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).expect("read echo");
        stream.write_all(&buf).expect("write echo");
    })
}
