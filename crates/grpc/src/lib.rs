//! gRPC control plane server for Zero.
//!
//! Wraps the existing `EngineHandle` (CommandService + QueryService + EventSource)
//! behind a gRPC endpoint.  All payloads are JSON-encoded, preserving full
//! compatibility with the HTTP and IPC API surfaces.

use std::net::SocketAddr;

use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use zero_api::event::EventFilter;
use zero_api::{CommandService, EventSource, QueryService};
use zero_engine::EngineHandle;

mod pb {
    tonic::include_proto!("zero.api.v1");
}

use pb::{
    control_server::{Control, ControlServer},
    Event as PbEvent, ExecuteRequest, ExecuteResponse, QueryRequest, QueryResponse,
    SubscribeRequest,
};

// ── Public API ─────────────────────────────────────────────────────────────

/// Start the gRPC server on `addr`.
pub async fn spawn(
    handle: EngineHandle,
    addr: SocketAddr,
) -> Result<GrpcHandle, Box<dyn std::error::Error>> {
    let svc = ControlServer::new(ControlService::new(handle));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let task = tokio::spawn(async move {
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                async {
                    let _ = shutdown_rx.await;
                },
            )
            .await
        {
            error!(%e, "gRPC server exited with error");
        }
    });

    info!(listen = %local_addr, "gRPC api server ready");

    Ok(GrpcHandle {
        shutdown: shutdown_tx,
        task,
    })
}

/// Handle to a running gRPC server. Shuts down on drop.
pub struct GrpcHandle {
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl GrpcHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.task.await;
    }
}

// ── Service ────────────────────────────────────────────────────────────────

struct ControlService {
    handle: EngineHandle,
}

impl ControlService {
    fn new(handle: EngineHandle) -> Self {
        Self { handle }
    }
}

#[tonic::async_trait]
impl Control for ControlService {
    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        let query: zero_api::QueryRequest = serde_json::from_slice(&req.payload)
            .map_err(|e| Status::invalid_argument(format!("query parse error: {e}")))?;
        let result = self
            .handle
            .query(query)
            .map_err(|e: zero_api::ApiError| Status::internal(e.to_string()))?;
        let json = serde_json::to_vec(&result).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(QueryResponse { payload: json }))
    }

    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();
        let cmd: zero_api::CommandRequest = serde_json::from_slice(&req.payload)
            .map_err(|e| Status::invalid_argument(format!("command parse error: {e}")))?;
        let result = self
            .handle
            .execute(cmd)
            .map_err(|e: zero_api::ApiError| Status::internal(e.to_string()))?;
        let json = serde_json::to_vec(&result).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ExecuteResponse { payload: json }))
    }

    type SubscribeStream = std::pin::Pin<
        Box<dyn futures_core::Stream<Item = Result<PbEvent, Status>> + Send + 'static>,
    >;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let filter = EventFilter {
            event_types: req.event_types,
            principal_keys: Vec::new(),
            inbound_tags: Vec::new(),
        };

        let subscriber = self
            .handle
            .subscribe(filter)
            .map_err(|e: zero_api::ApiError| Status::internal(format!("subscribe failed: {e}")))?;

        let (tx, rx) = mpsc::channel::<Result<PbEvent, Status>>(64);

        std::thread::spawn(move || loop {
            match subscriber.recv() {
                Some(event) => {
                    let pb_event = PbEvent {
                        event_type: event.event_type,
                        event_id: event.event_id,
                        occurred_at: event.occurred_at_unix_ms,
                        payload: serde_json::to_vec(&event.payload).unwrap_or_default(),
                    };
                    if tx.blocking_send(Ok(pb_event)).is_err() {
                        break;
                    }
                }
                None => break,
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::SubscribeStream))
    }
}
