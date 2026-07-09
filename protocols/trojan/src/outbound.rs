//! Trojan outbound protocol handler.

use std::future::Future;
use std::string::String;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncWrite, AsyncWriteExt};
use zero_core::{Address, Error, Session};
use zero_traits::{AsyncSocket, ClientTlsProfile, TcpTunnelProtocol};

use super::shared::CMD_TCP;

/// Trojan outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanOutbound;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrojanOutboundParts<'a> {
    password: &'a str,
    sni: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrojanTcpOutboundProfile {
    password: String,
}

impl TrojanTcpOutboundProfile {
    fn from_config_parts(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
        }
    }

    fn from_config_password(password: &str) -> Self {
        Self::from_config_parts(password)
    }

    async fn establish_tcp_tunnel_with_traffic<S>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<u64, Error>
    where
        S: AsyncSocket,
    {
        TrojanOutbound
            .send_request(stream, session, &self.password)
            .await
            .map(|written| written as u64)
    }
}

impl<'a> TrojanOutboundParts<'a> {
    fn new(
        password: &'a str,
        sni: Option<&'a str>,
        insecure: bool,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            password,
            sni,
            insecure,
            client_fingerprint,
        }
    }

    fn tcp_connect_request(self) -> TrojanTcpConnectRequest {
        TrojanTcpConnectRequest::from_config(
            self.password,
            self.sni,
            self.insecure,
            self.client_fingerprint,
        )
    }

    fn udp_direct_flow_plan(self) -> crate::udp::TrojanUdpFlowPlan {
        crate::udp::TrojanUdpFlowPlan::direct_from_config(
            self.password,
            self.sni,
            self.insecure,
            self.client_fingerprint,
        )
    }

    fn udp_relay_flow_plan(self) -> crate::udp::TrojanUdpFlowPlan {
        crate::udp::TrojanUdpFlowPlan::relay_from_config(
            self.password,
            self.sni,
            self.insecure,
            self.client_fingerprint,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrojanOutboundRequestBundle {
    tcp_connect: TrojanTcpConnectRequest,
    udp_direct: crate::udp::TrojanUdpFlowPlan,
    udp_relay: crate::udp::TrojanUdpFlowPlan,
}

impl TrojanOutboundRequestBundle {
    fn from_config(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        let parts = TrojanOutboundParts::new(password, sni, insecure, client_fingerprint);
        Self {
            tcp_connect: parts.tcp_connect_request(),
            udp_direct: parts.udp_direct_flow_plan(),
            udp_relay: parts.udp_relay_flow_plan(),
        }
    }

    fn tcp_connect_request(&self) -> TrojanTcpConnectRequest {
        self.tcp_connect.clone()
    }

    fn udp_direct_flow_plan(&self) -> crate::udp::TrojanUdpFlowPlan {
        self.udp_direct.clone()
    }

    fn udp_relay_flow_plan(&self) -> crate::udp::TrojanUdpFlowPlan {
        self.udp_relay.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTrojanOutboundRequestBundle {
    requests: TrojanOutboundRequestBundle,
}

impl PreparedTrojanOutboundRequestBundle {
    pub fn from_config(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            requests: TrojanOutboundRequestBundle::from_config(
                password,
                sni,
                insecure,
                client_fingerprint,
            ),
        }
    }

    #[cfg(feature = "tokio")]
    pub async fn open_tcp_stream_with_transport<S, OpenTransport, OpenTransportFut, E>(
        &self,
        session: &Session,
        open_transport: OpenTransport,
    ) -> Result<TrojanTcpStreamOpen<S>, E>
    where
        S: AsyncSocket + AsyncWrite + Unpin,
        OpenTransport: FnOnce(OwnedTrojanResolvedTlsProfile) -> OpenTransportFut,
        OpenTransportFut: Future<Output = Result<S, E>>,
        E: From<Error> + From<std::io::Error>,
    {
        let request = self.requests.tcp_connect_request();
        let tls_profile = request.owned_tls_profile();
        request
            .open_tcp_stream_with_transport(session, move || open_transport(tls_profile))
            .await
    }

    pub fn udp_direct_flow_plan(&self) -> crate::udp::PreparedTrojanUdpFlowPlan {
        crate::udp::PreparedTrojanUdpFlowPlan::new(self.requests.udp_direct_flow_plan())
    }

    pub fn udp_relay_flow_plan(&self) -> crate::udp::PreparedTrojanUdpFlowPlan {
        crate::udp::PreparedTrojanUdpFlowPlan::new(self.requests.udp_relay_flow_plan())
    }

    pub fn owned_tls_profile(&self) -> OwnedTrojanResolvedTlsProfile {
        self.requests.tcp_connect_request().owned_tls_profile()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrojanTcpTlsProfile {
    server_name: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrojanResolvedTlsProfile<'a> {
    server_name: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTrojanResolvedTlsProfile {
    server_name: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

impl<'a> TrojanResolvedTlsProfile<'a> {
    pub(crate) fn new(
        server_name: Option<&'a str>,
        insecure: bool,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            server_name,
            insecure,
            client_fingerprint,
        }
    }

    fn server_name(&self) -> Option<&str> {
        self.server_name
    }

    fn insecure(&self) -> bool {
        self.insecure
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint
    }

    pub(crate) fn into_owned(self) -> OwnedTrojanResolvedTlsProfile {
        OwnedTrojanResolvedTlsProfile::from_borrowed(self)
    }
}

impl OwnedTrojanResolvedTlsProfile {
    pub(crate) fn new(
        server_name: Option<String>,
        insecure: bool,
        client_fingerprint: Option<String>,
    ) -> Self {
        Self {
            server_name,
            insecure,
            client_fingerprint,
        }
    }

    pub(crate) fn from_borrowed(profile: TrojanResolvedTlsProfile<'_>) -> Self {
        Self::new(
            profile.server_name().map(ToOwned::to_owned),
            profile.insecure(),
            profile.client_fingerprint().map(ToOwned::to_owned),
        )
    }

    fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    fn insecure(&self) -> bool {
        self.insecure
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

impl ClientTlsProfile for OwnedTrojanResolvedTlsProfile {
    fn server_name(&self) -> Option<&str> {
        self.server_name()
    }

    fn disable_sni(&self) -> bool {
        false
    }

    fn ca_cert_path(&self) -> Option<&str> {
        None
    }

    fn insecure(&self) -> bool {
        self.insecure()
    }

    fn alpn(&self) -> &[String] {
        &[]
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint()
    }
}

impl TrojanTcpTlsProfile {
    fn from_config_parts(
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            server_name: sni.map(ToOwned::to_owned),
            insecure,
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    fn insecure(&self) -> bool {
        self.insecure
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }

    fn resolve(&self) -> TrojanResolvedTlsProfile<'_> {
        TrojanResolvedTlsProfile::new(
            self.server_name(),
            self.insecure(),
            self.client_fingerprint(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrojanTcpConnectConfig {
    outbound_profile: TrojanTcpOutboundProfile,
    tls_profile: TrojanTcpTlsProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrojanTcpConnectRequest {
    config: TrojanTcpConnectConfig,
}

#[derive(Debug)]
pub struct TrojanTcpStreamOpen<S> {
    stream: S,
    handshake_written_bytes: u64,
}

impl<S> TrojanTcpStreamOpen<S> {
    pub(crate) fn new(stream: S, handshake_written_bytes: u64) -> Self {
        Self {
            stream,
            handshake_written_bytes,
        }
    }

    pub fn into_parts(self) -> (S, u64) {
        (self.stream, self.handshake_written_bytes)
    }
}

impl TrojanTcpConnectRequest {
    pub fn from_config(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            config: TrojanTcpConnectConfig::from_config(
                password,
                sni,
                insecure,
                client_fingerprint,
            ),
        }
    }

    pub(crate) fn config(&self) -> &TrojanTcpConnectConfig {
        &self.config
    }

    pub(crate) fn tls_profile(&self) -> TrojanResolvedTlsProfile<'_> {
        self.config().tls_profile()
    }

    pub fn owned_tls_profile(&self) -> OwnedTrojanResolvedTlsProfile {
        self.tls_profile().into_owned()
    }

    #[cfg(feature = "tokio")]
    pub async fn open_tcp_stream_with_transport<S, OpenTransport, OpenTransportFut, E>(
        &self,
        session: &Session,
        open_transport: OpenTransport,
    ) -> Result<TrojanTcpStreamOpen<S>, E>
    where
        S: AsyncSocket + AsyncWrite + Unpin,
        OpenTransport: FnOnce() -> OpenTransportFut,
        OpenTransportFut: Future<Output = Result<S, E>>,
        E: From<Error> + From<std::io::Error>,
    {
        let mut stream = open_transport().await?;
        let handshake_written_bytes = self
            .config()
            .establish_tcp_tunnel_with_traffic(&mut stream, session)
            .await
            .map_err(E::from)?;
        stream.flush().await.map_err(E::from)?;
        Ok(TrojanTcpStreamOpen::new(stream, handshake_written_bytes))
    }
}

impl TrojanTcpConnectConfig {
    pub(crate) fn from_config(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            outbound_profile: TrojanTcpOutboundProfile::from_config_password(password),
            tls_profile: tcp_tls_profile_from_config(sni, insecure, client_fingerprint),
        }
    }

    pub(crate) fn tls_profile(&self) -> TrojanResolvedTlsProfile<'_> {
        self.tls_profile.resolve()
    }

    async fn establish_tcp_tunnel_with_traffic<S>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<u64, Error>
    where
        S: AsyncSocket,
    {
        self.outbound_profile
            .establish_tcp_tunnel_with_traffic(stream, session)
            .await
    }
}

fn tcp_tls_profile_from_config(
    sni: Option<&str>,
    insecure: bool,
    client_fingerprint: Option<&str>,
) -> TrojanTcpTlsProfile {
    TrojanTcpTlsProfile::from_config_parts(sni, insecure, client_fingerprint)
}

pub(crate) fn resolved_tls_profile_from_parts<'a>(
    server_name: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
    fallback_server_name: Option<&'a str>,
) -> TrojanResolvedTlsProfile<'a> {
    TrojanResolvedTlsProfile::new(
        server_name.or(fallback_server_name),
        insecure,
        client_fingerprint,
    )
}

impl TrojanOutbound {
    /// Send the Trojan request over an established TLS stream.
    ///
    /// Writes: password hash + CRLF + CMD + address + port + CRLF.
    /// The upstream server then connects to the target and relays data.
    async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<usize, Error> {
        let request = build_tcp_request(password, &session.target, session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write failed"))?;
        Ok(request.len())
    }
}

/// Target parameters for Trojan TCP tunnel.
#[derive(Debug, Clone, Copy)]
struct TrojanTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub password: &'a str,
}

impl<'a> TcpTunnelProtocol<TrojanTcpTunnelTarget<'a>> for TrojanOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &TrojanTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_request(stream, target.session, target.password)
            .await
            .map(|_| ())
    }
}

fn build_tcp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    super::shared::build_request(password, addr, port, CMD_TCP)
}
