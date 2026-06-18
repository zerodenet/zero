use super::*;

impl UdpDispatch {
    pub(super) async fn start_relay_flow(
        &mut self,
        proxy: &Proxy,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        // Datagram-over-packet-path: previous hop provides a packet path,
        // next hop encodes its datagram through it.
        #[cfg(feature = "shadowsocks")]
        if let Some(params) = resolve_udp_packet_path_chain(&chain) {
            let sent = self
                .packet_path_manager
                .send(
                    UdpFlowContext {
                        chain_tasks: &mut self.chain_tasks,
                        session_id: session.id,
                    },
                    proxy,
                    &params,
                    UdpPacketRef {
                        target: &session.target,
                        port: session.port,
                        payload,
                    },
                )
                .await?;

            return Ok(FlowStartResult::Flow {
                outbound: UdpFlowOutbound::Shadowsocks {
                    tag: params.datagram_tag.to_owned(),
                    server: params.datagram_server.to_owned(),
                    port: params.datagram_port,
                    password: params.datagram_password.to_owned(),
                    cipher: params.datagram_cipher.to_owned(),
                    packet_path_carrier: Some(owned_packet_path_carrier(&params.carrier)),
                },
                tx_bytes: sent as u64,
            });
        }

        // Single dispatch: resolve the final hop's adapter. Adding a protocol
        // = register an adapter; this function never matches on the protocol
        // enum for the final hop.
        let adapter = proxy
            .protocols
            .find_outbound_leaf(chain.last().expect("relay chain has at least 2 hops"))
            .map_err(|error| FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?;

        // Two-stream XHTTP path (VLESS legacy split_http packet-up/stream-up):
        // the adapter dials two carrier streams itself. stream-one / auto fall
        // through to the generic single-stream path below.
        if adapter
            .udp_relay_needs_two_streams(chain.last().expect("relay chain has at least 2 hops"))
        {
            return adapter
                .start_udp_relay_two_stream(self, proxy, session, chain, payload)
                .await;
        }

        // Generic single-stream path: run the relay prefix once, then apply the
        // final hop protocol over the carrier stream.
        let (carrier, final_hop) =
            proxy
                .dispatch_tcp_relay_prefix(chain)
                .await
                .map_err(|failure| FlowFailure {
                    stage: failure.stage,
                    error: failure.error,
                    upstream: failure.upstream_endpoint,
                })?;

        adapter
            .start_udp_relay_final_hop(self, proxy, session, carrier, &final_hop, payload)
            .await
    }
}
