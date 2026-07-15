//! Shadowsocks inbound preparation and protocol handshake handoff.

use ::shadowsocks::transport::{ShadowsocksInboundBindings, ShadowsocksInboundOptionsRef};
use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

use crate::runtime::inbound_operation::{
    InboundConnectionContext, TcpAndDatagramInboundListenerOperation,
};
use crate::runtime::tcp_ingress::NoClientResponseStreamProtocol;
use crate::transport::{MeteredStream, TcpRelayStream};

impl crate::adapters::shadowsocks::ShadowsocksAdapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let bindings = match &inbound.protocol {
            InboundProtocolConfig::Shadowsocks {
                password, cipher, ..
            } => ShadowsocksInboundBindings::from_options_refs(ShadowsocksInboundOptionsRef {
                cipher: cipher.as_str(),
                password: password.as_str(),
            })?,
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "shadowsocks inbound listener received non-shadowsocks inbound config",
                )));
            }
        };
        let (acceptor, udp_relay) = bindings.into_parts();
        Ok(Box::new(TcpAndDatagramInboundListenerOperation {
            protocol_name: "shadowsocks",
            error_protocol_name: "shadowsocks",
            listen_address: inbound.listen.address,
            listen_port: inbound.listen.port,
            tcp_request: acceptor,
            tcp_dispatch: |acceptor: ::shadowsocks::transport::ShadowsocksInboundTcpAcceptor,
                           socket,
                           context: InboundConnectionContext| async move {
                let (session, client) = acceptor
                    .accept_stream(MeteredStream::new(TcpRelayStream::from(socket)))
                    .await?;
                context
                    .serve(session, client, NoClientResponseStreamProtocol::new())
                    .await
            },
            udp_relay,
        }))
    }
}
