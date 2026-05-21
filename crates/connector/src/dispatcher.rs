use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tracing::{debug, warn};
use zero_api::{DeadLetterSink, EventFilter, EventSource, RawApiEvent};
use zero_config::ApiConfig;

use crate::registry::{build_event_sinks, ConfiguredEventSink};
use crate::{ConnectorError, ConnectorResult};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDispatcherOptions {
    pub poll_interval: Duration,
    pub max_retry_attempts: u32,
}

impl Default for EventDispatcherOptions {
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_retry_attempts: DEFAULT_MAX_RETRY_ATTEMPTS,
        }
    }
}

pub struct EventDispatcherHandle {
    shutdown: Option<mpsc::Sender<()>>,
    task: JoinHandle<()>,
}

impl EventDispatcherHandle {
    pub async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        let _ = self.task.await;
    }
}

pub fn spawn_event_dispatcher<S>(
    source: S,
    api: ApiConfig,
    source_dir: Option<PathBuf>,
    options: EventDispatcherOptions,
) -> ConnectorResult<Option<EventDispatcherHandle>>
where
    S: EventSource<Stream = Vec<RawApiEvent>> + Send + Sync + 'static,
{
    let (init_tx, init_rx) = mpsc::sync_channel(1);
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let dead_letter_path = api.dead_letter_path.clone();

    let task = tokio::task::spawn_blocking(move || {
        let sinks = match build_event_sinks(&api, source_dir.as_deref()) {
            Ok(sinks) => sinks,
            Err(error) => {
                let _ = init_tx.send(Err(error));
                return;
            }
        };

        if sinks.is_empty() && dead_letter_path.is_none() {
            let _ = init_tx.send(Ok(false));
            return;
        }

        let dead_letter = dead_letter_path
            .and_then(|p| match DeadLetterSink::new(&p) {
                Ok(dl) => {
                    debug!(path = %p, "dead-letter sink enabled");
                    Some(dl)
                }
                Err(e) => {
                    warn!(path = %p, error = %e.message, "failed to create dead-letter sink");
                    None
                }
            });

        let _ = init_tx.send(Ok(true));
        run_event_dispatcher(source, sinks, options, shutdown_rx, dead_letter);
    });

    if !init_rx
        .recv()
        .map_err(|_| ConnectorError::DispatcherStart)??
    {
        return Ok(None);
    }

    Ok(Some(EventDispatcherHandle {
        shutdown: Some(shutdown_tx),
        task,
    }))
}

fn run_event_dispatcher<S>(
    source: S,
    sinks: Vec<ConfiguredEventSink>,
    options: EventDispatcherOptions,
    shutdown: mpsc::Receiver<()>,
    dead_letter: Option<DeadLetterSink>,
) where
    S: EventSource<Stream = Vec<RawApiEvent>> + Send + Sync + 'static,
{
    let mut last_sequence = 0_u64;
    let mut pending = VecDeque::new();

    loop {
        retry_pending(
            &sinks,
            &mut pending,
            options.max_retry_attempts,
            dead_letter.as_ref(),
        );
        match source.subscribe(EventFilter::default()) {
            Ok(events) => {
                for event in events {
                    let should_dispatch = event
                        .sequence
                        .map(|sequence| sequence > last_sequence)
                        .unwrap_or(true);
                    if should_dispatch {
                        dispatch_event(&sinks, &mut pending, event.clone());
                        if let Some(sequence) = event.sequence {
                            last_sequence = last_sequence.max(sequence);
                        }
                    }
                }
            }
            Err(error) => warn!(error = %error, "event dispatcher failed to read source"),
        }

        match shutdown.recv_timeout(options.poll_interval) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    debug!("event dispatcher stopped");
}

fn dispatch_event(
    sinks: &[ConfiguredEventSink],
    pending: &mut VecDeque<PendingDelivery>,
    event: RawApiEvent,
) {
    for sink in sinks {
        if !sink.accepts(&event) {
            continue;
        }

        match sink.publish(&event) {
            Ok(result) if result.delivered => {}
            Ok(result) if result.retryable => pending.push_back(PendingDelivery::new(
                sink.tag.clone(),
                event.clone(),
                result.message,
            )),
            Ok(result) => warn!(
                sink = %sink.tag,
                event_id = %event.event_id,
                message = ?result.message,
                "event sink rejected event without retry"
            ),
            Err(error) => warn!(
                sink = %sink.tag,
                event_id = %event.event_id,
                error = %error,
                "event sink failed"
            ),
        }
    }
}

fn retry_pending(
    sinks: &[ConfiguredEventSink],
    pending: &mut VecDeque<PendingDelivery>,
    max_attempts: u32,
    dead_letter: Option<&DeadLetterSink>,
) {
    let now = Instant::now();
    let len = pending.len();

    for _ in 0..len {
        let Some(mut delivery) = pending.pop_front() else {
            break;
        };

        if delivery.next_due > now {
            pending.push_back(delivery);
            continue;
        }

        let Some(sink) = sinks.iter().find(|sink| sink.tag == delivery.sink_tag) else {
            warn!(
                sink = %delivery.sink_tag,
                event_id = %delivery.event.event_id,
                "dropping pending event for missing sink"
            );
            if let Some(dl) = dead_letter {
                let _ = dl.publish(&delivery.event);
            }
            continue;
        };

        match sink.publish(&delivery.event) {
            Ok(result) if result.delivered => {}
            Ok(result) if result.retryable && delivery.attempts < max_attempts => {
                delivery.attempts += 1;
                delivery.message = result.message;
                delivery.next_due = Instant::now() + retry_delay(delivery.attempts);
                pending.push_back(delivery);
            }
            Ok(result) => {
                warn!(
                    sink = %sink.tag,
                    event_id = %delivery.event.event_id,
                    attempts = delivery.attempts,
                    message = ?result.message,
                    "event delivery moved to dead-letter state"
                );
                if let Some(dl) = dead_letter {
                    let _ = dl.publish(&delivery.event);
                }
            }
            Err(error) if delivery.attempts < max_attempts => {
                delivery.attempts += 1;
                delivery.message = Some(error.to_string());
                delivery.next_due = Instant::now() + retry_delay(delivery.attempts);
                pending.push_back(delivery);
            }
            Err(error) => {
                warn!(
                    sink = %sink.tag,
                    event_id = %delivery.event.event_id,
                    attempts = delivery.attempts,
                    error = %error,
                    "event delivery moved to dead-letter state"
                );
                if let Some(dl) = dead_letter {
                    let _ = dl.publish(&delivery.event);
                }
            }
        }
    }
}

fn retry_delay(attempts: u32) -> Duration {
    Duration::from_secs(2_u64.saturating_pow(attempts.min(6)))
}

struct PendingDelivery {
    sink_tag: String,
    event: RawApiEvent,
    attempts: u32,
    next_due: Instant,
    message: Option<String>,
}

impl PendingDelivery {
    fn new(sink_tag: String, event: RawApiEvent, message: Option<String>) -> Self {
        Self {
            sink_tag,
            event,
            attempts: 1,
            next_due: Instant::now() + retry_delay(1),
            message,
        }
    }
}
