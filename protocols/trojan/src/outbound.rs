//! Trojan outbound protocol handler.

use zero_core::{Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

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
        use super::shared::{write_password, write_request, CMD_TCP};

        write_password(stream, password).await?;
        write_request(stream, CMD_TCP, &session.target, session.port).await
    }
}
