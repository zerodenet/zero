use std::future;
use std::io::ErrorKind;
use std::time::Duration;

use tokio::net::TcpListener;
use zero_config::RuntimeConfig;
use zero_engine::EngineError;
use zero_proxy::Proxy;

#[tokio::test]
async fn externally_occupied_port_fails_during_eager_inbound_bind() {
    let occupied = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("occupy ephemeral port");
    let port = occupied
        .local_addr()
        .expect("occupied local address")
        .port();
    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [{{
                "tag": "occupied-inbound",
                "listen": {{ "address": "127.0.0.1", "port": {port} }},
                "protocol": {{ "type": "direct" }}
            }}],
            "route": {{ "rules": [], "final": {{ "type": "direct" }} }}
        }}"#
    ))
    .expect("occupied-port config");
    let proxy = Proxy::new(config).expect("proxy construction");

    let error = tokio::time::timeout(
        Duration::from_secs(2),
        proxy.run_until(future::pending::<()>()),
    )
    .await
    .expect("eager bind should return before the run loop starts")
    .expect_err("occupied inbound port should fail");

    match error {
        EngineError::Io(error) => assert_eq!(error.kind(), ErrorKind::AddrInUse),
        other => panic!("expected address-in-use IO error, got {other}"),
    }
}
