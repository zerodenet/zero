use super::super::ChainTask;
use super::bridge;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::{Address, Error, Session};
use zero_traits::DatagramCodec;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum MieruKey {
    Leaf {
        server: String,
        port: u16,
        username: String,
        password: String,
    },
    Relay {
        session_id: u64,
    },
}

pub(super) struct MieruEntry {
    pub(super) send_tx: mpsc::Sender<Vec<u8>>,
    pub(super) recv_tx: bridge::ResponseSender,
    pub(super) codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
}

pub(crate) struct MieruSendExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) relay_chain: bool,
    pub(crate) codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct MieruRelayExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) stream: TcpRelayStream,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
