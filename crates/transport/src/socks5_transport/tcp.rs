use zero_core::Session;
use zero_engine::EngineError;

use crate::{MeteredStream, StreamTraffic, TcpRelayStream};

pub async fn establish_socks5_tcp_connect(
    mut stream: MeteredStream<TcpRelayStream>,
    session: &Session,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(TcpRelayStream, StreamTraffic), EngineError> {
    socks5::Socks5TcpOutboundProfile::from_config_parts(username, password)
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    let traffic = stream.drain_traffic();
    Ok((stream.into_inner(), traffic))
}

pub async fn apply_socks5_tcp_relay_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    socks5::Socks5TcpOutboundProfile::from_config_parts(username, password)
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(stream)
}
