use super::*;

use std::process::Command;
use std::sync::atomic::AtomicUsize;

static NEXT_REALITY_DIR: AtomicUsize = AtomicUsize::new(0);

const XRAY_IMAGE_ENV: &str = "ZERO_XRAY_IMAGE";
const DEFAULT_XRAY_IMAGE: &str = "ghcr.io/xtls/xray-core:latest";
const REALITY_PRIVATE_KEY: &str = "OKMOFBeltHBXaTQ8cIcsgabVQcqXeTB9Ih3lPtWMY04";
const REALITY_PUBLIC_KEY: &str = "9AwHi13y1rN6EWTSo8-HNCOhrzr251jNY7SSIxo0diA";
const REALITY_SHORT_ID: &str = "0123456789abcdef";
const REALITY_SERVER_NAME: &str = "www.cloudflare.com";

#[tokio::test]
#[ignore = "requires Docker and the Xray image; run explicitly with -- --ignored"]
#[cfg(all(feature = "inbound-socks5", feature = "outbound-vless"))]
async fn relays_tcp_through_vless_reality_xray() {
    let xray_host_port = free_port();
    let outer_port = free_port();

    let network = DockerNetwork::start(xray_host_port);
    let echo = DockerEchoServer::start(&network.name);
    echo.wait_ready(&network.name);
    let xray = XrayRealityServer::start(xray_host_port, &network.name);
    wait_for_listener(xray_host_port).await;

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
                    "tag": "vless-reality-chain",
                    "protocol": {{
                        "type": "vless",
                        "server": "127.0.0.1",
                        "port": {xray_host_port},
                        "id": "{USER_ID}",
                        "reality": {{
                            "public_key": "{REALITY_PUBLIC_KEY}",
                            "short_id": "{REALITY_SHORT_ID}",
                            "server_name": "{REALITY_SERVER_NAME}",
                            "cipher_suites": [
                                "TLS_AES_128_GCM_SHA256",
                                "TLS_AES_256_GCM_SHA384",
                                "TLS_CHACHA20_POLY1305_SHA256"
                            ]
                        }}
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "route", "outbound": "vless-reality-chain" }}
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
    if let Err(reply) = socks5_connect_domain(&mut client, "zero-reality-echo", 9000).await {
        panic!(
            "SOCKS CONNECT through VLESS Reality failed with reply=0x{reply:02x}; completed_sessions={:?}; xray_logs={}",
            outer_handle.completed_sessions(),
            xray.logs()
        );
    }

    client.write_all(b"real").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    if let Err(error) = client.read_exact(&mut echoed).await {
        panic!(
            "read payload through VLESS Reality failed: {error}; completed_sessions={:?}; xray_logs={}; echo_logs={}",
            outer_handle.completed_sessions(),
            xray.logs(),
            echo.logs(),
        );
    }
    assert_eq!(&echoed, b"real");

    outer_handle
        .shutdown()
        .await
        .expect("shutdown outer engine");
    drop(echo);
    drop(network);
}

async fn socks5_connect_domain(client: &mut TcpStream, domain: &str, port: u16) -> Result<(), u8> {
    client
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write socks auth");

    let mut auth = [0_u8; 2];
    client.read_exact(&mut auth).await.expect("read socks auth");
    assert_eq!(auth, [0x05, 0x00]);

    let domain = domain.as_bytes();
    assert!(domain.len() <= u8::MAX as usize);
    let mut request = vec![0x05, 0x01, 0x00, 0x03, domain.len() as u8];
    request.extend_from_slice(domain);
    request.extend_from_slice(&port.to_be_bytes());
    client
        .write_all(&request)
        .await
        .expect("write socks connect request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read socks connect response");
    if response[1] != 0x00 {
        return Err(response[1]);
    }

    Ok(())
}

struct DockerNetwork {
    name: String,
}

impl DockerNetwork {
    fn start(unique: u16) -> Self {
        let name = format!("zero-reality-net-{unique}");
        let output = Command::new("docker")
            .args(["network", "create", &name])
            .output()
            .expect("create docker network");
        if !output.status.success() {
            panic!(
                "failed to create docker network: status={:?}\nstdout={}\nstderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Self { name }
    }
}

impl Drop for DockerNetwork {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["network", "rm", self.name.as_str()])
            .output();
    }
}

struct DockerEchoServer {
    name: String,
}

impl DockerEchoServer {
    fn start(network: &str) -> Self {
        let name = format!("zero-reality-echo-{network}");
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "-d",
                "--name",
                &name,
                "--network",
                network,
                "--network-alias",
                "zero-reality-echo",
                "busybox",
                "sh",
                "-c",
                "while true; do nc -l -p 9000 -e cat; done",
            ])
            .output()
            .expect("run docker echo");

        if !output.status.success() {
            panic!(
                "failed to start echo container: status={:?}\nstdout={}\nstderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Self { name }
    }

    fn wait_ready(&self, network: &str) {
        let status = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                network,
                "busybox",
                "sh",
                "-c",
                "printf ping | nc -w 3 zero-reality-echo 9000 | grep -q ping",
            ])
            .status()
            .expect("probe docker echo");
        assert!(status.success(), "echo container did not become ready");
    }

    fn logs(&self) -> String {
        let output = Command::new("docker")
            .args(["logs", self.name.as_str()])
            .output();
        match output {
            Ok(output) => format!(
                "stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(error) => format!("failed to collect docker logs: {error}"),
        }
    }
}

impl Drop for DockerEchoServer {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", self.name.as_str()])
            .output();
    }
}

struct XrayRealityServer {
    name: String,
    dir: PathBuf,
}

impl XrayRealityServer {
    fn start(host_port: u16, network: &str) -> Self {
        let unique = NEXT_REALITY_DIR.fetch_add(1, Ordering::Relaxed);
        let name = format!("zero-vless-reality-{host_port}-{unique}");
        let dir = std::env::temp_dir().join(format!(
            "zero-vless-reality-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos(),
            unique
        ));
        std::fs::create_dir_all(&dir).expect("create xray config dir");
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, xray_reality_config()).expect("write xray config");

        let image = std::env::var(XRAY_IMAGE_ENV).unwrap_or_else(|_| DEFAULT_XRAY_IMAGE.to_owned());
        let mount = format!(
            "type=bind,source={},target=/etc/xray/config.json,readonly",
            config_path.display()
        );
        let publish = format!("127.0.0.1:{host_port}:8443");
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "-d",
                "--name",
                &name,
                "--network",
                network,
                "-p",
                &publish,
                "--mount",
                &mount,
                &image,
                "run",
                "-config",
                "/etc/xray/config.json",
            ])
            .output()
            .expect("run docker");

        if !output.status.success() {
            panic!(
                "failed to start xray container: status={:?}\nstdout={}\nstderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Self { name, dir }
    }

    fn logs(&self) -> String {
        let output = Command::new("docker")
            .args(["logs", self.name.as_str()])
            .output();
        match output {
            Ok(output) => format!(
                "stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(error) => format!("failed to collect docker logs: {error}"),
        }
    }
}

impl Drop for XrayRealityServer {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", self.name.as_str()])
            .output();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn xray_reality_config() -> String {
    format!(
        r#"{{
  "log": {{
    "loglevel": "debug"
  }},
  "inbounds": [
    {{
      "listen": "0.0.0.0",
      "port": 8443,
      "protocol": "vless",
      "settings": {{
        "clients": [
          {{
            "id": "{USER_ID}"
          }}
        ],
        "decryption": "none"
      }},
      "streamSettings": {{
        "network": "tcp",
        "security": "reality",
        "realitySettings": {{
          "show": false,
          "dest": "{REALITY_SERVER_NAME}:443",
          "xver": 0,
          "serverNames": [
            "{REALITY_SERVER_NAME}"
          ],
          "privateKey": "{REALITY_PRIVATE_KEY}",
          "shortIds": [
            "{REALITY_SHORT_ID}"
          ]
        }}
      }}
    }}
  ],
  "outbounds": [
    {{
      "protocol": "freedom",
      "tag": "direct"
    }}
  ]
}}"#
    )
}
