use crate::runtime::Proxy;

#[derive(Clone, Copy)]
pub(crate) struct InboundAdapterContext<'a> {
    proxy: &'a Proxy,
}

impl<'a> InboundAdapterContext<'a> {
    pub(crate) fn new(proxy: &'a Proxy) -> Self {
        Self { proxy }
    }

    pub(crate) fn proxy(&self) -> &'a Proxy {
        self.proxy
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OutboundAdapterContext<'a> {
    proxy: &'a Proxy,
}

impl<'a> OutboundAdapterContext<'a> {
    pub(crate) fn new(proxy: &'a Proxy) -> Self {
        Self { proxy }
    }

    pub(crate) fn proxy(&self) -> &'a Proxy {
        self.proxy
    }
}

#[derive(Clone, Copy)]
pub(crate) struct UdpAdapterContext<'a> {
    proxy: &'a Proxy,
}

impl<'a> UdpAdapterContext<'a> {
    pub(crate) fn new(proxy: &'a Proxy) -> Self {
        Self { proxy }
    }

    pub(crate) fn proxy(&self) -> &'a Proxy {
        self.proxy
    }
}
