#[derive(Debug, Clone, Copy)]
pub struct Socks5InboundUserRef<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub principal_key: Option<&'a str>,
    pub up_bps: Option<u64>,
    pub down_bps: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Socks5OutboundOptionsRef<'a> {
    pub username: Option<&'a str>,
    pub password: Option<&'a str>,
}
