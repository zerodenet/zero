use std::time::Duration;

use super::model::UpstreamAssociationState;

impl UpstreamAssociationState {
    pub(in crate::runtime::udp_flow::registered) fn touch_upstream_idle(
        &mut self,
        timeout: Duration,
    ) {
        for handler in &mut self.handlers.upstream {
            if handler.upstream_outbound_tag().is_some() {
                handler.touch_upstream_idle(timeout);
            }
        }
    }

    #[cfg(feature = "socks5")]
    pub(in crate::runtime::udp_flow::registered) fn drop_upstream_association(
        &mut self,
    ) -> Option<(String, String, u16)> {
        self.handlers
            .upstream
            .iter_mut()
            .find_map(|handler| handler.drop_upstream_association())
    }

    #[cfg(feature = "socks5")]
    pub(in crate::runtime::udp_flow::registered) fn close_idle_upstream(
        &mut self,
    ) -> Option<(String, String, u16)> {
        self.handlers
            .upstream
            .iter_mut()
            .find_map(|handler| handler.close_idle_upstream())
    }

    pub(in crate::runtime::udp_flow::registered) fn close_all_upstreams(&mut self) {
        for handler in &mut self.handlers.upstream {
            handler.close_all_upstreams();
        }
    }
}
