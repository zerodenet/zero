//! Integration test for `Proxy::probe_outbound_single` — the synchronous,
//! through-proxy single-node latency probe backing the
//! `diagnostics.probe_outbound` command.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use zero_config::RuntimeConfig;
use zero_proxy::Proxy;

/// A probe through a `direct` outbound must reach the target via the real proxy
/// dispatch path (TLS-less here since the URL is plain HTTP) and report a
/// sub-timeout latency. This is the single-node counterpart to the async
/// `url_test` group probe.
#[tokio::test]
async fn probe_outbound_single_measures_through_proxy_latency() {
    // Minimal HTTP/204 server on an ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind probe server");
    let port = listener.local_addr().expect("local_addr").port();
    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.expect("accept");
        // Drain the HEAD request (best-effort), then reply 204.
        let mut buf = [0u8; 512];
        let _ = sock.read(&mut buf).await;
        sock.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
            .await
            .expect("write 204");
    });

    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "outbounds": [
                { "tag": "probe-target", "protocol": { "type": "direct" } }
            ],
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("parse config");
    let proxy = Proxy::new(config).expect("build proxy");

    let url = format!("http://127.0.0.1:{port}/generate_204");
    let latency_ms = timeout(
        std::time::Duration::from_secs(5),
        proxy.probe_outbound_single("probe-target", &url),
    )
    .await
    .expect("probe_outbound_single did not complete within 5s")
    .expect("probe through direct outbound should succeed");

    // localhost RTT must be well under the 5s probe timeout.
    assert!(
        latency_ms < 5_000,
        "expected sub-timeout localhost latency, got {latency_ms} ms"
    );
    let _ = server.await;
}

/// Probing a tag that does not exist must surface a not-found error rather
/// than panicking or hanging.
#[tokio::test]
async fn probe_outbound_single_errors_on_unknown_tag() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "outbounds": [{ "tag": "direct-out", "protocol": { "type": "direct" } }],
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("parse config");
    let proxy = Proxy::new(config).expect("build proxy");

    let result = proxy
        .probe_outbound_single("no-such-tag", "http://127.0.0.1:1/")
        .await;
    assert!(result.is_err(), "probing an unknown tag should error");
}
