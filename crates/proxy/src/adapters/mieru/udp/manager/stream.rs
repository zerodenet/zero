use super::bridge;
use crate::transport::TcpRelayStream;
use zero_core::Address;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) sender: MieruFlowSender,
    pub(super) recv_tx: bridge::ResponseSender,
}

#[derive(Clone)]
pub(super) struct MieruFlowSender {
    sender: mieru::MieruUdpFlowSender,
}

impl MieruFlowSender {
    pub(super) async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        self.sender.send(target, port, payload).await
    }
}

pub(super) async fn spawn_packet_stream(
    mut stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let flow_io = mieru::MieruUdpFlowIo::establish_with_resume(&mut stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    let mieru::MieruUdpFlowHandle { sender, responses } = mieru::spawn_udp_flow(stream, flow_io);
    Ok(PacketStream {
        sender: MieruFlowSender { sender },
        recv_tx: responses,
    })
}
