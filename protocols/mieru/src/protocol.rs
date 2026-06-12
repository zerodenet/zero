use alloc::vec::Vec;

use zero_core::Error;
#[cfg(feature = "crypto")]
use zero_traits::TcpSessionProtocol;
use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability, UdpPacketFraming,
};

#[cfg(feature = "crypto")]
use zero_traits::AsyncSocket;

use crate::{
    unwrap_udp_associate, wrap_udp_associate, MieruUdpAssociatePacket, MieruUdpAssociatePayload,
};
#[cfg(feature = "crypto")]
use crate::{MieruOutbound, MieruTcpTarget};

#[derive(Debug, Default, Clone, Copy)]
pub struct MieruProtocol;

impl ProtocolMetadata for MieruProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let unsupported = ProtocolCapabilityState::unsupported(&[]);
        // Outbound TCP + UDP validated end-to-end against upstream mita
        // (enfein/mieru) via socks5-in-tunnel.
        let supported = ProtocolCapabilityState::supported();
        // Inbound is implemented (symmetric socks5-in-tunnel) but not yet
        // verified against an external mieru client.
        let inbound = ProtocolCapabilityState::partial(&["inbound_interop_unverified"]);

        ProtocolCapabilityDescriptor {
            protocol: "mieru",
            feature: "mieru",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "mieru",
            inbound: ProtocolNetworkCapability::new(inbound, inbound),
            outbound: ProtocolNetworkCapability::new(supported, supported),
            transports: &["tcp", "udp"],
            mux: unsupported,
            limitations: &["inbound_interop_unverified"],
        }
    }
}

impl<'a> UdpPacketFraming<MieruUdpAssociatePacket<'a>> for MieruProtocol {
    type Error = Error;
    type Decoded = MieruUdpAssociatePayload;

    fn encode_udp_packet(
        &self,
        packet: &MieruUdpAssociatePacket<'a>,
    ) -> Result<Vec<u8>, Self::Error> {
        Ok(wrap_udp_associate(packet.payload))
    }

    fn decode_udp_packet(&self, packet: &[u8]) -> Result<Self::Decoded, Self::Error> {
        Ok(MieruUdpAssociatePayload {
            payload: unwrap_udp_associate(packet)?,
        })
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
        MieruOutbound::connect(stream, target.username, target.password).await
    }
}
