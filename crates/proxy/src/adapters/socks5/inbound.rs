mod listener;

use ::socks5::transport::{Socks5InboundAcceptor, Socks5InboundUserRef};
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::socks5::Socks5Adapter;
use crate::runtime::inbound_operation::{InboundConnectionContext, TcpInboundListenerOperation};
use crate::transport::{MeteredStream, TcpRelayStream};

#[cfg(feature = "mixed")]
pub(crate) use listener::handle_socks5_connection;

impl Socks5Adapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let acceptor = match &inbound.protocol {
            InboundProtocolConfig::Socks5 { users } => {
                Socks5InboundAcceptor::from_options_refs(users.iter().map(|user| {
                    Socks5InboundUserRef {
                        username: user.username.as_str(),
                        password: user.password.as_str(),
                        principal_key: user.principal_key.as_deref(),
                        up_bps: user.up_bps,
                        down_bps: user.down_bps,
                    }
                }))
            }
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "socks5 inbound listener received non-socks5 inbound config",
                )));
            }
        };
        Ok(Box::new(TcpInboundListenerOperation {
            protocol_name: "socks5",
            error_protocol_name: "socks5",
            request: acceptor,
            dispatch: |acceptor: Socks5InboundAcceptor,
                       socket,
                       context: InboundConnectionContext| async move {
                listener::handle_socks5_connection(
                    context,
                    MeteredStream::new(TcpRelayStream::from(socket)),
                    &acceptor,
                )
                .await
            },
        }))
    }
}
