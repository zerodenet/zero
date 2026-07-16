//! Narrow ingress from an accepted inbound UDP packet into [`UdpPipe`].

mod dispatch;
mod model;
mod route;
mod session;

pub(crate) use model::UdpIngressRuntime;
