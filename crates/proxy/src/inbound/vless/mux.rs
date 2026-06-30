use tokio::task::JoinSet;
use tracing::info;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};

use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

struct VlessMuxOpenedHandler<'a> {
    proxy: &'a Proxy,
    relay_tasks: &'a mut JoinSet<()>,
    inbound_tag: &'a str,
    auth: &'a Option<zero_core::SessionAuth>,
}

impl vless::mux::VlessInboundMuxOpenedHandler for VlessMuxOpenedHandler<'_> {
    type Error = EngineError;

    async fn handle_tcp_opened(
        &mut self,
        session_id: u16,
        mut session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vless::mux::VlessInboundMuxWriter,
    ) -> Result<bool, Self::Error> {
        if let Some(ref auth) = self.auth {
            session.apply_auth(auth.clone());
        }
        self.proxy
            .prepare_session(&mut session, self.inbound_tag, None);
        let upstream = match TcpPipe::new(self.proxy)
            .dispatch(TcpPipeInput {
                session: &mut session,
            })
            .await
        {
            Ok(result) => result.upstream,
            Err(_) => return Ok(false),
        };

        self.relay_tasks.spawn(async move {
            vless::mux::relay_inbound_mux_stream(session_id, up_rx, writer, upstream).await;
        });

        info!(
            inbound_tag = self.inbound_tag,
            mux_stream_id = session_id,
            port = session.port,
            network = "tcp",
            "MUX stream accepted"
        );

        Ok(true)
    }

    async fn handle_udp_opened(
        &mut self,
        session_id: u16,
        session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vless::mux::VlessInboundMuxWriter,
    ) -> Result<(), Self::Error> {
        let proxy_clone = self.proxy.clone();
        let inbound_tag_owned = self.inbound_tag.to_owned();
        let auth_clone = self.auth.clone();
        self.relay_tasks.spawn(async move {
            proxy_clone
                .spawn_vless_mux_udp_stream_task(
                    session_id,
                    up_rx,
                    writer,
                    &inbound_tag_owned,
                    auth_clone.as_ref(),
                )
                .await;
        });

        info!(
            inbound_tag = self.inbound_tag,
            mux_stream_id = session_id,
            port = session.port,
            network = "udp",
            "MUX stream accepted"
        );

        Ok(())
    }
}

impl Proxy {
    pub(crate) async fn handle_vless_mux_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        mux_context: vless::mux::VlessInboundMuxContext,
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        vless::VlessInbound.send_response(&mut client).await?;
        self.record_session_inbound_traffic(0, client.drain_traffic());

        let mut mux_server = vless::mux::VlessInboundMuxServer::from_context(mux_context);
        let mut relay_tasks = JoinSet::new();

        info!(inbound_tag, "VLESS MUX session started");
        loop {
            let mut handler = VlessMuxOpenedHandler {
                proxy: self,
                relay_tasks: &mut relay_tasks,
                inbound_tag,
                auth,
            };
            if mux_server
                .dispatch_next_opened_stream(&mut client, &mut handler)
                .await
                .is_err()
            {
                break;
            }

            while let Some(joined) = relay_tasks.try_join_next() {
                if let Err(error) = joined {
                    tracing::warn!(error = %error, "vless mux task panicked");
                }
            }
        }

        relay_tasks.abort_all();
        info!(inbound_tag, "VLESS MUX session ended");
        Ok(())
    }
}
