// Hysteria2 inbound listener — hysteria2.rs

use std::io;

use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};

use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_protocol_hysteria2::Hysteria2UserStore;

use crate::runtime::Proxy;

impl Proxy {
    pub(crate) async fn run_hysteria2_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let password = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Hysteria2 { password, .. } => password.clone(),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "hysteria2 listener requires hysteria2 protocol config",
                )))
            }
        };

        let cert_path = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Hysteria2 { cert_path, .. } => {
                cert_path.clone().unwrap_or_else(|| "certs/fullchain.pem".to_string())
            }
            _ => "certs/fullchain.pem".to_string(),
        };
        let key_path = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Hysteria2 { key_path, .. } => {
                key_path.clone().unwrap_or_else(|| "certs/privkey.pem".to_string())
            }
            _ => "certs/privkey.pem".to_string(),
        };

        let listen_addr = format!("{}:{}", inbound.listen.address, inbound.listen.port);

        let quic_inbound = crate::transport::QuicInbound::bind(
            &listen_addr,
            &cert_path,
            &key_path,
            self.config.source_dir(),
        )
        .await?;

        let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "hysteria2",
            listen = %listen_addr,
            "inbound listener ready"
        );

        loop {
            select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = quic_inbound.accept() => {
                    match accept_result {
                        Ok(_quic_stream) => {
                            // TODO: handle Hysteria2 auth + stream dispatch
                            // For now, accept and log
                            info!(
                                inbound_tag = %inbound.tag,
                                protocol = "hysteria2",
                                "connection accepted"
                            );
                        }
                        Err(error) => {
                            error!(error = %error, "hysteria2 accept error");
                            break;
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "hysteria2 connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "hysteria2 connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "hysteria2",
            "inbound listener stopped"
        );

        Ok(())
    }
}
