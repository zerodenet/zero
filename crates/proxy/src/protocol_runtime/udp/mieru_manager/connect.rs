use super::super::packet_path_traits::MieruUdpPeer;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use mieru::MieruOutbound;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zero_engine::EngineError;

pub(super) struct EstablishedSession {
    pub(super) stream: TcpRelayStream,
    pub(super) outbound: MieruOutbound,
}

pub(super) async fn direct_stream(
    proxy: &Proxy,
    peer: &MieruUdpPeer<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(
            peer.endpoint.server,
            peer.endpoint.port,
            proxy.resolver.as_ref(),
        )
        .await?;
    Ok(TcpRelayStream::new(socket))
}

pub(super) async fn establish_udp_associate(
    mut stream: TcpRelayStream,
    username: &str,
    password: &str,
) -> Result<EstablishedSession, EngineError> {
    let mut outbound = MieruOutbound::connect(&mut stream, username, password)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp handshake: {error}"
            )))
        })?;

    send_udp_associate_request(&mut stream, &mut outbound).await?;
    read_udp_associate_response(&mut stream, &mut outbound).await?;

    Ok(EstablishedSession { stream, outbound })
}

async fn send_udp_associate_request(
    stream: &mut TcpRelayStream,
    outbound: &mut MieruOutbound,
) -> Result<(), EngineError> {
    let assoc_req = [0x05u8, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
    let assoc_seg = outbound.encrypt_client_data(&assoc_req).map_err(|error| {
        EngineError::Io(std::io::Error::other(format!(
            "mieru udp assoc encrypt: {error}"
        )))
    })?;
    stream.write_all(&assoc_seg).await.map_err(|error| {
        EngineError::Io(std::io::Error::other(format!(
            "mieru udp assoc write: {error}"
        )))
    })?;
    stream.flush().await.map_err(|error| {
        EngineError::Io(std::io::Error::other(format!(
            "mieru udp assoc flush: {error}"
        )))
    })
}

async fn read_udp_associate_response(
    stream: &mut TcpRelayStream,
    outbound: &mut MieruOutbound,
) -> Result<(), EngineError> {
    let mut assoc_raw = Vec::new();
    let assoc_resp = loop {
        match outbound.decrypt_server_data_with_consumed(&assoc_raw) {
            Ok((segment, consumed)) => {
                assoc_raw.drain(..consumed);
                break segment.payload;
            }
            Err(zero_core::Error::Protocol("mieru: need more data")) => {
                let mut scratch = [0u8; 4096];
                let n = stream.read(&mut scratch).await.map_err(|error| {
                    EngineError::Io(std::io::Error::other(format!(
                        "mieru udp assoc read: {error}"
                    )))
                })?;
                if n == 0 {
                    return Err(EngineError::Io(std::io::Error::other(
                        "mieru udp assoc: connection closed",
                    )));
                }
                assoc_raw.extend_from_slice(&scratch[..n]);
            }
            Err(error) => {
                return Err(EngineError::Io(std::io::Error::other(format!(
                    "mieru udp assoc decrypt: {error}"
                ))))
            }
        }
    };

    if assoc_resp.len() < 4 || assoc_resp[0] != 0x05 || assoc_resp[1] != 0x00 {
        return Err(EngineError::Io(std::io::Error::other(format!(
            "mieru udp assoc rejected: {:?}",
            &assoc_resp[..assoc_resp.len().min(4)]
        ))));
    }

    Ok(())
}
