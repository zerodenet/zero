#![allow(dead_code)]

use std::net::TcpListener as StdTcpListener;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};
use zero_engine::{Engine, RunningEngine};

pub fn free_port() -> u16 {
    let listener = StdTcpListener::bind(("127.0.0.1", 0)).expect("bind free port probe");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

pub async fn wait_for_listener(port: u16) {
    for _ in 0..150 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    panic!("listener on port {port} did not start in time");
}

pub async fn wait_for(description: &str, mut predicate: impl FnMut() -> bool) {
    for _ in 0..50 {
        if predicate() {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    panic!("{description} did not become ready in time");
}

pub fn spawn_engine(engine: Engine) -> RunningEngine {
    engine.spawn()
}

pub fn spawn_http_probe_server(port: u16) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .expect("bind probe server");

        loop {
            let (mut stream, _) = listener.accept().await.expect("accept probe");
            tokio::spawn(async move {
                let mut buf = [0_u8; 1024];
                let _ = stream.read(&mut buf).await;
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
            });
        }
    })
}

pub async fn wait_for_group_selection(engine: &RunningEngine, group_tag: &str, selected: &str) {
    for _ in 0..150 {
        let current = engine
            .export_status()
            .config
            .outbound_groups
            .iter()
            .find(|group| group.tag == group_tag)
            .and_then(|group| group.selected.clone());
        if current.as_deref() == Some(selected) {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    let current = engine
        .export_status()
        .config
        .outbound_groups
        .iter()
        .find(|group| group.tag == group_tag)
        .and_then(|group| group.selected.clone());
    panic!(
        "urltest group selection did not become ready in time; current={:?}",
        current
    );
}
