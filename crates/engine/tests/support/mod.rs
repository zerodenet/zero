#![allow(dead_code)]

use std::net::TcpListener as StdTcpListener;

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
    for _ in 0..50 {
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
