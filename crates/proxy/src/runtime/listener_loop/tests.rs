use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{watch, Notify};
use zero_config::RuntimeConfig;

use super::{run_tcp_listener_loop, TcpListenerLoopRequest};

#[tokio::test]
async fn tcp_listener_accepts_connection_and_stops_on_shutdown() {
    let config =
        RuntimeConfig::parse(r#"{ "route": { "rules": [], "final": { "type": "direct" } } }"#)
            .expect("minimal runtime config");
    let proxy = crate::runtime::Proxy::new(config).expect("minimal proxy");
    let listener = zero_platform_tokio::TokioListener::bind("127.0.0.1:0")
        .await
        .expect("test listener");
    let listen_addr = listener.local_addr().expect("listener address");
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let accepted = Arc::new(Notify::new());
    let observations = Arc::new(Mutex::new(Vec::new()));
    let task = {
        let accepted = accepted.clone();
        let observations = observations.clone();
        tokio::spawn(async move {
            run_tcp_listener_loop(TcpListenerLoopRequest {
                proxy: &proxy,
                inbound_tag: "listener-test".to_owned(),
                protocol_name: "test",
                listener,
                shutdown: shutdown_rx,
                handler: move |runtime: crate::runtime::route_runtime::InboundRouteRuntime, _| {
                    let accepted = accepted.clone();
                    let observations = observations.clone();
                    async move {
                        observations
                            .lock()
                            .expect("observations lock")
                            .push((runtime.inbound_tag().to_owned(), runtime.source_addr()));
                        accepted.notify_one();
                    }
                },
            })
            .await
        })
    };

    let client = tokio::net::TcpStream::connect(listen_addr)
        .await
        .expect("connect listener");
    tokio::time::timeout(Duration::from_secs(2), accepted.notified())
        .await
        .expect("handler was not invoked");

    shutdown_tx.send(true).expect("send listener shutdown");
    let result = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("listener did not stop")
        .expect("listener task panicked");
    result.expect("listener loop failed");
    drop(client);

    let observations = observations.lock().expect("observations lock");
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].0, "listener-test");
    assert!(observations[0].1.is_some());
}
