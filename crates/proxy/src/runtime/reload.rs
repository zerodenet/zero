use std::sync::mpsc::Receiver;
use std::time::Duration;

pub(super) fn subscribe_reload_bridge(
    reload_rx: Receiver<()>,
) -> tokio::sync::mpsc::UnboundedReceiver<()> {
    let (reload_tx, reload_async_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || loop {
        match reload_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(()) => {
                if reload_tx.send(()).is_err() {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if reload_tx.is_closed() {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    });
    reload_async_rx
}
