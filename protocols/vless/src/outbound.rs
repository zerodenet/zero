use alloc::vec::Vec;

use zero_core::{Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

#[cfg(feature = "reality")]
use crate::flow::flow_build_request;
use crate::mux::MuxClient;
use crate::shared::{read_addon, read_exact, write_address, CMD_MUX, VLESS_VERSION};

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

    pub async fn send_udp_request<S>(
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

        let request = build_udp_request(session, id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS UDP request"))
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

fn build_udp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(crate::shared::CMD_UDP);
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

async fn read_response<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut version = [0_u8; 1];
    read_exact(stream, &mut version).await?;
    if version[0] != VLESS_VERSION {
        return Err(Error::Protocol("unsupported VLESS response version"));
    }

    read_addon(stream).await
}
