use std::sync::Arc;

use zero_core::Address;

use super::bridge::BridgeWaiters;
use crate::runtime::Proxy;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SsKey {
    server: String,
    port: u16,
    cipher: String,
    password: String,
}

impl SsKey {
    pub(super) fn new(
        server: &str,
        port: u16,
        cipher: shadowsocks::CipherKind,
        password: &str,
    ) -> Self {
        Self {
            server: server.to_owned(),
            port,
            cipher: format!("{cipher:?}"),
            password: password.to_owned(),
        }
    }
}

pub(super) struct SsUpstream {
    pub(super) socket: Arc<tokio::net::UdpSocket>,
    pub(super) waiters: BridgeWaiters,
}

pub(crate) struct SsSendExisting<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<super::super::ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) proxy: &'a Proxy,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) cipher: &'a str,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
