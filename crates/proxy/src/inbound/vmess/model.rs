use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::Session;

pub(crate) struct VmessInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) profile: vmess::VmessInboundProfile,
    pub(crate) tls: Option<Box<zero_config::TlsConfig>>,
    pub(crate) ws: Option<Box<zero_config::WebSocketConfig>>,
    pub(crate) grpc: Option<Box<zero_config::GrpcConfig>>,
}

pub(crate) struct VmessMuxTcpStreamTask<'a> {
    pub(crate) tasks: &'a mut JoinSet<()>,
    pub(crate) mux_session_id: u16,
    pub(crate) session: Session,
    pub(crate) up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) writer: vmess::mux::VmessInboundMuxWriter,
    pub(crate) inbound_tag: String,
}

pub(crate) struct VmessMuxUdpStreamTask<'a> {
    pub(crate) tasks: &'a mut JoinSet<()>,
    pub(crate) mux_session_id: u16,
    pub(crate) session: Session,
    pub(crate) up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) writer: vmess::mux::VmessInboundMuxWriter,
    pub(crate) inbound_tag: String,
}
