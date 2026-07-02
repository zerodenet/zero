use crate::transport::ClientStream;

pub(crate) async fn upgrade_vless_reality_server<S>(
    stream: S,
    profile: &vless::VlessRealityServerProfile,
) -> std::io::Result<vless::RealityTlsStream<S>>
where
    S: ClientStream + 'static,
{
    profile.upgrade_server(stream).await
}
