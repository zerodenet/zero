use core::future::Future;

use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_core::{InboundMuxServer, Session};
use zero_engine::EngineError;

use crate::runtime::route_runtime::MuxSubstreamRuntime;

pub(crate) struct MuxSessionLoop<'a> {
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
}

pub(crate) trait MuxOpenedDispatcher {
    type Error;

    async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error>;
}

pub(crate) async fn run_mux_session_loop<D>(
    request: MuxSessionLoop<'_>,
    tasks: &mut JoinSet<()>,
    dispatcher: &mut D,
) -> Result<(), D::Error>
where
    D: MuxOpenedDispatcher,
{
    info!(
        inbound_tag = request.inbound_tag,
        protocol = request.protocol,
        "mux session started"
    );

    loop {
        if !dispatcher.dispatch_next(tasks).await? {
            break;
        }

        drain_completed_mux_tasks(tasks, request.panic_message);
    }

    if request.abort_on_end {
        tasks.abort_all();
    }

    info!(
        inbound_tag = request.inbound_tag,
        protocol = request.protocol,
        "mux session ended"
    );
    Ok(())
}

pub(crate) fn drain_completed_mux_tasks(tasks: &mut JoinSet<()>, panic_message: &'static str) {
    while let Some(joined) = tasks.try_join_next() {
        if let Err(error) = joined {
            warn!(error = %error, panic_message);
        }
    }
}

pub(crate) async fn run_protocol_mux_session<R, S, FTcp, FTcpFut, FUdp, FUdpFut>(
    runtime: MuxSubstreamRuntime,
    mut reader: R,
    mut mux_server: S,
    request: MuxSessionLoop<'_>,
    mut spawn_tcp: FTcp,
    mut spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: InboundMuxServer<R>,
    FTcp: FnMut(MuxSubstreamRuntime, Session, S::TcpRelay) -> FTcpFut + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(MuxSubstreamRuntime, S::UdpRelay) -> FUdpFut + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    struct OpenedDispatch<'a, R, S, FTcp, FUdp> {
        runtime: &'a MuxSubstreamRuntime,
        mux_server: &'a mut S,
        reader: &'a mut R,
        spawn_tcp: &'a mut FTcp,
        spawn_udp: &'a mut FUdp,
    }

    impl<R, S, FTcp, FTcpFut, FUdp, FUdpFut> MuxOpenedDispatcher
        for OpenedDispatch<'_, R, S, FTcp, FUdp>
    where
        S: InboundMuxServer<R>,
        FTcp: FnMut(MuxSubstreamRuntime, Session, S::TcpRelay) -> FTcpFut + Send,
        FTcpFut: Future<Output = ()> + Send + 'static,
        FUdp: FnMut(MuxSubstreamRuntime, S::UdpRelay) -> FUdpFut + Send,
        FUdpFut: Future<Output = ()> + Send + 'static,
    {
        type Error = EngineError;

        async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error> {
            let tasks = std::sync::Mutex::new(tasks);
            let spawn_tcp = &mut self.spawn_tcp;
            let spawn_udp = &mut self.spawn_udp;
            let runtime = self.runtime.clone();
            self.mux_server
                .dispatch_next_opened_route(
                    self.reader,
                    |session, relay| {
                        let mut tasks = tasks.lock().expect("mux task set poisoned");
                        tasks.spawn(spawn_tcp(runtime.clone(), session, relay));
                        Ok::<(), EngineError>(())
                    },
                    |relay| {
                        let mut tasks = tasks.lock().expect("mux task set poisoned");
                        tasks.spawn(spawn_udp(runtime.clone(), relay));
                        Ok::<(), EngineError>(())
                    },
                )
                .await
        }
    }

    let mut mux_tasks = JoinSet::new();
    let mut dispatcher = OpenedDispatch {
        runtime: &runtime,
        mux_server: &mut mux_server,
        reader: &mut reader,
        spawn_tcp: &mut spawn_tcp,
        spawn_udp: &mut spawn_udp,
    };

    run_mux_session_loop(request, &mut mux_tasks, &mut dispatcher).await
}
