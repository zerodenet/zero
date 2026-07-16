use zero_core::Session;

use super::super::resume::ManagedUdpFlowResume;
use crate::protocol_registry::UdpRuntimeServices;

pub(crate) struct ManagedDatagramFlow<'a> {
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}
