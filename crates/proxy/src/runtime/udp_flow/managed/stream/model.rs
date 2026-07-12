use super::super::model::ManagedStreamFlowHandler;

pub(in crate::runtime::udp_flow::managed) struct ManagedStreamState {
    pub(in crate::runtime::udp_flow::managed) handlers: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

impl ManagedStreamState {
    pub(in crate::runtime::udp_flow::managed) fn new(
        handlers: Vec<Box<dyn ManagedStreamFlowHandler>>,
    ) -> Self {
        Self { handlers }
    }
}
