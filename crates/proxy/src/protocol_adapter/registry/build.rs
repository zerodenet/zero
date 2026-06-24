use std::sync::Arc;

use crate::adapters::{
    DirectAdapter, HttpConnectAdapter, Hysteria2Adapter, MieruAdapter, MixedAdapter,
    ShadowsocksAdapter, Socks5Adapter, TrojanAdapter, VlessAdapter, VmessAdapter,
};

use super::ProtocolRegistry;
use crate::protocol_adapter::ProtocolAdapter;

impl ProtocolRegistry {
    pub(crate) fn build() -> Self {
        let mut r = Self::default();

        #[cfg(feature = "socks5")]
        r.register(Arc::new(Socks5Adapter));
        #[cfg(feature = "http_connect")]
        r.register(Arc::new(HttpConnectAdapter));
        #[cfg(feature = "vless")]
        r.register(Arc::new(VlessAdapter));
        #[cfg(feature = "hysteria2")]
        r.register(Arc::new(Hysteria2Adapter));
        #[cfg(feature = "shadowsocks")]
        r.register(Arc::new(ShadowsocksAdapter));
        #[cfg(feature = "trojan")]
        r.register(Arc::new(TrojanAdapter));
        #[cfg(feature = "vmess")]
        r.register(Arc::new(VmessAdapter));
        #[cfg(feature = "mieru")]
        r.register(Arc::new(MieruAdapter));
        #[cfg(feature = "mixed")]
        r.register(Arc::new(MixedAdapter));
        r.register(Arc::new(DirectAdapter));

        r
    }

    pub(crate) fn register(&mut self, adapter: Arc<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }
}
