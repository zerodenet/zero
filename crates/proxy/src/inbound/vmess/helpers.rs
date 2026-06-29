use vmess::{VmessAccept, VmessAeadStream};
use zero_engine::EngineError;

use crate::transport::TcpRelayStream;

pub(crate) fn wrap_vmess_client(
    stream: TcpRelayStream,
    accepted: VmessAccept,
) -> Result<TcpRelayStream, EngineError> {
    Ok(TcpRelayStream::new(VmessAeadStream::inbound(
        stream, accepted,
    )?))
}
