use zero_core::Session;
use zero_transport::RuntimeError;
use zero_transport::{MeteredStream, StreamTraffic, TcpRelayStream};

pub async fn establish_shadowsocks_tcp_connect(
    mut stream: MeteredStream<TcpRelayStream>,
    session: &Session,
    cipher: &str,
    password: &str,
) -> Result<(TcpRelayStream, StreamTraffic), RuntimeError> {
    let config = shadowsocks_tcp_connect_config(cipher, password)?;
    let ss_session = config
        .establish_tcp_session(&mut stream, session)
        .await
        .map_err(|error| RuntimeError::Io(std::io::Error::other(error)))?;
    let traffic = stream.drain_traffic();
    let stream = stream.into_inner();
    Ok((
        TcpRelayStream::new(config.wrap_outbound_stream(stream, ss_session)),
        traffic,
    ))
}

pub async fn apply_shadowsocks_tcp_relay_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    cipher: &str,
    password: &str,
) -> Result<TcpRelayStream, RuntimeError> {
    let config = shadowsocks_tcp_connect_config(cipher, password)?;
    let ss_session = config
        .establish_tcp_session(&mut stream, session)
        .await
        .map_err(|error| RuntimeError::Io(std::io::Error::other(error)))?;
    Ok(TcpRelayStream::new(
        config.wrap_outbound_stream(stream, ss_session),
    ))
}

fn shadowsocks_tcp_connect_config(
    cipher: &str,
    password: &str,
) -> Result<crate::ShadowsocksTcpConnectConfig, RuntimeError> {
    crate::tcp_connect_config_from_config(cipher, password).map_err(|error| {
        RuntimeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid shadowsocks tcp config: {error}"),
        ))
    })
}
