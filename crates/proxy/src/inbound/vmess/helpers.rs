//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use vmess::{VmessAccept, VmessAeadStream, VmessCipher, VmessInbound, VmessUser};
use zero_config::{GrpcConfig, InboundConfig, WebSocketConfig};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_associate::helpers::{
    log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

/// `AsyncSocket` for a rustls TLS stream over TcpRelayStream.

#[derive(Clone, Copy)]
pub(crate) enum VmessUdpPayloadMode {
    Unknown,
    VmessPacket,
    RawDatagram,
}

pub(crate) fn encode_vmess_mux_udp_response(
    mux_session_id: u16,
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    match mode {
        VmessUdpPayloadMode::Unknown | VmessUdpPayloadMode::VmessPacket => {
            let packet = vmess::build_udp_packet(target, port, payload)?;
            vmess::encode_mux_keep_stream(mux_session_id, &packet)
        }
        VmessUdpPayloadMode::RawDatagram => vmess::encode_mux_keep_stream(mux_session_id, payload),
    }
}

pub(crate) fn encode_vmess_udp_response(
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    match mode {
        VmessUdpPayloadMode::Unknown | VmessUdpPayloadMode::VmessPacket => {
            vmess::build_udp_packet(target, port, payload)
        }
        VmessUdpPayloadMode::RawDatagram => Ok(payload.to_vec()),
    }
}

pub(crate) fn wrap_vmess_client(
    stream: TcpRelayStream,
    accepted: VmessAccept,
) -> Result<TcpRelayStream, EngineError> {
    Ok(TcpRelayStream::new(VmessAeadStream::inbound(
        stream, accepted,
    )?))
}

pub(crate) async fn read_vmess_mux_frame_from_tokio<R>(
    reader: &mut R,
) -> Result<vmess::MuxFrame, EngineError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut len_buf = [0_u8; 2];
    tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf).await?;
    let meta_len = u16::from_be_bytes(len_buf) as usize;
    if meta_len > vmess::MUX_MAX_META_LEN {
        return Err(EngineError::Core(zero_core::Error::Protocol(
            "vmess mux metadata too large",
        )));
    }
    let mut meta = vec![0_u8; meta_len];
    tokio::io::AsyncReadExt::read_exact(reader, &mut meta).await?;
    let mut frame = vmess::decode_mux_metadata(&meta)?;
    if frame.option & vmess::MUX_OPTION_DATA != 0 {
        tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf).await?;
        let data_len = u16::from_be_bytes(len_buf) as usize;
        if data_len > vmess::MUX_MAX_DATA_LEN {
            return Err(EngineError::Core(zero_core::Error::Protocol(
                "vmess mux data too large",
            )));
        }
        frame.payload.resize(data_len, 0);
        if data_len > 0 {
            tokio::io::AsyncReadExt::read_exact(reader, &mut frame.payload).await?;
        }
    }
    Ok(frame)
}

pub(crate) fn remote_addr_to_socket(
    addr: Option<zero_traits::IpAddress>,
) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
