#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksInboundOptionsRef<'a> {
    pub cipher: &'a str,
    pub password: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksOutboundOptionsRef<'a> {
    pub cipher: &'a str,
    pub password: &'a str,
}
