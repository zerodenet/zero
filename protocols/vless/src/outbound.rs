use alloc::vec::Vec;

use zero_core::{Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{read_addon, read_exact, write_address, CMD_TCP, VLESS_VERSION};

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
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_tcp_request(session, id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))?;

        read_response(stream).await
    }
}

fn build_tcp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(CMD_TCP);
    request.extend_from_slice(&session.port.to_be_bytes());
    write_address(&mut request, &session.target)?;

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
