use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability, TcpSessionProtocol,
};

#[cfg(feature = "crypto")]
use zero_core::Error;
#[cfg(feature = "crypto")]
use zero_traits::AsyncSocket;

#[cfg(feature = "crypto")]
use crate::{MieruOutbound, MieruTcpTarget};

#[derive(Debug, Default, Clone, Copy)]
pub struct MieruProtocol;

impl ProtocolMetadata for MieruProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let unsupported = ProtocolCapabilityState::unsupported(&[]);
        let experimental =
            ProtocolCapabilityState::experimental(&["external_interop_coverage_is_incomplete"]);
        let partial = ProtocolCapabilityState::partial(&[
            "udp_relay_chain_is_not_supported",
            "external_interop_coverage_is_incomplete",
        ]);
        let partial_out = ProtocolCapabilityState::partial(&[
            "relay_chain_hop_is_not_supported",
            "external_interop_coverage_is_incomplete",
        ]);

        ProtocolCapabilityDescriptor {
            protocol: "mieru",
            feature: "mieru",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "mieru",
            inbound: ProtocolNetworkCapability::new(experimental, partial),
            outbound: ProtocolNetworkCapability::new(partial_out, partial),
            transports: &["tcp", "udp"],
            mux: unsupported,
            limitations: &[
                "udp_relay_chain_is_not_supported",
                "external_interop_coverage_is_incomplete",
                "relay_chain_hop_is_not_supported",
            ],
        }
    }
}

#[cfg(feature = "crypto")]
impl<'a> TcpSessionProtocol<MieruTcpTarget<'a>> for MieruProtocol {
    type Error = Error;
    type Session = MieruOutbound;

    async fn establish_tcp_session<S>(
        &self,
        stream: &mut S,
        target: &MieruTcpTarget<'a>,
    ) -> Result<Self::Session, Self::Error>
    where
        S: AsyncSocket,
    {
        MieruOutbound::connect(
            stream,
            target.username,
            target.password,
            target.target,
            target.port,
        )
        .await
    }
}
