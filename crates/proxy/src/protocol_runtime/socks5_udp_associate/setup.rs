use socks5::Socks5Reply;
use std::net::SocketAddr;
use tracing::info;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_flow::helpers::address_from_socket_addr;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, StreamTraffic};

pub(super) struct AssociateSetup {
    pub(super) relay: TokioDatagramSocket,
    pub(super) pending_control_traffic: StreamTraffic,
}

pub(super) async fn setup_association<S>(
    proxy: &Proxy,
    client: &mut MeteredStream<S>,
    inbound_tag: &str,
) -> Result<AssociateSetup, EngineError>
where
    S: ClientStream,
{
    let control_local_addr = client.local_addr()?;
    let relay = TokioDatagramSocket::bind_addr(SocketAddr::new(control_local_addr.ip(), 0)).await?;
    let relay_addr = relay.local_addr()?;
    let relay_bind = address_from_socket_addr(relay_addr);

    proxy
        .protocols
        .socks5_inbound_protocol()
        .send_response_with_bound(
            client,
            Socks5Reply::Succeeded,
            &relay_bind,
            relay_addr.port(),
        )
        .await?;

    info!(
        inbound_tag = inbound_tag,
        protocol = "socks5_udp",
        relay = %relay_addr,
        "socks5 udp association ready"
    );

    Ok(AssociateSetup {
        relay,
        pending_control_traffic: client.drain_traffic(),
    })
}
