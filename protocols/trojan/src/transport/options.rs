#[derive(Debug, Clone, Copy)]
pub struct TrojanInboundOptionsRef<'a> {
    pub password: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct TrojanOutboundOptionsRef<'a> {
    pub password: &'a str,
    pub sni: Option<&'a str>,
    pub insecure: bool,
    pub client_fingerprint: Option<&'a str>,
}
