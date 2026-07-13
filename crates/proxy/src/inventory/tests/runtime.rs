use std::sync::Arc;

use super::fixtures::TcpCapabilityCalls;
use super::tcp::proxy_with_fake_tcp;

#[test]
fn inventory_notifies_registered_capabilities_on_reload() {
    let calls = Arc::new(TcpCapabilityCalls::default());
    let proxy = proxy_with_fake_tcp(calls.clone());

    proxy.protocols.on_config_reloaded();
    proxy.protocols.on_config_reloaded();

    assert_eq!(calls.reloads(), 2);
}
