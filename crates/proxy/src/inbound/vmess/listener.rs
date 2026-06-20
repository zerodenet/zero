use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use vmess::{VmessCipher, VmessInbound, VmessUser};
use zero_config::InboundConfig;
use zero_engine::EngineError;

use super::*;
use crate::runtime::Proxy;

pub(crate) async fn run_vmess_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let (users, tls_cfg, ws_config, grpc_config) = match &inbound.protocol {
        zero_config::InboundProtocolConfig::Vmess {
            users,
            tls,
            ws,
            grpc,
        } => (users.clone(), tls.clone(), ws.clone(), grpc.clone()),
        _ => {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess config",
            )))
        }
    };
    if users.is_empty() {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vmess requires at least one user",
        )));
    }

    let vmess_users: Vec<VmessUser> = users
        .iter()
        .map(|u| {
            let uuid = vmess::parse_uuid(&u.id)
                .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, e)))?;
            let cipher = VmessCipher::from_name(&u.cipher).ok_or_else(|| {
                EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("vmess unknown cipher: {}", u.cipher),
                ))
            })?;
            Ok(VmessUser {
                id: uuid,
                cipher,
                credential_id: u.credential_id.clone(),
                principal_key: u.principal_key.clone(),
                up_bps: u.up_bps,
                down_bps: u.down_bps,
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;

    let tls_cfg = tls_cfg.ok_or_else(|| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vmess requires TLS",
        ))
    })?;
    let acceptor = crate::transport::build_tls_acceptor(&tls_cfg, proxy.config.source_dir())?;
    let tag = inbound.tag.clone();

    let handler = VmessInboundHandler {
        vmess_inbound: VmessInbound,
        users: vmess_users,
        tls_acceptor: acceptor,
    };

    let transport = match (&ws_config, &grpc_config) {
        (Some(_), Some(_)) => {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess: ws and grpc are mutually exclusive",
            )))
        }
        (Some(_), None) => "vmess+ws",
        (None, Some(_)) => "vmess+grpc",
        (None, None) => "vmess",
    };

    info!(inbound_tag = %tag, protocol = transport, listen = %listener.local_addr()?, "started");

    let mut conns = JoinSet::new();
    loop {
        select! {
            _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
            r = listener.accept() => {
                let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                let p = proxy.clone();
                let t = tag.clone();
                let h = handler.clone();
                let ws = ws_config.clone();
                let grpc = grpc_config.clone();
                let source_addr = remote_addr_to_socket(peer);
                conns.spawn(async move {
                    let res = if let Some(grpc_cfg) = &grpc {
                        handle_vmess_grpc(&p, &h, s.into(), grpc_cfg, &t, source_addr).await
                    } else if let Some(ws_cfg) = &ws {
                        handle_vmess_ws(&p, &h, s.into(), ws_cfg, &t, source_addr).await
                    } else {
                        handle_vmess_raw(&p, &h, s.into(), &t, source_addr).await
                    };
                    if let Err(e) = res {
                        if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                        { warn!(?source_addr, %e, "vmess failed"); }
                    }
                });
            }
            r = conns.join_next(), if !conns.is_empty() => {
                if let Some(Err(e)) = r { if !e.is_cancelled() { error!(%e, "task panicked"); } }
            }
        }
    }
    conns.abort_all();
    info!(inbound_tag = %tag, "stopped");
    Ok(())
}
