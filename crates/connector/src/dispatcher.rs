use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tracing::{debug, warn};
use zero_api::{DeadLetterSink, EventFilter, EventSink, EventSource, RawApiEvent, SinkStatus};
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

/// Per-sink delivery counters, updated by the dispatcher thread.
struct PerSinkStats {
    total_delivered: AtomicU64,
    total_failed: AtomicU64,
    last_success_ms: Mutex<Option<u64>>,
    last_failure_ms: Mutex<Option<u64>>,
    last_error: Mutex<Option<String>>,
}

impl PerSinkStats {
    fn new() -> Self {
        Self {
            total_delivered: AtomicU64::new(0),
            total_failed: AtomicU64::new(0),
            last_success_ms: Mutex::new(None),
            last_failure_ms: Mutex::new(None),
            last_error: Mutex::new(None),
        }
    }

    fn record_delivered(&self) {
        self.total_delivered.fetch_add(1, Ordering::Relaxed);
        *self.last_success_ms.lock().expect("sink stats") = Some(now_unix_ms());
        *self.last_error.lock().expect("sink stats") = None;
    }

    fn record_failed(&self, message: Option<String>) {
        self.total_failed.fetch_add(1, Ordering::Relaxed);
        *self.last_failure_ms.lock().expect("sink stats") = Some(now_unix_ms());
        *self.last_error.lock().expect("sink stats") = message;
    }

    fn snapshot(&self, name: String) -> SinkStatus {
        SinkStatus {
            name,
            total_delivered: self.total_delivered.load(Ordering::Relaxed),
            total_failed: self.total_failed.load(Ordering::Relaxed),
            last_success_at_unix_ms: *self.last_success_ms.lock().expect("sink stats"),
            last_failure_at_unix_ms: *self.last_failure_ms.lock().expect("sink stats"),
            last_error: self.last_error.lock().expect("sink stats").clone(),
        }
    }
}

#[derive(Clone)]
pub struct EventDispatcherHandle {
    shutdown: Option<mpsc::Sender<()>>,
    task: JoinHandle<()>,
    /// Per-sink delivery stats shared with the dispatcher thread.
    sink_stats: Arc<Vec<(String, Arc<PerSinkStats>)>>,
}

impl EventDispatcherHandle {
    pub async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        let _ = self.task.await;
    }

    /// Snapshot the current delivery status for all configured sinks.
    pub fn sink_status(&self) -> Vec<SinkStatus> {
        self.sink_stats
            .iter()
            .map(|(tag, stats)| stats.snapshot(tag.clone()))
            .collect()
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

    // Shared stats: populated by the dispatcher thread after sink construction,
    // read by the handle on demand.  Created empty here; tags are filled in
    // before the init signal so the handle always sees valid data.
    let sink_stats: Arc<Mutex<Vec<(String, Arc<PerSinkStats>)>>> = Arc::new(Mutex::new(Vec::new()));
    let stats_for_handle = sink_stats.clone();
    let stats_for_thread = sink_stats.clone();

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

        // Initialise per-sink stats with the constructed tags.
        {
            let mut shared = stats_for_thread.lock().expect("sink stats");
            for sink in &sinks {
                shared.push((sink.tag.clone(), Arc::new(PerSinkStats::new())));
            }
        }

        let dead_letter = dead_letter_path.and_then(|p| match DeadLetterSink::new(&p) {
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
        run_event_dispatcher(
            source,
            sinks,
            &stats_for_thread,
            options,
            shutdown_rx,
            dead_letter,
        );
    });

    let init_result = init_rx
        .recv()
        .map_err(|_| ConnectorError::DispatcherStart)??;

    if !init_result {
        return Ok(None);
    }

    // Stats are now populated; take them out of the mutex for the handle.
    let stats_snapshot = stats_for_handle.lock().expect("sink stats").clone();

    Ok(EventDispatcherHandle {
        shutdown: Some(shutdown_tx),
        task,
        sink_stats: Arc::new(stats_snapshot),
    })
}

fn run_event_dispatcher<S>(
    source: S,
    sinks: Vec<ConfiguredEventSink>,
    stats: &Arc<Mutex<Vec<(String, Arc<PerSinkStats>)>>>,
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
            stats,
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
                        dispatch_event(&sinks, stats, &mut pending, event.clone());
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
    stats: &Arc<Mutex<Vec<(String, Arc<PerSinkStats>)>>>,
    pending: &mut VecDeque<PendingDelivery>,
    event: RawApiEvent,
) {
    for sink in sinks {
        if !sink.accepts(&event) {
            continue;
        }

        let result = sink.publish(&event);
        record_delivery(stats, &sink.tag, &result);

        match result {
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
    stats: &Arc<Mutex<Vec<(String, Arc<PerSinkStats>)>>>,
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

        let result = sink.publish(&delivery.event);
        record_delivery(stats, &sink.tag, &result);

        match result {
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

/// Record the outcome of a sink delivery into shared per-sink stats.
fn record_delivery(
    stats: &Arc<Mutex<Vec<(String, Arc<PerSinkStats>)>>>,
    sink_tag: &str,
    result: &Result<zero_api::PublishResult, zero_api::ApiError>,
) {
    let shared = stats.lock().expect("sink stats");
    let Some(entry) = shared.iter().find(|(tag, _)| tag == sink_tag) else {
        return;
    };
    let s = &entry.1;
    match result {
        Ok(r) if r.delivered => s.record_delivered(),
        Ok(r) => s.record_failed(r.message.clone()),
        Err(e) => s.record_failed(Some(e.to_string())),
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
