use crate::runtime::udp_ingress::UdpIngressRuntime;

pub(super) fn record_stream_udp_client_io<S>(
    runtime: &UdpIngressRuntime,
    record_client_io: Option<fn(&UdpIngressRuntime, u64, &mut S)>,
    session_id: u64,
    client: &mut S,
) {
    if let Some(record_client_io) = record_client_io {
        record_client_io(runtime, session_id, client);
    }
}
