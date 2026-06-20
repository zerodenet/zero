#![cfg(all(feature = "socks5", feature = "mieru"))]

mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{timeout, Duration};
use zero_config::RuntimeConfig;
use zero_proxy::Proxy as Engine;

use support::interop::*;
use support::{free_port, spawn_engine, wait_for_listener};

// ── Zero → Mieru server (external node) ─────────────────────────────

#[tokio::test]
#[ignore = "requires MIERU_SERVER, MIERU_PORT, MIERU_USERNAME, MIERU_PASSWORD env vars"]
async fn zero_mieru_outbound_interops_with_external_mieru_tcp() {
    init_logs("mieru=debug");
    let zero_socks_port = free_port();
    let payload = b"GET / HTTP/1.0\r\nHost: httpbin.org\r\n\r\n";

    let Some(server) = require_env("MIERU_SERVER") else {
        return;
    };
    let Some(port_str) = require_env("MIERU_PORT") else {
        return;
    };
    let port: u16 = port_str.parse().expect("MIERU_PORT must be u16");
    let Some(username) = require_env("MIERU_USERNAME") else {
        return;
    };
    let Some(password) = require_env("MIERU_PASSWORD") else {
        return;
    };

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{"inbounds":[{{"tag":"socks-in","listen":{{"address":"127.0.0.1","port":{zero_socks_port}}},"protocol":{{"type":"socks5"}}}}],"outbounds":[{{"tag":"mieru-out","protocol":{{"type":"mieru","server":"{server}","port":{port},"username":"{username}","password":"{password}"}}}}],"route":{{"rules":[],"final":{{"type":"route","outbound":"mieru-out"}}}}}}"#
    ))
    .expect("parse zero config");

    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let echoed = match timeout(
        Duration::from_secs(30),
        socks5_tcp_http_get(zero_socks_port, "httpbin.org", 80, payload),
    )
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => panic!("zero -> mieru TCP interop failed: {error:?}"),
        Err(_) => panic!("zero -> mieru TCP interop timed out"),
    };

    assert!(
        !echoed.is_empty(),
        "expected HTTP response from httpbin.org through Mieru"
    );
    assert!(
        String::from_utf8_lossy(&echoed[..echoed.len().min(100)]).contains("HTTP"),
        "unexpected response"
    );

    zero.shutdown().await.expect("shutdown zero");
}

#[tokio::test]
#[ignore = "requires MIERU_SERVER, MIERU_PORT, MIERU_USERNAME, MIERU_PASSWORD env vars"]
async fn zero_mieru_outbound_interops_with_external_mieru_udp() {
    init_logs("mieru=debug");
    let zero_socks_port = free_port();

    let Some(server) = require_env("MIERU_SERVER") else {
        return;
    };
    let Some(port_str) = require_env("MIERU_PORT") else {
        return;
    };
    let port: u16 = port_str.parse().expect("MIERU_PORT must be u16");
    let Some(username) = require_env("MIERU_USERNAME") else {
        return;
    };
    let Some(password) = require_env("MIERU_PASSWORD") else {
        return;
    };

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{"inbounds":[{{"tag":"socks-in","listen":{{"address":"127.0.0.1","port":{zero_socks_port}}},"protocol":{{"type":"socks5"}}}}],"outbounds":[{{"tag":"mieru-out","protocol":{{"type":"mieru","server":"{server}","port":{port},"username":"{username}","password":"{password}"}}}}],"route":{{"rules":[],"final":{{"type":"route","outbound":"mieru-out"}}}}}}"#
    ))
    .expect("parse zero config");

    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let dns_query = build_dns_a_query("httpbin.org");
    let response = match timeout(
        Duration::from_secs(15),
        socks5_udp_send(zero_socks_port, "8.8.8.8", 53, &dns_query),
    )
    .await
    {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => panic!("zero -> mieru UDP interop failed: {e}"),
        Err(_) => panic!("zero -> mieru UDP interop timed out"),
    };

    assert!(!response.is_empty(), "expected DNS response through Mieru");
    assert!(
        response.len() > 12,
        "DNS response too short: {} bytes",
        response.len()
    );

    zero.shutdown().await.expect("shutdown zero");
}

// ── Helpers ─────────────────────────────────────────────────────────

async fn socks5_tcp_http_get(
    proxy_port: u16,
    host: &str,
    port: u16,
    request: &[u8],
) -> std::io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port)).await?;
    stream.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut auth = [0_u8; 2];
    stream.read_exact(&mut auth).await?;
    if auth != [0x05, 0x00] {
        return Err(std::io::Error::other("SOCKS5 auth failed"));
    }
    let host_bytes = host.as_bytes();
    let mut connect = vec![0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8];
    connect.extend_from_slice(host_bytes);
    connect.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&connect).await?;
    let mut response = [0_u8; 256];
    let n = stream.read(&mut response).await?;
    if n < 10 || response[1] != 0x00 {
        return Err(std::io::Error::other(format!(
            "SOCKS5 connect failed: {:?}",
            &response[..n]
        )));
    }
    stream.write_all(request).await?;
    let mut buf = vec![0_u8; 4096];
    let n = stream.read(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

async fn socks5_udp_send(
    proxy_port: u16,
    target_host: &str,
    target_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut control = TcpStream::connect(("127.0.0.1", proxy_port)).await?;
    control.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut auth = [0_u8; 2];
    control.read_exact(&mut auth).await?;
    assert_eq!(auth, [0x05, 0x00], "SOCKS5 auth failed");
    control
        .write_all(&[0x05, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    let mut resp = [0_u8; 10];
    control.read_exact(&mut resp).await?;
    assert_eq!(resp[1], 0x00, "UDP ASSOCIATE failed: {resp:?}");
    let relay_port = u16::from_be_bytes([resp[8], resp[9]]);

    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let target_ip = tokio::net::lookup_host(format!("{target_host}:{target_port}"))
        .await?
        .next()
        .ok_or("dns resolve failed")?;
    let mut packet = vec![0x00, 0x00, 0x00];
    match target_ip.ip() {
        std::net::IpAddr::V4(ip) => {
            packet.push(0x01);
            packet.extend_from_slice(&ip.octets());
        }
        std::net::IpAddr::V6(ip) => {
            packet.push(0x04);
            packet.extend_from_slice(&ip.octets());
        }
    }
    packet.extend_from_slice(&target_port.to_be_bytes());
    packet.extend_from_slice(payload);
    client.send_to(&packet, ("127.0.0.1", relay_port)).await?;

    let mut buf = [0_u8; 2048];
    let (n, _) = client.recv_from(&mut buf).await?;
    if n < 10 {
        return Err("UDP response too short".into());
    }
    Ok(buf[..n].to_vec())
}

fn build_dns_a_query(host: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&[
        0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    for label in host.split('.') {
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0x00);
    buf.extend_from_slice(&[0x00, 0x01]);
    buf.extend_from_slice(&[0x00, 0x01]);
    buf
}
