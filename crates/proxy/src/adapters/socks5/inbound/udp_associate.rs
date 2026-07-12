use tracing::info;

use crate::runtime::udp_association::{run_udp_association_loop, UdpAssociationLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

pub(crate) async fn run_socks5_udp_associate<S>(
    proxy: &Proxy,
    mut client: MeteredStream<S>,
    inbound_tag: &str,
    setup: zero_transport::socks5_transport::Socks5InboundUdpAssociationSetup,
) -> Result<(), EngineError>
where
    S: ClientStream,
{
    let relay_addr = setup.relay.local_addr()?;

    info!(
        inbound_tag = inbound_tag,
        protocol = "socks5_udp",
        relay = %relay_addr,
        "socks5 udp association ready"
    );

    run_udp_association_loop(UdpAssociationLoopRequest {
        proxy,
        client: &mut client,
        inbound_tag,
        relay: setup.relay,
        pending_control_traffic: setup.pending_control_traffic,
        handler: setup.handler,
    })
    .await
}
