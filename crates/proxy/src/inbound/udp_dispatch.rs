use zero_core::{InboundUdpDispatch, SessionAuth};
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;

pub(crate) async fn dispatch_inbound_udp_packet(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    inbound_dispatch: &InboundUdpDispatch,
    auth: Option<&SessionAuth>,
) -> Result<u64, EngineError> {
    UdpPipe::new(proxy, dispatch)
        .dispatch(UdpPipeInput::from_inbound_dispatch(inbound_dispatch, auth))
        .await
}
