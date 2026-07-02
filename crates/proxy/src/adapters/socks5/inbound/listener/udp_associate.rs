mod direct_response;
mod dispatch;
mod relay_socket;
mod setup;

use socks5::udp::Socks5UdpAssociateRequest;

use crate::runtime::udp_association::{run_udp_association_loop, UdpAssociationLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

pub(in crate::adapters) async fn run_socks5_udp_associate<S>(
    proxy: &Proxy,
    mut client: MeteredStream<S>,
    inbound_tag: &str,
    request: Socks5UdpAssociateRequest,
) -> Result<(), EngineError>
where
    S: ClientStream,
{
    let setup = setup::setup_association(&mut client, inbound_tag).await?;
    let relay = setup.relay;
    let handler = relay_socket::Socks5UdpAssociationHandler::new(request);

    run_udp_association_loop(UdpAssociationLoopRequest {
        proxy,
        client: &mut client,
        inbound_tag,
        relay,
        pending_control_traffic: setup.pending_control_traffic,
        handler,
    })
    .await
}
