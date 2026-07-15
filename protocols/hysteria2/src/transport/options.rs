#[derive(Debug, Clone, Copy)]
pub struct Hysteria2InboundBindOptionsRef<'a> {
    pub cert_path: Option<&'a str>,
    pub key_path: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct Hysteria2InboundOptionsRef<'a> {
    pub password: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct Hysteria2OutboundOptionsRef<'a> {
    pub password: &'a str,
    pub client_fingerprint: Option<&'a str>,
}
