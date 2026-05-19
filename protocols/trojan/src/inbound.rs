//! Trojan inbound protocol handler.

use zero_core::{Error, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use super::shared::{read_password, read_request, CMD_TCP, CMD_UDP};

/// Trojan inbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInbound;

/// Result of accepting a Trojan connection.
pub struct TrojanAccept {
    pub session: Session,
    pub command: u8,
}

impl TrojanInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Accept a Trojan TCP connection.
    ///
    /// Reads password hash + command + target address from the stream.
    /// The password is validated against `passwords` (hex SHA224 hashes).
    pub async fn accept<S: AsyncSocket>(
        &self,
        stream: &mut S,
        passwords: &[String],
    ) -> Result<TrojanAccept, Error> {
        let hex = read_password(stream).await?;

        // Validate password.
        if !passwords.iter().any(|p| {
            #[cfg(feature = "crypto")]
            {
                use sha2::{Digest, Sha224};
                hex == super::shared::hex::encode(&Sha224::digest(p.as_bytes()))
            }
            #[cfg(not(feature = "crypto"))]
            {
                let _ = p;
                false
            }
        }) {
            return Err(Error::Protocol("trojan: invalid password"));
        }

        let (cmd, addr, port) = read_request(stream).await?;

        let network = match cmd {
            CMD_TCP => Network::Tcp,
            CMD_UDP => Network::Udp,
            _ => return Err(Error::Protocol("trojan: unsupported command")),
        };

        Ok(TrojanAccept {
            session: Session::new(0, addr, port, network, ProtocolType::Trojan),
            command: cmd,
        })
    }
}
