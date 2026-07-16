use super::super::ProxyHandle;
use super::diagnostics::{
    execute_diagnostics_dns_cache, execute_diagnostics_fakeip_lookup,
    execute_diagnostics_probe_outbound,
};
use super::tun::{execute_tun_start, execute_tun_stop};

impl zero_api::CommandService for ProxyHandle {
    fn execute(
        &self,
        command: zero_api::CommandRequest,
    ) -> zero_api::ApiResult<zero_api::CommandResponse> {
        match &command {
            zero_api::CommandRequest::TunStart(cmd) => execute_tun_start(self, cmd),
            zero_api::CommandRequest::TunStop(_) => execute_tun_stop(self),
            zero_api::CommandRequest::DiagnosticsProbeOutbound(cmd) => {
                execute_diagnostics_probe_outbound(self, cmd)
            }
            zero_api::CommandRequest::DiagnosticsDnsCache(cmd) => {
                execute_diagnostics_dns_cache(self, cmd)
            }
            zero_api::CommandRequest::DiagnosticsFakeipLookup(cmd) => {
                execute_diagnostics_fakeip_lookup(self, cmd)
            }
            _ => self.inner.execute(command),
        }
    }
}
