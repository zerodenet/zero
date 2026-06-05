use alloc::vec::Vec;

use crate::AsyncSocket;

/// Support level for a protocol capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolCapabilityLevel {
    Supported,
    Partial,
    Experimental,
    Unsupported,
    NotApplicable,
}

impl ProtocolCapabilityLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Partial => "partial",
            Self::Experimental => "experimental",
            Self::Unsupported => "unsupported",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// Capability state for one protocol direction and network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolCapabilityState {
    pub supported: bool,
    pub level: ProtocolCapabilityLevel,
    pub notes: &'static [&'static str],
}

impl ProtocolCapabilityState {
    pub const fn supported() -> Self {
        Self {
            supported: true,
            level: ProtocolCapabilityLevel::Supported,
            notes: &[],
        }
    }

    pub const fn partial(notes: &'static [&'static str]) -> Self {
        Self {
            supported: true,
            level: ProtocolCapabilityLevel::Partial,
            notes,
        }
    }

    pub const fn experimental(notes: &'static [&'static str]) -> Self {
        Self {
            supported: true,
            level: ProtocolCapabilityLevel::Experimental,
            notes,
        }
    }

    pub const fn unsupported(notes: &'static [&'static str]) -> Self {
        Self {
            supported: false,
            level: ProtocolCapabilityLevel::Unsupported,
            notes,
        }
    }

    pub const fn not_applicable() -> Self {
        Self {
            supported: false,
            level: ProtocolCapabilityLevel::NotApplicable,
            notes: &[],
        }
    }
}

/// TCP/UDP support for one inbound or outbound direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolNetworkCapability {
    pub tcp: ProtocolCapabilityState,
    pub udp: ProtocolCapabilityState,
}

impl ProtocolNetworkCapability {
    pub const fn new(tcp: ProtocolCapabilityState, udp: ProtocolCapabilityState) -> Self {
        Self { tcp, udp }
    }
}

/// Runtime-neutral protocol capability descriptor.
///
/// This is intentionally not a serde or control-plane type. API adapters map it
/// to their own wire model, while protocol crates can expose the same facts
/// without depending on `zero-api` or the proxy runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolCapabilityDescriptor {
    pub protocol: &'static str,
    pub feature: &'static str,
    pub status: ProtocolCapabilityLevel,
    pub compatibility_baseline: &'static str,
    pub inbound: ProtocolNetworkCapability,
    pub outbound: ProtocolNetworkCapability,
    pub transports: &'static [&'static str],
    pub mux: ProtocolCapabilityState,
    pub limitations: &'static [&'static str],
}

/// Metadata boundary implemented by protocol adapters or protocol crates.
pub trait ProtocolMetadata {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor;
}

/// Protocol TCP tunnel behavior boundary.
///
/// Implementations hide protocol framing and handshake details over an already
/// established upstream stream. Runtime layers remain responsible for dialing,
/// routing, session lifecycle, stats, and events.
pub trait TcpTunnelProtocol<Target: ?Sized>: Send + Sync {
    type Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &Target,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket;
}

/// Protocol TCP tunnel behavior with deferred response validation.
///
/// Some protocols need to write the outbound request immediately but cannot
/// consume the protocol response during establishment because the response
/// must be validated by a stream wrapper on the first downstream read. The
/// runtime still owns dialing, transport setup, metering, lifecycle, stats,
/// and the concrete stream wrapper.
pub trait DeferredTcpTunnelProtocol<Target: ?Sized>: Send + Sync {
    type Error;

    /// Send the protocol request over an already established stream.
    ///
    /// Implementations must not read the protocol response here. The caller is
    /// responsible for wrapping the stream with protocol-specific deferred
    /// response validation before returning it to the relay path.
    async fn send_deferred_tcp_tunnel_request<S>(
        &self,
        stream: &mut S,
        target: &Target,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket;
}

/// Protocol UDP relay association behavior boundary.
///
/// Models protocols that establish a UDP relay through a control connection
/// (e.g. SOCKS5 UDP ASSOCIATE). The caller owns the control stream, UDP
/// socket binding, relay address resolution, association caching, idle
/// timeout, stats, events, session lifecycle, and fallback.
///
/// Implementations hide protocol-specific authentication negotiation and
/// UDP relay handshake details over an already established control stream.
pub trait UdpRelayProtocol<Target: ?Sized>: Send + Sync {
    type Error;

    /// The relay endpoint returned by the association handshake.
    /// The caller resolves this into a concrete socket address and binds
    /// a local UDP socket for sending framed packets to this endpoint.
    type RelayEndpoint;

    /// Perform the UDP relay association handshake over an already
    /// established control stream. Returns the relay endpoint that
    /// the caller should send framed packets to.
    async fn establish_udp_relay<S>(
        &self,
        control_stream: &mut S,
        target: &Target,
    ) -> Result<Self::RelayEndpoint, Self::Error>
    where
        S: AsyncSocket;
}

/// Protocol UDP packet tunnel behavior over an established stream.
///
/// Models protocols that carry UDP packets over a connected bidirectional
/// stream. The implementation owns the protocol request/response handshake.
/// The caller owns dialing, transport setup, packet framing after
/// establishment, session lifecycle, stats, events, and fallback behavior.
pub trait UdpPacketTunnelProtocol<Target: ?Sized>: Send + Sync {
    type Error;

    /// Establish the UDP packet tunnel over an already connected stream.
    ///
    /// Implementations should consume any protocol response required before
    /// the stream starts carrying UDP packet payloads.
    async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        target: &Target,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket;
}

/// Protocol UDP packet framing over an established packet tunnel.
///
/// The tunnel caller owns the transport and session lifecycle; protocol crates
/// own how each UDP datagram is encoded into or decoded from tunnel bytes.
pub trait UdpPacketFraming<Packet: ?Sized>: Send + Sync {
    type Error;
    type Decoded;

    fn encode_udp_packet(&self, packet: &Packet) -> Result<Vec<u8>, Self::Error>;

    fn decode_udp_packet(&self, packet: &[u8]) -> Result<Self::Decoded, Self::Error>;
}

/// Protocol UDP packet framing directly on a connected stream.
///
/// This covers protocols whose UDP packet boundary is part of the stream
/// format, for example a length-prefixed packet. The caller still owns
/// dialing, transport setup, caching, lifecycle, stats, and fallback behavior.
pub trait UdpPacketStreamFraming<Packet: ?Sized>: Send + Sync {
    type Error;
    type Decoded;

    async fn write_udp_packet<S>(&self, stream: &mut S, packet: &Packet) -> Result<(), Self::Error>
    where
        S: AsyncSocket;

    async fn read_udp_packet<S>(&self, stream: &mut S) -> Result<Self::Decoded, Self::Error>
    where
        S: AsyncSocket;
}

/// Protocol UDP datagram framing for packet-oriented transports.
///
/// This covers protocols that carry one complete protocol datagram over one
/// UDP datagram. The caller owns sockets, target resolution, caching, routing,
/// lifecycle, stats, events, and fallback behavior. Protocol crates own how a
/// payload is encoded into the wire datagram and decoded back.
pub trait UdpDatagramFraming<Packet: ?Sized, DecodeContext: ?Sized>: Send + Sync {
    type Error;
    type Decoded;

    fn encode_udp_datagram(&self, packet: &Packet) -> Result<Vec<u8>, Self::Error>;

    fn decode_udp_datagram(
        &self,
        context: &DecodeContext,
        datagram: &[u8],
    ) -> Result<Self::Decoded, Self::Error>;
}

/// Protocol TCP outbound behavior that returns session state.
///
/// For protocols whose handshake produces stream or session state (e.g.
/// AEAD encryption context), this trait captures the handshake result as
/// an associated type. The caller owns transport setup, metering, and
/// relay orchestration using the returned session state.
pub trait TcpSessionProtocol<Target: ?Sized>: Send + Sync {
    type Error;
    type Session;

    /// Perform the TCP session handshake over an already established stream.
    /// Returns protocol-specific session state used for subsequent relay.
    async fn establish_tcp_session<S>(
        &self,
        stream: &mut S,
        target: &Target,
    ) -> Result<Self::Session, Self::Error>
    where
        S: AsyncSocket;
}
