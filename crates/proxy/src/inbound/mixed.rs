use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::Error as CoreError;
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

use super::super::logging::log_listener_connection_error;
use super::super::runtime::{bind_listener, Proxy};
use super::super::transport::PrefixedSocket;
use zero_engine::EngineError;

impl Proxy {
    pub(crate) async fn run_mixed_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "mixed",
            listen = %local_addr,
            "inbound listener ready"
        );

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = listener.accept() => {
                    let (stream, remote_addr) = accept_result?;
                    let engine = self.clone();
                    let inbound_tag = inbound.tag.clone();
                    let socks5_users = inbound.protocol.socks5_users().to_vec();

                    connections.spawn(async move {
                        if let Err(error) = engine
                            .handle_mixed_connection(stream, inbound_tag.as_str(), &socks5_users)
                            .await
                        {
                            log_listener_connection_error(
                                "mixed",
                                inbound_tag.as_str(),
                                &remote_addr,
                                &error,
                            );
                        }
                    });
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "mixed connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "mixed connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "mixed",
            listen = %local_addr,
            "inbound listener stopped"
        );

        Ok(())
    }

    pub(crate) async fn handle_mixed_connection(
        &self,
        mut client: TokioSocket,
        inbound_tag: &str,
        socks5_users: &[zero_config::Socks5UserConfig],
    ) -> Result<(), EngineError> {
        let mut first = [0_u8; 1];
        let read = client.read(&mut first).await?;
        if read == 0 {
            return Ok(());
        }

        let protocol = detect_mixed_protocol(first[0]).ok_or(CoreError::Protocol(
            "mixed inbound could not determine client protocol",
        ))?;

        let client = PrefixedSocket::from_byte(client, first[0]);

        match protocol {
            MixedProtocol::Socks5 => {
                self.handle_socks5_client(client, inbound_tag, socks5_users)
                    .await
            }
            MixedProtocol::HttpConnect => {
                self.handle_http_connect_client(client, inbound_tag).await
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MixedProtocol {
    Socks5,
    HttpConnect,
}

fn detect_mixed_protocol(first: u8) -> Option<MixedProtocol> {
    if first == 0x05 {
        Some(MixedProtocol::Socks5)
    } else if first.is_ascii_alphabetic() {
        Some(MixedProtocol::HttpConnect)
    } else {
        None
    }
}
