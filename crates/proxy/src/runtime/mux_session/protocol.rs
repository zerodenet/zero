use core::future::Future;

use tokio::task::JoinSet;
use zero_core::{InboundMuxServer, Session};
use zero_engine::EngineError;

use super::lifecycle::run_mux_session_loop;
use super::model::{MuxOpenedDispatcher, MuxSessionLoop};
use crate::runtime::route_runtime::MuxSubstreamRuntime;

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
