#![cfg(all(feature = "socks5", feature = "vmess"))]

mod support;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::digest::{digest, SHA256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::{sleep, timeout, Duration};
use zero_config::RuntimeConfig;
use zero_core::Address;
use zero_proxy::Proxy as Engine;

use support::{free_port, free_udp_port, spawn_engine, wait_for_listener};

const USER_ID: &str = "11111111-2222-3333-4444-555555555555";
static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);
static LOG_INIT: Once = Once::new();
const XRAY_WS_PATH: &str = "/zero-vmess-ws";
const XRAY_GRPC_SERVICE_NAME: &str = "zero.vmess.grpc";
const ZERO_GRPC_SERVICE_PATH: &str = "/zero.vmess.grpc/Tun";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XrayTransport {
    Tcp,
    Ws,
    Grpc,
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp() {
    zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_cipher("aes-128-gcm").await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_ws_tcp() {
    zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_transport(XrayTransport::Ws).await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_grpc_tcp() {
    zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_transport(XrayTransport::Grpc).await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn zero_vmess_outbound_none_interops_with_xray_vmess_inbound_tcp() {
    zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_cipher("none").await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_udp() {
    init_logs();
    let material = TempMaterial::new("zero-xray-vmess-udp-out");
    let xray_port = free_port();
    let zero_socks_port = free_port();
    let echo_port = free_udp_port();
    let payload = b"xray-vmess-udp";

    let xray_config = material.path("xray-server.json");
    std::fs::write(&xray_config, xray_vmess_inbound_config(xray_port)).expect("write xray config");
    let mut xray = XrayProcess::start(&xray_config, &material);
    wait_for_listener(xray_port).await;

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_socks_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vmess-out",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {xray_port},
                        "id": "{USER_ID}",
                        "cipher": "aes-128-gcm"
                    }}
                }}
            ],
            "route": {{ "rules": [], "final": {{ "type": "route", "outbound": "vmess-out" }} }}
        }}"#
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let echo = spawn_udp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_udp_echo(zero_socks_port, echo_port, payload),
    )
    .await
    {
        Ok(echoed) => echoed,
        Err(error) => panic!(
            "zero -> xray UDP interop timed out: {error}; xray={}",
            xray.logs()
        ),
    };
    assert_eq!(echoed, payload, "xray={}", xray.logs());

    zero.shutdown().await.expect("shutdown zero");
    xray.kill();
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn zero_vmess_outbound_zero_is_rejected_by_xray_vmess_inbound_tcp() {
    init_logs();
    let material = TempMaterial::new("zero-xray-vmess-zero-out");
    let xray_port = free_port();
    let zero_socks_port = free_port();
    let echo_port = free_port();
    let payload = b"xray-vmess-zero-tcp";

    let xray_config = material.path("xray-server.json");
    std::fs::write(&xray_config, xray_vmess_inbound_config(xray_port)).expect("write xray config");
    let mut xray = XrayProcess::start(&xray_config, &material);
    wait_for_listener(xray_port).await;

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_socks_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vmess-out",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {xray_port},
                        "id": "{USER_ID}",
                        "cipher": "zero"
                    }}
                }}
            ],
            "route": {{ "rules": [], "final": {{ "type": "route", "outbound": "vmess-out" }} }}
        }}"#
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let echo = spawn_tcp_echo(echo_port, payload.len()).await;
    let result = timeout(
        Duration::from_secs(5),
        socks5_tcp_echo_once(zero_socks_port, echo_port, payload),
    )
    .await;
    assert!(
        !matches!(result, Ok(Ok(echoed)) if echoed == payload),
        "Xray unexpectedly accepted VMess cipher zero; xray={}",
        xray.logs()
    );

    zero.shutdown().await.expect("shutdown zero");
    xray.kill();
    echo.abort();
}

async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_cipher(cipher: &str) {
    zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_inner(cipher, XrayTransport::Tcp)
        .await;
}

async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_transport(
    transport: XrayTransport,
) {
    zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_inner("aes-128-gcm", transport).await;
}

async fn zero_vmess_outbound_interops_with_xray_vmess_inbound_tcp_inner(
    cipher: &str,
    transport: XrayTransport,
) {
    init_logs();
    let material = TempMaterial::new("zero-xray-vmess-out");
    let xray_port = free_port();
    let zero_socks_port = free_port();
    let echo_port = free_port();
    let payload = b"xray-vmess-tcp";
    let tls = material.tls();

    let xray_config = material.path("xray-server.json");
    std::fs::write(
        &xray_config,
        xray_vmess_inbound_config_with_transport(xray_port, transport, Some(&tls)),
    )
    .expect("write xray config");
    let mut xray = XrayProcess::start(&xray_config, &material);
    wait_for_listener(xray_port).await;

    let outbound_transport = zero_vmess_outbound_transport_config(transport, &tls.cert_path);
    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_socks_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vmess-out",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {xray_port},
                        "id": "{USER_ID}",
                        "cipher": "{cipher}"{outbound_transport}
                    }}
                }}
            ],
            "route": {{ "rules": [], "final": {{ "type": "route", "outbound": "vmess-out" }} }}
        }}"#
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let echo = spawn_tcp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_tcp_echo_once(zero_socks_port, echo_port, payload),
    )
    .await
    {
        Ok(Ok(echoed)) => echoed,
        Ok(Err(error)) => panic!(
            "zero -> xray interop failed: {error:?}; xray={}",
            xray.logs()
        ),
        Err(error) => panic!(
            "zero -> xray interop timed out: {error}; xray={}",
            xray.logs()
        ),
    };
    assert_eq!(echoed, payload, "xray={}", xray.logs());

    zero.shutdown().await.expect("shutdown zero");
    xray.kill();
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp() {
    xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_security("aes-128-gcm").await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn xray_vmess_outbound_interops_with_zero_vmess_inbound_ws_tcp() {
    xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_inner(
        "aes-128-gcm",
        XrayTransport::Ws,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn xray_vmess_outbound_interops_with_zero_vmess_inbound_grpc_tcp() {
    xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_inner(
        "aes-128-gcm",
        XrayTransport::Grpc,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn xray_vmess_outbound_none_interops_with_zero_vmess_inbound_tcp() {
    xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_security("none").await;
}

async fn xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_security(security: &str) {
    xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_inner(security, XrayTransport::Tcp)
        .await;
}

async fn xray_vmess_outbound_interops_with_zero_vmess_inbound_tcp_inner(
    security: &str,
    transport: XrayTransport,
) {
    init_logs();
    let material = TempMaterial::new("xray-zero-vmess-out");
    let xray_socks_port = free_port();
    let zero_vmess_port = free_port();
    let echo_port = free_port();
    let payload = b"zero-vmess-tcp";
    let tls = material.tls();

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vmess-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_vmess_port} }},
                    "protocol": {{
                        "type": "vmess",
                        "users": [{{ "id": "{USER_ID}", "cipher": "aes-128-gcm" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}{}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
        zero_vmess_inbound_transport_config(transport),
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_vmess_port).await;

    let xray_config = material.path("xray-client.json");
    std::fs::write(
        &xray_config,
        xray_vmess_outbound_tls_config(
            xray_socks_port,
            zero_vmess_port,
            &tls.cert_sha256_hex,
            security,
            false,
            transport,
        ),
    )
    .expect("write xray config");
    let mut xray = XrayProcess::start(&xray_config, &material);
    wait_for_listener(xray_socks_port).await;

    let echo = spawn_tcp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_tcp_echo(xray_socks_port, echo_port, payload),
    )
    .await
    {
        Ok(echoed) => echoed,
        Err(error) => panic!(
            "xray -> zero interop timed out: {error}; xray={}",
            xray.logs()
        ),
    };
    assert_eq!(echoed, payload);

    xray.kill();
    zero.shutdown().await.expect("shutdown zero");
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires XRAY_BIN pointing to an Xray executable"]
async fn xray_vmess_outbound_interops_with_zero_vmess_inbound_udp() {
    init_logs();
    let material = TempMaterial::new("xray-zero-vmess-udp-out");
    let xray_socks_port = free_port();
    let zero_vmess_port = free_port();
    let echo_port = free_udp_port();
    let payload = b"zero-vmess-udp";
    let tls = material.tls();

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vmess-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_vmess_port} }},
                    "protocol": {{
                        "type": "vmess",
                        "users": [{{ "id": "{USER_ID}", "cipher": "aes-128-gcm" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_vmess_port).await;

    let xray_config = material.path("xray-client.json");
    std::fs::write(
        &xray_config,
        xray_vmess_outbound_tls_config(
            xray_socks_port,
            zero_vmess_port,
            &tls.cert_sha256_hex,
            "aes-128-gcm",
            true,
            XrayTransport::Tcp,
        ),
    )
    .expect("write xray config");
    let mut xray = XrayProcess::start(&xray_config, &material);
    wait_for_listener(xray_socks_port).await;

    let echo = spawn_udp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_udp_echo(xray_socks_port, echo_port, payload),
    )
    .await
    {
        Ok(echoed) => echoed,
        Err(error) => panic!(
            "xray -> zero UDP interop timed out: {error}; xray={}",
            xray.logs()
        ),
    };
    assert_eq!(echoed, payload, "xray={}", xray.logs());

    xray.kill();
    zero.shutdown().await.expect("shutdown zero");
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires SING_BOX_BIN or downloaded sing-box under temp interop dir"]
async fn zero_vmess_outbound_interops_with_sing_box_vmess_inbound_tcp() {
    init_logs();
    let material = TempMaterial::new("zero-sing-vmess-out");
    let sing_port = free_port();
    let zero_socks_port = free_port();
    let echo_port = free_port();
    let payload = b"sing-box-vmess-tcp";

    let sing_config = material.path("sing-box-server.json");
    std::fs::write(&sing_config, sing_box_vmess_inbound_config(sing_port))
        .expect("write sing-box config");
    let mut sing_box = ExternalProcess::start(
        sing_box_bin(),
        &[
            "run",
            "-c",
            sing_config.to_str().expect("sing-box config path"),
        ],
        &material,
        "sing-box",
    );
    wait_for_listener(sing_port).await;

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_socks_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vmess-out",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {sing_port},
                        "id": "{USER_ID}",
                        "cipher": "aes-128-gcm"
                    }}
                }}
            ],
            "route": {{ "rules": [], "final": {{ "type": "route", "outbound": "vmess-out" }} }}
        }}"#
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let echo = spawn_tcp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_tcp_echo_once(zero_socks_port, echo_port, payload),
    )
    .await
    {
        Ok(Ok(echoed)) => echoed,
        Ok(Err(error)) => panic!(
            "zero -> sing-box interop failed: {error:?}; sing-box={}",
            sing_box.logs()
        ),
        Err(error) => panic!(
            "zero -> sing-box interop timed out: {error}; sing-box={}",
            sing_box.logs()
        ),
    };
    assert_eq!(echoed, payload, "sing-box={}", sing_box.logs());

    zero.shutdown().await.expect("shutdown zero");
    sing_box.kill();
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires SING_BOX_BIN or downloaded sing-box under temp interop dir"]
async fn zero_vmess_outbound_interops_with_sing_box_vmess_inbound_udp() {
    init_logs();
    let material = TempMaterial::new("zero-sing-vmess-udp-out");
    let sing_port = free_port();
    let zero_socks_port = free_port();
    let echo_port = free_udp_port();
    let payload = b"sing-box-vmess-udp";

    let sing_config = material.path("sing-box-server.json");
    std::fs::write(&sing_config, sing_box_vmess_inbound_config(sing_port))
        .expect("write sing-box config");
    let mut sing_box = ExternalProcess::start(
        sing_box_bin(),
        &[
            "run",
            "-c",
            sing_config.to_str().expect("sing-box config path"),
        ],
        &material,
        "sing-box",
    );
    wait_for_listener(sing_port).await;

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_socks_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "vmess-out",
                    "protocol": {{
                        "type": "vmess",
                        "server": "127.0.0.1",
                        "port": {sing_port},
                        "id": "{USER_ID}",
                        "cipher": "aes-128-gcm"
                    }}
                }}
            ],
            "route": {{ "rules": [], "final": {{ "type": "route", "outbound": "vmess-out" }} }}
        }}"#
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_socks_port).await;

    let echo = spawn_udp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_udp_echo(zero_socks_port, echo_port, payload),
    )
    .await
    {
        Ok(echoed) => echoed,
        Err(error) => panic!(
            "zero -> sing-box UDP interop timed out: {error}; sing-box={}",
            sing_box.logs()
        ),
    };
    assert_eq!(echoed, payload, "sing-box={}", sing_box.logs());

    zero.shutdown().await.expect("shutdown zero");
    sing_box.kill();
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires MIHOMO_BIN or downloaded mihomo under temp interop dir"]
async fn mihomo_vmess_outbound_interops_with_zero_vmess_inbound_tcp() {
    init_logs();
    let material = TempMaterial::new("mihomo-zero-vmess-out");
    let mihomo_mixed_port = free_port();
    let zero_vmess_port = free_port();
    let echo_port = free_port();
    let payload = b"mihomo-vmess-tcp";
    let tls = material.tls();

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vmess-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_vmess_port} }},
                    "protocol": {{
                        "type": "vmess",
                        "users": [{{ "id": "{USER_ID}", "cipher": "aes-128-gcm" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_vmess_port).await;

    let mihomo_config = material.path("mihomo-client.yaml");
    std::fs::write(
        &mihomo_config,
        mihomo_vmess_outbound_config(mihomo_mixed_port, zero_vmess_port),
    )
    .expect("write mihomo config");
    let mut mihomo = ExternalProcess::start(
        mihomo_bin(),
        &[
            "-f",
            mihomo_config.to_str().expect("mihomo config path"),
            "-d",
            material.dir.to_str().expect("mihomo work dir"),
        ],
        &material,
        "mihomo",
    );
    wait_for_listener(mihomo_mixed_port).await;

    let echo = spawn_tcp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_tcp_echo(mihomo_mixed_port, echo_port, payload),
    )
    .await
    {
        Ok(echoed) => echoed,
        Err(error) => panic!(
            "mihomo -> zero interop timed out: {error}; mihomo={}",
            mihomo.logs()
        ),
    };
    assert_eq!(echoed, payload, "mihomo={}", mihomo.logs());

    mihomo.kill();
    zero.shutdown().await.expect("shutdown zero");
    echo.await.expect("echo task");
}

#[tokio::test]
#[ignore = "requires MIHOMO_BIN or downloaded mihomo under temp interop dir"]
async fn mihomo_vmess_outbound_interops_with_zero_vmess_inbound_udp() {
    init_logs();
    let material = TempMaterial::new("mihomo-zero-vmess-udp-out");
    let mihomo_mixed_port = free_port();
    let zero_vmess_port = free_port();
    let echo_port = free_udp_port();
    let payload = b"mihomo-vmess-udp";
    let tls = material.tls();

    let zero_config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "vmess-in",
                    "listen": {{ "address": "127.0.0.1", "port": {zero_vmess_port} }},
                    "protocol": {{
                        "type": "vmess",
                        "users": [{{ "id": "{USER_ID}", "cipher": "aes-128-gcm" }}],
                        "tls": {{
                            "cert_path": "{}",
                            "key_path": "{}"
                        }}
                    }}
                }}
            ],
            "outbounds": [],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#,
        escape_json_path(&tls.cert_path),
        escape_json_path(&tls.key_path),
    ))
    .expect("parse zero config");
    let zero = spawn_engine(Engine::new(zero_config).expect("build zero engine"));
    wait_for_listener(zero_vmess_port).await;

    let mihomo_config = material.path("mihomo-client.yaml");
    std::fs::write(
        &mihomo_config,
        mihomo_vmess_outbound_config(mihomo_mixed_port, zero_vmess_port),
    )
    .expect("write mihomo config");
    let mut mihomo = ExternalProcess::start(
        mihomo_bin(),
        &[
            "-f",
            mihomo_config.to_str().expect("mihomo config path"),
            "-d",
            material.dir.to_str().expect("mihomo work dir"),
        ],
        &material,
        "mihomo",
    );
    wait_for_listener(mihomo_mixed_port).await;

    let echo = spawn_udp_echo(echo_port, payload.len()).await;
    let echoed = match timeout(
        Duration::from_secs(10),
        socks5_udp_echo(mihomo_mixed_port, echo_port, payload),
    )
    .await
    {
        Ok(echoed) => echoed,
        Err(error) => panic!(
            "mihomo -> zero UDP interop timed out: {error}; mihomo={}",
            mihomo.logs()
        ),
    };
    assert_eq!(echoed, payload, "mihomo={}", mihomo.logs());

    mihomo.kill();
    zero.shutdown().await.expect("shutdown zero");
    echo.await.expect("echo task");
}

fn xray_vmess_inbound_config(port: u16) -> String {
    xray_vmess_inbound_config_with_transport(port, XrayTransport::Tcp, None)
}

fn xray_vmess_inbound_config_with_transport(
    port: u16,
    transport: XrayTransport,
    tls: Option<&TestTlsMaterial>,
) -> String {
    let stream_settings = xray_inbound_stream_settings(transport, tls);
    format!(
        r#"{{
            "log": {{ "loglevel": "debug" }},
            "inbounds": [
                {{
                    "listen": "127.0.0.1",
                    "port": {port},
                    "protocol": "vmess",
                    "settings": {{
                        "clients": [{{ "id": "{USER_ID}", "alterId": 0, "security": "auto" }}]
                    }},
                    "streamSettings": {stream_settings}
                }}
            ],
            "outbounds": [{{ "protocol": "freedom", "settings": {{}} }}]
        }}"#
    )
}

fn xray_inbound_stream_settings(transport: XrayTransport, tls: Option<&TestTlsMaterial>) -> String {
    let (network, extra) = match transport {
        XrayTransport::Tcp => ("tcp", String::new()),
        XrayTransport::Ws => (
            "ws",
            format!(r#", "wsSettings": {{ "path": "{XRAY_WS_PATH}" }}"#),
        ),
        XrayTransport::Grpc => (
            "grpc",
            format!(r#", "grpcSettings": {{ "serviceName": "{XRAY_GRPC_SERVICE_NAME}" }}"#),
        ),
    };

    match tls {
        Some(tls) => format!(
            r#"{{
                "network": "{network}",
                "security": "tls",
                "tlsSettings": {{
                    "serverName": "localhost",
                    "certificates": [
                        {{
                            "certificateFile": "{}",
                            "keyFile": "{}"
                        }}
                    ]
                }}{extra}
            }}"#,
            escape_json_path(&tls.cert_path),
            escape_json_path(&tls.key_path),
        ),
        None => format!(r#"{{ "network": "{network}", "security": "none"{extra} }}"#),
    }
}

fn zero_vmess_outbound_transport_config(transport: XrayTransport, ca_cert_path: &Path) -> String {
    let transport_config = match transport {
        XrayTransport::Tcp => String::new(),
        XrayTransport::Ws => format!(r#", "ws": {{ "path": "{XRAY_WS_PATH}" }}"#),
        XrayTransport::Grpc => {
            format!(r#", "grpc": {{ "service_names": ["{ZERO_GRPC_SERVICE_PATH}"] }}"#)
        }
    };
    let alpn_config = match transport {
        XrayTransport::Grpc => {
            r#",
                            "alpn": ["h2"]"#
        }
        XrayTransport::Tcp | XrayTransport::Ws => "",
    };
    format!(
        r#",
                        "tls": {{
                            "server_name": "localhost",
                            "ca_cert_path": "{}"{alpn_config}
                        }}{transport_config}"#,
        escape_json_path(ca_cert_path),
    )
}

fn zero_vmess_inbound_transport_config(transport: XrayTransport) -> String {
    match transport {
        XrayTransport::Tcp => String::new(),
        XrayTransport::Ws => format!(r#", "ws": {{ "path": "{XRAY_WS_PATH}" }}"#),
        XrayTransport::Grpc => {
            format!(r#", "grpc": {{ "service_names": ["{ZERO_GRPC_SERVICE_PATH}"] }}"#)
        }
    }
}

fn sing_box_vmess_inbound_config(port: u16) -> String {
    format!(
        r#"{{
            "log": {{ "level": "debug" }},
            "inbounds": [
                {{
                    "type": "vmess",
                    "tag": "vmess-in",
                    "listen": "127.0.0.1",
                    "listen_port": {port},
                    "users": [{{ "uuid": "{USER_ID}" }}]
                }}
            ],
            "outbounds": [{{ "type": "direct", "tag": "direct" }}],
            "route": {{ "final": "direct" }}
        }}"#
    )
}

fn mihomo_vmess_outbound_config(mixed_port: u16, vmess_port: u16) -> String {
    format!(
        r#"mixed-port: {mixed_port}
allow-lan: false
mode: global
log-level: debug
ipv6: false
proxies:
  - name: zero-vmess
    type: vmess
    server: 127.0.0.1
    port: {vmess_port}
    uuid: {USER_ID}
    alterId: 0
    cipher: auto
    udp: true
    tls: true
    servername: localhost
    skip-cert-verify: true
proxy-groups:
  - name: GLOBAL
    type: select
    proxies:
      - zero-vmess
rules:
  - MATCH,zero-vmess
"#
    )
}

fn init_logs() {
    LOG_INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                std::env::var("RUST_LOG")
                    .unwrap_or_else(|_| "zero_proxy=debug,vmess=debug".to_owned()),
            )
            .with_test_writer()
            .try_init();
    });
}

fn sing_box_bin() -> String {
    std::env::var("SING_BOX_BIN").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("zero-vmess-interop")
            .join("sing-box")
            .join("sing-box.exe")
            .display()
            .to_string()
    })
}

fn mihomo_bin() -> String {
    std::env::var("MIHOMO_BIN").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("zero-vmess-interop")
            .join("mihomo")
            .join("mihomo.exe")
            .display()
            .to_string()
    })
}

fn xray_vmess_outbound_tls_config(
    socks_port: u16,
    vmess_port: u16,
    cert_sha256_hex: &str,
    security: &str,
    socks_udp: bool,
    transport: XrayTransport,
) -> String {
    let stream_settings = xray_outbound_stream_settings(transport, cert_sha256_hex);
    format!(
        r#"{{
            "log": {{ "loglevel": "debug" }},
            "inbounds": [
                {{
                    "listen": "127.0.0.1",
                    "port": {socks_port},
                    "protocol": "socks",
                    "settings": {{ "auth": "noauth", "udp": {socks_udp} }}
                }}
            ],
            "outbounds": [
                {{
                    "protocol": "vmess",
                    "settings": {{
                        "vnext": [
                            {{
                                "address": "127.0.0.1",
                                "port": {vmess_port},
                                "users": [{{ "id": "{USER_ID}", "alterId": 0, "security": "{security}" }}]
                            }}
                        ]
                    }},
                    "streamSettings": {stream_settings}
                }}
            ]
        }}"#,
    )
}

fn xray_outbound_stream_settings(transport: XrayTransport, cert_sha256_hex: &str) -> String {
    let (network, extra) = match transport {
        XrayTransport::Tcp => ("tcp", String::new()),
        XrayTransport::Ws => (
            "ws",
            format!(r#", "wsSettings": {{ "path": "{XRAY_WS_PATH}" }}"#),
        ),
        XrayTransport::Grpc => (
            "grpc",
            format!(r#", "grpcSettings": {{ "serviceName": "{XRAY_GRPC_SERVICE_NAME}" }}"#),
        ),
    };
    format!(
        r#"{{
            "network": "{network}",
            "security": "tls",
            "tlsSettings": {{
                "serverName": "localhost",
                "pinnedPeerCertSha256": "{cert_sha256_hex}"
            }}{extra}
        }}"#
    )
}

async fn spawn_tcp_echo(port: u16, payload_len: usize) -> tokio::task::JoinHandle<()> {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .expect("bind echo");
        let _ = ready_tx.send(());
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = vec![0_u8; payload_len];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });
    ready_rx.await.expect("echo ready");
    task
}

async fn spawn_udp_echo(port: u16, _payload_len: usize) -> tokio::task::JoinHandle<()> {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let socket = UdpSocket::bind(("127.0.0.1", port))
            .await
            .expect("bind udp echo");
        let _ = ready_tx.send(());
        let mut buf = vec![0_u8; 2048];
        let (read, peer) = socket.recv_from(&mut buf).await.expect("recv udp echo");
        socket
            .send_to(&buf[..read], peer)
            .await
            .expect("send udp echo");
    });
    ready_rx.await.expect("udp echo ready");
    task
}

async fn socks5_tcp_echo(proxy_port: u16, target_port: u16, payload: &[u8]) -> Vec<u8> {
    let mut last_error = None;
    for _ in 0..50 {
        match socks5_tcp_echo_once(proxy_port, target_port, payload).await {
            Ok(echoed) => return echoed,
            Err(error) => {
                last_error = Some(error);
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
    panic!("socks5 tcp echo failed: {:?}", last_error);
}

async fn socks5_tcp_echo_once(
    proxy_port: u16,
    target_port: u16,
    payload: &[u8],
) -> std::io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port)).await?;
    stream.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut auth = [0_u8; 2];
    stream.read_exact(&mut auth).await?;
    assert_eq!(auth, [0x05, 0x00]);

    let mut request = vec![0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1];
    request.extend_from_slice(&target_port.to_be_bytes());
    stream.write_all(&request).await?;
    let mut response = [0_u8; 10];
    stream.read_exact(&mut response).await?;
    assert_eq!(response[1], 0x00, "socks connect failed: {response:?}");

    stream.write_all(payload).await?;
    let mut echoed = vec![0_u8; payload.len()];
    stream.read_exact(&mut echoed).await?;
    Ok(echoed)
}

async fn socks5_udp_echo(proxy_port: u16, target_port: u16, payload: &[u8]) -> Vec<u8> {
    let mut control = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect socks5 control");
    control
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write socks5 auth");
    let mut auth = [0_u8; 2];
    control
        .read_exact(&mut auth)
        .await
        .expect("read socks5 auth");
    assert_eq!(auth, [0x05, 0x00]);

    control
        .write_all(&[
            0x05, 0x03, 0x00, 0x01, // UDP ASSOCIATE + IPv4
            0, 0, 0, 0, 0, 0,
        ])
        .await
        .expect("write udp associate");
    let mut response = [0_u8; 10];
    control
        .read_exact(&mut response)
        .await
        .expect("read udp associate response");
    assert_eq!(response[1], 0x00, "udp associate failed: {response:?}");
    let relay_port = u16::from_be_bytes([response[8], response[9]]);

    let client = UdpSocket::bind(("127.0.0.1", 0))
        .await
        .expect("bind udp client");
    let packet = socks5::build_udp_packet(&Address::Ipv4([127, 0, 0, 1]), target_port, payload)
        .expect("build socks5 udp packet");
    client
        .send_to(&packet, ("127.0.0.1", relay_port))
        .await
        .expect("send udp packet");

    let mut buf = [0_u8; 2048];
    let (read, _) = client.recv_from(&mut buf).await.expect("recv udp response");
    let response = socks5::parse_udp_packet(&buf[..read]).expect("parse socks5 udp response");
    assert_eq!(response.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(response.port, target_port);
    response.payload.to_vec()
}

struct XrayProcess {
    inner: ExternalProcess,
}

impl XrayProcess {
    fn start(config: &Path, material: &TempMaterial) -> Self {
        let xray_bin = std::env::var("XRAY_BIN").expect("XRAY_BIN must point to xray executable");
        Self {
            inner: ExternalProcess::start(
                xray_bin,
                &["run", "-config", config.to_str().expect("xray config path")],
                material,
                "xray",
            ),
        }
    }

    fn kill(&mut self) {
        self.inner.kill();
    }

    fn logs(&self) -> String {
        self.inner.logs()
    }
}

impl Drop for XrayProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

struct ExternalProcess {
    child: Child,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl ExternalProcess {
    fn start(program: String, args: &[&str], material: &TempMaterial, name: &str) -> Self {
        let stdout_path = material.path(&format!("{name}.stdout"));
        let stderr_path = material.path(&format!("{name}.stderr"));
        let stdout = std::fs::File::create(&stdout_path).expect("process stdout");
        let stderr = std::fs::File::create(&stderr_path).expect("process stderr");
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .unwrap_or_else(|error| panic!("start {name}: {error}"));
        Self {
            child,
            stdout_path,
            stderr_path,
        }
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn logs(&self) -> String {
        let stdout = std::fs::read_to_string(&self.stdout_path).unwrap_or_default();
        let stderr = std::fs::read_to_string(&self.stderr_path).unwrap_or_default();
        format!("stdout:\n{stdout}\nstderr:\n{stderr}")
    }
}

impl Drop for ExternalProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

struct TempMaterial {
    dir: PathBuf,
}

impl TempMaterial {
    fn new(prefix: &str) -> Self {
        let unique = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}-{unique}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos(),
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn tls(&self) -> TestTlsMaterial {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
            .expect("generate self-signed cert");
        let cert_path = self.path("server.crt");
        let key_path = self.path("server.key");
        let cert_sha256_hex = hex_lower(digest(&SHA256, certified.cert.der().as_ref()).as_ref());
        std::fs::write(&cert_path, certified.cert.pem()).expect("write cert");
        std::fs::write(&key_path, certified.signing_key.serialize_pem()).expect("write key");
        TestTlsMaterial {
            cert_path,
            key_path,
            cert_sha256_hex,
        }
    }
}

impl Drop for TempMaterial {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct TestTlsMaterial {
    cert_path: PathBuf,
    key_path: PathBuf,
    cert_sha256_hex: String,
}

fn escape_json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn hex_lower(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}
