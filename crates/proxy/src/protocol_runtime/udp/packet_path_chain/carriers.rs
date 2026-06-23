use std::sync::Arc;

use zero_engine::EngineError;

use crate::protocol_runtime::udp::PacketPathCarrier;
use crate::runtime::Proxy;

#[cfg(feature = "hysteria2")]
mod hysteria2_carrier;
mod shadowsocks_carrier;
#[cfg(feature = "hysteria2")]
use hysteria2_carrier::Hysteria2PacketPath;
use shadowsocks_carrier::ShadowsocksPacketPath;

/// Build a Shadowsocks packet-path carrier (raw UDP socket to the SS server).
pub(crate) async fn build_shadowsocks_packet_path(
    proxy: &Proxy,
    server: &str,
    port: u16,
    password: &str,
    cipher: &str,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let cipher_kind = shadowsocks::CipherKind::from_str(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown carrier cipher: {cipher}"),
        ))
    })?;
    let path = ShadowsocksPacketPath::establish(proxy, server, port, password, cipher_kind).await?;
    Ok(Arc::new(path))
}

/// Build a Hysteria2 packet-path carrier (QUIC datagrams to the H2 server).
#[cfg(feature = "hysteria2")]
pub(crate) async fn build_hysteria2_packet_path(
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let path = Hysteria2PacketPath::establish(server, port, password, client_fingerprint).await?;
    Ok(Arc::new(path))
}
