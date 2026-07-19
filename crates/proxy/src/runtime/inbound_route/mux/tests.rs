use std::future::Future;

use async_trait::async_trait;
use zero_core::{
    Address, InboundMuxStreamRoute, InboundStreamUdpRelay, Network, ProtocolType, Session,
    StreamUdpResponder,
};

use super::{dispatch_protocol_mux_route, MuxRouteBridge};
use crate::runtime::route_runtime::{
    InboundRouteRuntime, MuxSubstreamRuntime, SharedIngressRuntimeServices,
};
use crate::runtime::tcp_ingress::NoClientResponseInboundProtocol;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

struct DummyResponder;

#[async_trait]
impl StreamUdpResponder<()> for DummyResponder {
    async fn read_inbound_dispatch(
        &mut self,
        _: &mut (),
    ) -> Result<Option<zero_core::InboundUdpDispatch>, zero_core::Error> {
        Ok(None)
    }

    async fn write_response_for_target(
        &mut self,
        _: &mut (),
        _: &Address,
        _: u16,
        _: &[u8],
    ) -> Result<usize, zero_core::Error> {
        Ok(0)
    }
}

struct DummyUdpRelay;

impl InboundStreamUdpRelay for DummyUdpRelay {
    type Stream = ();
    type Responder = DummyResponder;

    fn into_stream_udp_parts(
        self,
    ) -> (
        Self::Stream,
        Self::Responder,
        Option<zero_core::SessionAuth>,
    ) {
        ((), DummyResponder, None)
    }
}

enum DummyMuxRoute {
    Udp(Box<Session>),
    Mux { reader: u64, server: &'static str },
}

#[async_trait]
impl InboundMuxStreamRoute for DummyMuxRoute {
    type TcpStream = tokio::io::DuplexStream;
    type UdpRelay = DummyUdpRelay;
    type MuxReader = u64;
    type MuxServer = &'static str;

    async fn dispatch_inbound_route<E, FTcp, FTcpFut, FUdp, FUdpFut, FMux, FMuxFut>(
        self,
        _on_tcp: FTcp,
        on_udp: FUdp,
        on_mux: FMux,
    ) -> Result<(), E>
    where
        FTcp: FnOnce(Session, Self::TcpStream) -> FTcpFut + Send,
        FTcpFut: Future<Output = Result<(), E>> + Send,
        FUdp: FnOnce(Session, Self::UdpRelay) -> FUdpFut + Send,
        FUdpFut: Future<Output = Result<(), E>> + Send,
        FMux: FnOnce(Self::MuxReader, Self::MuxServer) -> FMuxFut + Send,
        FMuxFut: Future<Output = Result<(), E>> + Send,
    {
        match self {
            Self::Udp(session) => on_udp(*session, DummyUdpRelay).await,
            Self::Mux { reader, server } => on_mux(reader, server).await,
        }
    }
}

fn proxy() -> Proxy {
    let config = zero_config::RuntimeConfig::parse(
        r#"{ "route": { "rules": [], "final": { "type": "direct" } } }"#,
    )
    .expect("minimal config");
    Proxy::new(config).expect("minimal proxy")
}

fn shared_services(proxy: &Proxy) -> SharedIngressRuntimeServices {
    SharedIngressRuntimeServices::new(proxy.tcp_runtime_services())
}

#[tokio::test]
async fn mux_route_preserves_udp_session_and_inbound_tag() {
    let session = Session::new(
        71,
        Address::Domain("mux-udp-target.test".to_owned()),
        5353,
        Network::Udp,
        ProtocolType::new("vless"),
    );
    let expected = session.clone();

    dispatch_protocol_mux_route(
        DummyMuxRoute::Udp(Box::new(session)),
        MuxRouteBridge {
            runtime: {
                let proxy = proxy();
                InboundRouteRuntime::new(
                    shared_services(&proxy),
                    "vless-mux-in".to_owned(),
                    None,
                )
            },
            protocol: NoClientResponseInboundProtocol,
            map_tcp_stream: TcpRelayStream::new,
            run_udp: move |runtime: InboundRouteRuntime,
                            actual: Session,
                            _: DummyUdpRelay| async move {
                assert_eq!(actual.id, expected.id);
                assert_eq!(actual.target, expected.target);
                assert_eq!(actual.port, expected.port);
                assert_eq!(actual.network, Network::Udp);
                assert_eq!(actual.protocol, ProtocolType::new("vless"));
                assert_eq!(runtime.inbound_tag(), "vless-mux-in");
                Ok(())
            },
            run_mux: |_: MuxSubstreamRuntime, _: u64, _: &'static str| async {
                panic!("unexpected mux branch")
            },
        },
    )
    .await
    .expect("dispatch UDP MUX route");
}

#[tokio::test]
async fn mux_route_preserves_mux_objects_and_inbound_tag() {
    dispatch_protocol_mux_route(
        DummyMuxRoute::Mux {
            reader: 91,
            server: "mux-server",
        },
        MuxRouteBridge {
            runtime: {
                let proxy = proxy();
                InboundRouteRuntime::new(shared_services(&proxy), "vmess-mux-in".to_owned(), None)
            },
            protocol: NoClientResponseInboundProtocol,
            map_tcp_stream: TcpRelayStream::new,
            run_udp: |_: InboundRouteRuntime, _: Session, _: DummyUdpRelay| async {
                panic!("unexpected UDP branch")
            },
            run_mux: |runtime: MuxSubstreamRuntime, reader: u64, server: &'static str| async move {
                assert_eq!(reader, 91);
                assert_eq!(server, "mux-server");
                assert_eq!(runtime.inbound_tag(), "vmess-mux-in");
                Ok(())
            },
        },
    )
    .await
    .expect("dispatch nested MUX route");
}
