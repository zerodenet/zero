use zero_core::{InboundUdpAssociationResponder, InboundUdpAssociationResponse};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

pub(super) async fn write_target_response<H>(
    association: &H,
    relay: &TokioDatagramSocket,
    target: &zero_core::Address,
    port: u16,
    payload: &[u8],
) -> Result<usize, EngineError>
where
    H: InboundUdpAssociationResponder,
{
    send_association_response(
        relay,
        association.build_response_for_target(target, port, payload)?,
    )
    .await
}

pub(super) async fn write_peer_response<H>(
    association: &H,
    relay: &TokioDatagramSocket,
    sender: zero_traits::SocketAddress,
    payload: &[u8],
) -> Result<usize, EngineError>
where
    H: InboundUdpAssociationResponder,
{
    send_association_response(relay, association.build_peer_response(sender, payload)?).await
}

async fn send_association_response(
    relay: &TokioDatagramSocket,
    response: Option<InboundUdpAssociationResponse>,
) -> Result<usize, EngineError> {
    let Some(response) = response else {
        return Ok(0);
    };
    let (recipient, payload) = response.into_parts();
    relay
        .send_to_addr(
            &payload,
            zero_platform_tokio::socket_address_to_socket_addr(recipient),
        )
        .await
        .map_err(EngineError::from)
}
