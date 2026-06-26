pub(crate) fn socks5(tag: &str, server: &str, port: u16, username: Option<&str>) -> String {
    let auth = username
        .map(|value| format!("|auth:{value}"))
        .unwrap_or_default();
    format!("socks5|{tag}|{server}:{port}{auth}")
}
