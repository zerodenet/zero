use super::model::InboundConnectionContext;

impl InboundConnectionContext {
    #[cfg(feature = "upstream-association-runtime")]
    pub(crate) async fn run_udp_association<S, H>(
        self,
        mut client: crate::transport::MeteredStream<S>,
        relay: zero_platform_tokio::TokioDatagramSocket,
        pending_control_traffic: crate::transport::StreamTraffic,
        handler: H,
    ) -> Result<(), zero_engine::EngineError>
    where
        S: crate::transport::ClientStream,
        H: zero_core::InboundUdpAssociation + zero_core::InboundUdpAssociationResponder,
    {
        let runtime = self.runtime;
        let inbound_tag = runtime.inbound_tag().to_owned();
        crate::runtime::udp_association::run_udp_association_loop(
            crate::runtime::udp_association::UdpAssociationLoopRequest {
                runtime: runtime.udp_runtime(),
                client: &mut client,
                inbound_tag: &inbound_tag,
                relay,
                pending_control_traffic,
                handler,
            },
        )
        .await
    }

    #[cfg(feature = "managed-stream-runtime")]
    pub(crate) async fn run_stream_udp_relay<R>(
        self,
        session: zero_core::Session,
        relay: R,
        protocol: &'static str,
    ) -> Result<(), zero_engine::EngineError>
    where
        R: zero_core::InboundStreamUdpRelay,
        R::Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        R::Responder: zero_core::StreamUdpResponder<R::Stream>,
    {
        let runtime = self.runtime;
        let inbound_tag = runtime.inbound_tag().to_owned();
        crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay(
            runtime.udp_runtime(),
            &session,
            relay,
            &inbound_tag,
            protocol,
            core::convert::identity,
            None,
        )
        .await
    }
}
