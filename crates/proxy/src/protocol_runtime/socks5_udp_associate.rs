mod chain_response;
mod cleanup;
mod direct_response;
mod dispatch;
mod idle_timeout;
mod relay_socket;
mod setup;
mod upstream_response;

use socks5::Socks5UdpAssociateRequest;
use std::net::SocketAddr;
use tokio::select;
use zero_traits::AsyncSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

use crate::protocol_runtime::socks5_udp::recv_upstream_packet;
use crate::runtime::udp_flow::helpers::wait_for_upstream_idle;

impl Proxy {
    pub(crate) async fn handle_socks5_udp_associate<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        _request: Socks5UdpAssociateRequest,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let setup = setup::setup_association(self, &mut client, inbound_tag).await?;
        let relay = setup.relay;
        let mut pending_control_traffic = setup.pending_control_traffic;

        let mut dispatch = UdpDispatch::new(inbound_tag).await?;

        let mut client_udp_addr: Option<SocketAddr> = None;
        let mut control_probe = [0_u8; 1];
        let mut packet = vec![0_u8; 64 * 1024];
        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];

        loop {
            // Extract all mutable/immutable borrows in one go to satisfy
            // select!'s requirement that all branches be independent.
            let (direct_sock, socks5_up, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                control = client.read(&mut control_probe) => {
                    match control {
                        Ok(0) => break,
                        Ok(_) => break,
                        Err(error) => return Err(error.into()),
                    }
                }
                recv = relay.recv_from_addr(&mut packet) => {
                    let (read, sender) = recv?;
                    relay_socket::handle_relay_packet(relay_socket::RelayPacketRequest {
                        proxy: self,
                        dispatch: &mut dispatch,
                        relay: &relay,
                        inbound_tag,
                        pending_control_traffic: &mut pending_control_traffic,
                        client_udp_addr: &mut client_udp_addr,
                        sender,
                        payload: &packet[..read],
                    })
                    .await?;
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    // Direct outbound response from the dispatcher's socket.
                    let (n, sender) = recv?;

                    direct_response::forward_dispatch_socket_response(
                        self,
                        &dispatch,
                        &relay,
                        client_udp_addr,
                        inbound_tag,
                        sender,
                        &direct_buf[..n],
                    )
                    .await;
                }
                upstream = recv_upstream_packet(socks5_up, &mut upstream_buf) => {
                    upstream_response::handle_upstream_response(
                        self,
                        &mut dispatch,
                        &relay,
                        client_udp_addr,
                        inbound_tag,
                        upstream,
                        &upstream_buf,
                    )
                    .await?;
                }
                Some(chain_result) = chain_tasks.join_next() => {
                    // Chain-outbound response (SS, H2, VLESS via JoinSet).
                    chain_response::handle_chain_result(
                        chain_response::ChainResponseRequest {
                            proxy: self,
                            relay: &relay,
                            client_addr: client_udp_addr,
                            inbound_tag,
                        },
                        chain_result,
                    )
                    .await;
                }
                _ = wait_for_upstream_idle(socks5_idle) => {
                    idle_timeout::handle_idle_timeout(self, &mut dispatch, inbound_tag);
                }
            }
        }

        cleanup::finish_dispatch(dispatch);

        Ok(())
    }
}
