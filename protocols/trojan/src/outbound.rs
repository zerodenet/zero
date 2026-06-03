//! Trojan outbound protocol handler.

use zero_core::{Address, Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

use super::shared::{CMD_TCP, CMD_UDP};

/// Trojan outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanOutbound;

impl TrojanOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Send the Trojan request over an established TLS stream.
    ///
    /// Writes: password hash + CRLF + CMD + address + port + CRLF.
    /// The upstream server then connects to the target and relays data.
    pub async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<(), Error> {
        let request = build_tcp_request(password, &session.target, session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write failed"))
    }
}

/// Build a Trojan UDP associate request (CMD_UDP).
///
/// This is a standalone request builder used by the proxy outbound
/// module to initiate a UDP relay connection.
pub fn build_udp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    build_trojan_request(password, addr, port, CMD_UDP)
}

fn build_tcp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    build_trojan_request(password, addr, port, CMD_TCP)
}

fn build_trojan_request(
    password: &str,
    addr: &Address,
    port: u16,
    cmd: u8,
) -> Result<Vec<u8>, Error> {
    use super::shared::{ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CRLF};

    let mut request = Vec::new();

    #[cfg(feature = "crypto")]
    {
        use sha2::{Digest, Sha224};
        let digest = Sha224::digest(password.as_bytes());
        request.extend_from_slice(super::shared::hex::encode(&digest).as_bytes());
    }
    #[cfg(not(feature = "crypto"))]
    {
        let _ = password;
        return Err(Error::Unsupported("trojan: crypto feature not enabled"));
    }

    request.extend_from_slice(CRLF);
    request.push(cmd);

    match addr {
        Address::Ipv4(bytes) => {
            request.push(ATYP_IPV4);
            request.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            request.push(ATYP_IPV6);
            request.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() || bytes.len() > 255 {
                return Err(Error::Protocol("trojan: domain too long"));
            }
            request.push(ATYP_DOMAIN);
            request.push(bytes.len() as u8);
            request.extend_from_slice(bytes);
        }
    }

    request.extend_from_slice(&port.to_be_bytes());
    request.extend_from_slice(CRLF);
    Ok(request)
}
