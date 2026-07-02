use alloc::vec::Vec;

use zero_core::{Error, ProtocolType, Session};
#[cfg(feature = "reality")]
use zero_traits::DeferredTcpTunnelProtocol;
use zero_traits::{AsyncSocket, TcpTunnelProtocol};

#[cfg(feature = "reality")]
use crate::flow::flow_build_request;
use crate::mux::MuxClient;
use crate::shared::{parse_uuid, read_response, write_address, CMD_MUX, VLESS_VERSION};

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessOutbound;

impl VlessOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vless
    }

    pub async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request(stream, session, id).await?;
        read_response(stream).await
    }

    #[cfg(feature = "reality")]
    pub async fn establish_tcp_tunnel_with_flow<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
        flow: Option<&str>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request_with_flow(stream, session, id, flow)
            .await?;
        read_response(stream).await
    }

    pub async fn send_tcp_request<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_tcp_request(session, id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))
    }

    #[cfg(feature = "reality")]
    pub async fn send_tcp_request_with_flow<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
        flow: Option<&str>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_tcp_request_with_flow(session, id, flow)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))
    }

    /// Send VLESS MUX header and read server response.
    /// Returns a MuxClient for subsequent stream allocation.
    pub async fn establish_mux<S>(&self, stream: &mut S, id: &[u8; 16]) -> Result<MuxClient, Error>
    where
        S: AsyncSocket,
    {
        let request = build_mux_request(id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS MUX request"))?;
        read_response(stream).await?;
        Ok(MuxClient::new())
    }
}

/// Target parameters for VLESS TCP tunnel (non-flow path).
#[derive(Debug, Clone, Copy)]
pub struct VlessTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
}

/// Parsed VLESS identity and flow settings built from external config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessTcpConnectConfig {
    id: [u8; 16],
    flow: Option<&'static str>,
}

impl VlessTcpConnectConfig {
    pub fn from_config(id: &str, flow: Option<&str>) -> Result<Self, Error> {
        #[cfg(feature = "reality")]
        let flow = flow.map(crate::flow::parse_flow).transpose()?;
        #[cfg(not(feature = "reality"))]
        let flow = {
            if flow.is_some() {
                return Err(Error::Unsupported(
                    "VLESS flow requires the `reality` feature",
                ));
            }
            None
        };
        Ok(Self {
            id: parse_uuid(id)?,
            flow,
        })
    }

    pub fn id(&self) -> [u8; 16] {
        self.id
    }

    pub fn id_ref(&self) -> &[u8; 16] {
        &self.id
    }

    pub fn flow(&self) -> Option<&'static str> {
        self.flow
    }

    #[cfg(feature = "reality")]
    pub fn should_open_mux_pool_for_tcp(&self) -> bool {
        self.flow == Some(crate::flow::FLOW_XTLS_RPRX_VISION)
    }

    #[cfg(feature = "reality")]
    pub fn has_flow(&self) -> bool {
        self.flow.is_some()
    }

    #[cfg(feature = "reality")]
    pub fn mux_pool_identity(&self) -> crate::mux_pool::MuxIdentity {
        crate::mux_pool::MuxIdentity::from_uuid(self.id)
    }

    pub fn tcp_target<'a>(&'a self, session: &'a Session) -> VlessTcpTunnelTarget<'a> {
        VlessTcpTunnelTarget {
            session,
            id: &self.id,
        }
    }

    #[cfg(feature = "reality")]
    pub fn flow_tcp_target<'a>(&'a self, session: &'a Session) -> VlessFlowTcpTunnelTarget<'a> {
        VlessFlowTcpTunnelTarget {
            session,
            id: &self.id,
            flow: self.flow,
        }
    }

    #[cfg(feature = "reality")]
    pub fn wrap_deferred_response_stream<S>(
        &self,
        stream: S,
    ) -> crate::DeferredVlessResponseStream<S> {
        crate::DeferredVlessResponseStream::new(stream)
    }
}

pub fn tcp_connect_config_from_config(
    id: &str,
    flow: Option<&str>,
) -> Result<VlessTcpConnectConfig, Error> {
    VlessTcpConnectConfig::from_config(id, flow)
}

impl<'a> TcpTunnelProtocol<VlessTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tcp_tunnel(stream, target.session, target.id)
            .await
    }
}

/// Target parameters for VLESS TCP tunnel with flow (Vision/Reality path).
///
/// The flow parameter controls the XTLS Vision flow negotiation. When `None`,
/// the handshake uses the standard path. This target is only available when
/// the `reality` feature is enabled because flow handling requires the
/// Vision/Reality code path.
#[cfg(feature = "reality")]
#[derive(Debug, Clone, Copy)]
pub struct VlessFlowTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
    pub flow: Option<&'a str>,
}

#[cfg(feature = "reality")]
impl<'a> TcpTunnelProtocol<VlessFlowTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessFlowTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        match target.flow {
            Some(f) => {
                self.establish_tcp_tunnel_with_flow(stream, target.session, target.id, Some(f))
                    .await
            }
            None => {
                self.establish_tcp_tunnel(stream, target.session, target.id)
                    .await
            }
        }
    }
}

#[cfg(feature = "reality")]
impl<'a> DeferredTcpTunnelProtocol<VlessFlowTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn send_deferred_tcp_tunnel_request<S>(
        &self,
        stream: &mut S,
        target: &VlessFlowTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request_with_flow(stream, target.session, target.id, target.flow)
            .await
    }
}

fn build_tcp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(crate::shared::CMD_TCP);
    request.extend_from_slice(&session.port.to_be_bytes());
    write_address(&mut request, &session.target)?;

    Ok(request)
}

fn build_mux_request(id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(CMD_MUX);
    // Dummy target — ignored by the MUX server
    request.extend_from_slice(&0u16.to_be_bytes());
    request.push(0x01); // ATYP_IPV4
    request.extend_from_slice(&[0u8; 4]);

    Ok(request)
}

#[cfg(feature = "reality")]
fn build_tcp_request_with_flow(
    session: &Session,
    id: &[u8; 16],
    flow: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let (fbyte, payload) = flow_build_request(
        id,
        flow,
        crate::shared::CMD_TCP,
        session.port,
        &session.target,
    )?;

    let mut request = Vec::with_capacity(24 + payload.len());
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(fbyte);
    request.extend_from_slice(&payload);

    Ok(request)
}
