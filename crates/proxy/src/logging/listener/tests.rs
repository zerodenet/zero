use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;
use zero_engine::EngineError;

use super::{log_listener_connection_error, INBOUND_ACCEPT_ROUTE_STAGE};

#[test]
fn accepted_route_failures_keep_a_stable_stage_across_log_levels() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(BufferMakeWriter(buffer.clone()))
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .compact()
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        log_listener_connection_error(
            INBOUND_ACCEPT_ROUTE_STAGE,
            "vless",
            "inbound-a",
            &"127.0.0.1:1234",
            &EngineError::InvalidPlan {
                message: "route construction failed".to_owned(),
            },
        );
        log_listener_connection_error(
            INBOUND_ACCEPT_ROUTE_STAGE,
            "vless",
            "inbound-a",
            &"127.0.0.1:1234",
            &EngineError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "client disconnected",
            )),
        );
    });

    let logs = String::from_utf8(buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains(" WARN "), "{logs}");
    assert!(logs.contains("DEBUG"), "{logs}");
    assert_eq!(
        logs.matches("stage=\"inbound_accept_route\"").count(),
        2,
        "{logs}"
    );
    assert_eq!(logs.matches("protocol=\"vless\"").count(), 2, "{logs}");
    assert_eq!(
        logs.matches("inbound_tag=\"inbound-a\"").count(),
        2,
        "{logs}"
    );
}

#[derive(Clone)]
struct BufferMakeWriter(Arc<Mutex<Vec<u8>>>);

impl<'a> MakeWriter<'a> for BufferMakeWriter {
    type Writer = BufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        BufferWriter(self.0.clone())
    }
}

struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("log buffer lock")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
