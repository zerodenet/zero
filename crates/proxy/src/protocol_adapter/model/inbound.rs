/// A pre-bound inbound listener: TCP or QUIC.
///
/// Produced by [`crate::protocol_adapter::InboundListenerCapability::bind_inbound`]
/// before the accept loop spawns, so port conflicts surface immediately via
/// `?` rather than surfacing later through `JoinSet::join_next()`. The bind
/// logic stays owned by the adapter instead of leaking protocol-private fields
/// into runtime dispatch.
pub(crate) enum BoundInbound {
    Tcp(zero_platform_tokio::TokioListener),
    #[cfg(any(feature = "vless", feature = "hysteria2"))]
    Quic(crate::transport::QuicInbound),
}

impl BoundInbound {
    /// Unwrap into a TCP listener. Panics if the variant is QUIC; that
    /// indicates a dispatch mismatch because bind and spawn disagreed.
    #[cfg(any(feature = "vless", feature = "hysteria2"))]
    pub(crate) fn into_tcp(self) -> zero_platform_tokio::TokioListener {
        match self {
            Self::Tcp(l) => l,
            Self::Quic(_) => {
                panic!("into_tcp: got QUIC listener, expected TCP (dispatch mismatch)")
            }
        }
    }

    #[cfg(not(any(feature = "vless", feature = "hysteria2")))]
    pub(crate) fn into_tcp(self) -> zero_platform_tokio::TokioListener {
        match self {
            Self::Tcp(l) => l,
        }
    }
}
