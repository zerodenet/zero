use std::future::Future;
use std::path::{Path, PathBuf};

use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::transport_plan::ProfiledTcpStreamTransportPlan;
pub(super) type TrojanTcpStreamOpen = trojan::outbound::TrojanTcpStreamOpen<TcpRelayStream>;

#[derive(Debug, Clone)]
pub(super) struct OwnedTrojanOutboundTlsPlan {
    server: String,
    port: u16,
    source_dir: Option<PathBuf>,
}

impl OwnedTrojanOutboundTlsPlan {
    pub(super) fn from_parts(source_dir: Option<&Path>, server: &str, port: u16) -> Self {
        Self {
            server: server.to_owned(),
            port,
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    fn server(&self) -> &str {
        &self.server
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn source_dir(&self) -> Option<&Path> {
        self.source_dir.as_deref()
    }

    #[cfg(feature = "trojan")]
    pub(super) async fn open_direct_with_profile<OpenSocket, OpenSocketFut, E>(
        &self,
        open_socket: OpenSocket,
        tls_profile: trojan::outbound::OwnedTrojanResolvedTlsProfile,
    ) -> Result<TcpRelayStream, EngineError>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>>,
        E: Into<EngineError>,
    {
        let upstream = open_socket(self.server(), self.port())
            .await
            .map_err(Into::into)?;
        open_trojan_tls_stream_with_profile(upstream, self.source_dir(), self.server(), tls_profile)
            .await
    }

    #[cfg(feature = "trojan")]
    pub(super) async fn open_relay_with_profile(
        &self,
        stream: TcpRelayStream,
        tls_profile: trojan::outbound::OwnedTrojanResolvedTlsProfile,
    ) -> Result<TcpRelayStream, EngineError> {
        open_trojan_tls_relay_stream_with_profile(
            stream,
            self.source_dir(),
            self.server(),
            tls_profile,
        )
        .await
    }
}

impl ProfiledTcpStreamTransportPlan<trojan::outbound::OwnedTrojanResolvedTlsProfile>
    for OwnedTrojanOutboundTlsPlan
{
    fn open_direct_stream_with_profile<'a, OpenSocket, OpenSocketFut>(
        &'a self,
        open_socket: OpenSocket,
        profile: trojan::outbound::OwnedTrojanResolvedTlsProfile,
    ) -> crate::transport_plan::TransportOpenFuture<'a>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut + Send + 'a,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send + 'a,
    {
        Box::pin(async move { self.open_direct_with_profile(open_socket, profile).await })
    }

    fn open_relay_stream_with_profile<'a>(
        &'a self,
        stream: TcpRelayStream,
        profile: trojan::outbound::OwnedTrojanResolvedTlsProfile,
    ) -> crate::transport_plan::TransportOpenFuture<'a> {
        Box::pin(async move { self.open_relay_with_profile(stream, profile).await })
    }
}

#[cfg(feature = "trojan")]
async fn open_trojan_tls_stream_with_profile(
    socket: TokioSocket,
    source_dir: Option<&Path>,
    server: &str,
    tls_profile: trojan::outbound::OwnedTrojanResolvedTlsProfile,
) -> Result<TcpRelayStream, EngineError> {
    crate::tls::connect_tls_upstream_with_profile(socket, &tls_profile, source_dir, server).await
}

#[cfg(feature = "trojan")]
async fn open_trojan_tls_relay_stream_with_profile(
    stream: TcpRelayStream,
    source_dir: Option<&Path>,
    server: &str,
    tls_profile: trojan::outbound::OwnedTrojanResolvedTlsProfile,
) -> Result<TcpRelayStream, EngineError> {
    crate::tls::connect_tls_stream_with_profile(stream, &tls_profile, source_dir, server).await
}
