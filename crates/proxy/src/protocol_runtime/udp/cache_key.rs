pub(crate) fn socks5(tag: &str, server: &str, port: u16, username: Option<&str>) -> String {
    let auth = username
        .map(|value| format!("|auth:{value}"))
        .unwrap_or_default();
    format!("socks5|{tag}|{server}:{port}{auth}")
}

pub(crate) fn hysteria2(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> String {
    let fingerprint = client_fingerprint
        .map(|value| format!("|fp:{value}"))
        .unwrap_or_default();
    format!("hysteria2|{tag}|{server}:{port}|{password}{fingerprint}")
}
