use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::Address;

pub(crate) struct VmessMuxTcpStreamTask<'a> {
    pub(crate) tasks: &'a mut JoinSet<()>,
    pub(crate) mux_session_id: u16,
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) write_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub(crate) inbound_tag: String,
}

pub(crate) struct VmessMuxUdpStreamTask<'a> {
    pub(crate) tasks: &'a mut JoinSet<()>,
    pub(crate) mux_session_id: u16,
    pub(crate) default_target: Address,
    pub(crate) default_port: u16,
    pub(crate) up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) write_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub(crate) inbound_tag: String,
}
